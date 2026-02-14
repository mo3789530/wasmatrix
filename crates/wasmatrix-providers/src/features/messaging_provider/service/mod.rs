use crate::features::messaging_provider::repo::MessagingProviderRepository;
use std::sync::Arc;
use wasmatrix_core::{CapabilityAssignment, CoreError, Result};

pub struct MessagingProviderService {
    repo: Arc<dyn MessagingProviderRepository>,
}

impl MessagingProviderService {
    pub fn new(repo: Arc<dyn MessagingProviderRepository>) -> Self {
        Self { repo }
    }

    pub fn publish(
        &self,
        assignment: &CapabilityAssignment,
        topic: &str,
        payload: &str,
    ) -> Result<serde_json::Value> {
        self.validate_publish_permission(assignment, topic)?;
        self.repo.publish(topic, payload)?;
        Ok(serde_json::json!({ "published": true }))
    }

    pub fn subscribe(
        &self,
        assignment: &CapabilityAssignment,
        topic: &str,
    ) -> Result<serde_json::Value> {
        self.validate_subscribe_permission(assignment, topic)?;
        self.repo.subscribe(&assignment.instance_id, topic)?;
        Ok(serde_json::json!({ "subscribed": true }))
    }

    pub fn unsubscribe(
        &self,
        assignment: &CapabilityAssignment,
        topic: &str,
    ) -> Result<serde_json::Value> {
        self.validate_subscribe_permission(assignment, topic)?;
        let removed = self.repo.unsubscribe(&assignment.instance_id, topic)?;
        Ok(serde_json::json!({ "unsubscribed": removed }))
    }

    fn validate_publish_permission(
        &self,
        assignment: &CapabilityAssignment,
        topic: &str,
    ) -> Result<()> {
        let exact = format!("msg:publish:{topic}");
        if assignment.has_permission("msg:publish") || assignment.has_permission(&exact) {
            return Ok(());
        }
        Err(CoreError::InvalidCapabilityAssignment(format!(
            "Permission denied: missing 'msg:publish' or '{exact}' permission"
        )))
    }

    fn validate_subscribe_permission(
        &self,
        assignment: &CapabilityAssignment,
        topic: &str,
    ) -> Result<()> {
        let exact = format!("msg:subscribe:{topic}");
        if assignment.has_permission("msg:subscribe") || assignment.has_permission(&exact) {
            return Ok(());
        }
        Err(CoreError::InvalidCapabilityAssignment(format!(
            "Permission denied: missing 'msg:subscribe' or '{exact}' permission"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::messaging_provider::repo::InMemoryMessagingProviderRepository;
    use wasmatrix_core::ProviderType;

    fn assignment(permissions: Vec<&str>) -> CapabilityAssignment {
        CapabilityAssignment::new(
            "i-1".to_string(),
            "messaging-provider".to_string(),
            ProviderType::Messaging,
            permissions.into_iter().map(|p| p.to_string()).collect(),
        )
    }

    #[test]
    fn test_publish_requires_topic_permission() {
        let service =
            MessagingProviderService::new(Arc::new(InMemoryMessagingProviderRepository::new()));
        let assignment = assignment(vec!["msg:subscribe:orders"]);

        let result = service.publish(&assignment, "orders", "hello");
        assert!(result.is_err());
    }

    #[test]
    fn test_subscribe_and_unsubscribe_with_scoped_permission() {
        let service =
            MessagingProviderService::new(Arc::new(InMemoryMessagingProviderRepository::new()));
        let assignment = assignment(vec!["msg:subscribe:orders"]);

        let subscribed = service.subscribe(&assignment, "orders").unwrap();
        assert_eq!(subscribed["subscribed"].as_bool(), Some(true));

        let unsubscribed = service.unsubscribe(&assignment, "orders").unwrap();
        assert_eq!(unsubscribed["unsubscribed"].as_bool(), Some(true));
    }

    #[test]
    fn test_publish_with_generic_permission() {
        let service =
            MessagingProviderService::new(Arc::new(InMemoryMessagingProviderRepository::new()));
        let assignment = assignment(vec!["msg:publish"]);
        let result = service.publish(&assignment, "orders", "created").unwrap();
        assert_eq!(result["published"].as_bool(), Some(true));
    }

    #[test]
    fn test_publish_rejects_wrong_topic_scope() {
        let service =
            MessagingProviderService::new(Arc::new(InMemoryMessagingProviderRepository::new()));
        let assignment = assignment(vec!["msg:publish:orders"]);

        let result = service.publish(&assignment, "payments", "created");
        assert!(result.is_err());
    }

    #[test]
    fn test_subscribe_with_generic_permission() {
        let service =
            MessagingProviderService::new(Arc::new(InMemoryMessagingProviderRepository::new()));
        let assignment = assignment(vec!["msg:subscribe"]);

        let result = service.subscribe(&assignment, "inventory").unwrap();
        assert_eq!(result["subscribed"].as_bool(), Some(true));
    }
}
