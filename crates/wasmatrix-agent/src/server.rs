use crate::features::status_reporting::controller::StatusReportController;
use crate::NodeAgent;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use wasmatrix_core::CapabilityAssignment;
use wasmatrix_proto::protocol;
use wasmatrix_proto::v1::node_agent_service_server::NodeAgentService;
use wasmatrix_proto::v1::{
    ListInstancesRequest, ListInstancesResponse, QueryInstanceRequest, QueryInstanceResponse,
    StartInstanceRequest, StartInstanceResponse, StopInstanceRequest, StopInstanceResponse,
};

pub struct NodeAgentServer {
    agent: Arc<NodeAgent>,
    status_report_controller: Option<Arc<StatusReportController>>,
}

impl NodeAgentServer {
    pub fn new(
        agent: Arc<NodeAgent>,
        status_report_controller: Option<Arc<StatusReportController>>,
    ) -> Self {
        Self {
            agent,
            status_report_controller,
        }
    }
}

// Helpers for conversion
fn convert_capability(cap: protocol::CapabilityAssignment) -> CapabilityAssignment {
    CapabilityAssignment {
        instance_id: cap.instance_id,
        capability_id: cap.capability_id,
        provider_type: cap.provider_type.into(),
        permissions: cap.permissions,
    }
}

#[tonic::async_trait]
impl NodeAgentService for NodeAgentServer {
    async fn start_instance(
        &self,
        request: Request<StartInstanceRequest>,
    ) -> Result<Response<StartInstanceResponse>, Status> {
        let req_proto = request.into_inner();

        // Convert to protocol type to handle validation/conversion
        let req: protocol::StartInstanceRequest = match req_proto.try_into() {
            Ok(r) => r,
            Err(e) => {
                return Ok(Response::new(StartInstanceResponse {
                    success: false,
                    message: format!("Invalid request: {}", e),
                    error_code: Some("INVALID_REQUEST".to_string()),
                }))
            }
        };

        // Convert capabilities
        let capabilities: Vec<CapabilityAssignment> = req
            .capabilities
            .into_iter()
            .map(convert_capability)
            .collect();

        // Convert restart policy
        let restart_policy = req.restart_policy.into();

        // Call agent
        let instance_id = req.instance_id;
        match self
            .agent
            .start_instance_local(
                instance_id.clone(),
                req.module_bytes,
                capabilities,
                restart_policy,
            )
            .await
        {
            Ok(_) => {
                if let Some(controller) = &self.status_report_controller {
                    if let Err(error) = controller
                        .report_status_change(
                            instance_id,
                            wasmatrix_core::InstanceStatus::Running,
                            None,
                        )
                        .await
                    {
                        tracing::warn!(error = %error, "Failed to report start status change");
                    }
                }

                Ok(Response::new(StartInstanceResponse {
                    success: true,
                    message: "Instance started successfully".to_string(),
                    error_code: None,
                }))
            }
            Err(e) => Ok(Response::new(StartInstanceResponse {
                success: false,
                message: e.to_string(),
                error_code: Some("INTERNAL_ERROR".to_string()),
            })),
        }
    }

    async fn stop_instance(
        &self,
        request: Request<StopInstanceRequest>,
    ) -> Result<Response<StopInstanceResponse>, Status> {
        let req_proto = request.into_inner();
        let req: protocol::StopInstanceRequest = req_proto.into();

        match self.agent.stop_instance_local(&req.instance_id).await {
            Ok(_) => {
                if let Some(controller) = &self.status_report_controller {
                    if let Err(error) = controller
                        .report_status_change(
                            req.instance_id,
                            wasmatrix_core::InstanceStatus::Stopped,
                            None,
                        )
                        .await
                    {
                        tracing::warn!(error = %error, "Failed to report stop status change");
                    }
                }

                Ok(Response::new(StopInstanceResponse {
                    success: true,
                    message: "Instance stopped successfully".to_string(),
                    error_code: None,
                }))
            }
            Err(e) => Ok(Response::new(StopInstanceResponse {
                success: false,
                message: e.to_string(),
                error_code: Some("INTERNAL_ERROR".to_string()),
            })),
        }
    }

