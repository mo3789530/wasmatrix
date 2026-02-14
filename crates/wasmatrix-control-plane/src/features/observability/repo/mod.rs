use prometheus::{
    opts, Counter, CounterVec, Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec,
    Registry, TextEncoder,
};

pub struct ObservabilityRepository {
    registry: Registry,
    active_instance_count: Gauge,
    instance_crash_total: Counter,
    invocation_latency_seconds: Histogram,
    api_request_total: CounterVec,
    api_request_latency_seconds: HistogramVec,
    node_agent_health: GaugeVec,
}

impl ObservabilityRepository {
    pub fn new() -> Result<Self, String> {
        let registry = Registry::new();

        let active_instance_count =
            Gauge::with_opts(opts!("wasmatrix_active_instance_count", "Active instances"))
                .map_err(|e| e.to_string())?;
        let instance_crash_total = Counter::with_opts(opts!(
            "wasmatrix_instance_crash_total",
            "Crashed instances total"
        ))
        .map_err(|e| e.to_string())?;
        let invocation_latency_seconds = Histogram::with_opts(HistogramOpts::new(
            "wasmatrix_capability_invocation_latency_seconds",
            "Capability invocation latency (seconds)",
        ))
        .map_err(|e| e.to_string())?;
        let api_request_total = CounterVec::new(
            opts!(
                "wasmatrix_api_request_total",
                "Control plane API request total"
            ),
            &["endpoint", "status"],
        )
        .map_err(|e| e.to_string())?;
        let api_request_latency_seconds = HistogramVec::new(
            HistogramOpts::new(
                "wasmatrix_api_request_latency_seconds",
                "Control plane API request latency (seconds)",
            ),
            &["endpoint"],
        )
        .map_err(|e| e.to_string())?;
        let node_agent_health = GaugeVec::new(
            opts!(
                "wasmatrix_node_agent_health",
                "Node Agent health status (1 healthy / 0 unhealthy)"
            ),
            &["node_id"],
        )
        .map_err(|e| e.to_string())?;

        registry
            .register(Box::new(active_instance_count.clone()))
            .map_err(|e| e.to_string())?;
        registry
            .register(Box::new(instance_crash_total.clone()))
            .map_err(|e| e.to_string())?;
        registry
            .register(Box::new(invocation_latency_seconds.clone()))
            .map_err(|e| e.to_string())?;
        registry
            .register(Box::new(api_request_total.clone()))
            .map_err(|e| e.to_string())?;
        registry
            .register(Box::new(api_request_latency_seconds.clone()))
            .map_err(|e| e.to_string())?;
        registry
            .register(Box::new(node_agent_health.clone()))
            .map_err(|e| e.to_string())?;

        Ok(Self {
            registry,
            active_instance_count,
            instance_crash_total,
            invocation_latency_seconds,
            api_request_total,
            api_request_latency_seconds,
            node_agent_health,
        })
    }

    pub fn set_active_instance_count(&self, count: f64) {
        self.active_instance_count.set(count);
    }

    pub fn inc_instance_crash_total(&self) {
        self.instance_crash_total.inc();
    }

    pub fn observe_invocation_latency(&self, seconds: f64) {
        self.invocation_latency_seconds.observe(seconds);
    }

    pub fn observe_api_request(&self, endpoint: &str, status: &str, seconds: f64) {
        self.api_request_total
            .with_label_values(&[endpoint, status])
            .inc();
        self.api_request_latency_seconds
            .with_label_values(&[endpoint])
            .observe(seconds);
    }

    pub fn set_node_agent_health(&self, node_id: &str, healthy: bool) {
        self.node_agent_health
            .with_label_values(&[node_id])
            .set(if healthy { 1.0 } else { 0.0 });
    }

    pub fn render_metrics(&self) -> Result<String, String> {
        let mut buffer = Vec::new();
        let encoder = TextEncoder::new();
        let families = self.registry.gather();
        encoder
            .encode(&families, &mut buffer)
            .map_err(|e| e.to_string())?;
        String::from_utf8(buffer).map_err(|e| e.to_string())
    }
}
