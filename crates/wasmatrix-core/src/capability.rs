use crate::{CapabilityAssignment, CoreError, ProviderType, Result};
use std::collections::HashMap;

/// Registry for managing capability assignments
#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    assignments: HashMap<String, Vec<CapabilityAssignment>>,
    /// Known provider IDs for validation
    known_providers: HashMap<String, ProviderType>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a known provider for validation
    pub fn register_provider(
        &mut self,
        provider_id: impl Into<String>,
        provider_type: ProviderType,
    ) {
        self.known_providers
            .insert(provider_id.into(), provider_type);
    }

    /// Validate a capability assignment
    fn validate_assignment(&self, assignment: &CapabilityAssignment) -> Result<()> {
        // Check that provider exists
        let provider_type = self
            .known_providers
            .get(&assignment.capability_id)
            .ok_or_else(|| {
                CoreError::InvalidCapabilityAssignment(format!(
                    "Provider '{}' not found",
                    assignment.capability_id
                ))
            })?;

        // Verify provider type matches
        if *provider_type != assignment.provider_type {
            return Err(CoreError::InvalidCapabilityAssignment(format!(
                "Provider type mismatch: expected {:?}, got {:?}",
                provider_type, assignment.provider_type
            )));
        }

        // Validate permissions based on provider type
        self.validate_permissions(assignment, *provider_type)?;

        Ok(())
    }

    /// Validate permissions for a specific provider type
    fn validate_permissions(
        &self,
        assignment: &CapabilityAssignment,
        provider_type: ProviderType,
    ) -> Result<()> {
        let valid_permissions: Vec<&str> = match provider_type {
            ProviderType::Kv => vec!["kv:read", "kv:write", "kv:delete"],
            ProviderType::Http => vec!["http:request"],
            ProviderType::Messaging => vec!["msg:publish", "msg:subscribe"],
        };

        for permission in &assignment.permissions {
            if !valid_permissions.contains(&permission.as_str()) {
                return Err(CoreError::InvalidCapabilityAssignment(format!(
                    "Invalid permission '{}' for provider type {:?}",
                    permission, provider_type
                )));
            }
        }

        Ok(())
    }

    /// Store a capability assignment for an instance
    pub fn assign_capability(&mut self, assignment: CapabilityAssignment) -> Result<()> {
        // Validate the assignment
        self.validate_assignment(&assignment)?;

        // Store the assignment
        self.assignments
            .entry(assignment.instance_id.clone())
            .or_insert_with(Vec::new)
            .push(assignment);

        Ok(())
    }

    /// Revoke a capability from an instance
    pub fn revoke_capability(&mut self, instance_id: &str, capability_id: &str) -> Result<bool> {
        if let Some(assignments) = self.assignments.get_mut(instance_id) {
            let original_len = assignments.len();
            assignments.retain(|a| a.capability_id != capability_id);
            let was_removed = assignments.len() < original_len;

            // Clean up empty entry if needed
            let should_remove = assignments.is_empty();
            drop(assignments); // Drop the mutable borrow before calling remove

            if should_remove {
                self.assignments.remove(instance_id);
            }

            Ok(was_removed)
        } else {
            Ok(false)
        }
    }

    /// Get all capability assignments for an instance
    pub fn get_capabilities(&self, instance_id: &str) -> Option<&Vec<CapabilityAssignment>> {
        self.assignments.get(instance_id)
    }

    /// Check if an instance has a specific capability
    pub fn has_capability(&self, instance_id: &str, capability_id: &str) -> bool {
        self.assignments
            .get(instance_id)
            .map(|assignments| assignments.iter().any(|a| a.capability_id == capability_id))
            .unwrap_or(false)
    }

    /// Check if an instance has a specific permission for a capability
    pub fn has_permission(&self, instance_id: &str, capability_id: &str, permission: &str) -> bool {
        self.assignments
            .get(instance_id)
            .map(|assignments| {
                assignments
                    .iter()
                    .filter(|a| a.capability_id == capability_id)
                    .any(|a| a.has_permission(permission))
            })
            .unwrap_or(false)
    }

    /// Get all instance IDs with capabilities
    pub fn get_instances(&self) -> Vec<&String> {
        self.assignments.keys().collect()
    }

    /// Clear all assignments for an instance (e.g., when instance is stopped)
    pub fn clear_instance(&mut self, instance_id: &str) {
        self.assignments.remove(instance_id);
    }

    /// Get total number of capability assignments
    pub fn assignment_count(&self) -> usize {
        self.assignments.values().map(|v| v.len()).sum()
    }
}

