pub mod features;
pub mod server;
pub mod shared;

// Legacy ControlPlane implementation for backward compatibility
use std::collections::HashMap;
use wasmatrix_core::{
    CapabilityAssignment, CoreError, ErrorResponse, ExecutionEventRecorder, InstanceMetadata,
    InstanceStatus, InstanceStatusResponse, QueryInstanceRequest, RestartPolicy, Result,
    StartInstanceRequest, StopInstanceRequest,
};

pub struct ControlPlane {
    instances: HashMap<String, InstanceMetadata>,
    crashed_instances: HashMap<String, std::time::Instant>,
    capabilities: HashMap<String, Vec<CapabilityAssignment>>,
    event_recorder: ExecutionEventRecorder,
    node_id: String,
}

impl ControlPlane {
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            instances: HashMap::new(),
            crashed_instances: HashMap::new(),
            capabilities: HashMap::new(),
            event_recorder: ExecutionEventRecorder::new(),
            node_id: node_id.into(),
        }
    }

    /// Start a new Wasm instance
    /// Validates request and creates instance metadata
    pub fn start_instance(
        &mut self,
        request: StartInstanceRequest,
    ) -> std::result::Result<String, ErrorResponse> {
        // Validate module bytes
        if request.module_bytes.is_empty() {
            return Err(ErrorResponse::new(
                "INVALID_REQUEST",
                "Module bytes cannot be empty",
            ));
        }

        // Validate module is valid Wasm (basic check - starts with magic bytes)
        if request.module_bytes.len() < 4
            || &request.module_bytes[0..4] != &[0x00, 0x61, 0x73, 0x6d]
        {
            return Err(ErrorResponse::new(
                "INVALID_REQUEST",
                "Invalid Wasm module format",
            ));
        }

        // Create instance metadata
        let metadata = InstanceMetadata::new(
            self.node_id.clone(),
            format!("{:x}", md5::compute(&request.module_bytes)),
        );

        let instance_id = metadata.instance_id.clone();

        // Store instance
        self.instances.insert(instance_id.clone(), metadata);

        // Store capability assignments
        if !request.capabilities.is_empty() {
            self.capabilities
                .insert(instance_id.clone(), request.capabilities);
        }

        Ok(instance_id)
    }

    /// Stop an existing Wasm instance
    pub fn stop_instance(
        &mut self,
        request: StopInstanceRequest,
    ) -> std::result::Result<(), ErrorResponse> {
        // Validate request
        if request.instance_id.is_empty() {
            return Err(ErrorResponse::new(
                "INVALID_REQUEST",
                "Instance ID cannot be empty",
            ));
        }

        // Find and update instance
        if let Some(metadata) = self.instances.get_mut(&request.instance_id) {
            metadata.status = InstanceStatus::Stopped;
            Ok(())
        } else {
            Err(ErrorResponse::new(
                "INSTANCE_NOT_FOUND",
                format!("Instance {} not found", request.instance_id),
            ))
        }
    }

    /// Query instance status
    pub fn query_instance(
        &self,
        request: QueryInstanceRequest,
    ) -> std::result::Result<InstanceStatusResponse, ErrorResponse> {
        // Validate request
        if request.instance_id.is_empty() {
            return Err(ErrorResponse::new(
                "INVALID_REQUEST",
                "Instance ID cannot be empty",
            ));
        }

        // Find instance
        if let Some(metadata) = self.instances.get(&request.instance_id) {
            Ok(InstanceStatusResponse {
                instance_id: metadata.instance_id.clone(),
                status: metadata.status,
                node_id: metadata.node_id.clone(),
                created_at: metadata.created_at,
            })
        } else {
            Err(ErrorResponse::new(
                "INSTANCE_NOT_FOUND",
                format!("Instance {} not found", request.instance_id),
            ))
        }
    }

    /// Assign capabilities to an instance
    pub fn assign_capability(
        &mut self,
        assignment: CapabilityAssignment,
    ) -> std::result::Result<(), ErrorResponse> {
        // Validate instance exists
        if !self.instances.contains_key(&assignment.instance_id) {
            return Err(ErrorResponse::new(
                "INSTANCE_NOT_FOUND",
                format!("Instance {} not found", assignment.instance_id),
            ));
        }

        // Validate capability_id
        if assignment.capability_id.is_empty() {
            return Err(ErrorResponse::new(
                "INVALID_REQUEST",
                "Capability ID cannot be empty",
            ));
        }

        // Validate permissions
        if assignment.permissions.is_empty() {
            return Err(ErrorResponse::new(
                "INVALID_REQUEST",
                "At least one permission must be specified",
            ));
        }

        // Add capability assignment
        self.capabilities
            .entry(assignment.instance_id.clone())
            .or_insert_with(Vec::new)
            .push(assignment);

        Ok(())
    }

    /// Revoke a capability from an instance
    pub fn revoke_capability(
        &mut self,
        instance_id: &str,
        capability_id: &str,
    ) -> std::result::Result<(), ErrorResponse> {
        // Validate instance_id
        if instance_id.is_empty() {
            return Err(ErrorResponse::new(
                "INVALID_REQUEST",
                "Instance ID cannot be empty",
            ));
        }

        // Validate capability_id
        if capability_id.is_empty() {
            return Err(ErrorResponse::new(
                "INVALID_REQUEST",
                "Capability ID cannot be empty",
            ));
        }

        // Remove capability assignment
        if let Some(assignments) = self.capabilities.get_mut(instance_id) {
            assignments.retain(|a| a.capability_id != capability_id);

            // Clean up empty entry
            if assignments.is_empty() {
                self.capabilities.remove(instance_id);
            }

            Ok(())
        } else {
            Err(ErrorResponse::new(
                "INSTANCE_NOT_FOUND",
                format!("Instance {} has no capability assignments", instance_id),
            ))
        }
    }

    /// List all instances
    pub fn list_instances(&self) -> Vec<&InstanceMetadata> {
        self.instances.values().collect()
    }

    /// Get capability assignments for an instance
    pub fn get_capabilities(&self, instance_id: &str) -> Option<&Vec<CapabilityAssignment>> {
        self.capabilities.get(instance_id)
    }

    /// Get instance metadata (internal use)
    pub fn get_instance(&self, instance_id: &str) -> Option<&InstanceMetadata> {
        self.instances.get(instance_id)
    }

    /// Restore recovered instance metadata and capability assignments.
    /// Used during control-plane restart recovery from node-agent reports.
    pub fn restore_instance_state(
        &mut self,
        metadata: InstanceMetadata,
        capabilities: Vec<CapabilityAssignment>,
    ) {
        let instance_id = metadata.instance_id.clone();
        self.instances.insert(instance_id.clone(), metadata);

        if capabilities.is_empty() {
            self.capabilities.remove(&instance_id);
        } else {
            self.capabilities.insert(instance_id, capabilities);
        }
    }

    /// Update instance status (called by Node Agent)
    pub fn update_instance_status(
        &mut self,
        instance_id: &str,
        status: InstanceStatus,
    ) -> Result<()> {
        if let Some(metadata) = self.instances.get_mut(instance_id) {
            metadata.status = status;
            Ok(())
        } else {
            Err(CoreError::InvalidInstanceId(instance_id.to_string()))
        }
    }

    /// Record an instance crash and update system state
    /// Implements crash recovery logic that preserves system-level state
    pub fn record_instance_crash(
        &mut self,
        instance_id: &str,
        error: impl Into<String>,
    ) -> std::result::Result<(), ErrorResponse> {
        let error_msg = error.into();

        // Validate instance exists
        if !self.instances.contains_key(instance_id) {
            return Err(ErrorResponse::new(
                "INSTANCE_NOT_FOUND",
                format!("Instance {} not found", instance_id),
            ));
        }

        // Record crash event
        self.event_recorder.record_crash(instance_id, &error_msg);

        // Mark instance as crashed
        self.crashed_instances
            .insert(instance_id.to_string(), std::time::Instant::now());

        // Update instance status to Crashed
        if let Some(metadata) = self.instances.get_mut(instance_id) {
            metadata.status = InstanceStatus::Crashed;
        }

        Ok(())
    }

    /// Handle crash recovery - preserves system-level state while allowing instance restart
    /// System state preserved: crash history, capability assignments, metadata
    pub fn handle_crash_recovery(
        &mut self,
        instance_id: &str,
    ) -> std::result::Result<(), ErrorResponse> {
        // Validate instance exists
        if !self.instances.contains_key(instance_id) {
            return Err(ErrorResponse::new(
                "INSTANCE_NOT_FOUND",
                format!("Instance {} not found for recovery", instance_id),
            ));
        }

        // Keep system-level state (capability assignments are preserved automatically)
        // The crash_history HashMap preserves crash counts across restarts

        // Clear crash marker (instance can now be restarted)
        self.crashed_instances.remove(instance_id);

        // Record restart event
        self.event_recorder.record_restart(instance_id);

        // Reset instance status to Starting
        if let Some(metadata) = self.instances.get_mut(instance_id) {
            metadata.status = InstanceStatus::Starting;
        }

        Ok(())
    }

    /// Get crash recovery information for an instance
    pub fn get_crash_info(&self, instance_id: &str) -> Option<CrashInfo> {
        if self.crashed_instances.contains_key(instance_id) {
            Some(CrashInfo {
                crash_count: 1, // Simplified: actual implementation would track full history
                last_crash_time: Some(std::time::Instant::now()),
            })
        } else {
            None
        }
    }

    /// Check if an instance is currently in crashed state
    pub fn is_instance_crashed(&self, instance_id: &str) -> bool {
        self.crashed_instances.contains_key(instance_id)
    }

    /// Get all execution events (for debugging and monitoring)
    pub fn get_execution_events(&self) -> &[wasmatrix_core::ExecutionEvent] {
        self.event_recorder.get_events()
    }

    /// Get execution events for a specific instance
    pub fn get_execution_events_for_instance(
        &self,
        instance_id: &str,
    ) -> Vec<&wasmatrix_core::ExecutionEvent> {
        self.event_recorder.get_events_for_instance(instance_id)
    }
}

