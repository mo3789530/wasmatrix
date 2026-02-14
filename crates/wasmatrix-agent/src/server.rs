use crate::features::status_reporting::controller::StatusReportController;
use crate::NodeAgent;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use wasmatrix_core::CapabilityAssignment;
use wasmatrix_proto::protocol;
use wasmatrix_proto::v1::node_agent_service_server::NodeAgentService;
use wasmatrix_proto::v1::{
    InvokeCapabilityRequest, InvokeCapabilityResponse, ListInstancesRequest, ListInstancesResponse,
    QueryInstanceRequest, QueryInstanceResponse, StartInstanceRequest, StartInstanceResponse,
    StopInstanceRequest, StopInstanceResponse,
};
use wasmatrix_providers::features::provider_lifecycle::repo::InMemoryProviderLifecycleRepository;
use wasmatrix_providers::features::provider_lifecycle::service::ProviderLifecycleService;
use wasmatrix_providers::{
    kv_provider::KvProvider, CapabilityProvider, HttpCapabilityProvider,
    MessagingCapabilityProvider, ProviderLifecycleController,
};

pub struct NodeAgentServer {
    agent: Arc<NodeAgent>,
    status_report_controller: Option<Arc<StatusReportController>>,
    provider_lifecycle_controller: Arc<ProviderLifecycleController>,
}

impl NodeAgentServer {
    pub fn new(
        agent: Arc<NodeAgent>,
        status_report_controller: Option<Arc<StatusReportController>>,
    ) -> Self {
        let lifecycle_controller = Arc::new(ProviderLifecycleController::new(
            ProviderLifecycleService::new(Arc::new(InMemoryProviderLifecycleRepository::new())),
        ));
        Self {
            agent,
            status_report_controller,
            provider_lifecycle_controller: lifecycle_controller,
        }
    }

    pub fn start_provider(&self, provider_id: &str) -> Result<(), Status> {
        self.provider_lifecycle_controller
            .start_provider(provider_id)
            .map_err(|e| Status::internal(e.to_string()))
    }

