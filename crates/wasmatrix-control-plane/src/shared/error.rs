use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Control plane specific errors
#[derive(Debug, Error)]
pub enum ControlPlaneError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Instance not found: {0}")]
    InstanceNotFound(String),
    #[error("Capability not found: {0}")]
    CapabilityNotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Wasm runtime error: {0}")]
    WasmRuntimeError(String),
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Instance crash detected: {0}")]
    CrashDetected(String),
    #[error("Restart policy violation: {0}")]
    RestartPolicyViolation(String),
}

impl From<ControlPlaneError> for wasmatrix_core::ErrorResponse {
    fn from(err: ControlPlaneError) -> Self {
        let (code, message) = match &err {
            ControlPlaneError::InvalidRequest(msg) => ("INVALID_REQUEST", msg.clone()),
            ControlPlaneError::InstanceNotFound(msg) => ("INSTANCE_NOT_FOUND", msg.clone()),
            ControlPlaneError::CapabilityNotFound(msg) => ("CAPABILITY_NOT_FOUND", msg.clone()),
            ControlPlaneError::PermissionDenied(msg) => ("PERMISSION_DENIED", msg.clone()),
            ControlPlaneError::StorageError(msg) => ("STORAGE_ERROR", msg.clone()),
            ControlPlaneError::ValidationError(msg) => ("VALIDATION_ERROR", msg.clone()),
            ControlPlaneError::WasmRuntimeError(msg) => ("WASM_RUNTIME_ERROR", msg.clone()),
            ControlPlaneError::ResourceExhausted(msg) => ("RESOURCE_EXHAUSTED", msg.clone()),
            ControlPlaneError::Timeout(msg) => ("TIMEOUT", msg.clone()),
            ControlPlaneError::CrashDetected(msg) => ("CRASH_DETECTED", msg.clone()),
            ControlPlaneError::RestartPolicyViolation(msg) => {
                ("RESTART_POLICY_VIOLATION", msg.clone())
            }
        };

        wasmatrix_core::ErrorResponse::new(code, message)
    }
}

pub type ControlPlaneResult<T> = std::result::Result<T, ControlPlaneError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_plane_error_creation() {
        let err = ControlPlaneError::InvalidRequest("test".to_string());
        assert!(err.to_string().contains("Invalid request"));
    }

    #[test]
    fn test_control_plane_error_instance_not_found() {
        let err = ControlPlaneError::InstanceNotFound("test-id".to_string());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_control_plane_error_permission_denied() {
        let err = ControlPlaneError::PermissionDenied("access denied".to_string());
        assert!(err.to_string().contains("Permission denied"));
    }

    #[test]
    fn test_control_plane_error_storage_error() {
        let err = ControlPlaneError::StorageError("database error".to_string());
        assert!(err.to_string().contains("Storage error"));
    }

    #[test]
    fn test_control_plane_error_validation_error() {
        let err = ControlPlaneError::ValidationError("invalid input".to_string());
        assert!(err.to_string().contains("Validation error"));
    }

    #[test]
    fn test_control_plane_error_conversion_to_error_response() {
        let err = ControlPlaneError::InvalidRequest("test".to_string());
        let error_response: wasmatrix_core::ErrorResponse = err.into();
        assert_eq!(error_response.error_code, "INVALID_REQUEST");
        assert_eq!(error_response.message, "test");
    }

    #[test]
    fn test_control_plane_result_type() {
        type TestResult = ControlPlaneResult<String>;
        let ok: TestResult = Ok("success".to_string());
        let err: TestResult = Err(ControlPlaneError::InvalidRequest("test".to_string()));

        assert!(ok.is_ok());
        assert!(err.is_err());
        assert_eq!(ok.unwrap(), "success");
        assert_eq!(err.unwrap_err().to_string(), "Invalid request: test");
    }

    #[test]
    fn test_control_plane_error_wasm_runtime() {
        let err = ControlPlaneError::WasmRuntimeError("Failed to compile".to_string());
        let error_response: wasmatrix_core::ErrorResponse = err.into();
        assert_eq!(error_response.error_code, "WASM_RUNTIME_ERROR");
        assert_eq!(error_response.message, "Failed to compile");
    }

    #[test]
    fn test_control_plane_error_resource_exhausted() {
        let err = ControlPlaneError::ResourceExhausted("Memory limit exceeded".to_string());
        let error_response: wasmatrix_core::ErrorResponse = err.into();
        assert_eq!(error_response.error_code, "RESOURCE_EXHAUSTED");
        assert_eq!(error_response.message, "Memory limit exceeded");
    }

    #[test]
    fn test_control_plane_error_timeout() {
        let err = ControlPlaneError::Timeout("Operation timed out".to_string());
        let error_response: wasmatrix_core::ErrorResponse = err.into();
        assert_eq!(error_response.error_code, "TIMEOUT");
        assert_eq!(error_response.message, "Operation timed out");
    }

    #[test]
    fn test_control_plane_error_crash_detected() {
        let err = ControlPlaneError::CrashDetected("Instance crashed".to_string());
        let error_response: wasmatrix_core::ErrorResponse = err.into();
        assert_eq!(error_response.error_code, "CRASH_DETECTED");
        assert_eq!(error_response.message, "Instance crashed");
    }

    #[test]
    fn test_control_plane_error_restart_policy_violation() {
        let err = ControlPlaneError::RestartPolicyViolation("Max retries exceeded".to_string());
        let error_response: wasmatrix_core::ErrorResponse = err.into();
        assert_eq!(error_response.error_code, "RESTART_POLICY_VIOLATION");
        assert_eq!(error_response.message, "Max retries exceeded");
    }
}
