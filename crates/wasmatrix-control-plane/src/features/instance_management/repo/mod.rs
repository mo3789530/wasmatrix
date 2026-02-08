use crate::shared::error::ControlPlaneResult;
use crate::shared::types::{InstanceMetadata, InstanceStatus};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Repository trait for instance storage
#[async_trait]
pub trait InstanceRepository: Send + Sync {
    /// Store a new instance
    async fn create(&self, metadata: InstanceMetadata) -> ControlPlaneResult<()>;

    /// Retrieve an instance by ID
    async fn get(&self, instance_id: &str) -> ControlPlaneResult<Option<InstanceMetadata>>;

    /// Update instance status
    async fn update_status(
        &self,
        instance_id: &str,
        status: InstanceStatus,
    ) -> ControlPlaneResult<()>;

    /// Delete an instance
    async fn delete(&self, instance_id: &str) -> ControlPlaneResult<bool>;

    /// List all instances
    async fn list(&self) -> ControlPlaneResult<Vec<InstanceMetadata>>;

    /// Check if instance exists
    async fn exists(&self, instance_id: &str) -> ControlPlaneResult<bool>;
}

/// In-memory implementation of instance repository
#[derive(Clone)]
pub struct InMemoryInstanceRepository {
    storage: Arc<RwLock<HashMap<String, InstanceMetadata>>>,
}

impl InMemoryInstanceRepository {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryInstanceRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InstanceRepository for InMemoryInstanceRepository {
    async fn create(&self, metadata: InstanceMetadata) -> ControlPlaneResult<()> {
        let mut storage = self.storage.write().map_err(|_| {
            crate::shared::error::ControlPlaneError::StorageError("Lock poisoned".to_string())
        })?;
        storage.insert(metadata.instance_id.clone(), metadata);
        Ok(())
    }

    async fn get(&self, instance_id: &str) -> ControlPlaneResult<Option<InstanceMetadata>> {
        let storage = self.storage.read().map_err(|_| {
            crate::shared::error::ControlPlaneError::StorageError("Lock poisoned".to_string())
        })?;
        Ok(storage.get(instance_id).cloned())
    }

    async fn update_status(
        &self,
        instance_id: &str,
        status: InstanceStatus,
    ) -> ControlPlaneResult<()> {
        let mut storage = self.storage.write().map_err(|_| {
            crate::shared::error::ControlPlaneError::StorageError("Lock poisoned".to_string())
        })?;

        if let Some(metadata) = storage.get_mut(instance_id) {
            metadata.status = status;
            Ok(())
        } else {
            Err(crate::shared::error::ControlPlaneError::InstanceNotFound(
                instance_id.to_string(),
            ))
        }
    }

    async fn delete(&self, instance_id: &str) -> ControlPlaneResult<bool> {
        let mut storage = self.storage.write().map_err(|_| {
            crate::shared::error::ControlPlaneError::StorageError("Lock poisoned".to_string())
        })?;
        Ok(storage.remove(instance_id).is_some())
    }

    async fn list(&self) -> ControlPlaneResult<Vec<InstanceMetadata>> {
        let storage = self.storage.read().map_err(|_| {
            crate::shared::error::ControlPlaneError::StorageError("Lock poisoned".to_string())
        })?;
        Ok(storage.values().cloned().collect())
    }

