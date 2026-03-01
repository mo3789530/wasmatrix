use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "etcd")]
use crate::features::node_routing::repo::etcd::EtcdConfig;
use crate::shared::error::{ControlPlaneError, ControlPlaneResult};
use crate::shared::types::InstanceMetadata;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrashHistoryRecord {
    pub instance_id: String,
    pub crash_count: u32,
    pub last_crash_at: DateTime<Utc>,
    pub last_error: Option<String>,
}

#[async_trait]
pub trait PersistentMetadataRepository: Send + Sync {
    async fn upsert_instance(&self, metadata: &InstanceMetadata) -> ControlPlaneResult<()>;
    async fn get_instance(&self, instance_id: &str) -> ControlPlaneResult<Option<InstanceMetadata>>;
    async fn list_instances(&self) -> ControlPlaneResult<Vec<InstanceMetadata>>;
    async fn delete_instance(&self, instance_id: &str) -> ControlPlaneResult<bool>;
    async fn record_crash(
        &self,
        instance_id: &str,
        error: Option<String>,
    ) -> ControlPlaneResult<CrashHistoryRecord>;
    async fn get_crash_history(
        &self,
        instance_id: &str,
    ) -> ControlPlaneResult<Option<CrashHistoryRecord>>;
}

#[derive(Clone, Default)]
pub struct EtcdBackedMetadataRepository {
    storage: Arc<RwLock<HashMap<String, String>>>,
    #[cfg(feature = "etcd")]
    client: Option<Arc<tokio::sync::Mutex<etcd_client::Client>>>,
}

impl EtcdBackedMetadataRepository {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(feature = "etcd")]
            client: None,
        }
    }

    #[cfg(feature = "etcd")]
    pub async fn connect(config: &EtcdConfig) -> ControlPlaneResult<Self> {
        let client = etcd_client::Client::connect(config.endpoints.clone(), None)
            .await
            .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;

        Ok(Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
            client: Some(Arc::new(tokio::sync::Mutex::new(client))),
        })
    }

    fn instance_key(instance_id: &str) -> String {
        format!("/wasmatrix/instances/{instance_id}")
    }

    fn crash_key(instance_id: &str) -> String {
        format!("/wasmatrix/crash-history/{instance_id}")
    }

    async fn put_json<T: Serialize>(&self, key: String, value: &T) -> ControlPlaneResult<()> {
        let serialized =
            serde_json::to_string(value).map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;

        #[cfg(feature = "etcd")]
        if let Some(client) = &self.client {
            let mut client = client.lock().await;
            client
                .put(key, serialized, None)
                .await
                .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
            return Ok(());
        }

        let mut storage = self.storage.write().await;
        storage.insert(key, serialized);
        Ok(())
    }

    async fn get_json<T>(&self, key: &str) -> ControlPlaneResult<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        #[cfg(feature = "etcd")]
        if let Some(client) = &self.client {
            let mut client = client.lock().await;
            let response = client
                .get(key, None)
                .await
                .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;

            let Some(kv) = response.kvs().first() else {
                return Ok(None);
            };
            let raw = std::str::from_utf8(kv.value())
                .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
            return serde_json::from_str(raw)
                .map(Some)
                .map_err(|e| ControlPlaneError::StorageError(e.to_string()));
        }

        let storage = self.storage.read().await;
        let Some(raw) = storage.get(key) else {
            return Ok(None);
        };

        serde_json::from_str(raw)
            .map(Some)
            .map_err(|e| ControlPlaneError::StorageError(e.to_string()))
    }

    pub async fn keys(&self) -> Vec<String> {
        #[cfg(feature = "etcd")]
        if let Some(client) = &self.client {
            let mut client = client.lock().await;
            let response = client
                .get(
                    "/wasmatrix/",
                    Some(etcd_client::GetOptions::new().with_prefix()),
                )
                .await;

            return match response {
                Ok(response) => response
                    .kvs()
                    .iter()
                    .filter_map(|kv| std::str::from_utf8(kv.key()).ok().map(ToString::to_string))
                    .collect(),
                Err(_) => Vec::new(),
            };
        }

        let storage = self.storage.read().await;
        storage.keys().cloned().collect()
    }
}

#[async_trait]
impl PersistentMetadataRepository for EtcdBackedMetadataRepository {
    async fn upsert_instance(&self, metadata: &InstanceMetadata) -> ControlPlaneResult<()> {
        self.put_json(Self::instance_key(&metadata.instance_id), metadata)
            .await
    }

