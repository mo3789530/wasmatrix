use std::collections::HashMap;
use wasmatrix_core::{CoreError, Result};
use wasmatrix_providers::CapabilityProvider;

pub struct CapabilityManager {
    providers: HashMap<String, Box<dyn CapabilityProvider + Send + Sync>>,
}

impl CapabilityManager {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register_provider(
        &mut self,
        provider_id: String,
        mut provider: Box<dyn CapabilityProvider + Send + Sync>,
    ) -> Result<()> {
        provider.initialize(serde_json::json!({}))?;
        self.providers.insert(provider_id, provider);
        Ok(())
    }

    pub fn invoke(
        &self,
        instance_id: &str,
        capability_id: &str,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let provider = self.providers.get(capability_id).ok_or_else(|| {
            CoreError::InvalidCapabilityAssignment(format!(
                "Provider '{}' is not registered",
                capability_id
            ))
        })?;
        provider.invoke(instance_id, operation, params)
    }
}

impl Default for CapabilityManager {
    fn default() -> Self {
        Self::new()
    }
}
