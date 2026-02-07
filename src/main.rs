use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    // Initialize logging
    rs_fast_mcp::server::logging::init_logging("info").expect("setting default subscriber failed");

    info!("Starting RsFastMCP server...");

    if let Err(e) = rs_fast_mcp::cli::run().await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
