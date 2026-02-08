use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use tonic::transport::Channel;
use tracing::warn;
use wasmatrix_proto::v1::node_agent_service_client::NodeAgentServiceClient;
use wasmatrix_proto::v1::{
    ListInstancesRequest, QueryInstanceRequest, StartInstanceRequest as ProtoStartInstanceRequest,
    StopInstanceRequest,
};

use crate::features::node_routing::repo::etcd::EtcdMetadataRepository;
use crate::features::node_routing::repo::{
    NodeAgentRecord, NodeRoutingRepository, ProviderMetadata,
};
use crate::shared::error::{ControlPlaneError, ControlPlaneResult};
use crate::shared::types::{
    InstanceMetadata, InstanceStatusResponse, QueryInstanceRequest as CoreQueryRequest,
    StartInstanceRequest,
};
use crate::ControlPlane;
use std::sync::Mutex;

pub struct NodeRoutingService {
    repo: Arc<dyn NodeRoutingRepository>,
    etcd_metadata_repo: Option<Arc<EtcdMetadataRepository>>,
}

impl NodeRoutingService {
    pub fn new(repo: Arc<dyn NodeRoutingRepository>) -> Self {
        Self {
            repo,
            etcd_metadata_repo: None,
        }
    }

    pub fn new_with_etcd(
        repo: Arc<dyn NodeRoutingRepository>,
        etcd_metadata_repo: Arc<EtcdMetadataRepository>,
    ) -> Self {
        Self {
            repo,
            etcd_metadata_repo: Some(etcd_metadata_repo),
        }
    }

    pub async fn register_node(
        &self,
        node_id: String,
        node_address: String,
        capabilities: Vec<String>,
        max_instances: u32,
    ) -> ControlPlaneResult<()> {
        self.repo
            .upsert_node(NodeAgentRecord {
                node_id: node_id.clone(),
                node_address: normalize_endpoint(&node_address),
                capabilities,
                max_instances,
                active_instances: 0,
                last_heartbeat: Some(Utc::now()),
                available: true,
            })
            .await?;

        if let Some(etcd_repo) = &self.etcd_metadata_repo {
            etcd_repo
                .put_node_presence(&node_id, &normalize_endpoint(&node_address), Utc::now())
                .await
                .map_err(ControlPlaneError::StorageError)?;
        }

        Ok(())
    }

    pub async fn register_provider_metadata(
        &self,
        provider_id: String,
        provider_type: String,
        node_id: String,
    ) -> ControlPlaneResult<()> {
        self.repo
            .upsert_provider_metadata(ProviderMetadata {
                provider_id: provider_id.clone(),
                provider_type: provider_type.clone(),
                node_id: node_id.clone(),
                last_updated: Utc::now(),
            })
            .await?;

        if let Some(etcd_repo) = &self.etcd_metadata_repo {
            etcd_repo
                .put_provider_metadata(&provider_id, &provider_type, &node_id, Utc::now())
                .await
                .map_err(ControlPlaneError::StorageError)?;
        }

        Ok(())
    }

    pub async fn record_status_report(
        &self,
        node_id: &str,
        timestamp: i64,
    ) -> ControlPlaneResult<()> {
        let heartbeat = unix_to_utc(timestamp).ok_or_else(|| {
            ControlPlaneError::ValidationError("invalid status report timestamp".to_string())
        })?;
        self.repo.update_heartbeat(node_id, heartbeat).await
    }

