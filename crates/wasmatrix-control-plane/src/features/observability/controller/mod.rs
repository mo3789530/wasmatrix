use crate::features::observability::repo::ObservabilityRepository;
use crate::features::observability::service::ObservabilityService;
use std::sync::{Arc, OnceLock};

pub struct ObservabilityController {
    service: ObservabilityService,
}

impl ObservabilityController {
    pub fn new(service: ObservabilityService) -> Self {
        Self { service }
    }

    pub fn record_api_request(&self, endpoint: &str, status: &str, seconds: f64) {
        self.service.record_api_request(endpoint, status, seconds);
    }

    pub fn set_active_instances(&self, count: usize) {
        self.service.set_active_instances(count);
    }

    pub fn record_crash(&self) {
        self.service.record_crash();
    }

    pub fn record_invocation_latency(&self, seconds: f64) {
        self.service.record_invocation_latency(seconds);
    }

    pub fn set_node_health(&self, node_id: &str, healthy: bool) {
        self.service.set_node_health(node_id, healthy);
    }

    pub fn render_metrics(&self) -> Result<String, String> {
        self.service.render_metrics()
    }
}

static GLOBAL_OBSERVABILITY: OnceLock<Arc<ObservabilityController>> = OnceLock::new();

pub fn global_observability_controller() -> Arc<ObservabilityController> {
    GLOBAL_OBSERVABILITY
        .get_or_init(|| {
            let repo = Arc::new(ObservabilityRepository::new().expect("metrics init"));
            Arc::new(ObservabilityController::new(ObservabilityService::new(
                repo,
            )))
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_render_contains_known_metric_names() {
        let controller = global_observability_controller();
        controller.record_api_request("test", "ok", 0.01);
        let rendered = controller.render_metrics().unwrap();
        assert!(rendered.contains("wasmatrix_api_request_total"));
    }
}