    async fn query_instance(
        &self,
        request: Request<QueryInstanceRequest>,
    ) -> Result<Response<QueryInstanceResponse>, Status> {
        let req_proto = request.into_inner();
        let instance_id = req_proto.instance_id;

        // This is a simplified implementation. Real query would return metadata.
        // But NodeAgent mainly manages local execution. Metadata is in ControlPlane.
        // However, NodeAgent can report status.

        let status = self.agent.get_instance_status(&instance_id).await;

        // TODO: Populate full metadata if needed. For now just returning status via error/message or specific fields?
        // The QueryInstanceResponse expects InstanceMetadata.
        // NodeAgent might not store all metadata (like creation time, module hash) locally in a way that matches InstanceMetadata fully?
        // Actually NodeAgent has InstanceHandle which has module_bytes, restart_policy.
        // But InstanceMetadata has created_at, node_id, module_hash.

        // For now, let's return "Not Implemented" or partial data.
        // But wait, QueryInstance is usually a ControlPlane operation.
        // Why is it in NodeAgentService?
        // Ah, Protocol definition puts QueryInstance in NodeAgentService too?

        // Let's check wasmatrix.proto
        // service NodeAgentService { rpc QueryInstance ... }

        // Okay, so Node Agent *should* answer this.

        // I'll leave it as "not found" or basic impl for now.

        let status_proto: protocol::InstanceStatus = status.into();

        // Construct minimal metadata
        let metadata = protocol::InstanceMetadata {
            instance_id: instance_id.clone(),
            node_id: "local".to_string(), // self.agent.node_id?
            module_hash: "unknown".to_string(),
            created_at: 0,
            status: status_proto,
        };

        Ok(Response::new(QueryInstanceResponse {
            success: true,
            instance: Some(metadata.into()),
            error_code: None,
        }))
    }

    async fn list_instances(
        &self,
        _request: Request<ListInstancesRequest>,
    ) -> Result<Response<ListInstancesResponse>, Status> {
        let instance_ids = self.agent.list_instances().await;

        let instances: Vec<wasmatrix_proto::v1::InstanceMetadata> = instance_ids
            .into_iter()
            .map(|id| {
                // Basic metadata
                protocol::InstanceMetadata {
                    instance_id: id,
                    node_id: "local".to_string(),
                    module_hash: "unknown".to_string(),
                    created_at: 0,
                    status: protocol::InstanceStatus::Running, // If it's in list, it's running (mostly)
                }
                .into()
            })
            .collect();

        Ok(Response::new(ListInstancesResponse {
            success: true,
            instances,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Request;
    use wasmatrix_proto::v1::node_agent_service_server::NodeAgentService;
    use wasmatrix_proto::v1::{
        CapabilityAssignment as ProtoCapabilityAssignment, InstanceStatus as ProtoInstanceStatus,
        ProviderType as ProtoProviderType, RestartPolicy as ProtoRestartPolicy,
        RestartPolicyType as ProtoRestartPolicyType,
    };

    fn create_valid_wasm_module() -> Vec<u8> {
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    fn create_server() -> NodeAgentServer {
        let agent = Arc::new(NodeAgent::new("test-node").expect("agent should be created"));
        NodeAgentServer::new(agent, None)
    }

    #[tokio::test]
    async fn test_start_instance_invalid_request_returns_error_response() {
        let server = create_server();
        let request = StartInstanceRequest {
            instance_id: "instance-invalid".to_string(),
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: None,
        };

        let response = server
            .start_instance(Request::new(request))
            .await
            .expect("rpc should respond")
            .into_inner();

        assert!(!response.success);
        assert_eq!(response.error_code.as_deref(), Some("INVALID_REQUEST"));
    }

    #[tokio::test]
    async fn test_start_query_list_stop_instance_flow() {
        let server = create_server();
        let instance_id = "instance-1".to_string();
        let request = StartInstanceRequest {
            instance_id: instance_id.clone(),
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![ProtoCapabilityAssignment {
                instance_id: instance_id.clone(),
                capability_id: "kv-1".to_string(),
                provider_type: ProtoProviderType::Kv as i32,
                permissions: vec!["kv:read".to_string()],
            }],
            restart_policy: Some(ProtoRestartPolicy {
                policy_type: ProtoRestartPolicyType::Always as i32,
                max_retries: None,
                backoff_seconds: None,
            }),
        };

        let start_response = server
            .start_instance(Request::new(request))
            .await
            .expect("start rpc should respond")
            .into_inner();
        assert!(start_response.success);
        assert!(start_response.error_code.is_none());

        let query_response = server
            .query_instance(Request::new(QueryInstanceRequest {
                instance_id: instance_id.clone(),
            }))
            .await
            .expect("query rpc should respond")
            .into_inner();
        assert!(query_response.success);
        let metadata = query_response
            .instance
            .expect("query should include instance");
        assert_eq!(metadata.instance_id, instance_id);
        assert_eq!(metadata.status, ProtoInstanceStatus::Running as i32);

        let list_response = server
            .list_instances(Request::new(ListInstancesRequest {}))
            .await
            .expect("list rpc should respond")
            .into_inner();
        assert!(list_response.success);
        assert_eq!(list_response.instances.len(), 1);
        assert_eq!(list_response.instances[0].instance_id, "instance-1");

        let stop_response = server
            .stop_instance(Request::new(StopInstanceRequest {
                instance_id: "instance-1".to_string(),
            }))
            .await
            .expect("stop rpc should respond")
            .into_inner();
        assert!(stop_response.success);
        assert!(stop_response.error_code.is_none());
    }
}
