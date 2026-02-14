use crate::features::messaging_provider::service::MessagingProviderService;
use serde_json::Value;
use wasmatrix_core::{CapabilityAssignment, CoreError, Result};

pub struct MessagingProviderController {
    service: MessagingProviderService,
}

impl MessagingProviderController {
    pub fn new(service: MessagingProviderService) -> Self {
        Self { service }
    }

    pub fn handle_invoke(
        &self,
        instance_id: &str,
        operation: &str,
        params: Value,
    ) -> Result<Value> {
        let topic = params.get("topic").and_then(Value::as_str).ok_or_else(|| {
            CoreError::InvalidCapabilityAssignment("Missing 'topic' parameter".to_string())
        })?;

        let assignment = CapabilityAssignment::new(
            instance_id.to_string(),
            "messaging-provider".to_string(),
            wasmatrix_core::ProviderType::Messaging,
            extract_permissions(&params),
        );

        match operation {
            "publish" => {
                let payload = params
                    .get("payload")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        CoreError::InvalidCapabilityAssignment(
                            "Missing 'payload' parameter".to_string(),
                        )
                    })?;
                self.service.publish(&assignment, topic, payload)
            }
            "subscribe" => self.service.subscribe(&assignment, topic),
            "unsubscribe" => self.service.unsubscribe(&assignment, topic),
            _ => Err(CoreError::InvalidCapabilityAssignment(format!(
                "Unknown messaging operation: {operation}"
            ))),
        }
    }
}

fn extract_permissions(params: &Value) -> Vec<String> {
    params
        .get("permissions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::messaging_provider::repo::InMemoryMessagingProviderRepository;
    use crate::features::messaging_provider::service::MessagingProviderService;
    use std::sync::Arc;

    #[test]
    fn test_handle_invoke_rejects_unknown_operation() {
        let controller = MessagingProviderController::new(MessagingProviderService::new(Arc::new(
            InMemoryMessagingProviderRepository::new(),
        )));
        let params = serde_json::json!({
            "topic": "orders",
            "permissions": ["msg:publish:orders"]
        });
        let result = controller.handle_invoke("i-1", "unknown", params);
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_invoke_publish_requires_payload() {
        let controller = MessagingProviderController::new(MessagingProviderService::new(Arc::new(
            InMemoryMessagingProviderRepository::new(),
        )));
        let params = serde_json::json!({
            "topic": "orders",
            "permissions": ["msg:publish:orders"]
        });
        let result = controller.handle_invoke("i-1", "publish", params);
        assert!(result.is_err());
    }
}
