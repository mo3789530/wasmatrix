use crate::features::external_api::repo::{ExternalApiPrincipal, ExternalApiRepository};
use crate::features::leader_election::controller::LeaderElectionController;
use crate::features::node_routing::controller::NodeRoutingController;
use crate::shared::error::{ControlPlaneError, ControlPlaneResult};
use crate::shared::types::{QueryInstanceRequest, StartInstanceRequest};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use chrono::Utc;
use std::sync::Arc;
use tracing::info;
use wasmatrix_core::{
    CapabilityAssignment, InstanceMetadata, InstanceStatus, ProviderType, RestartPolicy,
};

#[derive(Debug, Clone)]
pub struct ExternalInstanceRecord {
    pub metadata: InstanceMetadata,
    pub capabilities: Vec<CapabilityAssignment>,
}

#[derive(Debug, Clone)]
pub struct CreateInstanceCommand {
    pub module_base64: String,
    pub restart_policy: RestartPolicy,
    pub capabilities: Vec<CapabilityAssignment>,
}

#[derive(Debug, Clone)]
pub struct InvokeCapabilityCommand {
    pub instance_id: String,
    pub capability_id: String,
    pub operation: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct LeadershipSummary {
    pub node_id: String,
    pub is_leader: bool,
    pub current_leader: Option<String>,
}

pub struct ExternalApiService {
    repo: Arc<ExternalApiRepository>,
    node_routing_controller: Arc<NodeRoutingController>,
    leader_election_controller: Option<Arc<LeaderElectionController>>,
}

impl ExternalApiService {
    pub fn new(
        repo: Arc<ExternalApiRepository>,
        node_routing_controller: Arc<NodeRoutingController>,
        leader_election_controller: Option<Arc<LeaderElectionController>>,
    ) -> Self {
        Self {
            repo,
            node_routing_controller,
            leader_election_controller,
        }
    }

    pub fn authenticate(
        &self,
        authorization: Option<&str>,
        mtls_subject: Option<&str>,
    ) -> ControlPlaneResult<ExternalApiPrincipal> {
        if let Some(header) = authorization {
            let token = header
                .strip_prefix("Bearer ")
                .or_else(|| header.strip_prefix("bearer "))
                .ok_or_else(|| {
                    ControlPlaneError::Unauthorized(
                        "Authorization header must use Bearer".to_string(),
                    )
                })?;
            return self.repo.authenticate_jwt(token.trim());
        }

        if let Some(subject) = mtls_subject {
            return self.repo.resolve_mtls_principal(subject)?.ok_or_else(|| {
                ControlPlaneError::Unauthorized(
                    "mTLS subject is not mapped to an API principal".to_string(),
                )
            });
        }

        Err(ControlPlaneError::Unauthorized(
            "request requires JWT bearer auth or x-mtls-subject".to_string(),
        ))
    }

    pub fn authorize(
        &self,
        principal: &ExternalApiPrincipal,
        required_role: &str,
    ) -> ControlPlaneResult<()> {
        if principal.roles.iter().any(|role| role == required_role) {
            return Ok(());
        }

        Err(ControlPlaneError::PermissionDenied(format!(
            "principal '{}' lacks required role '{}'",
            principal.subject, required_role
        )))
    }

