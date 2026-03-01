use std::sync::Arc;

use crate::features::metadata_persistence::repo::CrashHistoryRecord;
use crate::features::metadata_persistence::service::MetadataPersistenceService;
use crate::shared::error::ControlPlaneResult;
use crate::shared::types::{InstanceMetadata, InstanceStatus};

pub struct MetadataPersistenceController {
    service: Arc<MetadataPersistenceService>,
}

impl MetadataPersistenceController {
    pub fn new(service: Arc<MetadataPersistenceService>) -> Self {
        Self { service }
    }

    pub async fn upsert_instance(&self, metadata: &InstanceMetadata) -> ControlPlaneResult<()> {
        self.service.persist_instance(metadata).await
    }

    pub async fn get_instance(&self, instance_id: &str) -> ControlPlaneResult<Option<InstanceMetadata>> {
        self.service.get_instance(instance_id).await
    }

    pub async fn list_instances(&self) -> ControlPlaneResult<Vec<InstanceMetadata>> {
        self.service.list_instances().await
    }

    pub async fn delete_instance(&self, instance_id: &str) -> ControlPlaneResult<bool> {
        self.service.delete_instance(instance_id).await
    }

    pub async fn sync_status(
        &self,
        metadata: &InstanceMetadata,
        status: InstanceStatus,
        error: Option<String>,
    ) -> ControlPlaneResult<Option<CrashHistoryRecord>> {
        self.service
            .persist_status_transition(metadata, status, error)
            .await
    }

    pub async fn get_crash_history(
        &self,
        instance_id: &str,
    ) -> ControlPlaneResult<Option<CrashHistoryRecord>> {
        self.service.get_crash_history(instance_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::metadata_persistence::repo::EtcdBackedMetadataRepository;
    use crate::features::metadata_persistence::service::MetadataPersistenceService;
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
    async fn test_controller_can_load_persisted_instance() {
        let repo = Arc::new(EtcdBackedMetadataRepository::new());
        let service = Arc::new(MetadataPersistenceService::new(repo));
        let controller = MetadataPersistenceController::new(service);
        let metadata = test_instance_metadata();

        controller.upsert_instance(&metadata).await.unwrap();

        let loaded = controller.get_instance(&metadata.instance_id).await.unwrap();
        assert!(loaded.is_some());
    }
}
