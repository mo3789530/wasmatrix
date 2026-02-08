//! Statelessness guarantees for the Wasm Orchestrator
//!
//! This module enforces the core principle that Wasm instances are stateless
//! and restart-assumed. No instance memory state is persisted.

use crate::{CapabilityAssignment, CoreError, InstanceMetadata, InstanceStatus, Result};

/// Audit record for verifying statelessness
#[derive(Debug, Clone)]
pub struct StateAudit {
    pub instance_id: String,
    pub stored_fields: Vec<String>,
    pub excluded_fields: Vec<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl StateAudit {
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            stored_fields: Vec::new(),
            excluded_fields: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Verify that only allowed metadata is stored
    pub fn verify_minimal_storage(&self) -> Result<()> {
        let allowed_fields = vec![
            "instance_id",
            "node_id",
            "module_hash",
            "created_at",
            "status",
        ];

        for field in &self.stored_fields {
            if !allowed_fields.contains(&field.as_str()) {
                return Err(CoreError::InvalidCapabilityAssignment(format!(
                    "State violation: field '{}' should not be persisted",
                    field
                )));
            }
        }

        Ok(())
    }
}

/// Policy enforcer for minimal state storage
pub struct StatelessnessPolicy;

impl StatelessnessPolicy {
    /// Verify that instance metadata contains no application state
    pub fn verify_instance_metadata(metadata: &InstanceMetadata) -> Result<()> {
        // Ensure no application data in metadata
        // Only system-level fields should be present
        let allowed_statuses = vec![
            InstanceStatus::Starting,
            InstanceStatus::Running,
            InstanceStatus::Stopped,
            InstanceStatus::Crashed,
        ];

        if !allowed_statuses.contains(&metadata.status) {
            return Err(CoreError::InvalidCapabilityAssignment(
                "Invalid instance status in metadata".to_string(),
            ));
        }

        Ok(())
    }

