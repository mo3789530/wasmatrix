use axum::extract::State;
use axum::routing::get;
use axum::Router;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tonic::transport::Server;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use wasmatrix_control_plane::features::leader_election::controller::LeaderElectionController;
use wasmatrix_control_plane::features::leader_election::repo::InMemoryLeaderElectionRepository;
use wasmatrix_control_plane::features::leader_election::service::{
    LeaderElectionConfig, LeaderElectionService,
};
use wasmatrix_control_plane::features::metadata_persistence::controller::MetadataPersistenceController;
use wasmatrix_control_plane::features::metadata_persistence::repo::EtcdBackedMetadataRepository;
use wasmatrix_control_plane::features::metadata_persistence::service::MetadataPersistenceService;
use wasmatrix_control_plane::features::node_routing::controller::NodeRoutingController;
use wasmatrix_control_plane::features::node_routing::repo::etcd::{
    validate_etcd_config, EtcdConfig, EtcdMetadataRepository,
};
use wasmatrix_control_plane::features::node_routing::repo::InMemoryNodeRoutingRepository;
use wasmatrix_control_plane::features::node_routing::service::NodeRoutingService;
use wasmatrix_control_plane::features::observability::controller::global_observability_controller;
use wasmatrix_control_plane::server::ControlPlaneServer;
use wasmatrix_proto::v1::control_plane_service_server::ControlPlaneServiceServer;

#[derive(Clone)]
struct HttpAppState {
    leader_election_controller: Arc<LeaderElectionController>,
}