    async fn exists(&self, instance_id: &str) -> ControlPlaneResult<bool> {
        let storage = self.storage.read().map_err(|_| {
            crate::shared::error::ControlPlaneError::StorageError("Lock poisoned".to_string())
        })?;
        Ok(storage.contains_key(instance_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_metadata(id: &str) -> InstanceMetadata {
        InstanceMetadata::new("test-node".to_string(), "test-hash".to_string())
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let repo = InMemoryInstanceRepository::new();
        let metadata = create_test_metadata("test-1");
        let id = metadata.instance_id.clone();

        repo.create(metadata.clone()).await.unwrap();

        let retrieved = repo.get(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().instance_id, id);
    }

    #[tokio::test]
    async fn test_update_status() {
        let repo = InMemoryInstanceRepository::new();
        let metadata = create_test_metadata("test-1");
        let id = metadata.instance_id.clone();

        repo.create(metadata).await.unwrap();
        repo.update_status(&id, InstanceStatus::Running)
            .await
            .unwrap();

        let retrieved = repo.get(&id).await.unwrap().unwrap();
        assert_eq!(retrieved.status, InstanceStatus::Running);
    }

    #[tokio::test]
    async fn test_delete() {
        let repo = InMemoryInstanceRepository::new();
        let metadata = create_test_metadata("test-1");
        let id = metadata.instance_id.clone();

        repo.create(metadata).await.unwrap();
        assert!(repo.delete(&id).await.unwrap());
        assert!(!repo.exists(&id).await.unwrap());
    }

    #[tokio::test]
    async fn test_list() {
        let repo = InMemoryInstanceRepository::new();

        for i in 0..3 {
            let metadata = create_test_metadata(&format!("test-{}", i));
            repo.create(metadata).await.unwrap();
        }

        let list = repo.list().await.unwrap();
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn test_exists_not_found() {
        let repo = InMemoryInstanceRepository::new();
        let exists = repo.exists("nonexistent").await.unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let repo = InMemoryInstanceRepository::new();
        let result = repo.get("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let repo = InMemoryInstanceRepository::new();
        let result = repo.delete("nonexistent").await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_default() {
        let repo = InMemoryInstanceRepository::default();
        let metadata = create_test_metadata("test-1");
        let id = metadata.instance_id.clone();
        repo.create(metadata).await.unwrap();
        assert!(repo.exists(&id).await.unwrap());
    }

    #[tokio::test]
    async fn test_arc_clone_behavior() {
        let repo: InMemoryInstanceRepository = InMemoryInstanceRepository::new();
        let repo_clone = repo.clone();

        let metadata1 = create_test_metadata("test-1");
        let metadata2 = create_test_metadata("test-2");
        let id1 = metadata1.instance_id.clone();
        let id2 = metadata2.instance_id.clone();

        repo.create(metadata1.clone()).await.unwrap();
        repo_clone.create(metadata2).await.unwrap();

        // Both repos should have the instances due to Arc
        assert!(repo.exists(&id1).await.unwrap());
        assert!(repo.exists(&id2).await.unwrap());
    }

    #[tokio::test]
    async fn test_list_empty() {
        let repo = InMemoryInstanceRepository::new();
        let list = repo.list().await.unwrap();
        assert_eq!(list.len(), 0);
    }

    #[tokio::test]
    async fn test_multiple_operations_sequence() {
        let repo = InMemoryInstanceRepository::new();
        let metadata1 = create_test_metadata("instance-1");
        let metadata2 = create_test_metadata("instance-2");
        let id1 = metadata1.instance_id.clone();
        let id2 = metadata2.instance_id.clone();

        // Create
        repo.create(metadata1).await.unwrap();
        repo.create(metadata2).await.unwrap();

        // Exists
        assert!(repo.exists(&id1).await.unwrap());
        assert!(repo.exists(&id2).await.unwrap());

        // Get
        let retrieved1 = repo.get(&id1).await.unwrap();
        assert!(retrieved1.is_some());

        // Update
        repo.update_status(&id1, InstanceStatus::Stopped)
            .await
            .unwrap();
        let updated = repo.get(&id1).await.unwrap().unwrap();
        assert_eq!(updated.status, InstanceStatus::Stopped);

        // List
        let list = repo.list().await.unwrap();
        assert_eq!(list.len(), 2);

        // Delete
        assert!(repo.delete(&id1).await.unwrap());
        assert!(!repo.exists(&id1).await.unwrap());

        // Should still have id2
        assert!(repo.exists(&id2).await.unwrap());
    }

    #[tokio::test]
    async fn test_metadata_immutability_in_repo() {
        let repo = InMemoryInstanceRepository::new();
        let id = "test-1".to_string();

        let original_metadata = create_test_metadata(&id);
        let original_status = original_metadata.status;
        let instance_id = original_metadata.instance_id.clone();

        repo.create(original_metadata).await.unwrap();

        // Modify retrieved metadata (doesn't affect stored one)
        let mut retrieved = repo.get(&instance_id).await.unwrap();
        if let Some(ref mut metadata) = retrieved {
            metadata.status = InstanceStatus::Stopped;
        }

        let stored = repo.get(&instance_id).await.unwrap();
        // Stored metadata should still have original status
        // (In real implementation, we'd need to use update_status)
        if let Some(metadata) = stored {
            assert_eq!(metadata.status, original_status);
        }
    }
}
