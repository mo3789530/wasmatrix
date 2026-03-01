use std::sync::Arc;

use crate::features::metadata_persistence::repo::{CrashHistoryRecord, PersistentMetadataRepository};
use crate::shared::error::ControlPlaneResult;
use crate::shared::types::{InstanceMetadata, InstanceStatus};

pub struct MetadataPersistenceService {
    repo: Arc<dyn PersistentMetadataRepository>,
}

impl MetadataPersistenceService {
    pub fn new(repo: Arc<dyn PersistentMetadataRepository>) -> Self {
        Self { repo }
    }

    pub async fn persist_instance(&self, metadata: &InstanceMetadata) -> ControlPlaneResult<()> {
        self.repo.upsert_instance(metadata).await
    }

    pub async fn get_instance(&self, instance_id: &str) -> ControlPlaneResult<Option<InstanceMetadata>> {
        self.repo.get_instance(instance_id).await
    }

    pub async fn list_instances(&self) -> ControlPlaneResult<Vec<InstanceMetadata>> {
        self.repo.list_instances().await
    }

    pub async fn delete_instance(&self, instance_id: &str) -> ControlPlaneResult<bool> {
        self.repo.delete_instance(instance_id).await
    }

    pub async fn persist_status_transition(
        &self,
        metadata: &InstanceMetadata,
        status: InstanceStatus,
        error: Option<String>,
    ) -> ControlPlaneResult<Option<CrashHistoryRecord>> {
        let mut updated = metadata.clone();
        updated.status = status;
        self.repo.upsert_instance(&updated).await?;

        if status == InstanceStatus::Crashed {
            let crash = self.repo.record_crash(&updated.instance_id, error).await?;
            return Ok(Some(crash));
        }

        Ok(None)
    }

    pub async fn get_crash_history(
        &self,
        instance_id: &str,
    ) -> ControlPlaneResult<Option<CrashHistoryRecord>> {
        self.repo.get_crash_history(instance_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::metadata_persistence::repo::EtcdBackedMetadataRepository;
    use chrono::Utc;

    fn test_instance_metadata() -> InstanceMetadata {
        InstanceMetadata {
            instance_id: "inst-1".to_string(),
            node_id: "node-1".to_string(),
            module_hash: "hash-1".to_string(),
            created_at: Utc::now(),
            status: InstanceStatus::Starting,
        }
    }

    #[tokio::test]
    async fn test_persist_status_transition_records_crash() {
        let repo = Arc::new(EtcdBackedMetadataRepository::new());
        let service = MetadataPersistenceService::new(repo);
        let metadata = test_instance_metadata();

        let crash = service
            .persist_status_transition(
                &metadata,
                InstanceStatus::Crashed,
                Some("instance trap".to_string()),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(crash.instance_id, metadata.instance_id);
        assert_eq!(crash.crash_count, 1);
    }
}
