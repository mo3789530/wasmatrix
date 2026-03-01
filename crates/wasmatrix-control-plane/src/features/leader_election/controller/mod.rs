use std::sync::Arc;

use crate::features::leader_election::service::{LeaderElectionService, LeadershipStatus};
use crate::shared::error::ControlPlaneResult;

pub struct LeaderElectionController {
    service: Arc<LeaderElectionService>,
}

impl LeaderElectionController {
    pub fn new(service: Arc<LeaderElectionService>) -> Self {
        Self { service }
    }

    pub fn start(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        self.service.start()
    }

    pub async fn is_leader(&self) -> bool {
        self.service.is_leader().await
    }

    pub async fn leadership_status(&self) -> LeadershipStatus {
        self.service.status().await
    }

    pub async fn release_leadership(&self) -> ControlPlaneResult<()> {
        self.service.release_leadership().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::leader_election::repo::InMemoryLeaderElectionRepository;
    use crate::features::leader_election::service::LeaderElectionConfig;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_controller_reports_leader_status() {
        let controller = Arc::new(LeaderElectionController::new(Arc::new(
            LeaderElectionService::new(
                Arc::new(InMemoryLeaderElectionRepository::new()),
                "node-1",
                LeaderElectionConfig {
                    lease_ttl: Duration::from_secs(1),
                    renew_interval: Duration::from_millis(100),
                },
            ),
        )));

        let task = controller.start();
        sleep(Duration::from_millis(150)).await;

        let status = controller.leadership_status().await;
        assert!(status.is_leader);
        assert_eq!(status.node_id, "node-1");

        task.abort();
    }
}
