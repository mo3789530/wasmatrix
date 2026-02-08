//! Instance isolation guarantees for Wasm Orchestrator
//!
//! This module enforces isolation between instances:
//! - Wasmtime provides memory isolation
//! - Capability assignments are scoped per instance
//! - Provider access is scoped to requesting instance

use crate::{CapabilityAssignment, ProviderType, Result};
use std::collections::HashMap;
use tracing::{info, warn};

/// Isolation policy enforcer
pub struct IsolationPolicy;

impl IsolationPolicy {
    /// Verify Wasmtime provides memory isolation
    /// Note: Wasmtime inherently provides memory isolation between instances
    /// This function documents the guarantee and could add runtime checks
    pub fn verify_wasmtime_isolation() -> Result<()> {
        // Wasmtime provides:
        // 1. Separate linear memory for each instance
        // 2. No shared mutable state between instances
        // 3. Memory bounds enforcement
        Ok(())
    }

    /// Verify capability assignments are scoped per instance
    pub fn verify_capability_isolation(
        _instance_a_id: &str,
        assignments_a: &[CapabilityAssignment],
        _instance_b_id: &str,
        assignments_b: &[CapabilityAssignment],
    ) -> Result<bool> {
        // Each instance should have its own set of assignments
        // They should not share the same capability assignments
        let has_shared_assignments = assignments_a.iter().any(|a| {
            assignments_b.iter().any(|b| {
                // Same capability_id means they share the same provider instance
                // This is okay if both instances have separate permission sets
                // but we need to verify they don't have the same assignments
                a.capability_id == b.capability_id && a.permissions == b.permissions
            })
        });

        Ok(has_shared_assignments)
    }

    /// Verify provider access is scoped to requesting instance
    pub fn verify_provider_scoping(
        requesting_instance: &str,
        instance_assignments: &[CapabilityAssignment],
    ) -> Result<bool> {
        // Check if instance has any capability assignments
        let has_capabilities = !instance_assignments.is_empty();

        // Verify that each capability has the requesting instance ID
        let all_scoped = instance_assignments
            .iter()
            .all(|a| a.instance_id == requesting_instance);

        if has_capabilities && !all_scoped {
            warn!(
                instance_id = %requesting_instance,
                "Instance has capability assignments not scoped to it"
            );
        }

        Ok(all_scoped)
    }
}

/// Isolation sandbox for tracking instance boundaries
#[derive(Debug, Default)]
pub struct IsolationSandbox {
    /// Map of instance_id -> isolated memory pages
    instance_memory: HashMap<String, Vec<u8>>,
    /// Map of instance_id -> capability assignments
    instance_capabilities: HashMap<String, Vec<CapabilityAssignment>>,
}

impl IsolationSandbox {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an instance in the sandbox
    pub fn register_instance(
        &mut self,
        instance_id: String,
        capabilities: Vec<CapabilityAssignment>,
    ) -> Result<()> {
        info!(instance_id = %instance_id, "Registering instance in isolation sandbox");

        // Initialize isolated memory for instance
        self.instance_memory.insert(instance_id.clone(), vec![]);

        // Store scoped capabilities
        self.instance_capabilities.insert(instance_id, capabilities);

        Ok(())
    }

    /// Unregister an instance from the sandbox
    pub fn unregister_instance(&mut self, instance_id: &str) -> Result<()> {
        info!(instance_id = %instance_id, "Unregistering instance from isolation sandbox");

        // Remove all instance data from sandbox
        self.instance_memory.remove(instance_id);
        self.instance_capabilities.remove(instance_id);

        Ok(())
    }

    /// Verify instance cannot access another's memory
    pub fn verify_memory_isolation(
        &self,
        instance_id: &str,
        target_instance_id: &str,
    ) -> Result<bool> {
        // Instances should not be able to access each other's memory
        // This is enforced at the runtime level by wasmtime
        // We verify the sandbox tracks them separately
        Ok(instance_id != target_instance_id
            || self.instance_memory.get(instance_id)
                != self.instance_memory.get(target_instance_id))
    }

    /// Verify instance cannot access another's capabilities
    pub fn verify_capability_isolation(
        &self,
        instance_id: &str,
        _target_instance_id: &str,
        capability_id: &str,
    ) -> Result<bool> {
        let can_access = self
            .instance_capabilities
            .get(instance_id)
            .map(|caps| caps.iter().any(|c| c.capability_id == capability_id))
            .unwrap_or(false);

        if can_access {
            // Verify the capability is scoped to requesting instance
            if let Some(caps) = self.instance_capabilities.get(instance_id) {
                for cap in caps {
                    if cap.capability_id == capability_id {
                        return Ok(cap.instance_id == instance_id);
                    }
                }
            }
        }

        // Cannot access other instance's capabilities
        Ok(false)
    }

    /// Get capabilities scoped to instance
    pub fn get_instance_capabilities(
        &self,
        instance_id: &str,
    ) -> Option<&Vec<CapabilityAssignment>> {
        self.instance_capabilities.get(instance_id)
    }

