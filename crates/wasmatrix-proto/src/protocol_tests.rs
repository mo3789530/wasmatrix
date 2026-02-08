#[cfg(test)]
mod tests {
    use crate::protocol::*;
    use crate::v1;

    #[test]
    fn test_start_instance_request_serialization() {
        let request = StartInstanceRequest {
            instance_id: "test-instance".to_string(),
            module_bytes: vec![0x00, 0x61, 0x73, 0x6d],
            capabilities: vec![CapabilityAssignment {
                instance_id: "test-instance".to_string(),
                capability_id: "kv-1".to_string(),
                provider_type: ProviderType::Kv,
                permissions: vec!["kv:read".to_string()],
            }],
            restart_policy: RestartPolicy::default(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: StartInstanceRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, request);
    }

    #[test]
    fn test_instance_metadata_serialization() {
        let metadata = InstanceMetadata {
            instance_id: "instance-1".to_string(),
            node_id: "node-1".to_string(),
            module_hash: "abc123".to_string(),
            created_at: 1234567890,
            status: InstanceStatus::Running,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: InstanceMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, metadata);
    }

    #[test]
    fn test_provider_type_serialization() {
        let provider_type = ProviderType::Kv;

        let json = serde_json::to_string(&provider_type).unwrap();
        let deserialized: ProviderType = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, ProviderType::Kv);
    }

    #[test]
    fn test_instance_status_serialization() {
        let status = InstanceStatus::Crashed;

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: InstanceStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, InstanceStatus::Crashed);
    }

    #[test]
    fn test_restart_policy_serialization() {
        let policy = RestartPolicy {
            policy_type: RestartPolicyType::OnFailure,
            max_retries: Some(3),
            backoff_seconds: Some(5),
        };

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: RestartPolicy = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, policy);
    }

    #[test]
    fn test_capability_assignment_serialization() {
        let assignment = CapabilityAssignment {
            instance_id: "instance-1".to_string(),
            capability_id: "http-1".to_string(),
            provider_type: ProviderType::Http,
            permissions: vec!["http:get".to_string(), "http:post".to_string()],
        };

        let json = serde_json::to_string(&assignment).unwrap();
        let deserialized: CapabilityAssignment = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, assignment);
    }

    #[test]
    fn test_status_report_serialization() {
        let report = StatusReport {
            node_id: "node-1".to_string(),
            instance_updates: vec![InstanceStatusUpdate {
                instance_id: "instance-1".to_string(),
                status: InstanceStatus::Running,
                error_message: None,
            }],
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&report).unwrap();
        let deserialized: StatusReport = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, report);
    }

    #[test]
    fn test_register_node_request_serialization() {
        let request = RegisterNodeRequest {
            node_id: "node-1".to_string(),
            node_address: "localhost:50051".to_string(),
            capabilities: vec!["kv".to_string(), "http".to_string()],
            max_instances: 100,
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: RegisterNodeRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, request);
    }

    #[test]
    fn test_success_response_serialization() {
        let response = StartInstanceResponse {
            success: true,
            message: "Instance started".to_string(),
            error_code: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: StartInstanceResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, response);
    }

    #[test]
    fn test_error_response_serialization() {
        let response = StopInstanceResponse {
            success: false,
            message: "Instance not found".to_string(),
            error_code: Some("INSTANCE_NOT_FOUND".to_string()),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: StopInstanceResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized, response);
    }

    #[test]
    fn test_restart_policy_default() {
        let policy = RestartPolicy::default();

        assert_eq!(policy.policy_type, RestartPolicyType::Never);
        assert!(policy.max_retries.is_none());
        assert!(policy.backoff_seconds.is_none());
    }

    #[test]
    fn test_provider_type_hashable() {
        let mut set = std::collections::HashSet::new();

        set.insert(ProviderType::Kv);
        set.insert(ProviderType::Http);
        set.insert(ProviderType::Messaging);

        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_instance_status_copy() {
        let status = InstanceStatus::Running;
        let status_copy = status;

        assert_eq!(status, status_copy);
    }

    // Property 16: Control Plane and Node Agent Protocol Communication
    // Validates that protocol <-> gRPC conversions preserve message semantics.
    #[test]
    fn property_protocol_start_instance_round_trip_v1() {
        for i in 0..100 {
            let request = StartInstanceRequest {
                instance_id: format!("instance-{i}"),
                module_bytes: vec![0x00, 0x61, 0x73, 0x6d, (i % 255) as u8],
                capabilities: vec![CapabilityAssignment {
                    instance_id: format!("instance-{i}"),
                    capability_id: format!("kv-{i}"),
                    provider_type: ProviderType::Kv,
                    permissions: vec!["kv:read".to_string(), format!("kv:scope:{i}")],
                }],
                restart_policy: RestartPolicy {
                    policy_type: if i % 2 == 0 {
                        RestartPolicyType::Always
                    } else {
                        RestartPolicyType::OnFailure
                    },
                    max_retries: Some((i % 5) as u32),
                    backoff_seconds: Some((i % 10 + 1) as u64),
                },
            };

            let v1_req: v1::StartInstanceRequest = request.clone().into();
            let round_trip: StartInstanceRequest = v1_req.try_into().unwrap();
            assert_eq!(round_trip, request);
        }
    }

    #[test]
    fn property_protocol_status_report_round_trip_v1() {
        let statuses = [
            InstanceStatus::Starting,
            InstanceStatus::Running,
            InstanceStatus::Stopped,
            InstanceStatus::Crashed,
        ];

        for i in 0..100 {
            let report = StatusReport {
                node_id: format!("node-{}", i % 7),
                instance_updates: statuses
                    .iter()
                    .enumerate()
                    .map(|(idx, status)| InstanceStatusUpdate {
                        instance_id: format!("instance-{i}-{idx}"),
                        status: *status,
                        error_message: if *status == InstanceStatus::Crashed {
                            Some("trap".to_string())
                        } else {
                            None
                        },
                    })
                    .collect(),
                timestamp: 1_700_000_000 + i as i64,
            };

            let v1_report: v1::StatusReport = report.clone().into();
            let round_trip: StatusReport = v1_report.try_into().unwrap();
            assert_eq!(round_trip, report);
        }
    }
}
