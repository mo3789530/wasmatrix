use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tonic::transport::Server;
use tracing::{info, warn};
use wasmatrix_control_plane::features::node_routing::controller::NodeRoutingController;
use wasmatrix_control_plane::features::node_routing::repo::etcd::{
    validate_etcd_config, EtcdConfig, EtcdMetadataRepository,
};
use wasmatrix_control_plane::features::node_routing::repo::InMemoryNodeRoutingRepository;
use wasmatrix_control_plane::features::node_routing::service::NodeRoutingService;
use wasmatrix_control_plane::server::ControlPlaneServer;
use wasmatrix_proto::v1::control_plane_service_server::ControlPlaneServiceServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let control_plane_addr = std::env::var("CONTROL_PLANE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string())
        .parse::<SocketAddr>()?;

    info!("Starting Wasmatrix Control Plane");

    let control_plane = Arc::new(Mutex::new(wasmatrix_control_plane::ControlPlane::new(
        "node-1",
    )));

    let mut etcd_metadata_repo: Option<Arc<EtcdMetadataRepository>> = None;
    if std::env::var("USE_ETCD").ok().as_deref() == Some("true") {
        if let Some(config) = EtcdConfig::from_env() {
            if let Err(error) = validate_etcd_config(&config).await {
                warn!(error = %error, "Failed to validate etcd configuration");
            } else {
                info!(endpoints = ?config.endpoints, "etcd configuration loaded");
                etcd_metadata_repo = Some(Arc::new(EtcdMetadataRepository::new()));
            }
        } else {
            warn!("USE_ETCD is true but ETCD_ENDPOINTS is not configured");
        }
    }

    let routing_repo = Arc::new(InMemoryNodeRoutingRepository::new());
    let routing_service = if let Some(etcd_repo) = etcd_metadata_repo {
        Arc::new(NodeRoutingService::new_with_etcd(routing_repo, etcd_repo))
    } else {
        Arc::new(NodeRoutingService::new(routing_repo))
    };
    let routing_controller = Arc::new(NodeRoutingController::new(routing_service));

    if let Ok(static_nodes) = std::env::var("STATIC_NODE_AGENTS") {
        for (idx, entry) in static_nodes.split(',').enumerate() {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }

            let node_id = format!("static-node-{}", idx + 1);
            if let Err(error) = routing_controller
                .register_node(node_id.clone(), trimmed.to_string(), vec![], 0)
                .await
            {
                warn!(%node_id, endpoint = %trimmed, error = %error, "Failed to register static node");
            } else {
                info!(%node_id, endpoint = %trimmed, "Registered static node");
            }
        }
    }

    let server = ControlPlaneServer::new(control_plane, routing_controller);

    info!(%control_plane_addr, "Control Plane initialized successfully");

    Server::builder()
        .add_service(ControlPlaneServiceServer::new(server))
        .serve(control_plane_addr)
        .await?;

    Ok(())
}
