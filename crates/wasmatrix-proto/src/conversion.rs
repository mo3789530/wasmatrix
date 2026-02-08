use crate::protocol;
use crate::v1;
use std::convert::TryFrom;

// StartInstanceRequest
impl From<protocol::StartInstanceRequest> for v1::StartInstanceRequest {
    fn from(req: protocol::StartInstanceRequest) -> Self {
        Self {
            instance_id: req.instance_id,
            module_bytes: req.module_bytes,
            capabilities: req.capabilities.into_iter().map(Into::into).collect(),
            restart_policy: Some(req.restart_policy.into()),
        }
    }
}

impl TryFrom<v1::StartInstanceRequest> for protocol::StartInstanceRequest {
    type Error = String;

    fn try_from(req: v1::StartInstanceRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            instance_id: req.instance_id,
            module_bytes: req.module_bytes,
            capabilities: req
                .capabilities
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            restart_policy: req
                .restart_policy
                .ok_or("restart_policy is missing")?
                .try_into()?,
        })
    }
}

// StartInstanceResponse
impl From<protocol::StartInstanceResponse> for v1::StartInstanceResponse {
    fn from(res: protocol::StartInstanceResponse) -> Self {
        Self {
            success: res.success,
            message: res.message,
            error_code: res.error_code,
        }
    }
}

impl From<v1::StartInstanceResponse> for protocol::StartInstanceResponse {
    fn from(res: v1::StartInstanceResponse) -> Self {
        Self {
            success: res.success,
            message: res.message,
            error_code: res.error_code,
        }
    }
}

// StopInstanceRequest
impl From<protocol::StopInstanceRequest> for v1::StopInstanceRequest {
    fn from(req: protocol::StopInstanceRequest) -> Self {
        Self {
            instance_id: req.instance_id,
        }
    }
}

impl From<v1::StopInstanceRequest> for protocol::StopInstanceRequest {
    fn from(req: v1::StopInstanceRequest) -> Self {
        Self {
            instance_id: req.instance_id,
        }
    }
}

// StopInstanceResponse
impl From<protocol::StopInstanceResponse> for v1::StopInstanceResponse {
    fn from(res: protocol::StopInstanceResponse) -> Self {
        Self {
            success: res.success,
            message: res.message,
            error_code: res.error_code,
        }
    }
}

impl From<v1::StopInstanceResponse> for protocol::StopInstanceResponse {
    fn from(res: v1::StopInstanceResponse) -> Self {
        Self {
            success: res.success,
            message: res.message,
            error_code: res.error_code,
        }
    }
}

// QueryInstanceRequest
impl From<protocol::QueryInstanceRequest> for v1::QueryInstanceRequest {
    fn from(req: protocol::QueryInstanceRequest) -> Self {
        Self {
            instance_id: req.instance_id,
        }
    }
}

impl From<v1::QueryInstanceRequest> for protocol::QueryInstanceRequest {
    fn from(req: v1::QueryInstanceRequest) -> Self {
        Self {
            instance_id: req.instance_id,
        }
    }
}

// QueryInstanceResponse
impl From<protocol::QueryInstanceResponse> for v1::QueryInstanceResponse {
    fn from(res: protocol::QueryInstanceResponse) -> Self {
        Self {
            success: res.success,
            instance: res.instance.map(Into::into),
            error_code: res.error_code,
        }
    }
}

impl TryFrom<v1::QueryInstanceResponse> for protocol::QueryInstanceResponse {
    type Error = String;

    fn try_from(res: v1::QueryInstanceResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            success: res.success,
            instance: res.instance.map(TryInto::try_into).transpose()?,
            error_code: res.error_code,
        })
    }
}

// ListInstancesRequest
impl From<protocol::ListInstancesRequest> for v1::ListInstancesRequest {
    fn from(_req: protocol::ListInstancesRequest) -> Self {
        Self {}
    }
}

impl From<v1::ListInstancesRequest> for protocol::ListInstancesRequest {
    fn from(_req: v1::ListInstancesRequest) -> Self {
        Self {}
    }
}

// ListInstancesResponse
impl From<protocol::ListInstancesResponse> for v1::ListInstancesResponse {
    fn from(res: protocol::ListInstancesResponse) -> Self {
        Self {
            success: res.success,
            instances: res.instances.into_iter().map(Into::into).collect(),
        }
    }
}