    pub async fn route_start_instance(
        &self,
        request: StartInstanceRequest,
    ) -> ControlPlaneResult<String> {
        let nodes = self.repo.list_nodes().await?;
        let candidates = select_candidate_nodes(nodes, &request);

        if candidates.is_empty() {
            return Err(ControlPlaneError::ResourceExhausted(
                "No registered node agents".to_string(),
            ));
        }

        let mut errors = Vec::new();
        let instance_id = uuid::Uuid::new_v4().to_string();

        for node in candidates {
            let mut client = match connect_client(&node.node_address).await {
                Ok(client) => client,
                Err(error) => {
                    errors.push(format!("{}: {}", node.node_id, error));
                    let _ = self.repo.set_availability(&node.node_id, false).await;
                    continue;
                }
            };

            let req = ProtoStartInstanceRequest {
                instance_id: instance_id.clone(),
                module_bytes: request.module_bytes.clone(),
                capabilities: request
                    .capabilities
                    .iter()
                    .map(|cap| {
                        wasmatrix_proto::protocol::CapabilityAssignment {
                            instance_id: instance_id.clone(),
                            capability_id: cap.capability_id.clone(),
                            provider_type: cap.provider_type.into(),
                            permissions: cap.permissions.clone(),
                        }
                        .into()
                    })
                    .collect(),
                restart_policy: Some(
                    wasmatrix_proto::protocol::RestartPolicy::from(request.restart_policy.clone())
                        .into(),
                ),
            };

            match client.start_instance(tonic::Request::new(req)).await {
                Ok(response) if response.get_ref().success => {
                    self.repo
                        .assign_instance(instance_id.clone(), node.node_id.clone())
                        .await?;
                    self.repo.increment_active_instances(&node.node_id).await?;
                    self.repo.set_availability(&node.node_id, true).await?;
                    return Ok(instance_id);
                }
                Ok(response) => {
                    errors.push(format!("{}: {}", node.node_id, response.get_ref().message));
                }
                Err(error) => {
                    errors.push(format!("{}: {}", node.node_id, error));
                    let _ = self.repo.set_availability(&node.node_id, false).await;
                }
            }
        }

        Err(ControlPlaneError::Timeout(format!(
            "No available node agent accepted start request: {}",
            errors.join(" | ")
        )))
    }

    pub async fn route_stop_instance(&self, instance_id: &str) -> ControlPlaneResult<()> {
        let node_id = self
            .repo
            .lookup_instance_node(instance_id)
            .await?
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(instance_id.to_string()))?;