/// Runtime permission enforcer for capability invocations
pub struct PermissionEnforcer;

impl PermissionEnforcer {
    /// Required permission for a KV operation
    pub fn kv_permission(operation: &str) -> Option<&'static str> {
        match operation {
            "get" | "list" | "exists" => Some("kv:read"),
            "set" => Some("kv:write"),
            "delete" => Some("kv:delete"),
            _ => None,
        }
    }

    /// Required permission for an HTTP operation
    pub fn http_permission(operation: &str) -> Option<&'static str> {
        match operation {
            "request" => Some("http:request"),
            _ => None,
        }
    }

    /// Required permission for a messaging operation
    pub fn messaging_permission(operation: &str) -> Option<&'static str> {
        match operation {
            "publish" => Some("msg:publish"),
            "subscribe" => Some("msg:subscribe"),
            _ => None,
        }
    }

    /// Get required permission for any operation
    pub fn required_permission(
        provider_type: ProviderType,
        operation: &str,
    ) -> Option<&'static str> {
        match provider_type {
            ProviderType::Kv => Self::kv_permission(operation),
            ProviderType::Http => Self::http_permission(operation),
            ProviderType::Messaging => Self::messaging_permission(operation),
        }
    }

    /// Enforce permission check for a capability invocation
    pub fn enforce(
        registry: &CapabilityRegistry,
        instance_id: &str,
        capability_id: &str,
        provider_type: ProviderType,
        operation: &str,
    ) -> Result<()> {
        // Check if assignment exists
        if !registry.has_capability(instance_id, capability_id) {
            return Err(CoreError::InvalidCapabilityAssignment(format!(
                "Instance '{}' does not have capability '{}' assigned",
                instance_id, capability_id
            )));
        }

        // Get required permission
        let required = Self::required_permission(provider_type, operation).ok_or_else(|| {
            CoreError::InvalidCapabilityAssignment(format!(
                "Unknown operation '{}' for provider type {:?}",
                operation, provider_type
            ))
        })?;

        // Check permission
        if !registry.has_permission(instance_id, capability_id, required) {
            return Err(CoreError::InvalidCapabilityAssignment(format!(
                "Permission denied: instance '{}' lacks '{}' permission for capability '{}'",
                instance_id, required, capability_id
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_capability_registry_basic() {
        let mut registry = CapabilityRegistry::new();

        // Register a provider
        registry.register_provider("kv-store", ProviderType::Kv);

        // Assign capability
        let assignment = create_test_assignment(
            "instance-1",
            "kv-store",
            ProviderType::Kv,
            vec!["kv:read", "kv:write"],
        );
        registry.assign_capability(assignment).unwrap();

        // Verify assignment exists
        assert!(registry.has_capability("instance-1", "kv-store"));
        assert!(!registry.has_capability("instance-1", "other-provider"));
        assert!(!registry.has_capability("other-instance", "kv-store"));

        // Verify permissions
        assert!(registry.has_permission("instance-1", "kv-store", "kv:read"));
        assert!(registry.has_permission("instance-1", "kv-store", "kv:write"));
        assert!(!registry.has_permission("instance-1", "kv-store", "kv:delete"));
    }

    #[test]
    fn test_capability_registry_unregistered_provider() {
        let mut registry = CapabilityRegistry::new();

        // Try to assign capability for unregistered provider
        let assignment = create_test_assignment(
            "instance-1",
            "unregistered",
            ProviderType::Kv,
            vec!["kv:read"],
        );
        let result = registry.assign_capability(assignment);
        assert!(result.is_err());
    }

    #[test]
    fn test_capability_registry_type_mismatch() {
        let mut registry = CapabilityRegistry::new();

        // Register as KV provider
        registry.register_provider("store", ProviderType::Kv);

        // Try to assign with wrong type
        let assignment = create_test_assignment(
            "instance-1",
            "store",
            ProviderType::Http, // Wrong type
            vec!["http:request"],
        );
        let result = registry.assign_capability(assignment);
        assert!(result.is_err());
    }

    #[test]
    fn test_capability_registry_invalid_permissions() {
        let mut registry = CapabilityRegistry::new();

        registry.register_provider("kv-store", ProviderType::Kv);

        // Try to assign with invalid permission for KV
        let assignment = create_test_assignment(
            "instance-1",
            "kv-store",
            ProviderType::Kv,
            vec!["invalid:permission"],
        );
        let result = registry.assign_capability(assignment);
        assert!(result.is_err());
    }

    #[test]
    fn test_capability_registry_revoke() {
        let mut registry = CapabilityRegistry::new();

        registry.register_provider("kv-store", ProviderType::Kv);
        registry.register_provider("http-client", ProviderType::Http);

        let assignment1 =
            create_test_assignment("instance-1", "kv-store", ProviderType::Kv, vec!["kv:read"]);
        let assignment2 = create_test_assignment(
            "instance-1",
            "http-client",
            ProviderType::Http,
            vec!["http:request"],
        );

        registry.assign_capability(assignment1).unwrap();
        registry.assign_capability(assignment2).unwrap();

        // Revoke one capability
        let revoked = registry
            .revoke_capability("instance-1", "kv-store")
            .unwrap();
        assert!(revoked);
        assert!(!registry.has_capability("instance-1", "kv-store"));
        assert!(registry.has_capability("instance-1", "http-client"));

        // Revoke same capability again should return false
        let revoked = registry
            .revoke_capability("instance-1", "kv-store")
            .unwrap();
        assert!(!revoked);

        // Revoke from non-existent instance
        let revoked = registry
            .revoke_capability("nonexistent", "kv-store")
            .unwrap();
        assert!(!revoked);
    }

    #[test]
    fn test_capability_registry_clear_instance() {
        let mut registry = CapabilityRegistry::new();

        registry.register_provider("kv-store", ProviderType::Kv);
        registry.register_provider("http-client", ProviderType::Http);

        let assignment1 =
            create_test_assignment("instance-1", "kv-store", ProviderType::Kv, vec!["kv:read"]);
        let assignment2 = create_test_assignment(
            "instance-1",
            "http-client",
            ProviderType::Http,
            vec!["http:request"],
        );

        registry.assign_capability(assignment1).unwrap();
        registry.assign_capability(assignment2).unwrap();

        assert_eq!(registry.assignment_count(), 2);

        registry.clear_instance("instance-1");

        assert_eq!(registry.assignment_count(), 0);
        assert!(!registry.has_capability("instance-1", "kv-store"));
        assert!(!registry.has_capability("instance-1", "http-client"));
    }

    #[test]
    fn test_permission_enforcer_kv() {
        let mut registry = CapabilityRegistry::new();

        registry.register_provider("kv-store", ProviderType::Kv);

        let assignment = create_test_assignment(
            "instance-1",
            "kv-store",
            ProviderType::Kv,
            vec!["kv:read", "kv:delete"], // Note: no kv:write
        );
        registry.assign_capability(assignment).unwrap();

        // Should allow get (kv:read)
        let result = PermissionEnforcer::enforce(
            &registry,
            "instance-1",
            "kv-store",
            ProviderType::Kv,
            "get",
        );
        assert!(result.is_ok());

        // Should allow list (kv:read)
        let result = PermissionEnforcer::enforce(
            &registry,
            "instance-1",
            "kv-store",
            ProviderType::Kv,
            "list",
        );
        assert!(result.is_ok());

        // Should deny set (missing kv:write)
        let result = PermissionEnforcer::enforce(
            &registry,
            "instance-1",
            "kv-store",
            ProviderType::Kv,
            "set",
        );
        assert!(result.is_err());

        // Should allow delete (kv:delete)
        let result = PermissionEnforcer::enforce(
            &registry,
            "instance-1",
            "kv-store",
            ProviderType::Kv,
            "delete",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_permission_enforcer_no_assignment() {
        let registry = CapabilityRegistry::new();

        // Try to enforce on instance with no assignments
        let result = PermissionEnforcer::enforce(
            &registry,
            "instance-1",
            "kv-store",
            ProviderType::Kv,
            "get",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_permission_enforcer_unknown_operation() {
        let mut registry = CapabilityRegistry::new();

        registry.register_provider("kv-store", ProviderType::Kv);

        let assignment =
            create_test_assignment("instance-1", "kv-store", ProviderType::Kv, vec!["kv:read"]);
        registry.assign_capability(assignment).unwrap();

        // Unknown operation should fail
        let result = PermissionEnforcer::enforce(
            &registry,
            "instance-1",
            "kv-store",
            ProviderType::Kv,
            "unknown_op",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_permission_enforcer_http() {
        let mut registry = CapabilityRegistry::new();

        registry.register_provider("http-client", ProviderType::Http);

        let assignment = create_test_assignment(
            "instance-1",
            "http-client",
            ProviderType::Http,
            vec!["http:request"],
        );
        registry.assign_capability(assignment).unwrap();

        // Should allow request
        let result = PermissionEnforcer::enforce(
            &registry,
            "instance-1",
            "http-client",
            ProviderType::Http,
            "request",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_permission_enforcer_messaging() {
        let mut registry = CapabilityRegistry::new();

        registry.register_provider("messaging", ProviderType::Messaging);

        let assignment = create_test_assignment(
            "instance-1",
            "messaging",
            ProviderType::Messaging,
            vec!["msg:publish"], // Note: no subscribe
        );
        registry.assign_capability(assignment).unwrap();

        // Should allow publish
        let result = PermissionEnforcer::enforce(
            &registry,
            "instance-1",
            "messaging",
            ProviderType::Messaging,
            "publish",
        );
        assert!(result.is_ok());

        // Should deny subscribe
        let result = PermissionEnforcer::enforce(
            &registry,
            "instance-1",
            "messaging",
            ProviderType::Messaging,
            "subscribe",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_multiple_instances() {
        let mut registry = CapabilityRegistry::new();

        registry.register_provider("kv-store", ProviderType::Kv);
        registry.register_provider("http-client", ProviderType::Http);

        // Assign different capabilities to different instances
        let assignment1 =
            create_test_assignment("instance-1", "kv-store", ProviderType::Kv, vec!["kv:read"]);
        let assignment2 = create_test_assignment(
            "instance-2",
            "http-client",
            ProviderType::Http,
            vec!["http:request"],
        );

        registry.assign_capability(assignment1).unwrap();
        registry.assign_capability(assignment2).unwrap();

        // Verify isolation
        assert!(registry.has_capability("instance-1", "kv-store"));
        assert!(!registry.has_capability("instance-1", "http-client"));
        assert!(!registry.has_capability("instance-2", "kv-store"));
        assert!(registry.has_capability("instance-2", "http-client"));

        // Get all instances
        let instances = registry.get_instances();
        assert_eq!(instances.len(), 2);
    }
}
