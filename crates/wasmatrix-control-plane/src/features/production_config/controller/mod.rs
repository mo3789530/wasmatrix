use crate::features::production_config::repo::ConfigurationRepository;
use crate::features::production_config::service::{
    BootstrapConfig, BootstrapConfigError, ProductionConfigService,
};

pub struct ProductionConfigController {
    service: ProductionConfigService,
}

impl ProductionConfigController {
    pub fn new(repo: Box<dyn ConfigurationRepository>) -> Self {
        Self {
            service: ProductionConfigService::new(repo),
        }
    }

    pub fn load(&self) -> Result<BootstrapConfig, BootstrapConfigError> {
        self.service.load()
    }
}