        let node = self
            .repo
            .get_node(&node_id)
            .await?
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(format!("node {}", node_id)))?;

        let mut client = connect_client(&node.node_address)
            .await
            .map_err(ControlPlaneError::Timeout)?;

        let response = client
            .stop_instance(tonic::Request::new(StopInstanceRequest {
                instance_id: instance_id.to_string(),
            }))
            .await
            .map_err(|e| ControlPlaneError::Timeout(e.to_string()))?;

        if !response.get_ref().success {
            return Err(ControlPlaneError::WasmRuntimeError(
                response.get_ref().message.clone(),
            ));
        }

        self.repo.remove_instance_assignment(instance_id).await?;
        self.repo.decrement_active_instances(&node_id).await?;
        Ok(())
    }

    pub async fn route_query_instance(
        &self,
        request: CoreQueryRequest,
    ) -> ControlPlaneResult<InstanceStatusResponse> {
        let node_id = self
            .repo
            .lookup_instance_node(&request.instance_id)
            .await?
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(request.instance_id.clone()))?;

        let node = self
            .repo
            .get_node(&node_id)
            .await?
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(format!("node {}", node_id)))?;

        let mut client = connect_client(&node.node_address)
            .await
            .map_err(ControlPlaneError::Timeout)?;

        let response = client
            .query_instance(tonic::Request::new(QueryInstanceRequest {
                instance_id: request.instance_id.clone(),
            }))
            .await
            .map_err(|e| ControlPlaneError::Timeout(e.to_string()))?;

        if !response.get_ref().success {
            return Err(ControlPlaneError::WasmRuntimeError(
                response
                    .get_ref()
                    .error_code
                    .clone()
                    .unwrap_or_else(|| "QUERY_FAILED".to_string()),
            ));
        }

        let meta = response
            .get_ref()
            .instance
            .clone()
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(request.instance_id.clone()))?;

        let status_proto =
            wasmatrix_proto::v1::InstanceStatus::try_from(meta.status).map_err(|_| {
                ControlPlaneError::ValidationError("invalid instance status".to_string())
            })?;
        let status = wasmatrix_proto::protocol::InstanceStatus::try_from(status_proto)
            .map_err(ControlPlaneError::ValidationError)?
            .into();
        let created_at = unix_to_utc(meta.created_at).ok_or_else(|| {
            ControlPlaneError::ValidationError("invalid created_at timestamp".to_string())
        })?;

        Ok(InstanceStatusResponse {
            instance_id: meta.instance_id,
            status,
            node_id: meta.node_id,
            created_at,
        })
    }

    pub async fn route_list_instances(&self) -> ControlPlaneResult<Vec<InstanceMetadata>> {
        let nodes = self.repo.list_nodes().await?;
        let mut all_instances = Vec::new();

        for node in nodes {
            let mut client = match connect_client(&node.node_address).await {
                Ok(client) => client,
                Err(error) => {
                    warn!(node_id = %node.node_id, error = %error, "Skipping unavailable node during list");
                    let _ = self.repo.set_availability(&node.node_id, false).await;
                    continue;
                }
            };

            let response = match client
                .list_instances(tonic::Request::new(ListInstancesRequest {}))
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    warn!(node_id = %node.node_id, error = %error, "ListInstances failed for node");
                    let _ = self.repo.set_availability(&node.node_id, false).await;
                    continue;
                }
            };

            if !response.get_ref().success {
                continue;
            }

            for meta in &response.get_ref().instances {
                let status_proto = match wasmatrix_proto::v1::InstanceStatus::try_from(meta.status)
                {
                    Ok(status) => status,
                    Err(_) => continue,
                };
                let status = match wasmatrix_proto::protocol::InstanceStatus::try_from(status_proto)
                {
                    Ok(status) => status,
                    Err(_) => continue,
                };
                let created_at = match unix_to_utc(meta.created_at) {
                    Some(ts) => ts,
                    None => continue,
                };

                all_instances.push(InstanceMetadata {
                    instance_id: meta.instance_id.clone(),
                    node_id: meta.node_id.clone(),
                    module_hash: meta.module_hash.clone(),
                    created_at,
                    status: status.into(),
                });
            }
        }

        Ok(all_instances)
    }

    /// Recover control-plane state for a registered node by querying NodeAgent `ListInstances`.
    pub async fn recover_node_state(
        &self,
        node_id: &str,
        control_plane: &Mutex<ControlPlane>,
    ) -> ControlPlaneResult<usize> {
        let node = self
            .repo
            .get_node(node_id)
            .await?
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(format!("node {}", node_id)))?;

        let mut client = connect_client(&node.node_address)
            .await
            .map_err(ControlPlaneError::Timeout)?;

        let response = client
            .list_instances(tonic::Request::new(ListInstancesRequest {}))
            .await
            .map_err(|e| ControlPlaneError::Timeout(e.to_string()))?;

        if !response.get_ref().success {
            return Err(ControlPlaneError::WasmRuntimeError(
                "failed to recover node instances".to_string(),
            ));
        }

        self.apply_recovered_instances(node_id, response.get_ref().instances.clone(), control_plane)
            .await
    }

    async fn apply_recovered_instances(
        &self,
        node_id: &str,
        instances: Vec<wasmatrix_proto::v1::InstanceMetadata>,
        control_plane: &Mutex<ControlPlane>,
    ) -> ControlPlaneResult<usize> {
        let mut recovered = 0usize;
        let mut active_count = 0u32;

        for meta in instances {
            let status_proto =
                wasmatrix_proto::v1::InstanceStatus::try_from(meta.status).map_err(|_| {
                    ControlPlaneError::ValidationError("invalid instance status".to_string())
                })?;
            let status: wasmatrix_core::InstanceStatus =
                wasmatrix_proto::protocol::InstanceStatus::try_from(status_proto)
                    .map_err(ControlPlaneError::ValidationError)?
                    .into();
            let created_at = unix_to_utc(meta.created_at).ok_or_else(|| {
                ControlPlaneError::ValidationError("invalid created_at timestamp".to_string())
            })?;

            if matches!(
                status,
                wasmatrix_core::InstanceStatus::Starting | wasmatrix_core::InstanceStatus::Running
            ) {
                active_count = active_count.saturating_add(1);
            }

            let metadata = wasmatrix_core::InstanceMetadata {
                instance_id: meta.instance_id.clone(),
                node_id: meta.node_id,
                module_hash: meta.module_hash,
                created_at,
                status,
            };

            {
                let mut cp = control_plane.lock().map_err(|_| {
                    ControlPlaneError::StorageError("control plane lock poisoned".to_string())
                })?;
                cp.restore_instance_state(metadata, vec![]);
            }

            self.repo
                .assign_instance(meta.instance_id, node_id.to_string())
                .await?;
            recovered += 1;
        }

        self.repo
            .set_active_instances(node_id, active_count)
            .await?;
        Ok(recovered)
    }
}