    async fn get_instance(&self, instance_id: &str) -> ControlPlaneResult<Option<InstanceMetadata>> {
        self.get_json(&Self::instance_key(instance_id)).await
    }

    async fn list_instances(&self) -> ControlPlaneResult<Vec<InstanceMetadata>> {
        #[cfg(feature = "etcd")]
        if let Some(client) = &self.client {
            let mut client = client.lock().await;
            let response = client
                .get(
                    "/wasmatrix/instances/",
                    Some(etcd_client::GetOptions::new().with_prefix()),
                )
                .await
                .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
            let mut instances = Vec::new();

            for kv in response.kvs() {
                let raw = std::str::from_utf8(kv.value())
                    .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
                let metadata = serde_json::from_str(raw)
                    .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
                instances.push(metadata);
            }

            return Ok(instances);
        }

        let storage = self.storage.read().await;
        let mut instances = Vec::new();

        for (key, value) in storage.iter() {
            if !key.starts_with("/wasmatrix/instances/") {
                continue;
            }

            let metadata = serde_json::from_str(value)
                .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
            instances.push(metadata);
        }

        Ok(instances)
    }

    async fn delete_instance(&self, instance_id: &str) -> ControlPlaneResult<bool> {
        #[cfg(feature = "etcd")]
        if let Some(client) = &self.client {
            let mut client = client.lock().await;
            let response = client
                .delete(Self::instance_key(instance_id), None)
                .await
                .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
            client
                .delete(Self::crash_key(instance_id), None)
                .await
                .map_err(|e| ControlPlaneError::StorageError(e.to_string()))?;
            return Ok(response.deleted() > 0);
        }

        let mut storage = self.storage.write().await;
        let removed_instance = storage.remove(&Self::instance_key(instance_id)).is_some();
        storage.remove(&Self::crash_key(instance_id));
        Ok(removed_instance)
    }

    async fn record_crash(
        &self,
        instance_id: &str,
        error: Option<String>,
    ) -> ControlPlaneResult<CrashHistoryRecord> {
        let mut crash_history = self
            .get_crash_history(instance_id)
            .await?
            .unwrap_or(CrashHistoryRecord {
                instance_id: instance_id.to_string(),
                crash_count: 0,
                last_crash_at: Utc::now(),
                last_error: None,
            });

        crash_history.crash_count += 1;
        crash_history.last_crash_at = Utc::now();
        crash_history.last_error = error;

        self.put_json(Self::crash_key(instance_id), &crash_history).await?;
        Ok(crash_history)
    }

    async fn get_crash_history(
        &self,
        instance_id: &str,
    ) -> ControlPlaneResult<Option<CrashHistoryRecord>> {
        self.get_json(&Self::crash_key(instance_id)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::types::InstanceStatus;

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
    async fn test_upsert_and_get_instance_metadata() {
        let repo = EtcdBackedMetadataRepository::new();
        let metadata = test_instance_metadata();

        repo.upsert_instance(&metadata).await.unwrap();

        let stored = repo.get_instance(&metadata.instance_id).await.unwrap().unwrap();
        assert_eq!(stored.instance_id, metadata.instance_id);
        assert_eq!(stored.node_id, metadata.node_id);
    }

    #[tokio::test]
    async fn test_record_crash_history() {
        let repo = EtcdBackedMetadataRepository::new();

        let first = repo
            .record_crash("inst-1", Some("boom".to_string()))
            .await
            .unwrap();
        let second = repo.record_crash("inst-1", None).await.unwrap();

        assert_eq!(first.crash_count, 1);
        assert_eq!(second.crash_count, 2);
        assert_eq!(repo.keys().await.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_removes_instance_and_crash_history() {
        let repo = EtcdBackedMetadataRepository::new();
        let metadata = test_instance_metadata();

        repo.upsert_instance(&metadata).await.unwrap();
        repo.record_crash(&metadata.instance_id, None).await.unwrap();

        assert!(repo.delete_instance(&metadata.instance_id).await.unwrap());
        assert!(repo.get_instance(&metadata.instance_id).await.unwrap().is_none());
        assert!(repo
            .get_crash_history(&metadata.instance_id)
            .await
            .unwrap()
            .is_none());
    }
}
