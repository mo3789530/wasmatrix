use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::Status;
use wasmatrix_proto::v1::control_plane_service_client::ControlPlaneServiceClient;
use wasmatrix_proto::v1::{InstanceStatusUpdate, StatusReport};

#[derive(Debug, thiserror::Error)]
pub enum StatusReportRepoError {
    #[error("failed to connect to control plane: {0}")]
    Connection(String),
    #[error("failed to report status to control plane: {0}")]
    Report(String),
}

#[derive(Clone)]
pub struct StatusReportRepo {
    client: Arc<Mutex<ControlPlaneServiceClient<Channel>>>,
}

impl StatusReportRepo {
    pub async fn connect(control_plane_addr: &str) -> Result<Self, StatusReportRepoError> {
        let client = ControlPlaneServiceClient::connect(control_plane_addr.to_string())
            .await
            .map_err(|e| StatusReportRepoError::Connection(e.to_string()))?;

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    pub async fn report_status(
        &self,
        node_id: &str,
        instance_updates: Vec<InstanceStatusUpdate>,
    ) -> Result<(), StatusReportRepoError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);

        let request = tonic::Request::new(StatusReport {
            node_id: node_id.to_string(),
            instance_updates,
            timestamp,
        });

        let mut client = self.client.lock().await;
        client
            .report_status(request)
            .await
            .map(|_| ())
            .map_err(map_tonic_status)
    }
}

fn map_tonic_status(status: Status) -> StatusReportRepoError {
    StatusReportRepoError::Report(status.to_string())
}
