use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "fastmcp")]
#[command(about = "FastMCP Rust Implementation", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Set log level
    #[arg(long, default_value = "info")]
    pub log_level: String,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the MCP server
    #[command(alias = "serve")]
    Run {
        /// Transport to use (stdio, sse)
        #[arg(long, default_value = "stdio")]
        transport: String,

        /// Port for SSE/HTTP transport
        #[arg(long, default_value_t = 3000)]
        port: u16,

        /// Path to configuration file (defaults to fastmcp.json if present)
        #[arg(long)]
        config: Option<String>,
    },

    /// Run the server with MCP Inspector for development
    Dev {
        /// Path to configuration file (defaults to fastmcp.json if present)
        #[arg(long)]
        config: Option<String>,

        /// Command to run the inspector (defaults to "npx")
        #[arg(long, default_value = "npx")]
        npx_path: String,
    },

    /// Basic client to interact with a server
    Client {
        /// Server URL (for SSE) or Command (for Stdio)
        #[arg(long)]
        server_url: Option<String>,

        /// Command to run (for Stdio transport)
        #[arg(long)]
        command: Option<String>,

        /// Arguments for the command (for Stdio transport)
        #[arg(long)]
        args: Vec<String>,
    },

    /// Inspect a server's capabilities
    Inspect {
        /// Server URL (for SSE) or Command (for Stdio)
        #[arg(long)]
        server_url: Option<String>,

        /// Command to run (for Stdio transport)
        #[arg(long)]
        command: Option<String>,

        /// Arguments for the command (for Stdio transport)
        #[arg(long)]
        args: Vec<String>,
    },

    /// Display version information
    Version,
}

pub mod commands;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Set log level? Already set in main.rs but could be adjusted here.

    match &cli.command {
        Commands::Run {
            transport,
            port,
            config,
        } => commands::run(transport, *port, config.as_deref()).await,
        Commands::Dev { config, npx_path } => {
            commands::dev(config.as_deref(), npx_path.clone()).await
        }
        Commands::Client {
            server_url,
            command,
            args,
        } => commands::client(server_url.as_deref(), command.as_deref(), args).await,
        Commands::Inspect {
            server_url,
            command,
            args,
        } => commands::inspect(server_url.as_deref(), command.as_deref(), args).await,
        Commands::Version => commands::version().await,
    }
}