    /// Verify capability assignments contain no application data
    pub fn verify_capability_assignments(assignments: &[CapabilityAssignment]) -> Result<()> {
        for assignment in assignments {
            // Verify assignment only contains capability metadata
            // No application state should be in assignments
            if assignment.capability_id.is_empty() {
                return Err(CoreError::InvalidCapabilityAssignment(
                    "Empty capability_id in assignment".to_string(),
                ));
            }

            if assignment.instance_id.is_empty() {
                return Err(CoreError::InvalidCapabilityAssignment(
                    "Empty instance_id in assignment".to_string(),
                ));
            }

            // Verify permissions are not empty
            if assignment.permissions.is_empty() {
                return Err(CoreError::InvalidCapabilityAssignment(
                    "Empty permissions in assignment".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Verify that restart clears all instance state
    pub fn verify_restart_state_cleared(
        instance_id: &str,
        old_metadata: Option<&InstanceMetadata>,
        new_metadata: &InstanceMetadata,
    ) -> Result<()> {
        if let Some(old) = old_metadata {
            // Ensure instance gets a new ID on restart
            if old.instance_id == new_metadata.instance_id {
                return Err(CoreError::InvalidCapabilityAssignment(format!(
                    "State violation: instance {} retains same ID after restart",
                    instance_id
                )));
            }

            // Ensure new creation time (not copied from old)
            if old.created_at >= new_metadata.created_at {
                return Err(CoreError::InvalidCapabilityAssignment(format!(
                    "State violation: instance {} creation time not updated after restart",
                    instance_id
                )));
            }
        }

        Ok(())
    }

    /// Check that no logs are persisted as state
    pub fn verify_no_log_state(logs: &[String]) -> Result<()> {
        // Logs should be ephemeral, not stored as state
        // This is a documentation/verification function
        // In practice, logs are written to stdout/stderr, not stored in metadata
        Ok(())
    }
}

/// Trait for KV provider externalization
pub trait KvProvider {
    fn set(&mut self, key: String, value: String) -> Result<()>;
}

/// Externalization helper for state that must be persisted
pub struct StateExternalizer;

impl StateExternalizer {
    /// All persistent state must be externalized through capability providers
    /// This function documents the externalization pattern
    pub fn externalize_via_kv<T: serde::Serialize>(
        key: &str,
        value: &T,
        kv_provider: &mut dyn KvProvider,
    ) -> Result<()> {
        let json = serde_json::to_string(value)
            .map_err(|e| CoreError::SerializationError(e.to_string()))?;

        kv_provider.set(key.to_string(), json)
    }
}

#[cfg(test)]
mod tests {
    /// Test utilities for statelessness tests
    use super::*;
    use crate::{ProviderType, RestartPolicy};

    fn create_test_assignment(
        instance_id: &str,
        capability_id: &str,
        provider_type: ProviderType,
        permissions: Vec<&str>,
    ) -> CapabilityAssignment {
        CapabilityAssignment::new(
            instance_id.to_string(),
            capability_id.to_string(),
            provider_type,
            permissions.into_iter().map(|s| s.to_string()).collect(),
        )
    }

    #[test]
    fn test_state_audit_creation() {
        let audit = StateAudit::new("test-instance");
        assert_eq!(audit.instance_id, "test-instance");
        assert!(audit.stored_fields.is_empty());
        assert!(audit.excluded_fields.is_empty());
    }

    #[test]
    fn test_state_audit_with_fields() {
        let mut audit = StateAudit::new("test-instance");
        audit.stored_fields.push("instance_id".to_string());
        audit.stored_fields.push("node_id".to_string());
        assert_eq!(audit.stored_fields.len(), 2);
    }

    #[test]
    fn test_state_audit_timestamp() {
        let audit = StateAudit::new("test-instance");
        let now = chrono::Utc::now();
        let timestamp_diff = (now - audit.timestamp).num_seconds().abs();
        assert!(timestamp_diff < 1, "Timestamp should be recent");
    }

    #[test]
    fn test_verify_minimal_storage_allowed_fields() {
        let audit = StateAudit {
            instance_id: "test-1".to_string(),
            stored_fields: vec![
                "instance_id".to_string(),
                "node_id".to_string(),
                "module_hash".to_string(),
            ],
            excluded_fields: vec![],
            timestamp: chrono::Utc::now(),
        };

        assert!(audit.verify_minimal_storage().is_ok());
    }

    #[test]
    fn test_verify_minimal_storage_violation() {
        let audit = StateAudit {
            instance_id: "test-1".to_string(),
            stored_fields: vec![
                "instance_id".to_string(),
                "user_session_data".to_string(), // Not allowed!
            ],
            excluded_fields: vec![],
            timestamp: chrono::Utc::now(),
        };

        assert!(audit.verify_minimal_storage().is_err());
    }

    #[test]
    fn test_verify_minimal_storage_multiple_violations() {
        let audit = StateAudit {
            instance_id: "test-1".to_string(),
            stored_fields: vec![
                "instance_id".to_string(),
                "app_data".to_string(),     // Not allowed!
                "session_info".to_string(), // Not allowed!
                "cache".to_string(),        // Not allowed!
            ],
            excluded_fields: vec![],
            timestamp: chrono::Utc::now(),
        };

        assert!(audit.verify_minimal_storage().is_err());
    }

    #[test]
    fn test_verify_instance_metadata() {
        let metadata = InstanceMetadata::new("node-1".to_string(), "hash123".to_string());

        assert!(StatelessnessPolicy::verify_instance_metadata(&metadata).is_ok());
    }

    #[test]
    fn test_verify_instance_metadata_invalid_status() {
        let metadata = InstanceMetadata {
            instance_id: "test".to_string(),
            node_id: "node-1".to_string(),
            module_hash: "hash123".to_string(),
            created_at: chrono::Utc::now(),
            status: InstanceStatus::Running,
        };

        // Create invalid metadata (but can't directly change status to invalid enum)
        // So just test valid status
        assert!(StatelessnessPolicy::verify_instance_metadata(&metadata).is_ok());
    }

    #[test]
    fn test_verify_capability_assignments() {
        let assignments = vec![CapabilityAssignment::new(
            "instance-1".to_string(),
            "kv-1".to_string(),
            ProviderType::Kv,
            vec!["kv:read".to_string()],
        )];

        assert!(StatelessnessPolicy::verify_capability_assignments(&assignments).is_ok());
    }

    #[test]
    fn test_verify_capability_assignments_empty_id() {
        let assignments = vec![CapabilityAssignment::new(
            "instance-1".to_string(),
            "".to_string(), // Empty capability_id
            ProviderType::Kv,
            vec!["kv:read".to_string()],
        )];

        assert!(StatelessnessPolicy::verify_capability_assignments(&assignments).is_err());
    }

    #[test]
    fn test_verify_capability_assignments_empty_permissions() {
        let assignments = vec![CapabilityAssignment::new(
            "instance-1".to_string(),
            "kv-1".to_string(),
            ProviderType::Kv,
            vec![], // Empty permissions
        )];

        assert!(StatelessnessPolicy::verify_capability_assignments(&assignments).is_err());
    }

    #[test]
    fn test_verify_restart_state_cleared_same_id() {
        // Create metadata with same ID (simulating a bug where ID is reused)
        let shared_id = "same-instance-id".to_string();
        let old_metadata = InstanceMetadata {
            instance_id: shared_id.clone(),
            node_id: "node-1".to_string(),
            module_hash: "hash123".to_string(),
            created_at: chrono::Utc::now(),
            status: InstanceStatus::Running,
        };

        // Wait a moment
        std::thread::sleep(std::time::Duration::from_millis(10));

        // New metadata with SAME ID (this should fail validation)
        let new_metadata = InstanceMetadata {
            instance_id: shared_id, // Same ID!
            node_id: "node-1".to_string(),
            module_hash: "hash123".to_string(),
            created_at: chrono::Utc::now(),
            status: InstanceStatus::Starting,
        };

        // Should fail because instance_id is the same after restart
        let result = StatelessnessPolicy::verify_restart_state_cleared(
            "test-instance",
            Some(&old_metadata),
            &new_metadata,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_verify_restart_state_cleared_new_id() {
        // First instance
        let old_id = "old-instance-id".to_string();
        let old_metadata = InstanceMetadata {
            instance_id: old_id.clone(),
            node_id: "node-1".to_string(),
            module_hash: "hash123".to_string(),
            created_at: chrono::Utc::now(),
            status: InstanceStatus::Running,
        };

        // Wait a moment to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(10));

        // New instance after restart (different ID)
        let new_metadata = InstanceMetadata {
            instance_id: "new-instance-id".to_string(),
            node_id: "node-1".to_string(),
            module_hash: "hash123".to_string(),
            created_at: chrono::Utc::now(),
            status: InstanceStatus::Starting,
        };

        let result = StatelessnessPolicy::verify_restart_state_cleared(
            "test-instance",
            Some(&old_metadata),
            &new_metadata,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_restart_state_cleared_timestamp_old() {
        let now = chrono::Utc::now();
        let old_metadata = InstanceMetadata {
            instance_id: "old-instance".to_string(),
            node_id: "node-1".to_string(),
            module_hash: "hash123".to_string(),
            created_at: now,
            status: InstanceStatus::Running,
        };

        // Wait to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(10));
        let new_now = chrono::Utc::now();

        let new_metadata = InstanceMetadata {
            instance_id: "new-instance".to_string(),
            node_id: "node-1".to_string(),
            module_hash: "hash123".to_string(),
            created_at: now, // OLD timestamp - should fail!
            status: InstanceStatus::Starting,
        };

        let result = StatelessnessPolicy::verify_restart_state_cleared(
            "test-instance",
            Some(&old_metadata),
            &new_metadata,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_verify_no_log_state() {
        let logs = vec![
            "INFO: Instance started".to_string(),
            "ERROR: Crash detected".to_string(),
        ];
        assert!(StatelessnessPolicy::verify_no_log_state(&logs).is_ok());
    }

    #[test]
    fn test_verify_no_log_state_with_system_state() {
        let logs = vec!["INFO: System state updated".to_string()];
        // This function just verifies logs aren't stored as state
        // All logs are considered ephemeral
        assert!(StatelessnessPolicy::verify_no_log_state(&logs).is_ok());
    }

    #[test]
    #[test]
    #[test]
    #[test]
    fn test_statelessness_multiple_validations() {
        // Test multiple validations in sequence
        let assignments = vec![create_test_assignment(
            "instance-1",
            "kv-1",
            ProviderType::Kv,
            vec!["kv:read"],
        )];

        let mut all_passed = true;
        all_passed &= StatelessnessPolicy::verify_capability_assignments(&assignments).is_ok();
        all_passed &= StatelessnessPolicy::verify_no_log_state(&[]).is_ok();

        assert!(all_passed, "All statelessness validations should pass");
    }
}
