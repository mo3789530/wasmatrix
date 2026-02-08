use std::sync::Arc;

use wasmatrix_core::InstanceStatus;
use wasmatrix_proto::v1::{InstanceStatus as ProtoInstanceStatus, InstanceStatusUpdate};

use crate::features::status_reporting::repo::{StatusReportRepo, StatusReportRepoError};
use crate::NodeAgent;

#[derive(Debug, thiserror::Error)]
pub enum StatusReportServiceError {
    #[error("status report repository error: {0}")]
    Repo(#[from] StatusReportRepoError),
}

#[derive(Clone)]
pub struct StatusReportService {
    node_id: String,
    agent: Arc<NodeAgent>,
    repo: StatusReportRepo,
}

impl StatusReportService {
    pub fn new(node_id: String, agent: Arc<NodeAgent>, repo: StatusReportRepo) -> Self {
        Self {
            node_id,
            agent,
            repo,
        }
    }

    pub async fn report_status_change(
        &self,
        instance_id: String,
        status: InstanceStatus,
        error_message: Option<String>,
    ) -> Result<(), StatusReportServiceError> {
        let update = InstanceStatusUpdate {
            instance_id,
            status: proto_status(status) as i32,
            error_message,
        };

        self.repo
            .report_status(&self.node_id, vec![update])
            .await
            .map_err(Into::into)
    }

    pub async fn report_heartbeat(&self) -> Result<(), StatusReportServiceError> {
        let instance_ids = self.agent.list_instances().await;
        let mut updates = Vec::with_capacity(instance_ids.len());

        for instance_id in instance_ids {
            let status = self.agent.get_instance_status(&instance_id).await;
            updates.push(InstanceStatusUpdate {
                instance_id,
                status: proto_status(status) as i32,
                error_message: None,
            });
        }

        self.repo
            .report_status(&self.node_id, updates)
            .await
            .map_err(Into::into)
    }
}

fn proto_status(status: InstanceStatus) -> ProtoInstanceStatus {
    match status {
        InstanceStatus::Starting => ProtoInstanceStatus::Starting,
        InstanceStatus::Running => ProtoInstanceStatus::Running,
        InstanceStatus::Stopped => ProtoInstanceStatus::Stopped,
        InstanceStatus::Crashed => ProtoInstanceStatus::Crashed,
    }
}
