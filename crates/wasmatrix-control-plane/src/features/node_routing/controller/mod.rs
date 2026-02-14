use std::sync::Arc;
use std::sync::Mutex;

use crate::features::node_routing::service::NodeRoutingService;
use crate::shared::error::ControlPlaneResult;
use crate::shared::types::{
    InstanceMetadata, InstanceStatusResponse, QueryInstanceRequest, StartInstanceRequest,
};
use crate::ControlPlane;
use wasmatrix_core::CapabilityAssignment;

pub struct NodeRoutingController {
    service: Arc<NodeRoutingService>,
}

impl NodeRoutingController {
    pub fn new(service: Arc<NodeRoutingService>) -> Self {
        Self { service }
    }

    pub async fn register_node(
        &self,
        node_id: String,
        node_address: String,
        capabilities: Vec<String>,
        max_instances: u32,
    ) -> ControlPlaneResult<()> {
        self.service
            .register_node(node_id, node_address, capabilities, max_instances)
            .await
    }

    pub async fn record_status_report(
        &self,
        node_id: &str,
        timestamp: i64,
    ) -> ControlPlaneResult<()> {
        self.service.record_status_report(node_id, timestamp).await
    }

    pub async fn start_instance(
        &self,
        request: StartInstanceRequest,
    ) -> ControlPlaneResult<String> {
        self.service.route_start_instance(request).await
    }

    pub async fn stop_instance(&self, instance_id: &str) -> ControlPlaneResult<()> {
        self.service.route_stop_instance(instance_id).await
    }

    pub async fn query_instance(
        &self,
        request: QueryInstanceRequest,
    ) -> ControlPlaneResult<InstanceStatusResponse> {
        self.service.route_query_instance(request).await
    }

    pub async fn list_instances(&self) -> ControlPlaneResult<Vec<InstanceMetadata>> {
        self.service.route_list_instances().await
    }

    pub async fn invoke_capability(
        &self,
        instance_id: &str,
        assignment: CapabilityAssignment,
        operation: &str,
        params: serde_json::Value,
    ) -> ControlPlaneResult<serde_json::Value> {
        self.service
            .route_capability_invocation(instance_id, assignment, operation, params)
            .await
    }

    pub async fn recover_node_state(
        &self,
        node_id: &str,
        control_plane: &Mutex<ControlPlane>,
    ) -> ControlPlaneResult<usize> {
        self.service
            .recover_node_state(node_id, control_plane)
            .await
    }
}
