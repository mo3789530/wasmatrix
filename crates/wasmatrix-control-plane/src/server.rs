use crate::features::leader_election::controller::LeaderElectionController;
use crate::features::metadata_persistence::controller::MetadataPersistenceController;
use crate::features::node_routing::controller::NodeRoutingController;
use crate::features::observability::controller::global_observability_controller;
use crate::ControlPlane;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tonic::{Request, Response, Status};
use wasmatrix_proto::v1::control_plane_service_server::ControlPlaneService;
use wasmatrix_proto::v1::{
    RegisterNodeRequest, RegisterNodeResponse, StatusReport, StatusReportResponse,
};

pub struct ControlPlaneServer {
    control_plane: Arc<Mutex<ControlPlane>>,
    node_routing_controller: Arc<NodeRoutingController>,
    metadata_persistence_controller: Option<Arc<MetadataPersistenceController>>,
    leader_election_controller: Option<Arc<LeaderElectionController>>,
}

impl ControlPlaneServer {
    pub fn new(
        control_plane: Arc<Mutex<ControlPlane>>,
        node_routing_controller: Arc<NodeRoutingController>,
    ) -> Self {
        Self::new_with_dependencies(control_plane, node_routing_controller, None, None)
    }

    pub fn new_with_metadata_persistence(
        control_plane: Arc<Mutex<ControlPlane>>,
        node_routing_controller: Arc<NodeRoutingController>,
        metadata_persistence_controller: Arc<MetadataPersistenceController>,
    ) -> Self {
        Self::new_with_dependencies(
            control_plane,
            node_routing_controller,
            Some(metadata_persistence_controller),
            None,
        )
    }

    pub fn new_with_dependencies(
        control_plane: Arc<Mutex<ControlPlane>>,
        node_routing_controller: Arc<NodeRoutingController>,
        metadata_persistence_controller: Option<Arc<MetadataPersistenceController>>,
        leader_election_controller: Option<Arc<LeaderElectionController>>,
    ) -> Self {
        Self {
            control_plane,
            node_routing_controller,
            metadata_persistence_controller,
            leader_election_controller,
        }
    }

    async fn require_leader(&self, operation: &str) -> Result<(), Status> {
        let Some(leader_election) = &self.leader_election_controller else {
            return Ok(());
        };

        if leader_election.is_leader().await {
            return Ok(());
        }

        let status = leader_election.leadership_status().await;
        let current_leader = status
            .current_leader
            .as_ref()
            .map(|lease| lease.leader_id.as_str())
            .unwrap_or("unknown");
        Err(Status::failed_precondition(format!(
            "{operation} rejected on follower node {}; active leader is {current_leader}",
            status.node_id
        )))
    }
}

#[tonic::async_trait]
impl ControlPlaneService for ControlPlaneServer {
    async fn register_node(
        &self,
        request: Request<RegisterNodeRequest>,
    ) -> Result<Response<RegisterNodeResponse>, Status> {
        self.require_leader("register_node").await?;
        let started = Instant::now();
        let correlation_id = correlation_id_from_request(&request);
        let req = request.into_inner();
        let observability = global_observability_controller();

        let register_result = self
            .node_routing_controller
            .register_node(
                req.node_id.clone(),
                req.node_address,
                req.capabilities,
                req.max_instances,
            )
            .await;
        if let Err(error) = register_result {
            observability.record_api_request(
                "register_node",
                "error",
                started.elapsed().as_secs_f64(),
            );
            return Err(Status::internal(error.to_string()));
        }

        match self
            .node_routing_controller
            .recover_node_state(&req.node_id, &self.control_plane)
            .await
        {
            Ok(recovered) => {
                tracing::info!(node_id = %req.node_id, recovered_instances = recovered, "Recovered node state");
            }
            Err(error) => {
                tracing::warn!(node_id = %req.node_id, error = %error, "Node registered but state recovery skipped");
            }
        }

        observability.set_node_health(&req.node_id, true);
        observability.record_api_request("register_node", "ok", started.elapsed().as_secs_f64());

        tracing::info!(
            %correlation_id,
            node_id = %req.node_id,
            "Registered node"
        );

        Ok(Response::new(RegisterNodeResponse {
            success: true,
            message: format!("Node {} registered successfully", req.node_id),
            error_code: None,
        }))
    }

