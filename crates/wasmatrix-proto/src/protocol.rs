// Protocol message types for Control Plane <-> Node Agent communication
// Generated types (manually defined instead of using protoc)

use serde::{Deserialize, Serialize};

// Version: 1.0.0

// Node Agent Service Messages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StartInstanceRequest {
    pub instance_id: String,
    pub module_bytes: Vec<u8>,
    pub capabilities: Vec<CapabilityAssignment>,
    pub restart_policy: RestartPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StartInstanceResponse {
    pub success: bool,
    pub message: String,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StopInstanceRequest {
    pub instance_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StopInstanceResponse {
    pub success: bool,
    pub message: String,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryInstanceRequest {
    pub instance_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryInstanceResponse {
    pub success: bool,
    pub instance: Option<InstanceMetadata>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListInstancesRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListInstancesResponse {
    pub success: bool,
    pub instances: Vec<InstanceMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InvokeCapabilityRequest {
    pub instance_id: String,
    pub capability_id: String,
    pub provider_type: ProviderType,
    pub operation: String,
    pub params_json: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InvokeCapabilityResponse {
    pub success: bool,
    pub message: String,
    pub result_json: Option<String>,
    pub error_code: Option<String>,
}

// Control Plane Service Messages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisterNodeRequest {
    pub node_id: String,
    pub node_address: String,
    pub capabilities: Vec<String>,
    pub max_instances: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisterNodeResponse {
    pub success: bool,
    pub message: String,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusReport {
    pub node_id: String,
    pub instance_updates: Vec<InstanceStatusUpdate>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusReportResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstanceStatusUpdate {
    pub instance_id: String,
    pub status: InstanceStatus,
    pub error_message: Option<String>,
}

// Common Types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityAssignment {
    pub instance_id: String,
    pub capability_id: String,
    pub provider_type: ProviderType,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstanceMetadata {
    pub instance_id: String,
    pub node_id: String,
    pub module_hash: String,
    pub created_at: i64,
    pub status: InstanceStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProviderType {
    Kv,
    Http,
    Messaging,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum InstanceStatus {
    Starting,
    Running,
    Stopped,
    Crashed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RestartPolicy {
    pub policy_type: RestartPolicyType,
    pub max_retries: Option<u32>,
    pub backoff_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum RestartPolicyType {
    Never,
    Always,
    OnFailure,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            policy_type: RestartPolicyType::Never,
            max_retries: None,
            backoff_seconds: None,
        }
    }
}

// Conversion helpers
impl From<wasmatrix_core::InstanceStatus> for InstanceStatus {
    fn from(status: wasmatrix_core::InstanceStatus) -> Self {
        match status {
            wasmatrix_core::InstanceStatus::Starting => InstanceStatus::Starting,
            wasmatrix_core::InstanceStatus::Running => InstanceStatus::Running,
            wasmatrix_core::InstanceStatus::Stopped => InstanceStatus::Stopped,
            wasmatrix_core::InstanceStatus::Crashed => InstanceStatus::Crashed,
        }
    }
}

impl From<InstanceStatus> for wasmatrix_core::InstanceStatus {
    fn from(status: InstanceStatus) -> Self {
        match status {
            InstanceStatus::Starting => wasmatrix_core::InstanceStatus::Starting,
            InstanceStatus::Running => wasmatrix_core::InstanceStatus::Running,
            InstanceStatus::Stopped => wasmatrix_core::InstanceStatus::Stopped,
            InstanceStatus::Crashed => wasmatrix_core::InstanceStatus::Crashed,
        }
    }
}

impl From<wasmatrix_core::ProviderType> for ProviderType {
    fn from(provider_type: wasmatrix_core::ProviderType) -> Self {
        match provider_type {
            wasmatrix_core::ProviderType::Kv => ProviderType::Kv,
            wasmatrix_core::ProviderType::Http => ProviderType::Http,
            wasmatrix_core::ProviderType::Messaging => ProviderType::Messaging,
        }
    }
}

impl From<ProviderType> for wasmatrix_core::ProviderType {
    fn from(provider_type: ProviderType) -> Self {
        match provider_type {
            ProviderType::Kv => wasmatrix_core::ProviderType::Kv,
            ProviderType::Http => wasmatrix_core::ProviderType::Http,
            ProviderType::Messaging => wasmatrix_core::ProviderType::Messaging,
        }
    }
}

impl From<wasmatrix_core::RestartPolicy> for RestartPolicy {
    fn from(policy: wasmatrix_core::RestartPolicy) -> Self {
        Self {
            policy_type: match policy.policy_type {
                wasmatrix_core::RestartPolicyType::Never => RestartPolicyType::Never,
                wasmatrix_core::RestartPolicyType::Always => RestartPolicyType::Always,
                wasmatrix_core::RestartPolicyType::OnFailure => RestartPolicyType::OnFailure,
            },
            max_retries: policy.max_retries,
            backoff_seconds: policy.backoff_seconds,
        }
    }
}

impl From<RestartPolicy> for wasmatrix_core::RestartPolicy {
    fn from(policy: RestartPolicy) -> Self {
        Self {
            policy_type: match policy.policy_type {
                RestartPolicyType::Never => wasmatrix_core::RestartPolicyType::Never,
                RestartPolicyType::Always => wasmatrix_core::RestartPolicyType::Always,
                RestartPolicyType::OnFailure => wasmatrix_core::RestartPolicyType::OnFailure,
            },
            max_retries: policy.max_retries,
            backoff_seconds: policy.backoff_seconds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restart_policy_default() {
        let policy = RestartPolicy::default();
        assert_eq!(policy.policy_type, RestartPolicyType::Never);
        assert_eq!(policy.max_retries, None);
        assert_eq!(policy.backoff_seconds, None);
    }

    #[test]
    fn test_instance_status_core_round_trip() {
        let statuses = [
            wasmatrix_core::InstanceStatus::Starting,
            wasmatrix_core::InstanceStatus::Running,
            wasmatrix_core::InstanceStatus::Stopped,
            wasmatrix_core::InstanceStatus::Crashed,
        ];

        for status in statuses {
            let proto_status: InstanceStatus = status.into();
            let round_trip: wasmatrix_core::InstanceStatus = proto_status.into();
            assert_eq!(round_trip, status);
        }
    }

    #[test]
    fn test_provider_type_core_round_trip() {
        let providers = [
            wasmatrix_core::ProviderType::Kv,
            wasmatrix_core::ProviderType::Http,
            wasmatrix_core::ProviderType::Messaging,
        ];

        for provider in providers {
            let proto_provider: ProviderType = provider.into();
            let round_trip: wasmatrix_core::ProviderType = proto_provider.into();
            assert_eq!(round_trip, provider);
        }
    }

    #[test]
    fn test_restart_policy_core_round_trip() {
        let policies = [
            wasmatrix_core::RestartPolicy {
                policy_type: wasmatrix_core::RestartPolicyType::Never,
                max_retries: None,
                backoff_seconds: None,
            },
            wasmatrix_core::RestartPolicy {
                policy_type: wasmatrix_core::RestartPolicyType::Always,
                max_retries: Some(1),
                backoff_seconds: Some(1),
            },
            wasmatrix_core::RestartPolicy {
                policy_type: wasmatrix_core::RestartPolicyType::OnFailure,
                max_retries: Some(5),
                backoff_seconds: Some(10),
            },
        ];

        for policy in policies {
            let proto_policy: RestartPolicy = policy.clone().into();
            let round_trip: wasmatrix_core::RestartPolicy = proto_policy.into();
            assert_eq!(round_trip.policy_type, policy.policy_type);
            assert_eq!(round_trip.max_retries, policy.max_retries);
            assert_eq!(round_trip.backoff_seconds, policy.backoff_seconds);
        }
    }
}