#[derive(Serialize)]
struct LeaderStatusResponse {
    node_id: String,
    is_leader: bool,
    current_leader: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("wasmatrix_control_plane=info,info")),
        )
        .init();

    let control_plane_addr = std::env::var("CONTROL_PLANE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string())
        .parse::<SocketAddr>()?;
    let metrics_addr = std::env::var("METRICS_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:9100".to_string())
        .parse::<SocketAddr>()?;

    info!("Starting Wasmatrix Control Plane");

    let node_id =
        std::env::var("CONTROL_PLANE_NODE_ID").unwrap_or_else(|_| "control-plane-1".to_string());
    let control_plane = Arc::new(Mutex::new(wasmatrix_control_plane::ControlPlane::new(
        node_id.clone(),
    )));

    let mut etcd_metadata_repo: Option<Arc<EtcdMetadataRepository>> = None;
    let mut metadata_persistence_controller: Option<Arc<MetadataPersistenceController>> = None;
    if std::env::var("USE_ETCD").ok().as_deref() == Some("true") {
        if let Some(config) = EtcdConfig::from_env() {
            if let Err(error) = validate_etcd_config(&config).await {
                warn!(error = %error, "Failed to validate etcd configuration");
            } else {
                info!(endpoints = ?config.endpoints, "etcd configuration loaded");
                etcd_metadata_repo = Some(Arc::new(EtcdMetadataRepository::new()));
                #[cfg(feature = "etcd")]
                {
                    match EtcdBackedMetadataRepository::connect(&config).await {
                        Ok(repo) => {
                            metadata_persistence_controller =
                                Some(Arc::new(MetadataPersistenceController::new(Arc::new(
                                    MetadataPersistenceService::new(Arc::new(repo)),
                                ))));
                        }
                        Err(error) => {
                            warn!(error = %error, "Failed to connect metadata persistence to etcd");
                        }
                    }
                }

                #[cfg(not(feature = "etcd"))]
                {
                    metadata_persistence_controller =
                        Some(Arc::new(MetadataPersistenceController::new(Arc::new(
                            MetadataPersistenceService::new(Arc::new(
                                EtcdBackedMetadataRepository::new(),
                            )),
                        ))));
                }
            }
        } else {
            warn!("USE_ETCD is true but ETCD_ENDPOINTS is not configured");
        }
    }

    let leader_election_config = LeaderElectionConfig {
        lease_ttl: Duration::from_secs(
            std::env::var("LEADER_ELECTION_TTL_SECS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(10),
        ),
        renew_interval: Duration::from_millis(
            std::env::var("LEADER_ELECTION_RENEW_INTERVAL_MS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(3_000),
        ),
    };

    let leader_election_controller = {
        #[cfg(feature = "etcd")]
        {
            if std::env::var("USE_ETCD").ok().as_deref() == Some("true") {
                if let Some(config) = EtcdConfig::from_env() {
                    match wasmatrix_control_plane::features::leader_election::repo::EtcdLeaderElectionRepository::connect(&config).await {
                        Ok(repo) => Arc::new(LeaderElectionController::new(Arc::new(
                            LeaderElectionService::new(
                                Arc::new(repo),
                                node_id.clone(),
                                leader_election_config.clone(),
                            ),
                        ))),
                        Err(error) => {
                            warn!(error = %error, "Falling back to in-memory leader election");
                            Arc::new(LeaderElectionController::new(Arc::new(
                                LeaderElectionService::new(
                                    Arc::new(InMemoryLeaderElectionRepository::new()),
                                    node_id.clone(),
                                    leader_election_config.clone(),
                                ),
                            )))
                        }
                    }
                } else {
                    Arc::new(LeaderElectionController::new(Arc::new(
                        LeaderElectionService::new(
                            Arc::new(InMemoryLeaderElectionRepository::new()),
                            node_id.clone(),
                            leader_election_config.clone(),
                        ),
                    )))
                }
            } else {
                Arc::new(LeaderElectionController::new(Arc::new(
                    LeaderElectionService::new(
                        Arc::new(InMemoryLeaderElectionRepository::new()),
                        node_id.clone(),
                        leader_election_config.clone(),
                    ),
                )))
            }
        }

        #[cfg(not(feature = "etcd"))]
        {
            Arc::new(LeaderElectionController::new(Arc::new(
                LeaderElectionService::new(
                    Arc::new(InMemoryLeaderElectionRepository::new()),
                    node_id.clone(),
                    leader_election_config,
                ),
            )))
        }
    };
    let _leader_election_task = leader_election_controller.start();

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

    let server = ControlPlaneServer::new_with_dependencies(
        control_plane,
        routing_controller,
        metadata_persistence_controller,
        Some(leader_election_controller.clone()),
    );

    info!(%control_plane_addr, "Control Plane initialized successfully");
    let http_state = HttpAppState {
        leader_election_controller: leader_election_controller.clone(),
    };
    tokio::spawn(async move {
        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .route("/healthz", get(health_handler))
            .route("/leader", get(leader_handler))
            .with_state(http_state);
        let listener = match tokio::net::TcpListener::bind(metrics_addr).await {
            Ok(listener) => listener,
            Err(error) => {
                warn!(error = %error, %metrics_addr, "Failed to bind metrics endpoint");
                return;
            }
        };
        info!(%metrics_addr, "Metrics endpoint listening");
        if let Err(error) = axum::serve(listener, app).await {
            warn!(error = %error, "Metrics endpoint exited with error");
        }
    });

    Server::builder()
        .add_service(ControlPlaneServiceServer::new(server))
        .serve(control_plane_addr)
        .await?;

    Ok(())
}

async fn metrics_handler() -> String {
    global_observability_controller()
        .render_metrics()
        .unwrap_or_else(|e| format!("metrics_render_error{{reason=\"{}\"}} 1", e))
}

async fn health_handler(State(state): State<HttpAppState>) -> String {
    if state.leader_election_controller.is_leader().await {
        "ok".to_string()
    } else {
        "standby".to_string()
    }
}

async fn leader_handler(State(state): State<HttpAppState>) -> axum::Json<LeaderStatusResponse> {
    let status = state.leader_election_controller.leadership_status().await;
    axum::Json(LeaderStatusResponse {
        node_id: status.node_id,
        is_leader: status.is_leader,
        current_leader: status.current_leader.map(|lease| lease.leader_id),
    })
}