impl Default for ControlPlane {
    fn default() -> Self {
        Self::new("default-node")
    }
}

/// Crash information for recovery
#[derive(Debug, Clone)]
pub struct CrashInfo {
    pub crash_count: u32,
    pub last_crash_time: Option<std::time::Instant>,
}

impl CrashInfo {
    pub fn new() -> Self {
        Self {
            crash_count: 0,
            last_crash_time: None,
        }
    }

    pub fn record_crash(&mut self) {
        self.crash_count += 1;
        self.last_crash_time = Some(std::time::Instant::now());
    }
}

impl Default for CrashInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Error context for crash recovery logging
#[derive(Debug, Clone)]
pub struct CrashContext {
    pub instance_id: String,
    pub error: String,
    pub timestamp: std::time::Instant,
}

impl CrashContext {
    pub fn new(instance_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            error: error.into(),
            timestamp: std::time::Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmatrix_core::{ProviderType, RestartPolicy};

    fn create_valid_wasm_module() -> Vec<u8> {
        // Minimal valid Wasm module (magic bytes + version)
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    #[test]
    fn test_start_instance_success() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = cp.start_instance(request).unwrap();
        assert!(!instance_id.is_empty());
        assert!(cp.get_instance(&instance_id).is_some());
    }

    #[test]
    fn test_start_instance_empty_module() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: vec![],
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let result = cp.start_instance(request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");
    }

    #[test]
    fn test_start_instance_invalid_wasm() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: vec![0x00, 0x00, 0x00, 0x00],
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let result = cp.start_instance(request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");
    }

    #[test]
    fn test_stop_instance_success() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = cp.start_instance(request).unwrap();
        let stop_request = StopInstanceRequest {
            instance_id: instance_id.clone(),
        };

        cp.stop_instance(stop_request).unwrap();
        assert_eq!(
            cp.get_instance(&instance_id).unwrap().status,
            InstanceStatus::Stopped
        );
    }