impl TryFrom<v1::ListInstancesResponse> for protocol::ListInstancesResponse {
    type Error = String;

    fn try_from(res: v1::ListInstancesResponse) -> Result<Self, Self::Error> {
        Ok(Self {
            success: res.success,
            instances: res
                .instances
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

// RegisterNodeRequest
impl From<protocol::RegisterNodeRequest> for v1::RegisterNodeRequest {
    fn from(req: protocol::RegisterNodeRequest) -> Self {
        Self {
            node_id: req.node_id,
            node_address: req.node_address,
            capabilities: req.capabilities,
            max_instances: req.max_instances,
        }
    }
}

impl From<v1::RegisterNodeRequest> for protocol::RegisterNodeRequest {
    fn from(req: v1::RegisterNodeRequest) -> Self {
        Self {
            node_id: req.node_id,
            node_address: req.node_address,
            capabilities: req.capabilities,
            max_instances: req.max_instances,
        }
    }
}

// RegisterNodeResponse
impl From<protocol::RegisterNodeResponse> for v1::RegisterNodeResponse {
    fn from(res: protocol::RegisterNodeResponse) -> Self {
        Self {
            success: res.success,
            message: res.message,
            error_code: res.error_code,
        }
    }
}

impl From<v1::RegisterNodeResponse> for protocol::RegisterNodeResponse {
    fn from(res: v1::RegisterNodeResponse) -> Self {
        Self {
            success: res.success,
            message: res.message,
            error_code: res.error_code,
        }
    }
}

// StatusReport
impl From<protocol::StatusReport> for v1::StatusReport {
    fn from(report: protocol::StatusReport) -> Self {
        Self {
            node_id: report.node_id,
            instance_updates: report
                .instance_updates
                .into_iter()
                .map(Into::into)
                .collect(),
            timestamp: report.timestamp,
        }
    }
}

impl TryFrom<v1::StatusReport> for protocol::StatusReport {
    type Error = String;

    fn try_from(report: v1::StatusReport) -> Result<Self, Self::Error> {
        Ok(Self {
            node_id: report.node_id,
            instance_updates: report
                .instance_updates
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            timestamp: report.timestamp,
        })
    }
}

// StatusReportResponse
impl From<protocol::StatusReportResponse> for v1::StatusReportResponse {
    fn from(res: protocol::StatusReportResponse) -> Self {
        Self {
            success: res.success,
            message: res.message,
        }
    }
}

impl From<v1::StatusReportResponse> for protocol::StatusReportResponse {
    fn from(res: v1::StatusReportResponse) -> Self {
        Self {
            success: res.success,
            message: res.message,
        }
    }
}

// InstanceStatusUpdate
impl From<protocol::InstanceStatusUpdate> for v1::InstanceStatusUpdate {
    fn from(update: protocol::InstanceStatusUpdate) -> Self {
        Self {
            instance_id: update.instance_id,
            status: v1::InstanceStatus::from(update.status).into(),
            error_message: update.error_message,
        }
    }
}

impl TryFrom<v1::InstanceStatusUpdate> for protocol::InstanceStatusUpdate {
    type Error = String;

    fn try_from(update: v1::InstanceStatusUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            instance_id: update.instance_id,
            status: v1::InstanceStatus::try_from(update.status)
                .map_err(|_| "Invalid InstanceStatus")?
                .try_into()?,
            error_message: update.error_message,
        })
    }
}

// CapabilityAssignment
impl From<protocol::CapabilityAssignment> for v1::CapabilityAssignment {
    fn from(assignment: protocol::CapabilityAssignment) -> Self {
        Self {
            instance_id: assignment.instance_id,
            capability_id: assignment.capability_id,
            provider_type: v1::ProviderType::from(assignment.provider_type).into(),
            permissions: assignment.permissions,
        }
    }
}

impl TryFrom<v1::CapabilityAssignment> for protocol::CapabilityAssignment {
    type Error = String;

    fn try_from(assignment: v1::CapabilityAssignment) -> Result<Self, Self::Error> {
        Ok(Self {
            instance_id: assignment.instance_id,
            capability_id: assignment.capability_id,
            provider_type: v1::ProviderType::try_from(assignment.provider_type)
                .map_err(|_| "Invalid ProviderType")?
                .try_into()?,
            permissions: assignment.permissions,
        })
    }
}

// InstanceMetadata
impl From<protocol::InstanceMetadata> for v1::InstanceMetadata {
    fn from(meta: protocol::InstanceMetadata) -> Self {
        Self {
            instance_id: meta.instance_id,
            node_id: meta.node_id,
            module_hash: meta.module_hash,
            created_at: meta.created_at,
            status: v1::InstanceStatus::from(meta.status).into(),
        }
    }
}

impl TryFrom<v1::InstanceMetadata> for protocol::InstanceMetadata {
    type Error = String;

    fn try_from(meta: v1::InstanceMetadata) -> Result<Self, Self::Error> {
        Ok(Self {
            instance_id: meta.instance_id,
            node_id: meta.node_id,
            module_hash: meta.module_hash,
            created_at: meta.created_at,
            status: v1::InstanceStatus::try_from(meta.status)
                .map_err(|_| "Invalid InstanceStatus")?
                .try_into()?,
        })
    }
}

// RestartPolicy
impl From<protocol::RestartPolicy> for v1::RestartPolicy {
    fn from(policy: protocol::RestartPolicy) -> Self {
        Self {
            policy_type: v1::RestartPolicyType::from(policy.policy_type).into(),
            max_retries: policy.max_retries,
            backoff_seconds: policy.backoff_seconds,
        }
    }
}

impl TryFrom<v1::RestartPolicy> for protocol::RestartPolicy {
    type Error = String;

    fn try_from(policy: v1::RestartPolicy) -> Result<Self, Self::Error> {
        Ok(Self {
            policy_type: v1::RestartPolicyType::try_from(policy.policy_type)
                .map_err(|_| "Invalid RestartPolicyType")?
                .try_into()?,
            max_retries: policy.max_retries,
            backoff_seconds: policy.backoff_seconds,
        })
    }
}

// Enums

impl From<protocol::ProviderType> for v1::ProviderType {
    fn from(t: protocol::ProviderType) -> Self {
        match t {
            protocol::ProviderType::Kv => v1::ProviderType::Kv,
            protocol::ProviderType::Http => v1::ProviderType::Http,
            protocol::ProviderType::Messaging => v1::ProviderType::Messaging,
        }
    }
}

impl TryFrom<v1::ProviderType> for protocol::ProviderType {
    type Error = String;

    fn try_from(t: v1::ProviderType) -> Result<Self, Self::Error> {
        match t {
            v1::ProviderType::Kv => Ok(protocol::ProviderType::Kv),
            v1::ProviderType::Http => Ok(protocol::ProviderType::Http),
            v1::ProviderType::Messaging => Ok(protocol::ProviderType::Messaging),
            v1::ProviderType::Unspecified => Err("ProviderType is UNSPECIFIED".to_string()),
        }
    }
}

impl From<protocol::InstanceStatus> for v1::InstanceStatus {
    fn from(s: protocol::InstanceStatus) -> Self {
        match s {
            protocol::InstanceStatus::Starting => v1::InstanceStatus::Starting,
            protocol::InstanceStatus::Running => v1::InstanceStatus::Running,
            protocol::InstanceStatus::Stopped => v1::InstanceStatus::Stopped,
            protocol::InstanceStatus::Crashed => v1::InstanceStatus::Crashed,
        }
    }
}

impl TryFrom<v1::InstanceStatus> for protocol::InstanceStatus {
    type Error = String;

    fn try_from(s: v1::InstanceStatus) -> Result<Self, Self::Error> {
        match s {
            v1::InstanceStatus::Starting => Ok(protocol::InstanceStatus::Starting),
            v1::InstanceStatus::Running => Ok(protocol::InstanceStatus::Running),
            v1::InstanceStatus::Stopped => Ok(protocol::InstanceStatus::Stopped),
            v1::InstanceStatus::Crashed => Ok(protocol::InstanceStatus::Crashed),
            v1::InstanceStatus::Unspecified => Err("InstanceStatus is UNSPECIFIED".to_string()),
        }
    }
}

impl From<protocol::RestartPolicyType> for v1::RestartPolicyType {
    fn from(t: protocol::RestartPolicyType) -> Self {
        match t {
            protocol::RestartPolicyType::Never => v1::RestartPolicyType::Never,
            protocol::RestartPolicyType::Always => v1::RestartPolicyType::Always,
            protocol::RestartPolicyType::OnFailure => v1::RestartPolicyType::OnFailure,
        }
    }
}

impl TryFrom<v1::RestartPolicyType> for protocol::RestartPolicyType {
    type Error = String;

    fn try_from(t: v1::RestartPolicyType) -> Result<Self, Self::Error> {
        match t {
            v1::RestartPolicyType::Never => Ok(protocol::RestartPolicyType::Never),
            v1::RestartPolicyType::Always => Ok(protocol::RestartPolicyType::Always),
            v1::RestartPolicyType::OnFailure => Ok(protocol::RestartPolicyType::OnFailure),
            v1::RestartPolicyType::Unspecified => {
                Err("RestartPolicyType is UNSPECIFIED".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_assignment() -> protocol::CapabilityAssignment {
        protocol::CapabilityAssignment {
            instance_id: "instance-1".to_string(),
            capability_id: "kv-1".to_string(),
            provider_type: protocol::ProviderType::Kv,
            permissions: vec!["kv:read".to_string()],
        }
    }

    #[test]
    fn test_start_instance_request_round_trip() {
        let req = protocol::StartInstanceRequest {
            instance_id: "instance-1".to_string(),
            module_bytes: vec![0x00, 0x61, 0x73, 0x6d],
            capabilities: vec![sample_assignment()],
            restart_policy: protocol::RestartPolicy {
                policy_type: protocol::RestartPolicyType::OnFailure,
                max_retries: Some(3),
                backoff_seconds: Some(5),
            },
        };

        let v1_req: v1::StartInstanceRequest = req.clone().into();
        let round_trip: protocol::StartInstanceRequest = v1_req.try_into().unwrap();
        assert_eq!(round_trip, req);
    }

    #[test]
    fn test_start_instance_request_missing_restart_policy_is_error() {
        let req = v1::StartInstanceRequest {
            instance_id: "instance-1".to_string(),
            module_bytes: vec![0x00, 0x61, 0x73, 0x6d],
            capabilities: vec![],
            restart_policy: None,
        };

        let result = protocol::StartInstanceRequest::try_from(req);
        assert!(result.is_err());
    }

    #[test]
    fn test_all_simple_message_conversions() {
        let start_res = protocol::StartInstanceResponse {
            success: true,
            message: "ok".to_string(),
            error_code: None,
        };
        let _: protocol::StartInstanceResponse =
            v1::StartInstanceResponse::from(start_res.clone()).into();

        let stop_req = protocol::StopInstanceRequest {
            instance_id: "instance-1".to_string(),
        };
        let _: protocol::StopInstanceRequest =
            v1::StopInstanceRequest::from(stop_req.clone()).into();

        let stop_res = protocol::StopInstanceResponse {
            success: true,
            message: "stopped".to_string(),
            error_code: None,
        };
        let _: protocol::StopInstanceResponse =
            v1::StopInstanceResponse::from(stop_res.clone()).into();

        let query_req = protocol::QueryInstanceRequest {
            instance_id: "instance-1".to_string(),
        };
        let _: protocol::QueryInstanceRequest =
            v1::QueryInstanceRequest::from(query_req.clone()).into();

        let query_res = protocol::QueryInstanceResponse {
            success: true,
            instance: Some(protocol::InstanceMetadata {
                instance_id: "instance-1".to_string(),
                node_id: "node-1".to_string(),
                module_hash: "abc".to_string(),
                created_at: 42,
                status: protocol::InstanceStatus::Running,
            }),
            error_code: None,
        };
        let v1_query: v1::QueryInstanceResponse = query_res.clone().into();
        let _: protocol::QueryInstanceResponse = v1_query.try_into().unwrap();

        let _: protocol::ListInstancesRequest =
            v1::ListInstancesRequest::from(protocol::ListInstancesRequest {}).into();

        let list_res = protocol::ListInstancesResponse {
            success: true,
            instances: vec![protocol::InstanceMetadata {
                instance_id: "instance-1".to_string(),
                node_id: "node-1".to_string(),
                module_hash: "hash".to_string(),
                created_at: 1,
                status: protocol::InstanceStatus::Running,
            }],
        };
        let v1_list: v1::ListInstancesResponse = list_res.clone().into();
        let _: protocol::ListInstancesResponse = v1_list.try_into().unwrap();

        let reg_req = protocol::RegisterNodeRequest {
            node_id: "node-1".to_string(),
            node_address: "127.0.0.1:50051".to_string(),
            capabilities: vec!["kv".to_string()],
            max_instances: 10,
        };
        let _: protocol::RegisterNodeRequest =
            v1::RegisterNodeRequest::from(reg_req.clone()).into();

        let reg_res = protocol::RegisterNodeResponse {
            success: true,
            message: "ok".to_string(),
            error_code: None,
        };
        let _: protocol::RegisterNodeResponse =
            v1::RegisterNodeResponse::from(reg_res.clone()).into();

        let status_report = protocol::StatusReport {
            node_id: "node-1".to_string(),
            instance_updates: vec![protocol::InstanceStatusUpdate {
                instance_id: "instance-1".to_string(),
                status: protocol::InstanceStatus::Crashed,
                error_message: Some("trap".to_string()),
            }],
            timestamp: 100,
        };
        let v1_status: v1::StatusReport = status_report.clone().into();
        let _: protocol::StatusReport = v1_status.try_into().unwrap();

        let status_res = protocol::StatusReportResponse {
            success: true,
            message: "ok".to_string(),
        };
        let _: protocol::StatusReportResponse =
            v1::StatusReportResponse::from(status_res.clone()).into();
    }

    #[test]
    fn test_status_update_and_metadata_round_trip() {
        let update = protocol::InstanceStatusUpdate {
            instance_id: "instance-1".to_string(),
            status: protocol::InstanceStatus::Stopped,
            error_message: None,
        };
        let v1_update: v1::InstanceStatusUpdate = update.clone().into();
        let update_rt: protocol::InstanceStatusUpdate = v1_update.try_into().unwrap();
        assert_eq!(update_rt, update);

        let meta = protocol::InstanceMetadata {
            instance_id: "instance-1".to_string(),
            node_id: "node-1".to_string(),
            module_hash: "hash".to_string(),
            created_at: 7,
            status: protocol::InstanceStatus::Starting,
        };
        let v1_meta: v1::InstanceMetadata = meta.clone().into();
        let meta_rt: protocol::InstanceMetadata = v1_meta.try_into().unwrap();
        assert_eq!(meta_rt, meta);
    }

    #[test]
    fn test_enum_conversions_and_unspecified_errors() {
        assert_eq!(
            protocol::ProviderType::try_from(v1::ProviderType::Kv).unwrap(),
            protocol::ProviderType::Kv
        );
        assert!(protocol::ProviderType::try_from(v1::ProviderType::Unspecified).is_err());

        assert_eq!(
            protocol::InstanceStatus::try_from(v1::InstanceStatus::Running).unwrap(),
            protocol::InstanceStatus::Running
        );
        assert!(protocol::InstanceStatus::try_from(v1::InstanceStatus::Unspecified).is_err());

        assert_eq!(
            protocol::RestartPolicyType::try_from(v1::RestartPolicyType::Always).unwrap(),
            protocol::RestartPolicyType::Always
        );
        assert!(protocol::RestartPolicyType::try_from(v1::RestartPolicyType::Unspecified).is_err());
    }

    #[test]
    fn test_invalid_integer_enum_values_are_errors() {
        let invalid_update = v1::InstanceStatusUpdate {
            instance_id: "instance-1".to_string(),
            status: v1::InstanceStatus::Unspecified as i32,
            error_message: None,
        };
        assert!(protocol::InstanceStatusUpdate::try_from(invalid_update).is_err());

        let invalid_assignment = v1::CapabilityAssignment {
            instance_id: "instance-1".to_string(),
            capability_id: "kv-1".to_string(),
            provider_type: v1::ProviderType::Unspecified as i32,
            permissions: vec!["kv:read".to_string()],
        };
        assert!(protocol::CapabilityAssignment::try_from(invalid_assignment).is_err());

        let invalid_policy = v1::RestartPolicy {
            policy_type: v1::RestartPolicyType::Unspecified as i32,
            max_retries: None,
            backoff_seconds: None,
        };
        assert!(protocol::RestartPolicy::try_from(invalid_policy).is_err());
    }

    #[test]
    fn test_capability_assignment_round_trip() {
        let assignment = sample_assignment();
        let v1_assignment: v1::CapabilityAssignment = assignment.clone().into();
        let round_trip: protocol::CapabilityAssignment = v1_assignment.try_into().unwrap();
        assert_eq!(round_trip, assignment);
    }
}
