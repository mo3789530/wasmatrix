use crate::features::instance_management::repo::InstanceRepository;
use crate::shared::error::{ControlPlaneError, ControlPlaneResult};
use crate::shared::types::{
    CapabilityAssignment, InstanceMetadata, InstanceStatus, InstanceStatusResponse, ProviderType,
    QueryInstanceRequest, StartInstanceRequest, StopInstanceRequest,
};
use std::sync::Arc;
use tracing::info;

/// Service for managing Wasm instances
pub struct InstanceService {
    repo: Arc<dyn InstanceRepository>,
    node_id: String,
}

impl InstanceService {
    pub fn new(repo: Arc<dyn InstanceRepository>, node_id: impl Into<String>) -> Self {
        Self {
            repo,
            node_id: node_id.into(),
        }
    }

    /// Validate Wasm module format
    fn validate_wasm_module(module_bytes: &[u8]) -> ControlPlaneResult<()> {
        if module_bytes.is_empty() {
            return Err(ControlPlaneError::ValidationError(
                "Module bytes cannot be empty".to_string(),
            ));
        }

        if module_bytes.len() < 4 || &module_bytes[0..4] != &[0x00, 0x61, 0x73, 0x6d] {
            return Err(ControlPlaneError::ValidationError(
                "Invalid Wasm module format".to_string(),
            ));
        }

        // Check for resource limits (prevent resource exhaustion)
        if module_bytes.len() > 10 * 1024 * 1024 {
            return Err(ControlPlaneError::ResourceExhausted(
                "Module size exceeds 10MB limit".to_string(),
            ));
        }

        Ok(())
    }

    /// Start a new instance
    pub async fn start_instance(
        &self,
        request: StartInstanceRequest,
    ) -> ControlPlaneResult<String> {
        // Validation
        Self::validate_wasm_module(&request.module_bytes)?;

        // Create metadata
        let metadata = InstanceMetadata::new(
            self.node_id.clone(),
            format!("{:x}", md5::compute(&request.module_bytes)),
        );

        let instance_id = metadata.instance_id.clone();

        // Store in repository
        self.repo.create(metadata).await?;

        info!(instance_id = %instance_id, "Instance created successfully");

        Ok(instance_id)
    }

    /// Stop an instance
    pub async fn stop_instance(&self, request: StopInstanceRequest) -> ControlPlaneResult<()> {
        // Validation
        if request.instance_id.is_empty() {
            return Err(ControlPlaneError::ValidationError(
                "Instance ID cannot be empty".to_string(),
            ));
        }

        // Check existence
        if !self.repo.exists(&request.instance_id).await? {
            return Err(ControlPlaneError::InstanceNotFound(
                request.instance_id.clone(),
            ));
        }

        // Update status
        self.repo
            .update_status(&request.instance_id, InstanceStatus::Stopped)
            .await?;

        info!(instance_id = %request.instance_id, "Instance stopped successfully");

        Ok(())
    }

    /// Query instance status
    pub async fn query_instance(
        &self,
        request: QueryInstanceRequest,
    ) -> ControlPlaneResult<InstanceStatusResponse> {
        // Validation
        if request.instance_id.is_empty() {
            return Err(ControlPlaneError::ValidationError(
                "Instance ID cannot be empty".to_string(),
            ));
        }

        // Retrieve from repository
        let metadata = self.repo.get(&request.instance_id).await?;

        if let Some(metadata) = metadata {
            Ok(InstanceStatusResponse {
                instance_id: metadata.instance_id,
                status: metadata.status,
                node_id: metadata.node_id,
                created_at: metadata.created_at,
            })
        } else {
            Err(ControlPlaneError::InstanceNotFound(
                request.instance_id.clone(),
            ))
        }
    }

    /// List all instances
    pub async fn list_instances(&self) -> ControlPlaneResult<Vec<InstanceMetadata>> {
        self.repo.list().await
    }