    async fn report_status(
        &self,
        request: Request<StatusReport>,
    ) -> Result<Response<StatusReportResponse>, Status> {
        self.require_leader("report_status").await?;
        let started = Instant::now();
        let correlation_id = correlation_id_from_request(&request);
        let req = request.into_inner();
        let observability = global_observability_controller();
        tracing::debug!(%correlation_id, node_id = %req.node_id, "Received status report");

        let status_result = self
            .node_routing_controller
            .record_status_report(&req.node_id, req.timestamp)
            .await;
        if let Err(error) = status_result {
            observability.set_node_health(&req.node_id, false);
            observability.record_api_request(
                "report_status",
                "error",
                started.elapsed().as_secs_f64(),
            );
            return Err(Status::internal(error.to_string()));
        }
        observability.set_node_health(&req.node_id, true);

        let mut persisted_updates = Vec::new();
        let mut missing_updates = Vec::new();
        let active_instances = {
            let mut control_plane = self
                .control_plane
                .lock()
                .map_err(|_| Status::internal("control plane lock poisoned"))?;

            for update in req.instance_updates {
                let proto_status = wasmatrix_proto::v1::InstanceStatus::try_from(update.status)
                    .map_err(|_| Status::invalid_argument("Invalid instance status"))?;
                let core_status: wasmatrix_core::InstanceStatus =
                    wasmatrix_proto::protocol::InstanceStatus::try_from(proto_status)
                        .map_err(Status::invalid_argument)?
                        .into();
                if matches!(core_status, wasmatrix_core::InstanceStatus::Crashed) {
                    observability.record_crash();
                }

                if let Err(error) =
                    control_plane.update_instance_status(&update.instance_id, core_status)
                {
                    tracing::warn!(
                        instance_id = %update.instance_id,
                        error = %error,
                        "Status report update skipped for unknown instance"
                    );
                    missing_updates.push((
                        update.instance_id.clone(),
                        core_status,
                        update.error_message.clone(),
                    ));
                    continue;
                }

                if let Some(metadata) = control_plane.get_instance(&update.instance_id) {
                    persisted_updates.push((
                        metadata.clone(),
                        core_status,
                        update.error_message.clone(),
                    ));
                }
            }

            control_plane
                .list_instances()
                .iter()
                .filter(|meta| {
                    matches!(
                        meta.status,
                        wasmatrix_core::InstanceStatus::Starting
                            | wasmatrix_core::InstanceStatus::Running
                    )
                })
                .count()
        };

        if let Some(metadata_persistence) = &self.metadata_persistence_controller {
            for (metadata, status, error_message) in persisted_updates {
                if let Err(error) = metadata_persistence
                    .sync_status(&metadata, status, error_message)
                    .await
                {
                    tracing::warn!(
                        instance_id = %metadata.instance_id,
                        error = %error,
                        "Failed to persist instance status update"
                    );
                }
            }

            for (instance_id, status, error_message) in missing_updates {
                match metadata_persistence.get_instance(&instance_id).await {
                    Ok(Some(metadata)) => {
                        if let Err(error) = metadata_persistence
                            .sync_status(&metadata, status, error_message)
                            .await
                        {
                            tracing::warn!(
                                instance_id = %instance_id,
                                error = %error,
                                "Failed to persist missing in-memory instance status update"
                            );
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        tracing::warn!(
                            instance_id = %instance_id,
                            error = %error,
                            "Failed to load persisted instance metadata for status update"
                        );
                    }
                }
            }
        }

        observability.set_active_instances(active_instances);
        observability.record_api_request("report_status", "ok", started.elapsed().as_secs_f64());
        tracing::info!(
            %correlation_id,
            node_id = %req.node_id,
            active_instances,
            "Status report applied"
        );

        Ok(Response::new(StatusReportResponse {
            success: true,
            message: "Status report received".to_string(),
        }))
    }
}

fn correlation_id_from_request<T>(request: &Request<T>) -> String {
    request
        .metadata()
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::leader_election::controller::LeaderElectionController;
    use crate::features::leader_election::repo::InMemoryLeaderElectionRepository;
    use crate::features::leader_election::service::{LeaderElectionConfig, LeaderElectionService};
    use crate::features::metadata_persistence::controller::MetadataPersistenceController;
    use crate::features::metadata_persistence::repo::EtcdBackedMetadataRepository;
    use crate::features::metadata_persistence::service::MetadataPersistenceService;
    use crate::features::node_routing::repo::InMemoryNodeRoutingRepository;
    use crate::features::node_routing::service::NodeRoutingService;
    use std::sync::Arc;
    use std::time::Duration;
    use wasmatrix_core::{QueryInstanceRequest, RestartPolicy, StartInstanceRequest};
    use wasmatrix_proto::v1::InstanceStatusUpdate;

    fn create_server_with_state() -> (ControlPlaneServer, Arc<Mutex<ControlPlane>>) {
        let control_plane = Arc::new(Mutex::new(ControlPlane::new("node-1")));
        let routing_repo = Arc::new(InMemoryNodeRoutingRepository::new());
        let routing_service = Arc::new(NodeRoutingService::new(routing_repo));
        let routing_controller = Arc::new(NodeRoutingController::new(routing_service));
        let server = ControlPlaneServer::new(control_plane.clone(), routing_controller);
        (server, control_plane)
    }

    fn create_server_with_state_and_persistence() -> (
        ControlPlaneServer,
        Arc<Mutex<ControlPlane>>,
        Arc<MetadataPersistenceController>,
    ) {
        let control_plane = Arc::new(Mutex::new(ControlPlane::new("node-1")));
        let routing_repo = Arc::new(InMemoryNodeRoutingRepository::new());
        let routing_service = Arc::new(NodeRoutingService::new(routing_repo));
        let routing_controller = Arc::new(NodeRoutingController::new(routing_service));
        let metadata_persistence = Arc::new(MetadataPersistenceController::new(Arc::new(
            MetadataPersistenceService::new(Arc::new(EtcdBackedMetadataRepository::new())),
        )));
        let server = ControlPlaneServer::new_with_metadata_persistence(
            control_plane.clone(),
            routing_controller,
            metadata_persistence.clone(),
        );
        (server, control_plane, metadata_persistence)
    }

    fn minimal_wasm_module() -> Vec<u8> {
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    // Property 9: Node Agent Status Reporting
    // Validates that status reports from a registered node update actual instance status.
    #[tokio::test]
    async fn property_status_reporting_reflects_latest_instance_state() {
        let (server, control_plane) = create_server_with_state();

        server
            .register_node(Request::new(RegisterNodeRequest {
                node_id: "node-1".to_string(),
                node_address: "127.0.0.1:50052".to_string(),
                capabilities: vec![],
                max_instances: 100,
            }))
            .await
            .unwrap();

        let instance_id = {
            let mut cp = control_plane.lock().unwrap();
            cp.start_instance(StartInstanceRequest {
                module_bytes: minimal_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            })
            .unwrap()
        };

        let sequence = [
            wasmatrix_proto::v1::InstanceStatus::Starting as i32,
            wasmatrix_proto::v1::InstanceStatus::Running as i32,
            wasmatrix_proto::v1::InstanceStatus::Crashed as i32,
            wasmatrix_proto::v1::InstanceStatus::Stopped as i32,
        ];

        for (i, status) in sequence.iter().enumerate() {
            let response = server
                .report_status(Request::new(StatusReport {
                    node_id: "node-1".to_string(),
                    instance_updates: vec![InstanceStatusUpdate {
                        instance_id: instance_id.clone(),
                        status: *status,
                        error_message: None,
                    }],
                    timestamp: 1_700_000_000 + i as i64,
                }))
                .await
                .unwrap();

            assert!(response.get_ref().success);
        }

        let final_status = {
            let cp = control_plane.lock().unwrap();
            cp.query_instance(QueryInstanceRequest {
                instance_id: instance_id.clone(),
            })
            .unwrap()
            .status
        };

        assert_eq!(final_status, wasmatrix_core::InstanceStatus::Stopped);
    }

    #[tokio::test]
    async fn test_grpc_register_node_message_exchange_success() {
        let (server, _) = create_server_with_state();

        let response = server
            .register_node(Request::new(RegisterNodeRequest {
                node_id: "node-2".to_string(),
                node_address: "127.0.0.1:51052".to_string(),
                capabilities: vec!["kv".to_string()],
                max_instances: 10,
            }))
            .await
            .unwrap();

        assert!(response.get_ref().success);
        assert!(response.get_ref().error_code.is_none());
    }

    #[tokio::test]
    async fn test_grpc_report_status_rejects_invalid_status_code() {
        let (server, _) = create_server_with_state();

        server
            .register_node(Request::new(RegisterNodeRequest {
                node_id: "node-1".to_string(),
                node_address: "127.0.0.1:50052".to_string(),
                capabilities: vec![],
                max_instances: 100,
            }))
            .await
            .unwrap();

        let result = server
            .report_status(Request::new(StatusReport {
                node_id: "node-1".to_string(),
                instance_updates: vec![InstanceStatusUpdate {
                    instance_id: "instance-1".to_string(),
                    status: 9999,
                    error_message: Some("bad".to_string()),
                }],
                timestamp: 1_700_000_000,
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_grpc_report_status_persists_crash_history_when_enabled() {
        let (server, control_plane, metadata_persistence) = create_server_with_state_and_persistence();

        server
            .register_node(Request::new(RegisterNodeRequest {
                node_id: "node-1".to_string(),
                node_address: "127.0.0.1:50052".to_string(),
                capabilities: vec![],
                max_instances: 100,
            }))
            .await
            .unwrap();

        let instance_id = {
            let mut cp = control_plane.lock().unwrap();
            cp.start_instance(StartInstanceRequest {
                module_bytes: minimal_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            })
            .unwrap()
        };

        let metadata = {
            let cp = control_plane.lock().unwrap();
            cp.get_instance(&instance_id).unwrap().clone()
        };
        metadata_persistence.upsert_instance(&metadata).await.unwrap();

        let response = server
            .report_status(Request::new(StatusReport {
                node_id: "node-1".to_string(),
                instance_updates: vec![InstanceStatusUpdate {
                    instance_id: instance_id.clone(),
                    status: wasmatrix_proto::v1::InstanceStatus::Crashed as i32,
                    error_message: Some("trap".to_string()),
                }],
                timestamp: 1_700_000_000,
            }))
            .await
            .unwrap();

        assert!(response.get_ref().success);
        let crash = metadata_persistence.get_crash_history(&instance_id).await.unwrap();
        assert!(crash.is_some());
        assert_eq!(crash.unwrap().crash_count, 1);
    }

    #[tokio::test]
    async fn test_grpc_mutating_calls_are_rejected_on_followers() {
        let control_plane = Arc::new(Mutex::new(ControlPlane::new("node-2")));
        let routing_repo = Arc::new(InMemoryNodeRoutingRepository::new());
        let routing_service = Arc::new(NodeRoutingService::new(routing_repo));
        let routing_controller = Arc::new(NodeRoutingController::new(routing_service));
        let shared_repo = Arc::new(InMemoryLeaderElectionRepository::new());

        let leader = Arc::new(LeaderElectionController::new(Arc::new(LeaderElectionService::new(
            shared_repo.clone(),
            "node-1",
            LeaderElectionConfig {
                lease_ttl: Duration::from_secs(1),
                renew_interval: Duration::from_millis(100),
            },
        ))));
        let follower = Arc::new(LeaderElectionController::new(Arc::new(
            LeaderElectionService::new(
                shared_repo,
                "node-2",
                LeaderElectionConfig {
                    lease_ttl: Duration::from_secs(1),
                    renew_interval: Duration::from_millis(100),
                },
            ),
        )));

        let leader_task = leader.start();
        let follower_task = follower.start();
        tokio::time::sleep(Duration::from_millis(150)).await;

        let server = ControlPlaneServer::new_with_dependencies(
            control_plane,
            routing_controller,
            None,
            Some(follower),
        );

        let result = server
            .register_node(Request::new(RegisterNodeRequest {
                node_id: "node-2".to_string(),
                node_address: "127.0.0.1:51052".to_string(),
                capabilities: vec![],
                max_instances: 10,
            }))
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::FailedPrecondition);

        leader_task.abort();
        follower_task.abort();
    }
}
