use crate::features::external_api::repo::ExternalApiPrincipal;
use crate::features::external_api::service::{
    CreateInstanceCommand, ExternalApiService, ExternalInstanceRecord, InvokeCapabilityCommand,
};
use crate::shared::error::ControlPlaneError;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wasmatrix_core::{CapabilityAssignment, ProviderType, RestartPolicy};

pub struct ExternalApiController {
    service: Arc<ExternalApiService>,
}

impl ExternalApiController {
    pub fn new(service: Arc<ExternalApiService>) -> Self {
        Self { service }
    }

    pub fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route(
                "/v1/instances",
                post(Self::create_instance).get(Self::list_instances),
            )
            .route("/v1/instances/:id", get(Self::get_instance))
            .route("/v1/instances/:id/stop", post(Self::stop_instance))
            .route(
                "/v1/instances/:id/capabilities",
                post(Self::assign_capability),
            )
            .route(
                "/v1/instances/:id/capabilities/:capability_id",
                delete(Self::revoke_capability),
            )
            .route("/v1/capabilities/invoke", post(Self::invoke_capability))
            .route("/v1/healthz", get(Self::healthz))
            .route("/v1/leader", get(Self::leader))
            .with_state(self)
    }

    async fn create_instance(
        State(controller): State<Arc<Self>>,
        headers: HeaderMap,
        Json(request): Json<CreateInstanceRequest>,
    ) -> Response {
        let principal = match controller.authorize_request(&headers, "instance.admin") {
            Ok(principal) => principal,
            Err(error) => return error_response(error),
        };

        let capabilities = request
            .capabilities
            .into_iter()
            .map(|capability| CapabilityAssignment {
                instance_id: String::new(),
                capability_id: capability.capability_id,
                provider_type: capability.provider_type,
                permissions: capability.permissions,
            })
            .collect();

        match controller
            .service
            .create_instance(
                &principal,
                CreateInstanceCommand {
                    module_base64: request.module_base64,
                    restart_policy: request.restart_policy.unwrap_or_default(),
                    capabilities,
                },
            )
            .await
        {
            Ok(instance) => {
                (StatusCode::CREATED, Json(InstanceResponse::from(instance))).into_response()
            }
            Err(error) => error_response(error),
        }
    }

    async fn list_instances(State(controller): State<Arc<Self>>, headers: HeaderMap) -> Response {
        if let Err(error) = controller.authorize_request(&headers, "instance.read") {
            return error_response(error);
        }

        match controller.service.list_instances() {
            Ok(instances) => Json(ListInstancesResponse {
                instances: instances.into_iter().map(InstanceResponse::from).collect(),
            })
            .into_response(),
            Err(error) => error_response(error),
        }
    }

    async fn get_instance(
        State(controller): State<Arc<Self>>,
        headers: HeaderMap,
        Path(instance_id): Path<String>,
    ) -> Response {
        if let Err(error) = controller.authorize_request(&headers, "instance.read") {
            return error_response(error);
        }

        match controller.service.get_instance(&instance_id) {
            Ok(instance) => Json(InstanceResponse::from(instance)).into_response(),
            Err(error) => error_response(error),
        }
    }

    async fn stop_instance(
        State(controller): State<Arc<Self>>,
        headers: HeaderMap,
        Path(instance_id): Path<String>,
    ) -> Response {
        let principal = match controller.authorize_request(&headers, "instance.admin") {
            Ok(principal) => principal,
            Err(error) => return error_response(error),
        };

        match controller
            .service
            .stop_instance(&principal, &instance_id)
            .await
        {
            Ok(()) => Json(MessageResponse {
                success: true,
                message: format!("Instance {instance_id} stopped"),
            })
            .into_response(),
            Err(error) => error_response(error),
        }
    }

    async fn assign_capability(
        State(controller): State<Arc<Self>>,
        headers: HeaderMap,
        Path(instance_id): Path<String>,
        Json(request): Json<AssignCapabilityRequest>,
    ) -> Response {
        let principal = match controller.authorize_request(&headers, "instance.admin") {
            Ok(principal) => principal,
            Err(error) => return error_response(error),
        };

        match controller
            .service
            .assign_capability(
                &principal,
                &instance_id,
                request.capability_id,
                request.provider_type,
                request.permissions,
            )
            .await
        {
            Ok(instance) => Json(InstanceResponse::from(instance)).into_response(),
            Err(error) => error_response(error),
        }
    }

    async fn revoke_capability(
        State(controller): State<Arc<Self>>,
        headers: HeaderMap,
        Path((instance_id, capability_id)): Path<(String, String)>,
    ) -> Response {
        let principal = match controller.authorize_request(&headers, "instance.admin") {
            Ok(principal) => principal,
            Err(error) => return error_response(error),
        };

        match controller
            .service
            .revoke_capability(&principal, &instance_id, &capability_id)
            .await
        {
            Ok(instance) => Json(InstanceResponse::from(instance)).into_response(),
            Err(error) => error_response(error),
        }
    }

    async fn invoke_capability(
        State(controller): State<Arc<Self>>,
        headers: HeaderMap,
        Json(request): Json<InvokeCapabilityRequest>,
    ) -> Response {
        let principal = match controller.authorize_request(&headers, "capability.invoke") {
            Ok(principal) => principal,
            Err(error) => return error_response(error),
        };

        match controller
            .service
            .invoke_capability(
                &principal,
                InvokeCapabilityCommand {
                    instance_id: request.instance_id,
                    capability_id: request.capability_id,
                    operation: request.operation,
                    params: request.params,
                },
            )
            .await
        {
            Ok(result) => Json(InvokeCapabilityResponse { result }).into_response(),
            Err(error) => error_response(error),
        }
    }

    async fn healthz(State(controller): State<Arc<Self>>) -> Response {
        controller.service.health_status().await.into_response()
    }

    async fn leader(State(controller): State<Arc<Self>>) -> Response {
        let leader = controller.service.leadership_status().await;
        Json(LeaderStatusResponse {
            node_id: leader.node_id,
            is_leader: leader.is_leader,
            current_leader: leader.current_leader,
        })
        .into_response()
    }

    fn authorize_request(
        &self,
        headers: &HeaderMap,
        required_role: &str,
    ) -> Result<ExternalApiPrincipal, ControlPlaneError> {
        let authorization = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok());
        let mtls_subject = headers
            .get("x-mtls-subject")
            .and_then(|value| value.to_str().ok());
        let principal = self.service.authenticate(authorization, mtls_subject)?;
        self.service.authorize(&principal, required_role)?;
        Ok(principal)
    }
}

