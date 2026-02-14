pub mod features;
pub mod kv_provider;

use wasmatrix_core::Result;

pub use features::http_provider::HttpCapabilityProvider;
pub use features::messaging_provider::MessagingCapabilityProvider;
pub use features::provider_lifecycle::controller::ProviderLifecycleController;

pub trait CapabilityProvider {
    fn initialize(&mut self, config: serde_json::Value) -> Result<()>;
    fn invoke(
        &self,
        instance_id: &str,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value>;
    fn shutdown(&mut self) -> Result<()>;
    fn get_metadata(&self) -> ProviderMetadata;
}

#[derive(Debug, Clone)]
pub struct ProviderMetadata {
    pub provider_id: String,
    pub provider_type: wasmatrix_core::ProviderType,
    pub version: String,
}
