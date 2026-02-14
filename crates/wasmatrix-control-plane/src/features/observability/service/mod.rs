use crate::features::observability::repo::ObservabilityRepository;
use std::sync::Arc;

pub struct ObservabilityService {
    repo: Arc<ObservabilityRepository>,
}

impl ObservabilityService {
    pub fn new(repo: Arc<ObservabilityRepository>) -> Self {
        Self { repo }
    }

    pub fn record_api_request(&self, endpoint: &str, status: &str, seconds: f64) {
        self.repo.observe_api_request(endpoint, status, seconds);
    }

    pub fn set_active_instances(&self, count: usize) {
        self.repo.set_active_instance_count(count as f64);
    }

    pub fn record_crash(&self) {
        self.repo.inc_instance_crash_total();
    }

    pub fn record_invocation_latency(&self, seconds: f64) {
        self.repo.observe_invocation_latency(seconds);
    }

    pub fn set_node_health(&self, node_id: &str, healthy: bool) {
        self.repo.set_node_agent_health(node_id, healthy);
    }

    pub fn render_metrics(&self) -> Result<String, String> {
        self.repo.render_metrics()
    }
}
