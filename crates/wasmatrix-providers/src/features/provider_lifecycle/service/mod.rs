use crate::features::provider_lifecycle::repo::{ProviderLifecycleRepository, ProviderState};
use std::sync::Arc;
use wasmatrix_core::{CoreError, Result};

pub struct ProviderLifecycleService {
    repo: Arc<dyn ProviderLifecycleRepository>,
}

impl ProviderLifecycleService {
    pub fn new(repo: Arc<dyn ProviderLifecycleRepository>) -> Self {
        Self { repo }
    }

    pub fn start_provider(&self, provider_id: &str) -> Result<()> {
        self.repo.upsert_state(provider_id, ProviderState::Running)
    }

    pub fn stop_provider(&self, provider_id: &str) -> Result<()> {
        self.repo.upsert_state(provider_id, ProviderState::Stopped)
    }

    pub fn ensure_provider_available(&self, provider_id: &str) -> Result<()> {
        match self.repo.get_state(provider_id)? {
            Some(ProviderState::Stopped) => Err(CoreError::InvalidCapabilityAssignment(format!(
                "Provider '{}' is stopped",
                provider_id
            ))),
            Some(ProviderState::Running) => Ok(()),
            None => {
                // Default behavior: first sighting auto-registers as running.
                self.start_provider(provider_id)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::provider_lifecycle::repo::InMemoryProviderLifecycleRepository;

    #[test]
    fn test_start_and_stop_provider_independently() {
        let service =
            ProviderLifecycleService::new(Arc::new(InMemoryProviderLifecycleRepository::new()));
        service.start_provider("http-provider").unwrap();
        assert!(service.ensure_provider_available("http-provider").is_ok());

        service.stop_provider("http-provider").unwrap();
        assert!(service.ensure_provider_available("http-provider").is_err());
    }

    #[test]
    fn test_unknown_provider_defaults_to_running() {
        let service =
            ProviderLifecycleService::new(Arc::new(InMemoryProviderLifecycleRepository::new()));
        assert!(service.ensure_provider_available("new-provider").is_ok());
    }

    #[test]
    fn property_graceful_provider_shutdown_handling() {
        let service =
            ProviderLifecycleService::new(Arc::new(InMemoryProviderLifecycleRepository::new()));

        for i in 0..100 {
            let provider_id = format!("provider-{i}");
            service.start_provider(&provider_id).unwrap();
            assert!(service.ensure_provider_available(&provider_id).is_ok());

            service.stop_provider(&provider_id).unwrap();
            assert!(service.ensure_provider_available(&provider_id).is_err());

            service.start_provider(&provider_id).unwrap();
            assert!(service.ensure_provider_available(&provider_id).is_ok());
        }
    }
}
