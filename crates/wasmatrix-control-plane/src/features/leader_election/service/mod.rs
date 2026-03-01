use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use crate::features::leader_election::repo::{LeaderElectionRepository, LeaderLeaseRecord};
use crate::shared::error::ControlPlaneResult;

#[derive(Debug, Clone)]
pub struct LeaderElectionConfig {
    pub lease_ttl: Duration,
    pub renew_interval: Duration,
}

impl Default for LeaderElectionConfig {
    fn default() -> Self {
        Self {
            lease_ttl: Duration::from_secs(10),
            renew_interval: Duration::from_secs(3),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LeadershipStatus {
    pub node_id: String,
    pub is_leader: bool,
    pub current_leader: Option<LeaderLeaseRecord>,
}

#[derive(Debug, Default)]
struct LocalLeadershipState {
    lease: Option<LeaderLeaseRecord>,
    current_leader: Option<LeaderLeaseRecord>,
}

pub struct LeaderElectionService {
    repo: Arc<dyn LeaderElectionRepository>,
    local_node_id: String,
    config: LeaderElectionConfig,
    state: Arc<RwLock<LocalLeadershipState>>,
}

impl LeaderElectionService {
    pub fn new(
        repo: Arc<dyn LeaderElectionRepository>,
        local_node_id: impl Into<String>,
        config: LeaderElectionConfig,
    ) -> Self {
        Self {
            repo,
            local_node_id: local_node_id.into(),
            config,
            state: Arc::new(RwLock::new(LocalLeadershipState::default())),
        }
    }

    pub fn start(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let service = Arc::clone(self);
        tokio::spawn(async move {
            service.run().await;
        })
    }

    async fn run(self: Arc<Self>) {
        let mut interval = tokio::time::interval(self.config.renew_interval);
        loop {
            interval.tick().await;
            if let Err(error) = self.synchronize().await {
                tracing::warn!(error = %error, "Leader election synchronization failed");
            }
        }
    }

    async fn synchronize(&self) -> ControlPlaneResult<()> {
        let local_lease = { self.state.read().await.lease.clone() };
        let ttl_seconds = self.config.lease_ttl.as_secs().max(1) as i64;

        let updated_lease = if let Some(lease) = local_lease {
            self.repo
                .renew_leadership(&self.local_node_id, lease.lease_id, ttl_seconds)
                .await?
        } else {
            self.repo
                .try_acquire_leadership(&self.local_node_id, ttl_seconds)
                .await?
        };
        let current_leader = self.repo.get_current_leader().await?;

        let mut state = self.state.write().await;
        state.lease = updated_lease;
        state.current_leader = current_leader;
        Ok(())
    }

    pub async fn is_leader(&self) -> bool {
        self.state.read().await.lease.is_some()
    }

    pub async fn status(&self) -> LeadershipStatus {
        let state = self.state.read().await;
        LeadershipStatus {
            node_id: self.local_node_id.clone(),
            is_leader: state.lease.is_some(),
            current_leader: state.current_leader.clone(),
        }
    }

    pub async fn release_leadership(&self) -> ControlPlaneResult<()> {
        let lease = { self.state.read().await.lease.clone() };
        if let Some(lease) = lease {
            self.repo
                .release_leadership(&self.local_node_id, lease.lease_id)
                .await?;
        }

        let current_leader = self.repo.get_current_leader().await?;
        let mut state = self.state.write().await;
        state.lease = None;
        state.current_leader = current_leader;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::leader_election::repo::InMemoryLeaderElectionRepository;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_service_acquires_and_releases_leadership() {
        let service = Arc::new(LeaderElectionService::new(
            Arc::new(InMemoryLeaderElectionRepository::new()),
            "node-1",
            LeaderElectionConfig {
                lease_ttl: Duration::from_secs(1),
                renew_interval: Duration::from_millis(100),
            },
        ));

        let task = service.start();
        sleep(Duration::from_millis(150)).await;

        assert!(service.is_leader().await);

        service.release_leadership().await.unwrap();
        assert!(!service.is_leader().await);

        task.abort();
    }
}
