use crate::features::node_routing::controller::NodeRoutingController;
use crate::ControlPlane;
use std::sync::{Arc, Mutex};
use tonic::{Request, Response, Status};
use wasmatrix_proto::v1::control_plane_service_server::ControlPlaneService;
use wasmatrix_proto::v1::{
    RegisterNodeRequest, RegisterNodeResponse, StatusReport, StatusReportResponse,
};

pub struct ControlPlaneServer {
    control_plane: Arc<Mutex<ControlPlane>>,
    node_routing_controller: Arc<NodeRoutingController>,
}

impl ControlPlaneServer {
    pub fn new(
        control_plane: Arc<Mutex<ControlPlane>>,
        node_routing_controller: Arc<NodeRoutingController>,
    ) -> Self {
        Self {
            control_plane,
            node_routing_controller,
        }
    }
}

#[tonic::async_trait]
impl ControlPlaneService for ControlPlaneServer {
    async fn register_node(
        &self,
        request: Request<RegisterNodeRequest>,
    ) -> Result<Response<RegisterNodeResponse>, Status> {
        let req = request.into_inner();

        self.node_routing_controller
            .register_node(
                req.node_id.clone(),
                req.node_address,
                req.capabilities,
                req.max_instances,
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

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

        tracing::info!("Registered node: {}", req.node_id);

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
        let req = request.into_inner();
        tracing::debug!("Received status report from node: {}", req.node_id);

        self.node_routing_controller
            .record_status_report(&req.node_id, req.timestamp)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

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

            if let Err(error) =
                control_plane.update_instance_status(&update.instance_id, core_status)
            {
                tracing::warn!(
                    instance_id = %update.instance_id,
                    error = %error,
                    "Status report update skipped for unknown instance"
                );
            }
        }

        Ok(Response::new(StatusReportResponse {
            success: true,
            message: "Status report received".to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::node_routing::repo::InMemoryNodeRoutingRepository;
    use crate::features::node_routing::service::NodeRoutingService;
    use std::sync::Arc;
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
}
