pub mod etcd;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::shared::error::{ControlPlaneError, ControlPlaneResult};

#[derive(Debug, Clone)]
pub struct NodeAgentRecord {
    pub node_id: String,
    pub node_address: String,
    pub capabilities: Vec<String>,
    pub max_instances: u32,
    pub active_instances: u32,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub available: bool,
}

#[derive(Debug, Clone)]
pub struct ProviderMetadata {
    pub provider_id: String,
    pub provider_type: String,
    pub node_id: String,
    pub last_updated: DateTime<Utc>,
}

#[async_trait]
pub trait NodeRoutingRepository: Send + Sync {
    async fn upsert_node(&self, node: NodeAgentRecord) -> ControlPlaneResult<()>;
    async fn get_node(&self, node_id: &str) -> ControlPlaneResult<Option<NodeAgentRecord>>;
    async fn list_nodes(&self) -> ControlPlaneResult<Vec<NodeAgentRecord>>;
    async fn update_heartbeat(
        &self,
        node_id: &str,
        heartbeat: DateTime<Utc>,
    ) -> ControlPlaneResult<()>;
    async fn set_availability(&self, node_id: &str, available: bool) -> ControlPlaneResult<()>;
    async fn increment_active_instances(&self, node_id: &str) -> ControlPlaneResult<()>;
    async fn decrement_active_instances(&self, node_id: &str) -> ControlPlaneResult<()>;
    async fn set_active_instances(&self, node_id: &str, count: u32) -> ControlPlaneResult<()>;
    async fn assign_instance(&self, instance_id: String, node_id: String)
        -> ControlPlaneResult<()>;
    async fn lookup_instance_node(&self, instance_id: &str) -> ControlPlaneResult<Option<String>>;
    async fn remove_instance_assignment(
        &self,
        instance_id: &str,
    ) -> ControlPlaneResult<Option<String>>;
    async fn upsert_provider_metadata(&self, provider: ProviderMetadata) -> ControlPlaneResult<()>;
    async fn list_provider_metadata(&self) -> ControlPlaneResult<Vec<ProviderMetadata>>;
}

#[derive(Clone, Default)]
pub struct InMemoryNodeRoutingRepository {
    nodes: Arc<RwLock<HashMap<String, NodeAgentRecord>>>,
    assignments: Arc<RwLock<HashMap<String, String>>>,
    providers: Arc<RwLock<HashMap<String, ProviderMetadata>>>,
}

impl InMemoryNodeRoutingRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl NodeRoutingRepository for InMemoryNodeRoutingRepository {
    async fn upsert_node(&self, node: NodeAgentRecord) -> ControlPlaneResult<()> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.node_id.clone(), node);
        Ok(())
    }

    async fn get_node(&self, node_id: &str) -> ControlPlaneResult<Option<NodeAgentRecord>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.get(node_id).cloned())
    }

    async fn list_nodes(&self) -> ControlPlaneResult<Vec<NodeAgentRecord>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().cloned().collect())
    }

    async fn update_heartbeat(
        &self,
        node_id: &str,
        heartbeat: DateTime<Utc>,
    ) -> ControlPlaneResult<()> {
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .get_mut(node_id)
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(format!("node {}", node_id)))?;

        node.last_heartbeat = Some(heartbeat);
        node.available = true;
        Ok(())
    }

    async fn set_availability(&self, node_id: &str, available: bool) -> ControlPlaneResult<()> {
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .get_mut(node_id)
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(format!("node {}", node_id)))?;
        node.available = available;
        Ok(())
    }

    async fn increment_active_instances(&self, node_id: &str) -> ControlPlaneResult<()> {
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .get_mut(node_id)
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(format!("node {}", node_id)))?;
        node.active_instances = node.active_instances.saturating_add(1);
        Ok(())
    }

    async fn decrement_active_instances(&self, node_id: &str) -> ControlPlaneResult<()> {
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .get_mut(node_id)
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(format!("node {}", node_id)))?;
        node.active_instances = node.active_instances.saturating_sub(1);
        Ok(())
    }

    async fn set_active_instances(&self, node_id: &str, count: u32) -> ControlPlaneResult<()> {
        let mut nodes = self.nodes.write().await;
        let node = nodes
            .get_mut(node_id)
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(format!("node {}", node_id)))?;
        node.active_instances = count;
        Ok(())
    }

    async fn assign_instance(
        &self,
        instance_id: String,
        node_id: String,
    ) -> ControlPlaneResult<()> {
        let mut assignments = self.assignments.write().await;
        assignments.insert(instance_id, node_id);
        Ok(())
    }

    async fn lookup_instance_node(&self, instance_id: &str) -> ControlPlaneResult<Option<String>> {
        let assignments = self.assignments.read().await;
        Ok(assignments.get(instance_id).cloned())
    }

    async fn remove_instance_assignment(
        &self,
        instance_id: &str,
    ) -> ControlPlaneResult<Option<String>> {
        let mut assignments = self.assignments.write().await;
        Ok(assignments.remove(instance_id))
    }

    async fn upsert_provider_metadata(&self, provider: ProviderMetadata) -> ControlPlaneResult<()> {
        let mut providers = self.providers.write().await;
        providers.insert(provider.provider_id.clone(), provider);
        Ok(())
    }

    async fn list_provider_metadata(&self) -> ControlPlaneResult<Vec<ProviderMetadata>> {
        let providers = self.providers.read().await;
        Ok(providers.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_lookup_node() {
        let repo = InMemoryNodeRoutingRepository::new();
        repo.upsert_node(NodeAgentRecord {
            node_id: "node-1".to_string(),
            node_address: "http://127.0.0.1:50052".to_string(),
            capabilities: vec![],
            max_instances: 10,
            active_instances: 0,
            last_heartbeat: None,
            available: true,
        })
        .await
        .unwrap();

        let node = repo.get_node("node-1").await.unwrap();
        assert!(node.is_some());
    }

    #[tokio::test]
    async fn test_assignment_roundtrip() {
        let repo = InMemoryNodeRoutingRepository::new();
        repo.assign_instance("instance-1".to_string(), "node-1".to_string())
            .await
            .unwrap();

        let node_id = repo.lookup_instance_node("instance-1").await.unwrap();
        assert_eq!(node_id.as_deref(), Some("node-1"));

        let removed = repo.remove_instance_assignment("instance-1").await.unwrap();
        assert_eq!(removed.as_deref(), Some("node-1"));
    }

    #[tokio::test]
    async fn test_provider_metadata_stored_separately_from_instances() {
        let repo = InMemoryNodeRoutingRepository::new();

        repo.upsert_provider_metadata(ProviderMetadata {
            provider_id: "kv-provider-1".to_string(),
            provider_type: "kv".to_string(),
            node_id: "node-1".to_string(),
            last_updated: Utc::now(),
        })
        .await
        .unwrap();

        repo.assign_instance("instance-1".to_string(), "node-1".to_string())
            .await
            .unwrap();

        let providers = repo.list_provider_metadata().await.unwrap();
        let assignment_node = repo.lookup_instance_node("instance-1").await.unwrap();

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].provider_id, "kv-provider-1");
        assert_eq!(assignment_node.as_deref(), Some("node-1"));
    }
}
