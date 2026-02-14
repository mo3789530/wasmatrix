use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tonic::transport::Server;
use tracing::{info, warn, Level};
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use wasmatrix_agent::features::status_reporting::controller::StatusReportController;
use wasmatrix_agent::features::status_reporting::repo::StatusReportRepo;
use wasmatrix_agent::features::status_reporting::service::StatusReportService;
use wasmatrix_agent::server::NodeAgentServer;
use wasmatrix_agent::NodeAgent;
use wasmatrix_proto::v1::node_agent_service_server::NodeAgentServiceServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("wasmatrix_agent=info,info")),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let node_id = std::env::var("NODE_ID").unwrap_or_else(|_| "node-1".to_string());
    let node_agent_addr = std::env::var("NODE_AGENT_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50052".to_string())
        .parse::<SocketAddr>()?;
    let control_plane_addr = std::env::var("CONTROL_PLANE_ADDR")
        .unwrap_or_else(|_| "http://127.0.0.1:50051".to_string());
    let report_interval_secs = std::env::var("STATUS_REPORT_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(10);

    info!(%node_id, %node_agent_addr, %control_plane_addr, "Starting Wasmatrix Node Agent");

    let agent = Arc::new(NodeAgent::new(node_id.clone())?);

    let status_report_controller = match StatusReportRepo::connect(&control_plane_addr).await {
        Ok(repo) => {
            let service = Arc::new(StatusReportService::new(
                node_id.clone(),
                agent.clone(),
                repo,
            ));
            let controller = Arc::new(StatusReportController::new(
                service,
                Duration::from_secs(report_interval_secs),
            ));

            if let Err(error) = controller.report_heartbeat().await {
                warn!(error = %error, "Initial heartbeat report failed");
            }

            controller.clone().spawn_periodic_reporting();
            Some(controller)
        }
        Err(error) => {
            warn!(error = %error, "Status reporting disabled because control plane is unreachable");
            None
        }
    };

    let server = NodeAgentServer::new(agent, status_report_controller);
    Server::builder()
        .add_service(NodeAgentServiceServer::new(server))
        .serve(node_agent_addr)
        .await?;

    Ok(())
}
