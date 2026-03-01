use std::collections::HashMap;

pub trait ConfigurationRepository: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
}

#[derive(Default)]
pub struct EnvConfigurationRepository;

impl ConfigurationRepository for EnvConfigurationRepository {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

#[derive(Default)]
pub struct InMemoryConfigurationRepository {
    values: HashMap<String, String>,
}

impl InMemoryConfigurationRepository {
    pub fn with(mut self, key: &str, value: &str) -> Self {
        self.values.insert(key.to_string(), value.to_string());
        self
    }
}

impl ConfigurationRepository for InMemoryConfigurationRepository {
    fn get(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }
}
