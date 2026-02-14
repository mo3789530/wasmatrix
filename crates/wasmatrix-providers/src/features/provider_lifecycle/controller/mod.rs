use crate::features::provider_lifecycle::service::ProviderLifecycleService;
use wasmatrix_core::Result;

pub struct ProviderLifecycleController {
    service: ProviderLifecycleService,
}

impl ProviderLifecycleController {
    pub fn new(service: ProviderLifecycleService) -> Self {
        Self { service }
    }

    pub fn start_provider(&self, provider_id: &str) -> Result<()> {
        self.service.start_provider(provider_id)
    }

    pub fn stop_provider(&self, provider_id: &str) -> Result<()> {
        self.service.stop_provider(provider_id)
    }

    pub fn ensure_provider_available(&self, provider_id: &str) -> Result<()> {
        self.service.ensure_provider_available(provider_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::provider_lifecycle::repo::InMemoryProviderLifecycleRepository;
    use crate::features::provider_lifecycle::service::ProviderLifecycleService;
    use std::sync::Arc;

    #[test]
    fn test_controller_provider_lifecycle() {
        let controller = ProviderLifecycleController::new(ProviderLifecycleService::new(Arc::new(
            InMemoryProviderLifecycleRepository::new(),
        )));
        controller.start_provider("messaging-provider").unwrap();
        assert!(controller
            .ensure_provider_available("messaging-provider")
            .is_ok());
        controller.stop_provider("messaging-provider").unwrap();
        assert!(controller
            .ensure_provider_available("messaging-provider")
            .is_err());
    }
}