    pub async fn create_instance(
        &self,
        principal: &ExternalApiPrincipal,
        command: CreateInstanceCommand,
    ) -> ControlPlaneResult<ExternalInstanceRecord> {
        self.require_leader("create_instance").await?;

        let module_bytes = STANDARD
            .decode(command.module_base64.as_bytes())
            .map_err(|_| {
                ControlPlaneError::ValidationError("module_base64 must be valid base64".to_string())
            })?;

        let request = StartInstanceRequest {
            module_bytes,
            capabilities: command.capabilities.clone(),
            restart_policy: command.restart_policy,
        };

        let instance_id = self.node_routing_controller.start_instance(request).await?;
        let queried = self
            .node_routing_controller
            .query_instance(QueryInstanceRequest {
                instance_id: instance_id.clone(),
            })
            .await?;

        let capabilities = command
            .capabilities
            .into_iter()
            .map(|assignment| CapabilityAssignment {
                instance_id: instance_id.clone(),
                capability_id: assignment.capability_id,
                provider_type: assignment.provider_type,
                permissions: assignment.permissions,
            })
            .collect::<Vec<_>>();

        let metadata = InstanceMetadata {
            instance_id: queried.instance_id,
            node_id: queried.node_id,
            module_hash: "managed-by-node-agent".to_string(),
            created_at: queried.created_at,
            status: queried.status,
        };

        self.repo
            .restore_instance_state(metadata.clone(), capabilities.clone())?;
        self.audit_write("instance.create", principal, &metadata.instance_id);

        Ok(ExternalInstanceRecord {
            metadata,
            capabilities,
        })
    }

    pub fn list_instances(&self) -> ControlPlaneResult<Vec<ExternalInstanceRecord>> {
        self.repo
            .list_instances()?
            .into_iter()
            .map(|metadata| {
                let capabilities = self.repo.get_capabilities(&metadata.instance_id)?;
                Ok(ExternalInstanceRecord {
                    metadata,
                    capabilities,
                })
            })
            .collect()
    }

    pub fn get_instance(&self, instance_id: &str) -> ControlPlaneResult<ExternalInstanceRecord> {
        let metadata = self
            .repo
            .get_instance(instance_id)?
            .ok_or_else(|| ControlPlaneError::InstanceNotFound(instance_id.to_string()))?;
        let capabilities = self.repo.get_capabilities(instance_id)?;

        Ok(ExternalInstanceRecord {
            metadata,
            capabilities,
        })
    }

    pub async fn stop_instance(
        &self,
        principal: &ExternalApiPrincipal,
        instance_id: &str,
    ) -> ControlPlaneResult<()> {
        self.require_leader("stop_instance").await?;
        self.node_routing_controller
            .stop_instance(instance_id)
            .await?;
        self.repo
            .set_instance_status(instance_id, InstanceStatus::Stopped)?;
        self.audit_write("instance.stop", principal, instance_id);
        Ok(())
    }

    pub async fn assign_capability(
        &self,
        principal: &ExternalApiPrincipal,
        instance_id: &str,
        capability_id: String,
        provider_type: ProviderType,
        permissions: Vec<String>,
    ) -> ControlPlaneResult<ExternalInstanceRecord> {
        self.require_leader("assign_capability").await?;

        let assignment = CapabilityAssignment {
            instance_id: instance_id.to_string(),
            capability_id: capability_id.clone(),
            provider_type,
            permissions,
        };

        self.repo.assign_capability(assignment)?;
        self.audit_write(
            "instance.assign_capability",
            principal,
            &format!("{instance_id}:{capability_id}"),
        );
        self.get_instance(instance_id)
    }

    pub async fn revoke_capability(
        &self,
        principal: &ExternalApiPrincipal,
        instance_id: &str,
        capability_id: &str,
    ) -> ControlPlaneResult<ExternalInstanceRecord> {
        self.require_leader("revoke_capability").await?;
        self.repo.revoke_capability(instance_id, capability_id)?;
        self.audit_write(
            "instance.revoke_capability",
            principal,
            &format!("{instance_id}:{capability_id}"),
        );
        self.get_instance(instance_id)
    }

    pub async fn invoke_capability(
        &self,
        principal: &ExternalApiPrincipal,
        command: InvokeCapabilityCommand,
    ) -> ControlPlaneResult<serde_json::Value> {
        self.require_leader("invoke_capability").await?;

        let assignment = self
            .repo
            .get_capabilities(&command.instance_id)?
            .into_iter()
            .find(|item| item.capability_id == command.capability_id)
            .ok_or_else(|| ControlPlaneError::CapabilityNotFound(command.capability_id.clone()))?;

        let result = self
            .node_routing_controller
            .invoke_capability(
                &command.instance_id,
                assignment,
                &command.operation,
                command.params,
            )
            .await?;
        self.audit_write(
            "capability.invoke",
            principal,
            &format!("{}:{}", command.instance_id, command.capability_id),
        );
        Ok(result)
    }