    pub fn stop_provider(&self, provider_id: &str) -> Result<(), Status> {
        self.provider_lifecycle_controller
            .stop_provider(provider_id)
            .map_err(|e| Status::internal(e.to_string()))
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
        let correlation_id = correlation_id_from_request(&request);
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
                tracing::info!(%correlation_id, instance_id = %instance_id, "Instance started");
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
        let correlation_id = correlation_id_from_request(&request);
        let req_proto = request.into_inner();
        let req: protocol::StopInstanceRequest = req_proto.into();

        match self.agent.stop_instance_local(&req.instance_id).await {
            Ok(_) => {
                tracing::info!(%correlation_id, instance_id = %req.instance_id, "Instance stopped");
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
        let correlation_id = correlation_id_from_request(&request);
        let req_proto = request.into_inner();
        let instance_id = req_proto.instance_id;
        tracing::debug!(%correlation_id, %instance_id, "Querying instance");

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
        request: Request<ListInstancesRequest>,
    ) -> Result<Response<ListInstancesResponse>, Status> {
        let correlation_id = correlation_id_from_request(&request);
        let instance_ids = self.agent.list_instances().await;
        tracing::debug!(%correlation_id, count = instance_ids.len(), "Listing instances");

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

    async fn invoke_capability(
        &self,
        request: Request<InvokeCapabilityRequest>,
    ) -> Result<Response<InvokeCapabilityResponse>, Status> {
        let correlation_id = correlation_id_from_request(&request);
        let req = request.into_inner();
        tracing::info!(
            %correlation_id,
            instance_id = %req.instance_id,
            capability_id = %req.capability_id,
            operation = %req.operation,
            "Invoking capability"
        );
        self.provider_lifecycle_controller
            .ensure_provider_available(&req.capability_id)
            .map_err(|e| Status::failed_precondition(e.to_string()))?;

        let provider_type = match wasmatrix_proto::v1::ProviderType::try_from(req.provider_type) {
            Ok(v) => protocol::ProviderType::try_from(v)
                .map_err(|e| Status::invalid_argument(format!("Invalid provider type: {e}")))?,
            Err(_) => {
                return Err(Status::invalid_argument(
                    "Invalid provider type integer code".to_string(),
                ))
            }
        };

        let mut params: serde_json::Value = if req.params_json.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&req.params_json).map_err(|e| {
                Status::invalid_argument(format!("Invalid params_json payload: {e}"))
            })?
        };

        if let Some(map) = params.as_object_mut() {
            map.insert(
                "permissions".to_string(),
                serde_json::Value::Array(
                    req.permissions
                        .iter()
                        .cloned()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        let result = match provider_type {
            protocol::ProviderType::Kv => {
                let provider = KvProvider::new(req.capability_id.clone());
                provider.invoke(&req.instance_id, &req.operation, params)
            }
            protocol::ProviderType::Http => {
                let capability_id = req.capability_id.clone();
                let instance_id = req.instance_id.clone();
                let operation = req.operation.clone();
                tokio::task::spawn_blocking(move || {
                    let provider = HttpCapabilityProvider::new(capability_id).map_err(|e| {
                        wasmatrix_core::CoreError::WasmRuntimeError(format!(
                            "Failed to initialize HTTP provider: {e}"
                        ))
                    })?;
                    provider.invoke(&instance_id, &operation, params)
                })
                .await
                .map_err(|e| Status::internal(format!("HTTP invocation join error: {e}")))?
            }
            protocol::ProviderType::Messaging => {
                let provider = MessagingCapabilityProvider::new(req.capability_id.clone());
                provider.invoke(&req.instance_id, &req.operation, params)
            }
        };

        match result {
            Ok(value) => Ok(Response::new(InvokeCapabilityResponse {
                success: true,
                message: "Capability invocation completed".to_string(),
                result_json: Some(value.to_string()),
                error_code: None,
            })),
            Err(error) => Ok(Response::new(InvokeCapabilityResponse {
                success: false,
                message: error.to_string(),
                result_json: None,
                error_code: Some("INVOKE_FAILED".to_string()),
            })),
        }
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
    use tonic::Request;
    use wasmatrix_proto::v1::node_agent_service_server::NodeAgentService;
    use wasmatrix_proto::v1::{
        CapabilityAssignment as ProtoCapabilityAssignment, InstanceStatus as ProtoInstanceStatus,
        InvokeCapabilityRequest as ProtoInvokeCapabilityRequest, ProviderType as ProtoProviderType,
        RestartPolicy as ProtoRestartPolicy, RestartPolicyType as ProtoRestartPolicyType,
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

    #[tokio::test]
    async fn test_invoke_capability_http_permission_denied() {
        let server = create_server();
        let response = server
            .invoke_capability(Request::new(ProtoInvokeCapabilityRequest {
                instance_id: "instance-1".to_string(),
                capability_id: "http-provider".to_string(),
                provider_type: ProtoProviderType::Http as i32,
                operation: "request".to_string(),
                params_json: "{\"method\":\"GET\",\"url\":\"https://example.com\"}".to_string(),
                permissions: vec![],
            }))
            .await
            .expect("invoke rpc should respond")
            .into_inner();

        assert!(!response.success);
        assert_eq!(response.error_code.as_deref(), Some("INVOKE_FAILED"));
    }

    #[tokio::test]
    async fn test_invoke_capability_messaging_publish_success() {
        let server = create_server();
        let response = server
            .invoke_capability(Request::new(ProtoInvokeCapabilityRequest {
                instance_id: "instance-1".to_string(),
                capability_id: "messaging-provider".to_string(),
                provider_type: ProtoProviderType::Messaging as i32,
                operation: "publish".to_string(),
                params_json: "{\"topic\":\"orders\",\"payload\":\"created\"}".to_string(),
                permissions: vec!["msg:publish:orders".to_string()],
            }))
            .await
            .expect("invoke rpc should respond")
            .into_inner();

        assert!(response.success);
        assert!(response.result_json.is_some());
    }

    #[tokio::test]
    async fn test_provider_stopped_returns_unavailable_error() {
        let server = create_server();
        server.stop_provider("messaging-provider").unwrap();

        let response = server
            .invoke_capability(Request::new(ProtoInvokeCapabilityRequest {
                instance_id: "instance-1".to_string(),
                capability_id: "messaging-provider".to_string(),
                provider_type: ProtoProviderType::Messaging as i32,
                operation: "publish".to_string(),
                params_json: "{\"topic\":\"orders\",\"payload\":\"created\"}".to_string(),
                permissions: vec!["msg:publish:orders".to_string()],
            }))
            .await;

        assert!(response.is_err());
        assert_eq!(
            response.unwrap_err().code(),
            tonic::Code::FailedPrecondition
        );
    }

    #[tokio::test]
    async fn test_provider_restart_allows_invocation_again() {
        let server = create_server();
        server.stop_provider("messaging-provider").unwrap();
        server.start_provider("messaging-provider").unwrap();

        let response = server
            .invoke_capability(Request::new(ProtoInvokeCapabilityRequest {
                instance_id: "instance-1".to_string(),
                capability_id: "messaging-provider".to_string(),
                provider_type: ProtoProviderType::Messaging as i32,
                operation: "publish".to_string(),
                params_json: "{\"topic\":\"orders\",\"payload\":\"created\"}".to_string(),
                permissions: vec!["msg:publish:orders".to_string()],
            }))
            .await
            .expect("invoke rpc should respond")
            .into_inner();

        assert!(response.success);
    }
}