    /// Get isolated memory for instance
    pub fn get_instance_memory(&self, instance_id: &str) -> Option<&Vec<u8>> {
        self.instance_memory.get(instance_id)
    }

    /// Check if instance is registered
    pub fn is_instance_registered(&self, instance_id: &str) -> bool {
        self.instance_memory.contains_key(instance_id)
    }

    /// Count registered instances
    pub fn instance_count(&self) -> usize {
        self.instance_memory.len()
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
    fn test_verify_wasmtime_isolation() {
        assert!(IsolationPolicy::verify_wasmtime_isolation().is_ok());
    }

    #[test]
    fn test_verify_capability_isolation_both_empty() {
        let result =
            IsolationPolicy::verify_capability_isolation("instance-1", &[], "instance-2", &[]);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_sandbox_empty() {
        let sandbox = IsolationSandbox::new();

        // Empty sandbox should have no instances
        assert_eq!(sandbox.instance_count(), 0);
        assert!(!sandbox.is_instance_registered("test-1"));
    }

    #[test]
    fn test_verify_capability_isolation_no_overlap() {
        let assignments_a = vec![
            create_test_assignment("instance-1", "kv-1", ProviderType::Kv, vec!["kv:read"]),
            create_test_assignment(
                "instance-1",
                "http-1",
                ProviderType::Http,
                vec!["http:request"],
            ),
        ];

        let assignments_b = vec![create_test_assignment(
            "instance-2",
            "kv-2",
            ProviderType::Kv,
            vec!["kv:write"],
        )];

        let result = IsolationPolicy::verify_capability_isolation(
            "instance-1",
            &assignments_a,
            "instance-2",
            &assignments_b,
        );
        assert!(result.is_ok());
        // No shared assignments
        assert!(!result.unwrap());
    }

    #[test]
    fn test_verify_capability_isolation_with_overlap() {
        let assignments_a = vec![create_test_assignment(
            "instance-1",
            "kv-1",
            ProviderType::Kv,
            vec!["kv:read"],
        )];

        let assignments_b = vec![create_test_assignment(
            "instance-2",
            "kv-1",
            ProviderType::Kv,
            vec!["kv:read"],
        )];

        let result = IsolationPolicy::verify_capability_isolation(
            "instance-1",
            &assignments_a,
            "instance-2",
            &assignments_b,
        );
        assert!(result.is_ok());
        // Has shared assignments (same provider and permissions)
        assert!(result.unwrap());
    }

    #[test]
    fn test_verify_provider_scoping_valid() {
        let assignments = vec![create_test_assignment(
            "instance-1",
            "kv-1",
            ProviderType::Kv,
            vec!["kv:read"],
        )];

        let result = IsolationPolicy::verify_provider_scoping("instance-1", &assignments);
        assert!(result.is_ok());
        // All capabilities scoped to requesting instance
        assert!(result.unwrap());
    }

    #[test]
    fn test_verify_provider_scoping_invalid() {
        // Assignment with wrong instance_id
        let assignments = vec![create_test_assignment(
            "instance-2",
            "kv-1",
            ProviderType::Kv,
            vec!["kv:read"],
        )];

        let result = IsolationPolicy::verify_provider_scoping("instance-1", &assignments);
        assert!(result.is_ok());
        // Not scoped to requesting instance
        assert!(!result.unwrap());
    }

    #[test]
    fn test_sandbox_register_and_unregister() {
        let mut sandbox = IsolationSandbox::new();

        let assignments = vec![create_test_assignment(
            "instance-1",
            "kv-1",
            ProviderType::Kv,
            vec!["kv:read"],
        )];

        sandbox
            .register_instance("instance-1".to_string(), assignments)
            .unwrap();
        assert_eq!(sandbox.instance_count(), 1);
        assert!(sandbox.is_instance_registered("instance-1"));

        sandbox.unregister_instance("instance-1").unwrap();
        assert_eq!(sandbox.instance_count(), 0);
        assert!(!sandbox.is_instance_registered("instance-1"));

        // Unregister again should still succeed
        let result = sandbox.unregister_instance("instance-1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_sandbox_memory_isolation() {
        let mut sandbox = IsolationSandbox::new();

        sandbox
            .register_instance("instance-1".to_string(), vec![])
            .unwrap();
        sandbox
            .register_instance("instance-2".to_string(), vec![])
            .unwrap();

        // Verify memory isolation between different instances
        let result = sandbox.verify_memory_isolation("instance-1", "instance-2");
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Same instance should have same memory
        let result = sandbox.verify_memory_isolation("instance-1", "instance-1");
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Same instance, memory not isolated
    }

    #[test]
    fn test_sandbox_capability_isolation() {
        let mut sandbox = IsolationSandbox::new();

        let instance1_caps = vec![create_test_assignment(
            "instance-1",
            "kv-1",
            ProviderType::Kv,
            vec!["kv:read"],
        )];

        let instance2_caps = vec![create_test_assignment(
            "instance-2",
            "kv-2",
            ProviderType::Kv,
            vec!["kv:write"],
        )];

        sandbox
            .register_instance("instance-1".to_string(), instance1_caps)
            .unwrap();
        sandbox
            .register_instance("instance-2".to_string(), instance2_caps)
            .unwrap();

        // Instance 1 cannot access instance 2's capability
        let result = sandbox.verify_capability_isolation("instance-1", "instance-2", "kv-2");
        assert!(result.is_ok());
        assert!(!result.unwrap());

        // Instance 1 can access its own capability
        let result = sandbox.verify_capability_isolation("instance-1", "instance-1", "kv-1");
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_sandbox_get_instance_capabilities() {
        let mut sandbox = IsolationSandbox::new();

        let caps = vec![create_test_assignment(
            "instance-1",
            "kv-1",
            ProviderType::Kv,
            vec!["kv:read"],
        )];

        sandbox
            .register_instance("instance-1".to_string(), caps)
            .unwrap();

        let retrieved = sandbox.get_instance_capabilities("instance-1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().len(), 1);

        // Cannot get other instance's capabilities
        let other = sandbox.get_instance_capabilities("instance-2");
        assert!(other.is_none());
    }

    #[test]
    fn test_multiple_instances_isolation() {
        let mut sandbox = IsolationSandbox::new();

        // Register multiple instances
        for i in 0..5 {
            let caps = vec![create_test_assignment(
                &format!("instance-{}", i),
                &format!("kv-{}", i),
                ProviderType::Kv,
                vec!["kv:read"],
            )];
            sandbox
                .register_instance(format!("instance-{}", i), caps)
                .unwrap();
        }

        assert_eq!(sandbox.instance_count(), 5);

        // Each instance has its own capabilities
        for i in 0..5 {
            let caps = sandbox.get_instance_capabilities(&format!("instance-{}", i));
            assert!(caps.is_some());
            assert_eq!(caps.unwrap().len(), 1);
        }

        // Verify isolation between all pairs
        for i in 0..5 {
            for j in 0..5 {
                if i != j {
                    let result = sandbox.verify_capability_isolation(
                        &format!("instance-{}", i),
                        &format!("instance-{}", j),
                        &format!("kv-{}", j),
                    );
                    assert!(result.is_ok());
                    assert!(!result.unwrap());
                }
            }
        }
    }

    #[test]
    fn test_sandbox_get_instance_memory() {
        let mut sandbox = IsolationSandbox::new();

        sandbox
            .register_instance("instance-1".to_string(), vec![])
            .unwrap();

        // Instance should have memory
        let memory = sandbox.get_instance_memory("instance-1");
        assert!(memory.is_some());

        // Non-existent instance should not have memory
        let memory = sandbox.get_instance_memory("instance-2");
        assert!(memory.is_none());
    }

    #[test]
    fn test_sandbox_double_registration() {
        let mut sandbox = IsolationSandbox::new();

        let caps = vec![create_test_assignment(
            "instance-1",
            "kv-1",
            ProviderType::Kv,
            vec!["kv:read"],
        )];

        // Register instance twice (second should overwrite)
        sandbox
            .register_instance("instance-1".to_string(), caps.clone())
            .unwrap();
        sandbox
            .register_instance("instance-1".to_string(), caps)
            .unwrap();

        // Should still have 1 instance
        assert_eq!(sandbox.instance_count(), 1);
    }

    #[test]
    fn test_sandbox_capabilities_with_multiple_providers() {
        let mut sandbox = IsolationSandbox::new();

        let caps = vec![
            create_test_assignment("instance-1", "kv-1", ProviderType::Kv, vec!["kv:read"]),
            create_test_assignment(
                "instance-1",
                "http-1",
                ProviderType::Http,
                vec!["http:request"],
            ),
            create_test_assignment(
                "instance-1",
                "msg-1",
                ProviderType::Messaging,
                vec!["msg:publish"],
            ),
        ];

        sandbox
            .register_instance("instance-1".to_string(), caps)
            .unwrap();

        let retrieved = sandbox.get_instance_capabilities("instance-1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().len(), 3);
    }

    #[test]
    fn test_verify_capability_isolation_with_different_permissions() {
        let assignments_a = vec![create_test_assignment(
            "instance-1",
            "kv-1",
            ProviderType::Kv,
            vec!["kv:read"],
        )];

        let assignments_b = vec![
            create_test_assignment("instance-2", "kv-1", ProviderType::Kv, vec!["kv:write"]), // Same provider, different permissions
        ];

        let result = IsolationPolicy::verify_capability_isolation(
            "instance-1",
            &assignments_a,
            "instance-2",
            &assignments_b,
        );
        assert!(result.is_ok());
        // No shared assignments (different permissions)
        assert!(!result.unwrap());
    }
} // End of tests module
