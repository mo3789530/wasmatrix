use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use wasmatrix_core::{CoreError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderState {
    Running,
    Stopped,
}

pub trait ProviderLifecycleRepository: Send + Sync {
    fn upsert_state(&self, provider_id: &str, state: ProviderState) -> Result<()>;
    fn get_state(&self, provider_id: &str) -> Result<Option<ProviderState>>;
}

#[derive(Clone, Default)]
pub struct InMemoryProviderLifecycleRepository {
    states: Arc<RwLock<HashMap<String, ProviderState>>>,
}

impl InMemoryProviderLifecycleRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ProviderLifecycleRepository for InMemoryProviderLifecycleRepository {
    fn upsert_state(&self, provider_id: &str, state: ProviderState) -> Result<()> {
        let mut states = self.states.write().map_err(|_| {
            CoreError::InvalidCapabilityAssignment("provider lifecycle lock poisoned".to_string())
        })?;
        states.insert(provider_id.to_string(), state);
        Ok(())
    }

    fn get_state(&self, provider_id: &str) -> Result<Option<ProviderState>> {
        let states = self.states.read().map_err(|_| {
            CoreError::InvalidCapabilityAssignment("provider lifecycle lock poisoned".to_string())
        })?;
        Ok(states.get(provider_id).copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_upsert_and_get_state() {
        let repo = InMemoryProviderLifecycleRepository::new();
        repo.upsert_state("http-provider", ProviderState::Running)
            .unwrap();
        assert_eq!(
            repo.get_state("http-provider").unwrap(),
            Some(ProviderState::Running)
        );
    }
}
