use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "etcd")]
use crate::shared::error::ControlPlaneError;
use crate::shared::error::ControlPlaneResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaderLeaseRecord {
    pub leader_id: String,
    pub lease_id: i64,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[async_trait]
pub trait LeaderElectionRepository: Send + Sync {
    async fn try_acquire_leadership(
        &self,
        node_id: &str,
        ttl_seconds: i64,
    ) -> ControlPlaneResult<Option<LeaderLeaseRecord>>;
    async fn renew_leadership(
        &self,
        node_id: &str,
        lease_id: i64,
        ttl_seconds: i64,
    ) -> ControlPlaneResult<Option<LeaderLeaseRecord>>;
    async fn release_leadership(&self, node_id: &str, lease_id: i64) -> ControlPlaneResult<()>;
    async fn get_current_leader(&self) -> ControlPlaneResult<Option<LeaderLeaseRecord>>;
}

#[derive(Clone, Default)]
pub struct InMemoryLeaderElectionRepository {
    current_leader: Arc<RwLock<Option<LeaderLeaseRecord>>>,
    lease_sequence: Arc<AtomicI64>,
}

impl InMemoryLeaderElectionRepository {
    pub fn new() -> Self {
        Self::default()
    }

    async fn next_lease_id(&self) -> i64 {
        self.lease_sequence.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn build_lease(node_id: &str, lease_id: i64, ttl_seconds: i64) -> LeaderLeaseRecord {
        let now = Utc::now();
        LeaderLeaseRecord {
            leader_id: node_id.to_string(),
            lease_id,
            acquired_at: now,
            expires_at: now + ChronoDuration::seconds(ttl_seconds.max(1)),
        }
    }
}

#[async_trait]
impl LeaderElectionRepository for InMemoryLeaderElectionRepository {
    async fn try_acquire_leadership(
        &self,
        node_id: &str,
        ttl_seconds: i64,
    ) -> ControlPlaneResult<Option<LeaderLeaseRecord>> {
        {
            let current_leader = self.current_leader.read().await;
            if let Some(existing) = current_leader.as_ref() {
                if existing.expires_at > Utc::now() && existing.leader_id != node_id {
                    return Ok(None);
                }
            }
        }

        let lease = Self::build_lease(node_id, self.next_lease_id().await, ttl_seconds);
        let mut current_leader = self.current_leader.write().await;
        *current_leader = Some(lease.clone());
        Ok(Some(lease))
    }

    async fn renew_leadership(
        &self,
        node_id: &str,
        lease_id: i64,
        ttl_seconds: i64,
    ) -> ControlPlaneResult<Option<LeaderLeaseRecord>> {
        let mut current_leader = self.current_leader.write().await;
        let Some(existing) = current_leader.as_ref() else {
            return Ok(None);
        };

        if existing.expires_at <= Utc::now()
            || existing.leader_id != node_id
            || existing.lease_id != lease_id
        {
            *current_leader = None;
            return Ok(None);
        }

        let lease = Self::build_lease(node_id, lease_id, ttl_seconds);
        *current_leader = Some(lease.clone());
        Ok(Some(lease))
    }

    async fn release_leadership(&self, node_id: &str, lease_id: i64) -> ControlPlaneResult<()> {
        let mut current_leader = self.current_leader.write().await;
        if current_leader
            .as_ref()
            .is_some_and(|lease| lease.leader_id == node_id && lease.lease_id == lease_id)
        {
            *current_leader = None;
        }
        Ok(())
    }

    async fn get_current_leader(&self) -> ControlPlaneResult<Option<LeaderLeaseRecord>> {
        let mut current_leader = self.current_leader.write().await;
        if current_leader
            .as_ref()
            .is_some_and(|lease| lease.expires_at <= Utc::now())
        {
            *current_leader = None;
        }

        Ok(current_leader.clone())
    }
}

#[cfg(feature = "etcd")]
#[derive(Clone)]
pub struct EtcdLeaderElectionRepository {
    client: Arc<tokio::sync::Mutex<etcd_client::Client>>,
    leader_key: String,
}

#[cfg(feature = "etcd")]
impl EtcdLeaderElectionRepository {
    pub async fn connect(
        config: &crate::features::node_routing::repo::etcd::EtcdConfig,
    ) -> ControlPlaneResult<Self> {
        let client = etcd_client::Client::connect(config.endpoints.clone(), None)
            .await
            .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;

        Ok(Self {
            client: Arc::new(tokio::sync::Mutex::new(client)),
            leader_key: "/wasmatrix/leader-election/lease".to_string(),
        })
    }

    async fn get_current_leader_internal(
        &self,
        client: &mut etcd_client::Client,
    ) -> ControlPlaneResult<Option<LeaderLeaseRecord>> {
        let response = client
            .get(self.leader_key.clone(), None)
            .await
            .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;

        let Some(kv) = response.kvs().first() else {
            return Ok(None);
        };
        let raw = std::str::from_utf8(kv.value())
            .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;

        serde_json::from_str(raw).map_err(|e| ControlPlaneError::StorageError(e.to_string()))
    }
}

#[cfg(feature = "etcd")]
#[async_trait]
impl LeaderElectionRepository for EtcdLeaderElectionRepository {
    async fn try_acquire_leadership(
        &self,
        node_id: &str,
        ttl_seconds: i64,
    ) -> ControlPlaneResult<Option<LeaderLeaseRecord>> {
        let mut client = self.client.lock().await;
        let lease = client
            .lease_grant(ttl_seconds.max(1), None)
            .await
            .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
        let lease_record = LeaderLeaseRecord {
            leader_id: node_id.to_string(),
            lease_id: lease.id(),
            acquired_at: Utc::now(),
            expires_at: Utc::now() + ChronoDuration::seconds(ttl_seconds.max(1)),
        };

        let put = etcd_client::TxnOp::put(
            self.leader_key.clone(),
            serde_json::to_string(&lease_record)
                .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?,
            Some(etcd_client::PutOptions::new().with_lease(lease.id())),
        );
        let txn = etcd_client::Txn::new()
            .when([etcd_client::Compare::create_revision(
                self.leader_key.clone(),
                etcd_client::CompareOp::Equal,
                0,
            )])
            .and_then([put]);
        let response = client
            .txn(txn)
            .await
            .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;

        if response.succeeded() {
            return Ok(Some(lease_record));
        }

        client
            .lease_revoke(lease.id())
            .await
            .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
        self.get_current_leader_internal(&mut client).await.map(|_| None)
    }

    async fn renew_leadership(
        &self,
        node_id: &str,
        lease_id: i64,
        ttl_seconds: i64,
    ) -> ControlPlaneResult<Option<LeaderLeaseRecord>> {
        let mut client = self.client.lock().await;
        let Some(current) = self.get_current_leader_internal(&mut client).await? else {
            return Ok(None);
        };
        if current.leader_id != node_id || current.lease_id != lease_id {
            return Ok(None);
        }

        client
            .lease_keep_alive(lease_id)
            .await
            .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;

        let renewed = LeaderLeaseRecord {
            leader_id: node_id.to_string(),
            lease_id,
            acquired_at: current.acquired_at,
            expires_at: Utc::now() + ChronoDuration::seconds(ttl_seconds.max(1)),
        };
        client
            .put(
                self.leader_key.clone(),
                serde_json::to_string(&renewed)
                    .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?,
                Some(etcd_client::PutOptions::new().with_lease(lease_id)),
            )
            .await
            .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;

        Ok(Some(renewed))
    }

    async fn release_leadership(&self, node_id: &str, lease_id: i64) -> ControlPlaneResult<()> {
        let mut client = self.client.lock().await;
        let Some(current) = self.get_current_leader_internal(&mut client).await? else {
            return Ok(());
        };
        if current.leader_id == node_id && current.lease_id == lease_id {
            client
                .delete(self.leader_key.clone(), None)
                .await
                .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
            client
                .lease_revoke(lease_id)
                .await
                .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
        }
        Ok(())
    }

    async fn get_current_leader(&self) -> ControlPlaneResult<Option<LeaderLeaseRecord>> {
        let mut client = self.client.lock().await;
        self.get_current_leader_internal(&mut client).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_in_memory_leader_election_failover_after_expiry() {
        let repo = InMemoryLeaderElectionRepository::new();

        let first = repo
            .try_acquire_leadership("node-a", 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.leader_id, "node-a");

        let second_attempt = repo.try_acquire_leadership("node-b", 1).await.unwrap();
        assert!(second_attempt.is_none());

        sleep(Duration::from_millis(1100)).await;

        let second = repo
            .try_acquire_leadership("node-b", 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.leader_id, "node-b");
    }

    #[tokio::test]
    async fn test_in_memory_leader_renew_requires_matching_lease() {
        let repo = InMemoryLeaderElectionRepository::new();
        let lease = repo
            .try_acquire_leadership("node-a", 5)
            .await
            .unwrap()
            .unwrap();

        let renewed = repo
            .renew_leadership("node-a", lease.lease_id, 5)
            .await
            .unwrap();
        assert!(renewed.is_some());

        let rejected = repo
            .renew_leadership("node-b", lease.lease_id, 5)
            .await
            .unwrap();
        assert!(rejected.is_none());
    }
}
