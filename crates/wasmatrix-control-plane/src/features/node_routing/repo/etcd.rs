use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EtcdMetadataKind {
    Node,
    Provider,
}

#[derive(Clone, Default)]
pub struct EtcdMetadataRepository {
    storage: Arc<RwLock<HashMap<String, String>>>,
}

impl EtcdMetadataRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn put_node_presence(
        &self,
        node_id: &str,
        node_address: &str,
        heartbeat: DateTime<Utc>,
    ) -> Result<(), String> {
        let key = format!("/wasmatrix/nodes/{node_id}");
        let value = format!(
            "{{\"node_id\":\"{}\",\"node_address\":\"{}\",\"heartbeat\":\"{}\"}}",
            node_id,
            node_address,
            heartbeat.to_rfc3339()
        );
        self.put_limited_metadata(&key, value).await
    }

    pub async fn put_provider_metadata(
        &self,
        provider_id: &str,
        provider_type: &str,
        node_id: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<(), String> {
        let key = format!("/wasmatrix/providers/{provider_id}");
        let value = format!(
            "{{\"provider_id\":\"{}\",\"provider_type\":\"{}\",\"node_id\":\"{}\",\"updated_at\":\"{}\"}}",
            provider_id,
            provider_type,
            node_id,
            updated_at.to_rfc3339()
        );
        self.put_limited_metadata(&key, value).await
    }

    pub async fn put_limited_metadata(&self, key: &str, value: String) -> Result<(), String> {
        match classify_key(key) {
            Some(EtcdMetadataKind::Node) | Some(EtcdMetadataKind::Provider) => {
                let mut storage = self.storage.write().await;
                storage.insert(key.to_string(), value);
                Ok(())
            }
            None => Err(format!("disallowed etcd key: {key}")),
        }
    }

    pub async fn keys(&self) -> Vec<String> {
        let storage = self.storage.read().await;
        storage.keys().cloned().collect()
    }
}

pub struct EtcdConfig {
    pub endpoints: Vec<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl EtcdConfig {
    pub fn from_env() -> Option<Self> {
        let endpoints_raw = std::env::var("ETCD_ENDPOINTS").ok()?;
        let endpoints: Vec<String> = endpoints_raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();

        if endpoints.is_empty() {
            return None;
        }

        Some(Self {
            endpoints,
            username: std::env::var("ETCD_USERNAME").ok(),
            password: std::env::var("ETCD_PASSWORD").ok(),
        })
    }
}

#[cfg(feature = "etcd")]
pub async fn validate_etcd_config(_config: &EtcdConfig) -> Result<(), String> {
    let _type_marker = std::any::TypeId::of::<etcd_client::Client>();
    Ok(())
}

#[cfg(not(feature = "etcd"))]
pub async fn validate_etcd_config(_config: &EtcdConfig) -> Result<(), String> {
    Err("etcd feature is not enabled".to_string())
}

pub fn classify_key(key: &str) -> Option<EtcdMetadataKind> {
    if key.starts_with("/wasmatrix/nodes/") {
        return Some(EtcdMetadataKind::Node);
    }
    if key.starts_with("/wasmatrix/providers/") {
        return Some(EtcdMetadataKind::Provider);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_etcd_config_none_when_missing_endpoints() {
        unsafe {
            std::env::remove_var("ETCD_ENDPOINTS");
        }
        assert!(EtcdConfig::from_env().is_none());
    }

    // Property 11: etcd Limited Usage When Enabled
    // Metadata keys are constrained to nodes/providers prefixes only.
    #[test]
    fn property_etcd_limited_usage_key_classification() {
        for i in 0..100 {
            let allowed_node = format!("/wasmatrix/nodes/node-{i}");
            let allowed_provider = format!("/wasmatrix/providers/provider-{i}");
            let disallowed_instance = format!("/wasmatrix/instances/instance-{i}");
            let disallowed_logs = format!("/wasmatrix/logs/instance-{i}");
            let disallowed_desired = format!("/wasmatrix/desired/instance-{i}");

            assert_eq!(classify_key(&allowed_node), Some(EtcdMetadataKind::Node));
            assert_eq!(
                classify_key(&allowed_provider),
                Some(EtcdMetadataKind::Provider)
            );
            assert_eq!(classify_key(&disallowed_instance), None);
            assert_eq!(classify_key(&disallowed_logs), None);
            assert_eq!(classify_key(&disallowed_desired), None);
        }
    }

    #[tokio::test]
    async fn test_etcd_node_registration_storage() {
        let repo = EtcdMetadataRepository::new();
        repo.put_node_presence("node-1", "127.0.0.1:50052", Utc::now())
            .await
            .unwrap();

        let keys = repo.keys().await;
        assert_eq!(keys.len(), 1);
        assert!(keys.iter().any(|k| k.starts_with("/wasmatrix/nodes/")));
    }

    #[tokio::test]
    async fn test_etcd_provider_metadata_storage() {
        let repo = EtcdMetadataRepository::new();
        repo.put_provider_metadata("kv-1", "kv", "node-1", Utc::now())
            .await
            .unwrap();

        let keys = repo.keys().await;
        assert_eq!(keys.len(), 1);
        assert!(keys.iter().any(|k| k.starts_with("/wasmatrix/providers/")));
    }

    #[tokio::test]
    async fn test_etcd_rejects_instance_state_storage() {
        let repo = EtcdMetadataRepository::new();
        let result = repo
            .put_limited_metadata("/wasmatrix/instances/instance-1", "{}".to_string())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_operation_without_etcd_feature() {
        let config = EtcdConfig {
            endpoints: vec!["http://127.0.0.1:2379".to_string()],
            username: None,
            password: None,
        };

        let result = validate_etcd_config(&config).await;
        if cfg!(feature = "etcd") {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
        }
    }
}