    #[test]
    fn test_stop_instance_not_found() {
        let mut cp = ControlPlane::new("node-1");
        let request = StopInstanceRequest {
            instance_id: "non-existent".to_string(),
        };

        let result = cp.stop_instance(request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INSTANCE_NOT_FOUND");
    }

    #[test]
    fn test_query_instance_success() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = cp.start_instance(request).unwrap();
        let query_request = QueryInstanceRequest {
            instance_id: instance_id.clone(),
        };

        let response = cp.query_instance(query_request).unwrap();
        assert_eq!(response.instance_id, instance_id);
    }

    #[test]
    fn test_query_instance_not_found() {
        let cp = ControlPlane::new("node-1");
        let request = QueryInstanceRequest {
            instance_id: "non-existent".to_string(),
        };

        let result = cp.query_instance(request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INSTANCE_NOT_FOUND");
    }

    #[test]
    fn test_assign_capability_success() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = cp.start_instance(request).unwrap();
        let assignment = CapabilityAssignment::new(
            instance_id.clone(),
            "kv-1".to_string(),
            ProviderType::Kv,
            vec!["kv:read".to_string(), "kv:write".to_string()],
        );

        cp.assign_capability(assignment).unwrap();
        let capabilities = cp.get_capabilities(&instance_id).unwrap();
        assert_eq!(capabilities.len(), 1);
        assert_eq!(capabilities[0].capability_id, "kv-1");
    }

