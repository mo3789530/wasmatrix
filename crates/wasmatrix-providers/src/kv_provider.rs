use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use wasmatrix_core::{CapabilityAssignment, CoreError, ProviderType, Result};

use crate::{CapabilityProvider, ProviderMetadata};

/// Thread-safe KV Provider with in-memory storage and permission validation
pub struct KvProvider {
    storage: Arc<RwLock<HashMap<String, String>>>,
    metadata: ProviderMetadata,
}

impl KvProvider {
    pub fn new(provider_id: String) -> Self {
        Self {
            storage: Arc::new(RwLock::new(HashMap::new())),
            metadata: ProviderMetadata {
                provider_id,
                provider_type: ProviderType::Kv,
                version: "0.1.0".to_string(),
            },
        }
    }

    /// Validate that the capability assignment has the required permission
    fn validate_permission(
        &self,
        assignment: &CapabilityAssignment,
        operation: &str,
    ) -> Result<()> {
        let required_permission = match operation {
            "get" | "list" => "kv:read",
            "set" => "kv:write",
            "delete" => "kv:delete",
            _ => {
                return Err(CoreError::InvalidCapabilityAssignment(format!(
                    "Unknown operation: {}",
                    operation
                )))
            }
        };

        if !assignment.has_permission(required_permission) {
            return Err(CoreError::InvalidCapabilityAssignment(format!(
                "Permission denied: missing '{}' permission",
                required_permission
            )));
        }

        Ok(())
    }

    /// Get a value by key (direct API)
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let storage = self.storage.read().map_err(|_| {
            CoreError::InvalidCapabilityAssignment("Storage lock poisoned".to_string())
        })?;
        Ok(storage.get(key).cloned())
    }

    /// Set a key-value pair (direct API)
    pub fn set(&self, key: String, value: String) -> Result<()> {
        let mut storage = self.storage.write().map_err(|_| {
            CoreError::InvalidCapabilityAssignment("Storage lock poisoned".to_string())
        })?;
        storage.insert(key, value);
        Ok(())
    }

    /// Delete a key (direct API)
    pub fn delete(&self, key: &str) -> Result<bool> {
        let mut storage = self.storage.write().map_err(|_| {
            CoreError::InvalidCapabilityAssignment("Storage lock poisoned".to_string())
        })?;
        Ok(storage.remove(key).is_some())
    }

    /// List keys with a prefix (direct API)
    pub fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let storage = self.storage.read().map_err(|_| {
            CoreError::InvalidCapabilityAssignment("Storage lock poisoned".to_string())
        })?;
        Ok(storage
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect())
    }

    /// Check if a key exists (direct API)
    pub fn exists(&self, key: &str) -> Result<bool> {
        let storage = self.storage.read().map_err(|_| {
            CoreError::InvalidCapabilityAssignment("Storage lock poisoned".to_string())
        })?;
        Ok(storage.contains_key(key))
    }

    /// Clear all data (direct API)
    pub fn clear(&self) -> Result<()> {
        let mut storage = self.storage.write().map_err(|_| {
            CoreError::InvalidCapabilityAssignment("Storage lock poisoned".to_string())
        })?;
        storage.clear();
        Ok(())
    }
}

impl CapabilityProvider for KvProvider {
    fn initialize(&mut self, _config: Value) -> Result<()> {
        // Clear any existing data on initialization
        self.clear()
    }

    fn invoke(&self, _instance_id: &str, operation: &str, params: Value) -> Result<Value> {
        // For now, we assume the caller has already validated permissions
        // In a real implementation, we'd look up the capability assignment here

        match operation {
            "get" => {
                let key = params["key"].as_str().ok_or_else(|| {
                    CoreError::InvalidCapabilityAssignment("Missing 'key' parameter".to_string())
                })?;
                let value = self.get(key)?;
                Ok(Value::String(value.unwrap_or_default()))
            }
            "set" => {
                let key = params["key"].as_str().ok_or_else(|| {
                    CoreError::InvalidCapabilityAssignment("Missing 'key' parameter".to_string())
                })?;
                let value = params["value"].as_str().ok_or_else(|| {
                    CoreError::InvalidCapabilityAssignment("Missing 'value' parameter".to_string())
                })?;
                self.set(key.to_string(), value.to_string())?;
                Ok(Value::Bool(true))
            }
            "delete" => {
                let key = params["key"].as_str().ok_or_else(|| {
                    CoreError::InvalidCapabilityAssignment("Missing 'key' parameter".to_string())
                })?;
                let existed = self.delete(key)?;
                Ok(Value::Bool(existed))
            }
            "list" => {
                let prefix = params["prefix"].as_str().unwrap_or("");
                let keys = self.list(prefix)?;
                let values: Vec<Value> = keys.into_iter().map(Value::String).collect();
                Ok(Value::Array(values))
            }
            "exists" => {
                let key = params["key"].as_str().ok_or_else(|| {
                    CoreError::InvalidCapabilityAssignment("Missing 'key' parameter".to_string())
                })?;
                let exists = self.exists(key)?;
                Ok(Value::Bool(exists))
            }
            _ => Err(CoreError::InvalidCapabilityAssignment(format!(
                "Unknown operation: {}",
                operation
            ))),
        }
    }