fn can_accept_instance(node: &NodeAgentRecord) -> bool {
    node.available && (node.max_instances == 0 || node.active_instances < node.max_instances)
}

fn node_supports_required_providers(
    node: &NodeAgentRecord,
    request: &StartInstanceRequest,
) -> bool {
    let required = required_provider_types(request);
    if required.is_empty() || node.capabilities.is_empty() {
        return true;
    }

    required
        .into_iter()
        .all(|provider| node.capabilities.iter().any(|cap| cap == &provider))
}

fn required_provider_types(request: &StartInstanceRequest) -> Vec<String> {
    let mut providers: Vec<String> = request
        .capabilities
        .iter()
        .map(|cap| match cap.provider_type {
            wasmatrix_core::ProviderType::Kv => "kv".to_string(),
            wasmatrix_core::ProviderType::Http => "http".to_string(),
            wasmatrix_core::ProviderType::Messaging => "messaging".to_string(),
        })
        .collect();
    providers.sort();
    providers.dedup();
    providers
}

fn select_candidate_nodes(
    mut nodes: Vec<NodeAgentRecord>,
    request: &StartInstanceRequest,
) -> Vec<NodeAgentRecord> {
    nodes.retain(|node| {
        can_accept_instance(node) && node_supports_required_providers(node, request)
    });
    nodes.sort_by_key(|n| n.active_instances);
    nodes
}

fn normalize_endpoint(address: &str) -> String {
    if address.starts_with("http://") || address.starts_with("https://") {
        address.to_string()
    } else {
        format!("http://{}", address)
    }
}

fn unix_to_utc(ts: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(ts, 0).single()
}