    /// Update instance status (called by node agent)
    pub async fn update_status(
        &self,
        instance_id: &str,
        status: InstanceStatus,
    ) -> ControlPlaneResult<()> {
        if !self.repo.exists(instance_id).await? {
            return Err(ControlPlaneError::InstanceNotFound(instance_id.to_string()));
        }

        self.repo.update_status(instance_id, status).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::instance_management::repo::InMemoryInstanceRepository;
    use crate::shared::types::RestartPolicy;

    fn create_test_service() -> InstanceService {
        let repo = Arc::new(InMemoryInstanceRepository::new());
        InstanceService::new(repo, "test-node")
    }

    fn create_valid_wasm_module() -> Vec<u8> {
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    #[tokio::test]
    async fn test_start_instance_success() {
        let service = create_test_service();

        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = service.start_instance(request).await.unwrap();
        assert!(!instance_id.is_empty());
    }

    #[tokio::test]
    async fn test_start_instance_invalid_wasm() {
        let service = create_test_service();

        let request = StartInstanceRequest {
            module_bytes: vec![0x00, 0x00, 0x00, 0x00],
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let result = service.start_instance(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stop_instance_success() {
        let service = create_test_service();

        // Create instance
        let start_request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };
        let instance_id = service.start_instance(start_request).await.unwrap();

        // Stop instance
        let stop_request = StopInstanceRequest { instance_id };
        service.stop_instance(stop_request).await.unwrap();
    }

    #[tokio::test]
    async fn test_stop_instance_not_found() {
        let service = create_test_service();

        let request = StopInstanceRequest {
            instance_id: "nonexistent".to_string(),
        };

        let result = service.stop_instance(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_query_instance() {
        let service = create_test_service();

        // Create instance
        let start_request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };
        let instance_id = service.start_instance(start_request).await.unwrap();

        // Query instance
        let _query_request = QueryInstanceRequest {
            instance_id: instance_id.clone(),
        };
        let response = service
            .query_instance(QueryInstanceRequest {
                instance_id: instance_id.clone(),
            })
            .await
            .unwrap();

        assert_eq!(response.instance_id, instance_id);
    }

    #[tokio::test]
    async fn test_list_instances() {
        let service = create_test_service();

        // Create multiple instances
        for _ in 0..3 {
            let request = StartInstanceRequest {
                module_bytes: create_valid_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };
            service.start_instance(request).await.unwrap();
        }

        let list = service.list_instances().await.unwrap();
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn test_update_status() {
        let service = create_test_service();

        // Create instance
        let start_request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };
        let instance_id = service.start_instance(start_request).await.unwrap();

        // Update status
        service
            .update_status(&instance_id, InstanceStatus::Running)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_update_status_not_found() {
        let service = create_test_service();

        let result = service
            .update_status("nonexistent", InstanceStatus::Stopped)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_instance_empty_module() {
        let service = create_test_service();

        let request = StartInstanceRequest {
            module_bytes: vec![],
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let result = service.start_instance(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_query_instance_not_found() {
        let service = create_test_service();

        let request = QueryInstanceRequest {
            instance_id: "nonexistent".to_string(),
        };

        let result = service.query_instance(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stop_instance_empty_id() {
        let service = create_test_service();

        let request = StopInstanceRequest {
            instance_id: "".to_string(),
        };

        let result = service.stop_instance(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_query_instance_empty_id() {
        let service = create_test_service();

        let request = QueryInstanceRequest {
            instance_id: "".to_string(),
        };

        let result = service.query_instance(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_instances_empty() {
        let service = create_test_service();

        let list = service.list_instances().await.unwrap();
        assert_eq!(list.len(), 0);
    }

    #[tokio::test]
    async fn test_service_with_capabilities() {
        let service = create_test_service();

        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![CapabilityAssignment::new(
                "test-instance".to_string(),
                "kv-1".to_string(),
                ProviderType::Kv,
                vec!["kv:read".to_string()],
            )],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = service.start_instance(request).await.unwrap();
        assert!(!instance_id.is_empty());
    }

    #[tokio::test]
    async fn test_full_lifecycle() {
        let service = create_test_service();

        // Start
        let start_request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };
        let instance_id = service.start_instance(start_request).await.unwrap();

        // Query (should be Running status)
        let _query_request = QueryInstanceRequest {
            instance_id: instance_id.clone(),
        };
        let query_response = service
            .query_instance(QueryInstanceRequest {
                instance_id: instance_id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(query_response.status, InstanceStatus::Starting);

        // Update to Running
        service
            .update_status(&instance_id, InstanceStatus::Running)
            .await
            .unwrap();

        // Query again
        let query_response2 = service
            .query_instance(QueryInstanceRequest {
                instance_id: instance_id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(query_response2.status, InstanceStatus::Running);

        // Stop
        let stop_request = StopInstanceRequest {
            instance_id: instance_id.clone(),
        };
        service.stop_instance(stop_request).await.unwrap();

        // Query (should be Stopped)
        let query_response3 = service
            .query_instance(QueryInstanceRequest {
                instance_id: instance_id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(query_response3.status, InstanceStatus::Stopped);
    }
}