    fn shutdown(&mut self) -> Result<()> {
        self.clear()
    }

    fn get_metadata(&self) -> ProviderMetadata {
        self.metadata.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_provider() -> KvProvider {
        KvProvider::new("test-kv".to_string())
    }

    fn create_test_assignment(permissions: Vec<&str>) -> CapabilityAssignment {
        CapabilityAssignment::new(
            "test-instance".to_string(),
            "test-kv".to_string(),
            ProviderType::Kv,
            permissions.into_iter().map(|s| s.to_string()).collect(),
        )
    }

    #[test]
    fn test_kv_provider_basic_operations() {
        let provider = create_test_provider();

        // Test set and get
        provider
            .set("key1".to_string(), "value1".to_string())
            .unwrap();
        assert_eq!(provider.get("key1").unwrap(), Some("value1".to_string()));

        // Test update
        provider
            .set("key1".to_string(), "value2".to_string())
            .unwrap();
        assert_eq!(provider.get("key1").unwrap(), Some("value2".to_string()));

        // Test non-existent key
        assert_eq!(provider.get("nonexistent").unwrap(), None);
    }

    #[test]
    fn test_kv_provider_delete() {
        let provider = create_test_provider();

        provider
            .set("key1".to_string(), "value1".to_string())
            .unwrap();
        assert!(provider.exists("key1").unwrap());

        let deleted = provider.delete("key1").unwrap();
        assert!(deleted);
        assert!(!provider.exists("key1").unwrap());

        // Deleting non-existent key returns false
        let deleted = provider.delete("nonexistent").unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_kv_provider_list() {
        let provider = create_test_provider();

        provider
            .set("app:config:host".to_string(), "localhost".to_string())
            .unwrap();
        provider
            .set("app:config:port".to_string(), "8080".to_string())
            .unwrap();
        provider
            .set("app:data:users".to_string(), "100".to_string())
            .unwrap();
        provider
            .set("other:key".to_string(), "value".to_string())
            .unwrap();

        // List all keys with "app:" prefix
        let keys = provider.list("app:").unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"app:config:host".to_string()));
        assert!(keys.contains(&"app:config:port".to_string()));
        assert!(keys.contains(&"app:data:users".to_string()));

        // List keys with "app:config:" prefix
        let keys = provider.list("app:config:").unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"app:config:host".to_string()));
        assert!(keys.contains(&"app:config:port".to_string()));

        // Empty prefix lists all keys
        let keys = provider.list("").unwrap();
        assert_eq!(keys.len(), 4);
    }

    #[test]
    fn test_kv_provider_clear() {
        let provider = create_test_provider();

        provider
            .set("key1".to_string(), "value1".to_string())
            .unwrap();
        provider
            .set("key2".to_string(), "value2".to_string())
            .unwrap();
        assert_eq!(provider.list("").unwrap().len(), 2);

        provider.clear().unwrap();
        assert_eq!(provider.list("").unwrap().len(), 0);
    }

    #[test]
    fn test_permission_validation() {
        let provider = create_test_provider();

        // Test read permission
        let read_assignment = create_test_assignment(vec!["kv:read"]);
        assert!(provider
            .validate_permission(&read_assignment, "get")
            .is_ok());
        assert!(provider
            .validate_permission(&read_assignment, "list")
            .is_ok());
        assert!(provider
            .validate_permission(&read_assignment, "set")
            .is_err());
        assert!(provider
            .validate_permission(&read_assignment, "delete")
            .is_err());

        // Test write permission
        let write_assignment = create_test_assignment(vec!["kv:write"]);
        assert!(provider
            .validate_permission(&write_assignment, "set")
            .is_ok());
        assert!(provider
            .validate_permission(&write_assignment, "get")
            .is_err());
        assert!(provider
            .validate_permission(&write_assignment, "delete")
            .is_err());

        // Test delete permission
        let delete_assignment = create_test_assignment(vec!["kv:delete"]);
        assert!(provider
            .validate_permission(&delete_assignment, "delete")
            .is_ok());
        assert!(provider
            .validate_permission(&delete_assignment, "get")
            .is_err());
        assert!(provider
            .validate_permission(&delete_assignment, "set")
            .is_err());

        // Test combined permissions
        let combined_assignment = create_test_assignment(vec!["kv:read", "kv:write", "kv:delete"]);
        assert!(provider
            .validate_permission(&combined_assignment, "get")
            .is_ok());
        assert!(provider
            .validate_permission(&combined_assignment, "set")
            .is_ok());
        assert!(provider
            .validate_permission(&combined_assignment, "delete")
            .is_ok());

        // Test unknown operation
        assert!(provider
            .validate_permission(&read_assignment, "unknown")
            .is_err());
    }

    #[test]
    fn test_capability_provider_invoke_get() {
        let provider = create_test_provider();
        provider
            .set("testkey".to_string(), "testvalue".to_string())
            .unwrap();

        let params = serde_json::json!({"key": "testkey"});
        let result = provider.invoke("instance-1", "get", params).unwrap();

        assert_eq!(result, Value::String("testvalue".to_string()));
    }

    #[test]
    fn test_capability_provider_invoke_set() {
        let provider = create_test_provider();

        let params = serde_json::json!({"key": "newkey", "value": "newvalue"});
        let result = provider.invoke("instance-1", "set", params).unwrap();

        assert_eq!(result, Value::Bool(true));
        assert_eq!(
            provider.get("newkey").unwrap(),
            Some("newvalue".to_string())
        );
    }

    #[test]
    fn test_capability_provider_invoke_delete() {
        let provider = create_test_provider();
        provider
            .set("delete_me".to_string(), "value".to_string())
            .unwrap();

        let params = serde_json::json!({"key": "delete_me"});
        let result = provider.invoke("instance-1", "delete", params).unwrap();

        assert_eq!(result, Value::Bool(true));
        assert!(!provider.exists("delete_me").unwrap());

        // Delete non-existent key
        let params = serde_json::json!({"key": "nonexistent"});
        let result = provider.invoke("instance-1", "delete", params).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_capability_provider_invoke_list() {
        let provider = create_test_provider();
        provider
            .set("prefix:key1".to_string(), "value1".to_string())
            .unwrap();
        provider
            .set("prefix:key2".to_string(), "value2".to_string())
            .unwrap();
        provider
            .set("other:key".to_string(), "value3".to_string())
            .unwrap();

        let params = serde_json::json!({"prefix": "prefix:"});
        let result = provider.invoke("instance-1", "list", params).unwrap();

        if let Value::Array(keys) = result {
            assert_eq!(keys.len(), 2);
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_capability_provider_invoke_exists() {
        let provider = create_test_provider();
        provider
            .set("existing".to_string(), "value".to_string())
            .unwrap();

        let params = serde_json::json!({"key": "existing"});
        let result = provider.invoke("instance-1", "exists", params).unwrap();
        assert_eq!(result, Value::Bool(true));

        let params = serde_json::json!({"key": "nonexistent"});
        let result = provider.invoke("instance-1", "exists", params).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_capability_provider_missing_params() {
        let provider = create_test_provider();

        // Missing key parameter for get
        let result = provider.invoke("instance-1", "get", Value::Object(Default::default()));
        assert!(result.is_err());

        // Missing key parameter for set
        let params = serde_json::json!({"value": "test"});
        let result = provider.invoke("instance-1", "set", params);
        assert!(result.is_err());

        // Missing value parameter for set
        let params = serde_json::json!({"key": "test"});
        let result = provider.invoke("instance-1", "set", params);
        assert!(result.is_err());
    }

    #[test]
    fn test_capability_provider_unknown_operation() {
        let provider = create_test_provider();

        let result = provider.invoke("instance-1", "unknown_op", Value::Null);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_shutdown() {
        let mut provider = create_test_provider();
        provider
            .set("key1".to_string(), "value1".to_string())
            .unwrap();
        provider
            .set("key2".to_string(), "value2".to_string())
            .unwrap();

        provider.shutdown().unwrap();

        // All data should be cleared
        assert_eq!(provider.list("").unwrap().len(), 0);
    }

    #[test]
    fn test_provider_metadata() {
        let provider = create_test_provider();
        let metadata = provider.get_metadata();

        assert_eq!(metadata.provider_id, "test-kv");
        assert_eq!(metadata.provider_type, ProviderType::Kv);
        assert_eq!(metadata.version, "0.1.0");
    }
}