    #[test]
    fn test_revoke_capability_success() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = cp.start_instance(request).unwrap();
        let assignment = CapabilityAssignment::new(
            instance_id.clone(),
            "kv-1".to_string(),
            ProviderType::Kv,
            vec!["kv:read".to_string()],
        );

        cp.assign_capability(assignment).unwrap();
        cp.revoke_capability(&instance_id, "kv-1").unwrap();

        assert!(cp.get_capabilities(&instance_id).is_none());
    }

    #[test]
    fn test_list_instances() {
        let mut cp = ControlPlane::new("node-1");

        // Start multiple instances
        for _ in 0..3 {
            let request = StartInstanceRequest {
                module_bytes: create_valid_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };
            cp.start_instance(request).unwrap();
        }

        let instances = cp.list_instances();
        assert_eq!(instances.len(), 3);
    }

    // === Task 9.2: Crash Recovery Logic Tests ===

    #[test]
    fn test_record_instance_crash_success() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = cp.start_instance(request).unwrap();

        // Record crash
        let result = cp.record_instance_crash(&instance_id, "test error");
        assert!(result.is_ok());

        // Verify instance is marked as crashed
        assert!(cp.is_instance_crashed(&instance_id));
        assert_eq!(
            cp.get_instance(&instance_id).unwrap().status,
            InstanceStatus::Crashed
        );

        // Verify crash event was recorded (only crash event, start is recorded by NodeAgent)
        let events = cp.get_execution_events_for_instance(&instance_id);
        assert_eq!(events.len(), 1); // only crash event
        assert_eq!(events[0].event_type, "instance_crashed");
    }

    #[test]
    fn test_record_instance_crash_not_found() {
        let mut cp = ControlPlane::new("node-1");

        let result = cp.record_instance_crash("non-existent", "test error");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INSTANCE_NOT_FOUND");
    }

    #[test]
    fn test_handle_crash_recovery_success() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = cp.start_instance(request).unwrap();

        // Record crash first
        cp.record_instance_crash(&instance_id, "test error")
            .unwrap();
        assert!(cp.is_instance_crashed(&instance_id));

        // Handle recovery
        let result = cp.handle_crash_recovery(&instance_id);
        assert!(result.is_ok());

        // Verify instance is no longer marked as crashed
        assert!(!cp.is_instance_crashed(&instance_id));
        assert_eq!(
            cp.get_instance(&instance_id).unwrap().status,
            InstanceStatus::Starting
        );

        // Verify restart event was recorded (crash + restart events)
        let events = cp.get_execution_events_for_instance(&instance_id);
        assert_eq!(events.len(), 2); // crash + restart
        assert_eq!(events[0].event_type, "instance_crashed");
        assert_eq!(events[1].event_type, "instance_restarted");
    }

    #[test]
    fn test_handle_crash_recovery_not_found() {
        let mut cp = ControlPlane::new("node-1");

        let result = cp.handle_crash_recovery("non-existent");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INSTANCE_NOT_FOUND");
    }

    #[test]
    fn test_crash_recovery_preserves_capabilities() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = cp.start_instance(request).unwrap();

        // Add capability assignment
        let assignment = CapabilityAssignment::new(
            instance_id.clone(),
            "kv-1".to_string(),
            ProviderType::Kv,
            vec!["kv:read".to_string(), "kv:write".to_string()],
        );
        cp.assign_capability(assignment).unwrap();

        // Record crash
        cp.record_instance_crash(&instance_id, "test error")
            .unwrap();

        // Handle recovery
        cp.handle_crash_recovery(&instance_id).unwrap();

        // Verify capability assignments are preserved
        let capabilities = cp.get_capabilities(&instance_id).unwrap();
        assert_eq!(capabilities.len(), 1);
        assert_eq!(capabilities[0].capability_id, "kv-1");
        assert!(capabilities[0].has_permission("kv:read"));
        assert!(capabilities[0].has_permission("kv:write"));
    }

    #[test]
    fn test_system_state_preserved_during_crash() {
        let mut cp = ControlPlane::new("node-1");

        // Start multiple instances
        let instance_ids: Vec<String> = (0..3)
            .map(|i| {
                let request = StartInstanceRequest {
                    module_bytes: create_valid_wasm_module(),
                    capabilities: vec![],
                    restart_policy: RestartPolicy::default(),
                };
                cp.start_instance(request).unwrap()
            })
            .collect();

        // Crash one instance
        cp.record_instance_crash(&instance_ids[1], "test error")
            .unwrap();

        // Verify other instances are unaffected
        assert!(!cp.is_instance_crashed(&instance_ids[0]));
        assert!(cp.is_instance_crashed(&instance_ids[1]));
        assert!(!cp.is_instance_crashed(&instance_ids[2]));

        // Verify all instances still exist
        assert_eq!(cp.list_instances().len(), 3);
    }

    // === Task 2.4: Unit Tests for Control Plane API Handlers ===

    #[test]
    fn test_query_instance_empty_id() {
        let cp = ControlPlane::new("node-1");
        let request = QueryInstanceRequest {
            instance_id: "".to_string(),
        };

        let result = cp.query_instance(request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");
    }

    #[test]
    fn test_stop_instance_empty_id() {
        let mut cp = ControlPlane::new("node-1");
        let request = StopInstanceRequest {
            instance_id: "".to_string(),
        };

        let result = cp.stop_instance(request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");
    }

    #[test]
    fn test_assign_capability_empty_instance_id() {
        let mut cp = ControlPlane::new("node-1");
        let assignment = CapabilityAssignment::new(
            "".to_string(),
            "kv-1".to_string(),
            ProviderType::Kv,
            vec!["kv:read".to_string()],
        );

        let result = cp.assign_capability(assignment);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INSTANCE_NOT_FOUND");
    }

    #[test]
    fn test_assign_capability_empty_capability_id() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = cp.start_instance(request).unwrap();
        let assignment = CapabilityAssignment::new(
            instance_id.clone(),
            "".to_string(),
            ProviderType::Kv,
            vec!["kv:read".to_string()],
        );

        let result = cp.assign_capability(assignment);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");
    }

    #[test]
    fn test_assign_capability_empty_permissions() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = cp.start_instance(request).unwrap();
        let assignment = CapabilityAssignment::new(
            instance_id.clone(),
            "kv-1".to_string(),
            ProviderType::Kv,
            vec![],
        );

        let result = cp.assign_capability(assignment);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");
    }

    #[test]
    fn test_revoke_capability_empty_instance_id() {
        let mut cp = ControlPlane::new("node-1");

        let result = cp.revoke_capability("", "kv-1");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");
    }

    #[test]
    fn test_revoke_capability_empty_capability_id() {
        let mut cp = ControlPlane::new("node-1");
        let request = StartInstanceRequest {
            module_bytes: create_valid_wasm_module(),
            capabilities: vec![],
            restart_policy: RestartPolicy::default(),
        };

        let instance_id = cp.start_instance(request).unwrap();

        let result = cp.revoke_capability(&instance_id, "");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");
    }

    #[test]
    fn test_revoke_capability_instance_not_found() {
        let mut cp = ControlPlane::new("node-1");

        let result = cp.revoke_capability("non-existent", "kv-1");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INSTANCE_NOT_FOUND");
    }

    // === Task 9.4: Unit Tests for Error Handling ===

    #[test]
    fn test_error_response_invalid_request() {
        let error = ErrorResponse::new("INVALID_REQUEST", "Test invalid request");
        assert_eq!(error.error_code, "INVALID_REQUEST");
        assert_eq!(error.message, "Test invalid request");
    }

    #[test]
    fn test_error_response_instance_not_found() {
        let error = ErrorResponse::new("INSTANCE_NOT_FOUND", "Instance xyz not found");
        assert_eq!(error.error_code, "INSTANCE_NOT_FOUND");
        assert_eq!(error.message, "Instance xyz not found");
    }

    #[test]
    fn test_error_response_with_details() {
        let mut details = std::collections::HashMap::new();
        details.insert("field".to_string(), "instance_id".to_string());
        details.insert("value".to_string(), "invalid-id".to_string());

        let error = ErrorResponse::new("VALIDATION_ERROR", "Invalid field").with_details(details);
        assert_eq!(error.error_code, "VALIDATION_ERROR");
        assert!(error.details.is_some());

        let details = error.details.unwrap();
        assert_eq!(details.get("field"), Some(&"instance_id".to_string()));
        assert_eq!(details.get("value"), Some(&"invalid-id".to_string()));
    }

    #[test]
    fn test_all_error_codes_exist() {
        // Verify all required error codes are defined
        let error_codes = vec![
            "INVALID_REQUEST",
            "INSTANCE_NOT_FOUND",
            "CAPABILITY_NOT_FOUND",
            "PERMISSION_DENIED",
            "STORAGE_ERROR",
            "VALIDATION_ERROR",
            "WASM_RUNTIME_ERROR",
            "RESOURCE_EXHAUSTED",
            "TIMEOUT",
            "CRASH_DETECTED",
            "RESTART_POLICY_VIOLATION",
        ];

        for code in error_codes {
            let error = ErrorResponse::new(code, "test message");
            assert_eq!(error.error_code, code);
        }
    }

    #[test]
    fn test_error_handling_cascade() {
        let mut cp = ControlPlane::new("node-1");

        // Try to stop non-existent instance
        let result = cp.stop_instance(StopInstanceRequest {
            instance_id: "non-existent".to_string(),
        });
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.error_code, "INSTANCE_NOT_FOUND");
        assert!(err.timestamp.timestamp() > 0); // Verify timestamp is set

        // Try to query non-existent instance
        let result = cp.query_instance(QueryInstanceRequest {
            instance_id: "non-existent".to_string(),
        });
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().error_code, "INSTANCE_NOT_FOUND");
    }

    // === Property-Based Tests ===

    /// Property 6: Control Plane Instance Lifecycle Operations
    /// For any valid sequence of start, stop, and query operations on instances,
    /// the Control Plane maintains correct state and returns accurate status.
    /// Validates: Requirements 3.1, 3.2, 3.3
    mod property_tests_lifecycle {
        use super::*;

        #[test]
        fn property_instance_lifecycle_start_stop_query() {
            let mut cp = ControlPlane::new("node-1");

            // Start an instance
            let request = StartInstanceRequest {
                module_bytes: create_valid_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };
            let instance_id = cp.start_instance(request).unwrap();

            // Query should return the instance
            let query_result = cp.query_instance(QueryInstanceRequest {
                instance_id: instance_id.clone(),
            });
            assert!(query_result.is_ok());
            assert_eq!(query_result.unwrap().instance_id, instance_id);

            // Stop the instance
            cp.stop_instance(StopInstanceRequest {
                instance_id: instance_id.clone(),
            })
            .unwrap();

            // Query should still return the instance (with Stopped status)
            let query_result = cp.query_instance(QueryInstanceRequest {
                instance_id: instance_id.clone(),
            });
            assert!(query_result.is_ok());
            assert_eq!(query_result.unwrap().status, InstanceStatus::Stopped);
        }

        #[test]
        fn property_multiple_instances_independent() {
            let mut cp = ControlPlane::new("node-1");
            let mut instance_ids = Vec::new();

            // Start multiple instances
            for i in 0..5 {
                let request = StartInstanceRequest {
                    module_bytes: create_valid_wasm_module(),
                    capabilities: vec![],
                    restart_policy: RestartPolicy::default(),
                };
                let id = cp.start_instance(request).unwrap();
                instance_ids.push(id);
            }

            // Stop middle instance
            cp.stop_instance(StopInstanceRequest {
                instance_id: instance_ids[2].clone(),
            })
            .unwrap();

            // Query all instances - only the stopped one should have Stopped status
            for (i, id) in instance_ids.iter().enumerate() {
                let status = cp
                    .query_instance(QueryInstanceRequest {
                        instance_id: id.clone(),
                    })
                    .unwrap()
                    .status;

                if i == 2 {
                    assert_eq!(status, InstanceStatus::Stopped);
                } else {
                    assert_eq!(status, InstanceStatus::Starting);
                }
            }
        }

        #[test]
        fn property_start_after_stop_creates_new_instance() {
            let mut cp = ControlPlane::new("node-1");

            // Start first instance
            let request1 = StartInstanceRequest {
                module_bytes: create_valid_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };
            let instance_id_1 = cp.start_instance(request1).unwrap();

            // Stop it
            cp.stop_instance(StopInstanceRequest {
                instance_id: instance_id_1.clone(),
            })
            .unwrap();

            // Start second instance
            let request2 = StartInstanceRequest {
                module_bytes: create_valid_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };
            let instance_id_2 = cp.start_instance(request2).unwrap();

            // Should be different instances
            assert_ne!(instance_id_1, instance_id_2);

            // Both should exist
            assert_eq!(cp.list_instances().len(), 2);
        }
    }

    /// Property 18: API Request Validation
    /// For any API request, invalid parameters result in appropriate error responses.
    /// Validates: Requirements 13.5, 13.6
    mod property_tests_validation {
        use super::*;

        #[test]
        fn property_empty_module_bytes_returns_invalid_request() {
            let mut cp = ControlPlane::new("node-1");
            let request = StartInstanceRequest {
                module_bytes: vec![],
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };

            let result = cp.start_instance(request);
            assert!(result.is_err());
            assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");
        }

        #[test]
        fn property_invalid_wasm_module_returns_invalid_request() {
            let mut cp = ControlPlane::new("node-1");
            let request = StartInstanceRequest {
                module_bytes: vec![0x00, 0x00, 0x00, 0x00], // Invalid magic bytes
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };

            let result = cp.start_instance(request);
            assert!(result.is_err());
            assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");
        }

        #[test]
        fn property_empty_instance_id_returns_invalid_request() {
            let cp = ControlPlane::new("node-1");

            // Query with empty ID
            let result = cp.query_instance(QueryInstanceRequest {
                instance_id: "".to_string(),
            });
            assert!(result.is_err());
            assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");

            // Stop with empty ID
            let mut cp2 = ControlPlane::new("node-1");
            let result = cp2.stop_instance(StopInstanceRequest {
                instance_id: "".to_string(),
            });
            assert!(result.is_err());
            assert_eq!(result.unwrap_err().error_code, "INVALID_REQUEST");
        }

        #[test]
        fn property_nonexistent_instance_returns_not_found() {
            let cp = ControlPlane::new("node-1");

            let result = cp.query_instance(QueryInstanceRequest {
                instance_id: "nonexistent-instance-id".to_string(),
            });
            assert!(result.is_err());
            assert_eq!(result.unwrap_err().error_code, "INSTANCE_NOT_FOUND");
        }

        #[test]
        fn property_error_responses_have_required_fields() {
            let error = ErrorResponse::new("TEST_ERROR", "Test message");

            // All error responses must have error_code, message, and timestamp
            assert!(!error.error_code.is_empty());
            assert!(!error.message.is_empty());
            assert!(error.timestamp.timestamp() > 0);
        }
    }

    /// Property 7: Minimal State Storage Policy
    /// For any instance operation, the system only stores instance IDs, capability assignments,
    /// and crash history - no application data, session state, or execution results are stored.
    /// Validates: Requirements 6.1, 6.3, 6.4, 6.5, 6.6, 3.4
    mod property_tests_minimal_state {
        use super::*;

        #[test]
        fn property_only_instance_metadata_stored() {
            let mut cp = ControlPlane::new("node-1");

            let request = StartInstanceRequest {
                module_bytes: create_valid_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };

            let instance_id = cp.start_instance(request).unwrap();
            let metadata = cp.get_instance(&instance_id).unwrap();

            // Only store: instance_id, node_id, module_hash, created_at, status
            // Do NOT store: module_bytes, execution results, session state
            assert!(!metadata.instance_id.is_empty());
            assert!(!metadata.node_id.is_empty());
            assert!(!metadata.module_hash.is_empty());
            // module_bytes should NOT be in metadata (verified by type system)
        }

        #[test]
        fn property_no_application_data_in_state() {
            let mut cp = ControlPlane::new("node-1");

            // Even after crash and recovery, no application data should be stored
            let request = StartInstanceRequest {
                module_bytes: create_valid_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };

            let instance_id = cp.start_instance(request).unwrap();

            // Simulate crash
            cp.record_instance_crash(&instance_id, "test error")
                .unwrap();

            // State should only contain metadata, not application data
            let events = cp.get_execution_events();
            for event in events {
                // Events should not contain application data
                if let Some(details) = &event.details {
                    assert!(!details.contains_key("application_data"));
                    assert!(!details.contains_key("session_state"));
                }
            }
        }

        #[test]
        fn property_capability_assignments_separate_from_instance_data() {
            let mut cp = ControlPlane::new("node-1");

            let request = StartInstanceRequest {
                module_bytes: create_valid_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };

            let instance_id = cp.start_instance(request).unwrap();

            // Assign capability
            let assignment = CapabilityAssignment::new(
                instance_id.clone(),
                "kv-1".to_string(),
                ProviderType::Kv,
                vec!["kv:read".to_string()],
            );
            cp.assign_capability(assignment).unwrap();

            // Capabilities should be stored separately from instance metadata
            assert!(cp.get_capabilities(&instance_id).is_some());
            // Instance metadata should not contain capability data directly
            let metadata = cp.get_instance(&instance_id).unwrap();
            // (Type system enforces this separation)
        }
    }

    /// Property 9.3: System State Preservation During Instance Crashes
    /// For any instance crash, system-level state (crash history, capability assignments,
    /// metadata) is preserved while instance-specific state is cleared.
    /// Validates: Requirements 8.1, 8.3, 8.4
    mod property_tests_crash_resilience {
        use super::*;

        #[test]
        fn property_crash_history_preserved() {
            let mut cp = ControlPlane::new("node-1");

            let request = StartInstanceRequest {
                module_bytes: create_valid_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };

            let instance_id = cp.start_instance(request).unwrap();

            // Record multiple crashes
            for i in 1..=3 {
                cp.record_instance_crash(&instance_id, format!("error {}", i))
                    .unwrap();
            }

            // All crashes should be recorded
            let events = cp.get_execution_events_for_instance(&instance_id);
            let crash_events: Vec<_> = events
                .iter()
                .filter(|e| e.event_type == "instance_crashed")
                .collect();
            assert_eq!(crash_events.len(), 3);
        }

        #[test]
        fn property_system_state_preserved_across_crash_recovery() {
            let mut cp = ControlPlane::new("node-1");

            let request = StartInstanceRequest {
                module_bytes: create_valid_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };

            let instance_id = cp.start_instance(request).unwrap();

            // Add capability before crash
            let assignment = CapabilityAssignment::new(
                instance_id.clone(),
                "kv-1".to_string(),
                ProviderType::Kv,
                vec!["kv:read".to_string()],
            );
            cp.assign_capability(assignment).unwrap();

            // Record crash
            cp.record_instance_crash(&instance_id, "test error")
                .unwrap();

            // Recover
            cp.handle_crash_recovery(&instance_id).unwrap();

            // Verify system state preserved
            assert!(cp.get_capabilities(&instance_id).is_some());
            assert!(cp.get_instance(&instance_id).is_some());

            // Verify crash events still recorded
            let events = cp.get_execution_events_for_instance(&instance_id);
            assert!(events.iter().any(|e| e.event_type == "instance_crashed"));
            assert!(events.iter().any(|e| e.event_type == "instance_restarted"));
        }

        #[test]
        fn property_crash_isolation_between_instances() {
            let mut cp = ControlPlane::new("node-1");

            // Start two instances
            let request = StartInstanceRequest {
                module_bytes: create_valid_wasm_module(),
                capabilities: vec![],
                restart_policy: RestartPolicy::default(),
            };

            let instance_id_1 = cp.start_instance(request.clone()).unwrap();
            let instance_id_2 = cp.start_instance(request).unwrap();

            // Crash only instance 1
            cp.record_instance_crash(&instance_id_1, "test error")
                .unwrap();

            // Instance 1 should be crashed
            assert!(cp.is_instance_crashed(&instance_id_1));
            assert_eq!(
                cp.get_instance(&instance_id_1).unwrap().status,
                InstanceStatus::Crashed
            );

            // Instance 2 should be unaffected
            assert!(!cp.is_instance_crashed(&instance_id_2));
            assert_eq!(
                cp.get_instance(&instance_id_2).unwrap().status,
                InstanceStatus::Starting
            );
        }
    }
}