    pub async fn health_status(&self) -> String {
        if let Some(controller) = &self.leader_election_controller {
            if controller.is_leader().await {
                "ok".to_string()
            } else {
                "standby".to_string()
            }
        } else {
            "ok".to_string()
        }
    }

    pub async fn leadership_status(&self) -> LeadershipSummary {
        if let Some(controller) = &self.leader_election_controller {
            let status = controller.leadership_status().await;
            return LeadershipSummary {
                node_id: status.node_id,
                is_leader: status.is_leader,
                current_leader: status.current_leader.map(|lease| lease.leader_id),
            };
        }

        LeadershipSummary {
            node_id: "standalone".to_string(),
            is_leader: true,
            current_leader: None,
        }
    }

    async fn require_leader(&self, operation: &str) -> ControlPlaneResult<()> {
        let Some(controller) = &self.leader_election_controller else {
            return Ok(());
        };

        if controller.is_leader().await {
            return Ok(());
        }

        let status = controller.leadership_status().await;
        Err(ControlPlaneError::PermissionDenied(format!(
            "{operation} rejected on follower node {}; active leader is {}",
            status.node_id,
            status
                .current_leader
                .map(|lease| lease.leader_id)
                .unwrap_or_else(|| "unknown".to_string())
        )))
    }

    fn audit_write(&self, action: &str, principal: &ExternalApiPrincipal, target: &str) {
        info!(
            audit = true,
            action,
            subject = %principal.subject,
            authn = ?principal.authn_type,
            roles = ?principal.roles,
            tenant_id = principal.tenant_id.as_deref().unwrap_or(""),
            target,
            at = %Utc::now(),
            "external_api_audit"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::external_api::repo::{AuthnType, ExternalApiPrincipal};

    fn test_principal() -> ExternalApiPrincipal {
        ExternalApiPrincipal {
            subject: "client-a".to_string(),
            authn_type: AuthnType::Jwt,
            roles: vec!["instance.read".to_string(), "instance.admin".to_string()],
            tenant_id: Some("tenant-a".to_string()),
            expires_at: None,
        }
    }

    #[test]
    fn test_authorize_success() {
        let service = ExternalApiService::new(
            Arc::new(ExternalApiRepository::from_env(Arc::new(
                std::sync::Mutex::new(crate::ControlPlane::new("node-1")),
            ))),
            Arc::new(
                crate::features::node_routing::controller::NodeRoutingController::new(Arc::new(
                    crate::features::node_routing::service::NodeRoutingService::new(Arc::new(
                        crate::features::node_routing::repo::InMemoryNodeRoutingRepository::new(),
                    )),
                )),
            ),
            None,
        );

        assert!(service
            .authorize(&test_principal(), "instance.admin")
            .is_ok());
    }

    #[test]
    fn test_authorize_rejects_missing_role() {
        let service = ExternalApiService::new(
            Arc::new(ExternalApiRepository::from_env(Arc::new(
                std::sync::Mutex::new(crate::ControlPlane::new("node-1")),
            ))),
            Arc::new(
                crate::features::node_routing::controller::NodeRoutingController::new(Arc::new(
                    crate::features::node_routing::service::NodeRoutingService::new(Arc::new(
                        crate::features::node_routing::repo::InMemoryNodeRoutingRepository::new(),
                    )),
                )),
            ),
            None,
        );

        let error = service
            .authorize(&test_principal(), "capability.invoke")
            .unwrap_err();
        assert!(matches!(error, ControlPlaneError::PermissionDenied(_)));
    }
}
