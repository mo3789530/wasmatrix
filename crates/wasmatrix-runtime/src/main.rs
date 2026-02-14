use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use wasmatrix_runtime::runtime::WasmRuntime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let runtime = WasmRuntime::from_env();
    info!(backend = ?runtime.backend(), "Starting Wasmatrix Runtime");

    Ok(())
}
