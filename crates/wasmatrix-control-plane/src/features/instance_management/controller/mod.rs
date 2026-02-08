use crate::features::instance_management::service::InstanceService;
use crate::shared::error::ControlPlaneError;
use crate::shared::types::{
    InstanceStatus, InstanceStatusResponse, QueryInstanceRequest, StartInstanceRequest,
    StopInstanceRequest,
};
use std::sync::Arc;
use tracing::info;

/// Controller for instance management API endpoints
pub struct InstanceController {
    service: Arc<InstanceService>,
}

impl InstanceController {
    pub fn new(service: Arc<InstanceService>) -> Self {
        Self { service }
    }

    /// Handle start instance request
    /// Thin wrapper that delegates to service
    pub async fn start_instance(
        &self,
        request: StartInstanceRequest,
    ) -> Result<String, wasmatrix_core::ErrorResponse> {
        info!("Received start instance request");

        self.service
            .start_instance(request)
            .await
            .map_err(|e| e.into())
    }

    /// Handle stop instance request
    pub async fn stop_instance(
        &self,
        request: StopInstanceRequest,
    ) -> Result<(), wasmatrix_core::ErrorResponse> {
        info!(instance_id = %request.instance_id, "Received stop instance request");

        self.service
            .stop_instance(request)
            .await
            .map_err(|e| e.into())
    }

    /// Handle query instance request
    pub async fn query_instance(
        &self,
        request: QueryInstanceRequest,
    ) -> Result<InstanceStatusResponse, wasmatrix_core::ErrorResponse> {
        info!(instance_id = %request.instance_id, "Received query instance request");

        self.service
            .query_instance(request)
            .await
            .map_err(|e| e.into())
    }

    /// Handle list instances request
    pub async fn list_instances(
        &self,
    ) -> Result<Vec<wasmatrix_core::InstanceMetadata>, wasmatrix_core::ErrorResponse> {
        info!("Received list instances request");

        self.service.list_instances().await.map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::instance_management::repo::InMemoryInstanceRepository;
    use crate::shared::types::RestartPolicy;

    fn create_test_controller() -> InstanceController {
        let repo = Arc::new(InMemoryInstanceRepository::new());
        let service = Arc::new(InstanceService::new(repo, "test-node"));
        InstanceController::new(service)
    }

    fn create_valid_wasm_module() -> Vec<u8> {
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    #[tokio::test]
    async fn test_controller_start_instance() {
        let controller = create_test_controller();

        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let result = controller.start_instance(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_controller_stop_instance() {
        let controller = create_test_controller();

        // Create instance
        let start_request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };
        let instance_id = controller.start_instance(start_request).await.unwrap();

        // Stop instance
        let stop_request = StopInstanceRequest { instance_id };
        let result = controller.stop_instance(stop_request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_controller_query_instance() {
        let controller = create_test_controller();

        // Create instance
        let start_request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };
        let instance_id = controller.start_instance(start_request).await.unwrap();

        // Query instance
        let query_request = QueryInstanceRequest {
            instance_id: instance_id.clone(),
        };
        let result = controller.query_instance(query_request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().instance_id, instance_id);
    }

    #[tokio::test]
    async fn test_controller_list_instances() {
        let controller = create_test_controller();

        let result = controller.list_instances().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_controller_start_instance_invalid_wasm() {
        let controller = create_test_controller();

        let request = StartInstanceRequest {
            module_bytes: vec![0x00, 0x00, 0x00, 0x00],
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let result = controller.start_instance(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_controller_stop_instance_not_found() {
        let controller = create_test_controller();

        let request = StopInstanceRequest {
            instance_id: "nonexistent".to_string(),
        };

        let result = controller.stop_instance(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_controller_query_instance_not_found() {
        let controller = create_test_controller();

        let request = QueryInstanceRequest {
            instance_id: "nonexistent".to_string(),
        };

        let result = controller.query_instance(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_controller_full_workflow() {
        let controller = create_test_controller();

        // Start instance
        let start_request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };
        let instance_id = controller
            .start_instance(start_request.clone())
            .await
            .unwrap();

        // Query instance
        let query_request = QueryInstanceRequest {
            instance_id: instance_id.clone(),
        };
        let query_response = controller
            .query_instance(query_request.clone())
            .await
            .unwrap();
        assert_eq!(query_response.instance_id, instance_id);

        // List instances
        let list_response = controller.list_instances().await.unwrap();
        assert_eq!(list_response.len(), 1);

        // Stop instance
        let stop_request = StopInstanceRequest { instance_id };
        let stop_response = controller.stop_instance(stop_request).await;
        assert!(stop_response.is_ok());

        // Query again (should still exist)
        let query_response2 = controller.query_instance(query_request).await.unwrap();
        assert_eq!(query_response2.status, InstanceStatus::Stopped);
    }

    #[tokio::test]
    async fn test_controller_error_conversion() {
        let controller = create_test_controller();

        // Trigger service error
        let request = StopInstanceRequest {
            instance_id: "nonexistent".to_string(),
        };

        let result = controller.stop_instance(request).await;
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.error_code, "INSTANCE_NOT_FOUND");
    }
}
