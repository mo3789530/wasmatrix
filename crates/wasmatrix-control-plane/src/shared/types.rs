pub use wasmatrix_core::{
    CapabilityAssignment, InstanceMetadata, InstanceStatus, ProviderType, RestartPolicy,
};

/// Request to start a new instance
#[derive(Debug, Clone)]
pub struct StartInstanceRequest {
    pub module_bytes: Vec<u8>,
    pub capabilities: Vec<CapabilityAssignment>,
    pub restart_policy: RestartPolicy,
}

/// Request to stop an instance
#[derive(Debug, Clone)]
pub struct StopInstanceRequest {
    pub instance_id: String,
}

/// Request to query an instance
#[derive(Debug, Clone)]
pub struct QueryInstanceRequest {
    pub instance_id: String,
}

/// Instance status response
#[derive(Debug, Clone)]
pub struct InstanceStatusResponse {
    pub instance_id: String,
    pub status: InstanceStatus,
    pub node_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Request to assign a capability
#[derive(Debug, Clone)]
pub struct AssignCapabilityRequest {
    pub instance_id: String,
    pub capability: CapabilityAssignment,
}

/// Request to revoke a capability
#[derive(Debug, Clone)]
pub struct RevokeCapabilityRequest {
    pub instance_id: String,
    pub capability_id: String,
}