async fn connect_client(address: &str) -> Result<NodeAgentServiceClient<Channel>, String> {
    NodeAgentServiceClient::connect(address.to_string())
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::node_routing::repo::etcd::EtcdMetadataRepository;
    use crate::features::node_routing::repo::InMemoryNodeRoutingRepository;
    use crate::shared::types::RestartPolicy;

    #[tokio::test]
    async fn test_start_route_without_nodes() {
        let repo = Arc::new(InMemoryNodeRoutingRepository::new());
        let service = NodeRoutingService::new(repo);

        let result = service
            .route_start_instance(StartInstanceRequest {
                module_bytes: vec![0, 97, 115, 109, 1, 0, 0, 0],
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_route_node_unavailable() {
        let repo = Arc::new(InMemoryNodeRoutingRepository::new());
        let service = NodeRoutingService::new(repo.clone());

        service
            .register_node(
                "node-1".to_string(),
                "127.0.0.1:65099".to_string(),
                vec![],
                10,
            )
            .await
            .unwrap();

        let result = service
            .route_start_instance(StartInstanceRequest {
                module_bytes: vec![0, 97, 115, 109, 1, 0, 0, 0],
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            })
            .await;

        assert!(result.is_err());
        let node = repo.get_node("node-1").await.unwrap().unwrap();
        assert!(!node.available);
    }

    #[tokio::test]
    async fn test_register_node_persists_etcd_metadata_when_enabled() {
        let repo = Arc::new(InMemoryNodeRoutingRepository::new());
        let etcd_repo = Arc::new(EtcdMetadataRepository::new());
        let service = NodeRoutingService::new_with_etcd(repo, etcd_repo.clone());

        service
            .register_node(
                "node-1".to_string(),
                "127.0.0.1:50052".to_string(),
                vec![],
                10,
            )
            .await
            .unwrap();

        let keys = etcd_repo.keys().await;
        assert!(keys.iter().any(|k| k.starts_with("/wasmatrix/nodes/")));
        assert!(!keys.iter().any(|k| k.contains("/instances/")));
    }

    #[tokio::test]
    async fn test_register_provider_metadata_persists_etcd_metadata_when_enabled() {
        let repo = Arc::new(InMemoryNodeRoutingRepository::new());
        let etcd_repo = Arc::new(EtcdMetadataRepository::new());
        let service = NodeRoutingService::new_with_etcd(repo, etcd_repo.clone());

        service
            .register_provider_metadata(
                "provider-kv-1".to_string(),
                "kv".to_string(),
                "node-1".to_string(),
            )
            .await
            .unwrap();

        let keys = etcd_repo.keys().await;
        assert!(keys.iter().any(|k| k.starts_with("/wasmatrix/providers/")));
        assert!(!keys.iter().any(|k| k.contains("/instances/")));
    }

    #[tokio::test]
    async fn test_recover_node_state_applies_instance_statuses() {
        let repo = Arc::new(InMemoryNodeRoutingRepository::new());
        let service = NodeRoutingService::new(repo.clone());
        let control_plane = Mutex::new(ControlPlane::new("cp-node"));

        service
            .register_node(
                "node-1".to_string(),
                "127.0.0.1:50052".to_string(),
                vec![],
                10,
            )
            .await
            .unwrap();

        let recovered_instances = vec![
            wasmatrix_proto::v1::InstanceMetadata {
                instance_id: "inst-a".to_string(),
                node_id: "node-1".to_string(),
                module_hash: "hash-a".to_string(),
                created_at: 1_700_000_000,
                status: wasmatrix_proto::v1::InstanceStatus::Running as i32,
            },
            wasmatrix_proto::v1::InstanceMetadata {
                instance_id: "inst-b".to_string(),
                node_id: "node-1".to_string(),
                module_hash: "hash-b".to_string(),
                created_at: 1_700_000_001,
                status: wasmatrix_proto::v1::InstanceStatus::Stopped as i32,
            },
        ];

        let recovered = service
            .apply_recovered_instances("node-1", recovered_instances, &control_plane)
            .await
            .unwrap();
        assert_eq!(recovered, 2);

        let cp = control_plane.lock().unwrap();
        let inst_a = cp
            .query_instance(wasmatrix_core::QueryInstanceRequest {
                instance_id: "inst-a".to_string(),
            })
            .unwrap();
        let inst_b = cp
            .query_instance(wasmatrix_core::QueryInstanceRequest {
                instance_id: "inst-b".to_string(),
            })
            .unwrap();
        assert_eq!(inst_a.status, wasmatrix_core::InstanceStatus::Running);
        assert_eq!(inst_b.status, wasmatrix_core::InstanceStatus::Stopped);
    }

    // Property 12: Node Failure Resilience
    #[test]
    fn property_node_failure_resilience_candidate_selection() {
        for i in 0..100 {
            let request = StartInstanceRequest {
                module_bytes: vec![0, 97, 115, 109, 1, 0, 0, 0],
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };

            let nodes = vec![
                NodeAgentRecord {
                    node_id: format!("failed-{i}"),
                    node_address: "http://127.0.0.1:9".to_string(),
                    capabilities: vec![],
                    max_instances: 10,
                    active_instances: 0,
                    last_heartbeat: Some(Utc::now()),
                    available: false,
                },
                NodeAgentRecord {
                    node_id: format!("healthy-{i}"),
                    node_address: "http://127.0.0.1:8".to_string(),
                    capabilities: vec![],
                    max_instances: 10,
                    active_instances: 1,
                    last_heartbeat: Some(Utc::now()),
                    available: true,
                },
            ];

            let selected = select_candidate_nodes(nodes, &request);
            assert_eq!(selected.len(), 1);
            assert!(selected[0].node_id.starts_with("healthy-"));
        }
    }

    #[test]
    fn test_select_candidate_nodes_prefers_lower_load() {
        let request = StartInstanceRequest {
            module_bytes: vec![0, 97, 115, 109, 1, 0, 0, 0],
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let nodes = vec![
            NodeAgentRecord {
                node_id: "node-2".to_string(),
                node_address: "http://127.0.0.1:2".to_string(),
                capabilities: vec![],
                max_instances: 10,
                active_instances: 5,
                last_heartbeat: Some(Utc::now()),
                available: true,
            },
            NodeAgentRecord {
                node_id: "node-1".to_string(),
                node_address: "http://127.0.0.1:1".to_string(),
                capabilities: vec![],
                max_instances: 10,
                active_instances: 1,
                last_heartbeat: Some(Utc::now()),
                available: true,
            },
        ];

        let selected = select_candidate_nodes(nodes, &request);
        assert_eq!(selected.first().map(|n| n.node_id.as_str()), Some("node-1"));
    }

    #[test]
    fn test_select_candidate_nodes_filters_by_provider_capability() {
        let request = StartInstanceRequest {
            module_bytes: vec![0, 97, 115, 109, 1, 0, 0, 0],
            capabilities: vec![wasmatrix_core::CapabilityAssignment {
                instance_id: "i-1".to_string(),
                capability_id: "http-1".to_string(),
                provider_type: wasmatrix_core::ProviderType::Http,
                permissions: vec!["http:request".to_string()],
            }],
            restart_policy: RestartPolicy::default(),
        };

        let nodes = vec![
            NodeAgentRecord {
                node_id: "node-kv".to_string(),
                node_address: "http://127.0.0.1:2".to_string(),
                capabilities: vec!["kv".to_string()],
                max_instances: 10,
                active_instances: 0,
                last_heartbeat: Some(Utc::now()),
                available: true,
            },
            NodeAgentRecord {
                node_id: "node-http".to_string(),
                node_address: "http://127.0.0.1:1".to_string(),
                capabilities: vec!["http".to_string()],
                max_instances: 10,
                active_instances: 0,
                last_heartbeat: Some(Utc::now()),
                available: true,
            },
        ];

        let selected = select_candidate_nodes(nodes, &request);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].node_id, "node-http");
    }

    #[test]
    fn test_multi_node_distribution_selects_least_loaded() {
        let request = StartInstanceRequest {
            module_bytes: vec![0, 97, 115, 109, 1, 0, 0, 0],
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let nodes = vec![
            NodeAgentRecord {
                node_id: "node-1".to_string(),
                node_address: "http://127.0.0.1:50052".to_string(),
                capabilities: vec![],
                max_instances: 10,
                active_instances: 2,
                last_heartbeat: Some(Utc::now()),
                available: true,
            },
            NodeAgentRecord {
                node_id: "node-2".to_string(),
                node_address: "http://127.0.0.1:50053".to_string(),
                capabilities: vec![],
                max_instances: 10,
                active_instances: 0,
                last_heartbeat: Some(Utc::now()),
                available: true,
            },
        ];

        let selected = select_candidate_nodes(nodes, &request);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].node_id, "node-2");
    }

    #[test]
    fn test_multi_node_failure_handling_excludes_unavailable_nodes() {
        let request = StartInstanceRequest {
            module_bytes: vec![0, 97, 115, 109, 1, 0, 0, 0],
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let nodes = vec![
            NodeAgentRecord {
                node_id: "failing-node".to_string(),
                node_address: "http://127.0.0.1:65098".to_string(),
                capabilities: vec![],
                max_instances: 10,
                active_instances: 0,
                last_heartbeat: Some(Utc::now()),
                available: false,
            },
            NodeAgentRecord {
                node_id: "healthy-node".to_string(),
                node_address: "http://127.0.0.1:50053".to_string(),
                capabilities: vec![],
                max_instances: 10,
                active_instances: 1,
                last_heartbeat: Some(Utc::now()),
                available: true,
            },
        ];

        let selected = select_candidate_nodes(nodes, &request);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].node_id, "healthy-node");
    }
}
