pub mod controller;
pub mod repo;
pub mod service;

use crate::{CapabilityProvider, ProviderMetadata};
use controller::HttpProviderController;
use repo::ReqwestHttpProviderRepository;
use service::HttpProviderService;
use std::sync::Arc;
use wasmatrix_core::{ProviderType, Result};

/// HTTP capability provider for outbound requests with permission checks.
pub struct HttpCapabilityProvider {
    controller: HttpProviderController,
    metadata: ProviderMetadata,
}

impl HttpCapabilityProvider {
    pub fn new(provider_id: String) -> Result<Self> {
        let repo = Arc::new(ReqwestHttpProviderRepository::new()?);
        let service = HttpProviderService::new(repo);
        let controller = HttpProviderController::new(service);

        Ok(Self {
            controller,
            metadata: ProviderMetadata {
                provider_id,
                provider_type: ProviderType::Http,
                version: "0.1.0".to_string(),
            },
        })
    }
}

impl CapabilityProvider for HttpCapabilityProvider {
    fn initialize(&mut self, _config: serde_json::Value) -> Result<()> {
        Ok(())
    }

    fn invoke(
        &self,
        instance_id: &str,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.controller
            .handle_invoke(instance_id, operation, params)
    }

    fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    fn get_metadata(&self) -> ProviderMetadata {
        self.metadata.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmatrix_core::CoreError;

    #[test]
    fn test_http_provider_metadata() {
        let provider = HttpCapabilityProvider::new("http-provider".to_string()).unwrap();
        let metadata = provider.get_metadata();

        assert_eq!(metadata.provider_id, "http-provider");
        assert_eq!(metadata.provider_type, ProviderType::Http);
    }

    #[test]
    fn test_http_provider_invoke_requires_permissions() {
        let provider = HttpCapabilityProvider::new("http-provider".to_string()).unwrap();
        let params = serde_json::json!({
            "method": "GET",
            "url": "https://example.com"
        });

        let result = provider.invoke("inst-1", "request", params);
        assert!(matches!(
            result,
            Err(CoreError::InvalidCapabilityAssignment(_))
        ));
    }
}