#[derive(Debug, Deserialize)]
struct CreateInstanceRequest {
    module_base64: String,
    #[serde(default)]
    restart_policy: Option<RestartPolicy>,
    #[serde(default)]
    capabilities: Vec<AssignCapabilityRequest>,
}

#[derive(Debug, Deserialize)]
struct AssignCapabilityRequest {
    capability_id: String,
    provider_type: ProviderType,
    permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct InvokeCapabilityRequest {
    instance_id: String,
    capability_id: String,
    operation: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ListInstancesResponse {
    instances: Vec<InstanceResponse>,
}

#[derive(Debug, Serialize)]
struct InstanceResponse {
    instance_id: String,
    node_id: String,
    module_hash: String,
    created_at: chrono::DateTime<chrono::Utc>,
    status: wasmatrix_core::InstanceStatus,
    capabilities: Vec<CapabilityAssignment>,
}

impl From<ExternalInstanceRecord> for InstanceResponse {
    fn from(record: ExternalInstanceRecord) -> Self {
        Self {
            instance_id: record.metadata.instance_id,
            node_id: record.metadata.node_id,
            module_hash: record.metadata.module_hash,
            created_at: record.metadata.created_at,
            status: record.metadata.status,
            capabilities: record.capabilities,
        }
    }
}

#[derive(Debug, Serialize)]
struct InvokeCapabilityResponse {
    result: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct MessageResponse {
    success: bool,
    message: String,
}

#[derive(Debug, Serialize)]
struct LeaderStatusResponse {
    node_id: String,
    is_leader: bool,
    current_leader: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    error_code: String,
    message: String,
}

fn error_response(error: ControlPlaneError) -> Response {
    let status = match &error {
        ControlPlaneError::InvalidRequest(_) | ControlPlaneError::ValidationError(_) => {
            StatusCode::BAD_REQUEST
        }
        ControlPlaneError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
        ControlPlaneError::InstanceNotFound(_) | ControlPlaneError::CapabilityNotFound(_) => {
            StatusCode::NOT_FOUND
        }
        ControlPlaneError::PermissionDenied(_) => StatusCode::FORBIDDEN,
        ControlPlaneError::ResourceExhausted(_) => StatusCode::TOO_MANY_REQUESTS,
        ControlPlaneError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
        ControlPlaneError::StorageError(_)
        | ControlPlaneError::WasmRuntimeError(_)
        | ControlPlaneError::CrashDetected(_)
        | ControlPlaneError::RestartPolicyViolation(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };

    let error_code = match &error {
        ControlPlaneError::InvalidRequest(_) => "INVALID_REQUEST",
        ControlPlaneError::Unauthorized(_) => "UNAUTHORIZED",
        ControlPlaneError::InstanceNotFound(_) => "INSTANCE_NOT_FOUND",
        ControlPlaneError::CapabilityNotFound(_) => "CAPABILITY_NOT_FOUND",
        ControlPlaneError::PermissionDenied(_) => "PERMISSION_DENIED",
        ControlPlaneError::StorageError(_) => "STORAGE_ERROR",
        ControlPlaneError::ValidationError(_) => "VALIDATION_ERROR",
        ControlPlaneError::WasmRuntimeError(_) => "WASM_RUNTIME_ERROR",
        ControlPlaneError::ResourceExhausted(_) => "RESOURCE_EXHAUSTED",
        ControlPlaneError::Timeout(_) => "TIMEOUT",
        ControlPlaneError::CrashDetected(_) => "CRASH_DETECTED",
        ControlPlaneError::RestartPolicyViolation(_) => "RESTART_POLICY_VIOLATION",
    };

    (
        status,
        Json(ApiErrorResponse {
            error_code: error_code.to_string(),
            message: error.to_string(),
        }),
    )
        .into_response()
}
