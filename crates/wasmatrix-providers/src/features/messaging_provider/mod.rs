pub mod controller;
pub mod repo;
pub mod service;

use crate::{CapabilityProvider, ProviderMetadata};
use controller::MessagingProviderController;
use repo::InMemoryMessagingProviderRepository;
use service::MessagingProviderService;
use std::sync::Arc;
use wasmatrix_core::{ProviderType, Result};

pub struct MessagingCapabilityProvider {
    controller: MessagingProviderController,
    metadata: ProviderMetadata,
}

impl MessagingCapabilityProvider {
    pub fn new(provider_id: String) -> Self {
        let repo = Arc::new(InMemoryMessagingProviderRepository::new());
        let service = MessagingProviderService::new(repo);
        let controller = MessagingProviderController::new(service);
        Self {
            controller,
            metadata: ProviderMetadata {
                provider_id,
                provider_type: ProviderType::Messaging,
                version: "0.1.0".to_string(),
            },
        }
    }
}

impl CapabilityProvider for MessagingCapabilityProvider {
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
    fn test_messaging_provider_metadata() {
        let provider = MessagingCapabilityProvider::new("messaging-provider".to_string());
        let metadata = provider.get_metadata();

        assert_eq!(metadata.provider_id, "messaging-provider");
        assert_eq!(metadata.provider_type, ProviderType::Messaging);
    }

    #[test]
    fn test_messaging_provider_invoke_publish_success() {
        let provider = MessagingCapabilityProvider::new("messaging-provider".to_string());
        let params = serde_json::json!({
            "topic": "orders",
            "payload": "created",
            "permissions": ["msg:publish:orders"]
        });

        let result = provider.invoke("inst-1", "publish", params).unwrap();
        assert_eq!(result["published"].as_bool(), Some(true));
    }

    #[test]
    fn test_messaging_provider_subscribe_permission_denied() {
        let provider = MessagingCapabilityProvider::new("messaging-provider".to_string());
        let params = serde_json::json!({
            "topic": "orders",
            "permissions": ["msg:publish:orders"]
        });

        let result = provider.invoke("inst-1", "subscribe", params);
        assert!(matches!(
            result,
            Err(CoreError::InvalidCapabilityAssignment(_))
        ));
    }

    #[test]
    fn test_messaging_provider_subscribe_and_unsubscribe_success() {
        let provider = MessagingCapabilityProvider::new("messaging-provider".to_string());
        let subscribe_params = serde_json::json!({
            "topic": "orders",
            "permissions": ["msg:subscribe:orders"]
        });
        let unsub_params = serde_json::json!({
            "topic": "orders",
            "permissions": ["msg:subscribe:orders"]
        });

        let subscribed = provider
            .invoke("inst-1", "subscribe", subscribe_params)
            .unwrap();
        assert_eq!(subscribed["subscribed"].as_bool(), Some(true));

        let unsubscribed = provider
            .invoke("inst-1", "unsubscribe", unsub_params)
            .unwrap();
        assert_eq!(unsubscribed["unsubscribed"].as_bool(), Some(true));
    }
}
