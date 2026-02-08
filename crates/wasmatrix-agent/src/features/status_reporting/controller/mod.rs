use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time;
use tracing::{debug, warn};
use wasmatrix_core::InstanceStatus;

use crate::features::status_reporting::service::{StatusReportService, StatusReportServiceError};

#[derive(Clone)]
pub struct StatusReportController {
    service: Arc<StatusReportService>,
    interval: Duration,
}

impl StatusReportController {
    pub fn new(service: Arc<StatusReportService>, interval: Duration) -> Self {
        Self { service, interval }
    }

    pub fn spawn_periodic_reporting(self: Arc<Self>) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = time::interval(self.interval);
            loop {
                ticker.tick().await;

                if let Err(error) = self.service.report_heartbeat().await {
                    warn!(error = %error, "Failed to send heartbeat status report");
                } else {
                    debug!("Heartbeat status report sent");
                }
            }
        })
    }

    pub async fn report_status_change(
        &self,
        instance_id: String,
        status: InstanceStatus,
        error_message: Option<String>,
    ) -> Result<(), StatusReportServiceError> {
        self.service
            .report_status_change(instance_id, status, error_message)
            .await
    }

    pub async fn report_heartbeat(&self) -> Result<(), StatusReportServiceError> {
        self.service.report_heartbeat().await
    }
}
