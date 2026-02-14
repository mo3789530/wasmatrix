use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use wasmatrix_core::{CoreError, Result};

#[derive(Debug, Clone)]
pub struct PublishedMessage {
    pub topic: String,
    pub payload: String,
}

pub trait MessagingProviderRepository: Send + Sync {
    fn publish(&self, topic: &str, payload: &str) -> Result<()>;
    fn subscribe(&self, instance_id: &str, topic: &str) -> Result<()>;
    fn unsubscribe(&self, instance_id: &str, topic: &str) -> Result<bool>;
}

/// In-memory pub/sub repository used by the messaging provider.
pub struct InMemoryMessagingProviderRepository {
    subscriptions: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    published_messages: Arc<RwLock<Vec<PublishedMessage>>>,
}

impl InMemoryMessagingProviderRepository {
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            published_messages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn published_count(&self) -> usize {
        self.published_messages.read().map(|m| m.len()).unwrap_or(0)
    }

    pub fn is_subscribed(&self, instance_id: &str, topic: &str) -> bool {
        self.subscriptions
            .read()
            .ok()
            .and_then(|map| map.get(instance_id).cloned())
            .map(|topics| topics.contains(topic))
            .unwrap_or(false)
    }
}

impl Default for InMemoryMessagingProviderRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagingProviderRepository for InMemoryMessagingProviderRepository {
    fn publish(&self, topic: &str, payload: &str) -> Result<()> {
        let mut messages = self.published_messages.write().map_err(|_| {
            CoreError::InvalidCapabilityAssignment("Messaging lock poisoned".to_string())
        })?;
        messages.push(PublishedMessage {
            topic: topic.to_string(),
            payload: payload.to_string(),
        });
        Ok(())
    }

    fn subscribe(&self, instance_id: &str, topic: &str) -> Result<()> {
        let mut subscriptions = self.subscriptions.write().map_err(|_| {
            CoreError::InvalidCapabilityAssignment("Messaging lock poisoned".to_string())
        })?;
        subscriptions
            .entry(instance_id.to_string())
            .or_insert_with(HashSet::new)
            .insert(topic.to_string());
        Ok(())
    }

    fn unsubscribe(&self, instance_id: &str, topic: &str) -> Result<bool> {
        let mut subscriptions = self.subscriptions.write().map_err(|_| {
            CoreError::InvalidCapabilityAssignment("Messaging lock poisoned".to_string())
        })?;
        let removed = subscriptions
            .get_mut(instance_id)
            .map(|topics| topics.remove(topic))
            .unwrap_or(false);

        if subscriptions
            .get(instance_id)
            .map(|topics| topics.is_empty())
            .unwrap_or(false)
        {
            subscriptions.remove(instance_id);
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_publish_and_subscribe_lifecycle() {
        let repo = InMemoryMessagingProviderRepository::new();
        repo.subscribe("inst-1", "orders").unwrap();
        assert!(repo.is_subscribed("inst-1", "orders"));

        repo.publish("orders", "created").unwrap();
        assert_eq!(repo.published_count(), 1);

        let removed = repo.unsubscribe("inst-1", "orders").unwrap();
        assert!(removed);
        assert!(!repo.is_subscribed("inst-1", "orders"));
    }
}
