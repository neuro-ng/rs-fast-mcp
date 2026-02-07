use crate::error::FastMCPError;
use crate::server::core::FastMCPServer;
use crate::server::transport::Transport;

use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::error;

pub struct Server {
    pub core: FastMCPServer,
    transports: Vec<Box<dyn Transport>>,
}

impl Server {
    pub fn new(core: FastMCPServer, transports: Vec<Box<dyn Transport>>) -> Self {
        Self { core, transports }
    }

    pub fn builder(name: &str, version: &str) -> crate::server::builder::ServerBuilder {
        crate::server::builder::ServerBuilder::new(name, version)
    }

    pub async fn run(self) -> Result<(), FastMCPError> {
        // 1. Run startup hooks
        self.core.run_startup().await?;

        let mut set = JoinSet::new();
        // Handler needs a clone of core, but core is cheap to clone (Arc wrapper)
        // Handler needs a clone of core, but core is cheap to clone (Arc wrapper)
        let handler = Arc::new(self.core.clone());

        for transport in self.transports {
            let h = handler.clone();
            // Subscribe to notifications for each transport
            let rx = self.core.subscribe_notifications();
            set.spawn(async move { transport.start(h, Some(rx)).await });
        }

        let shutdown_signal = async {
            #[cfg(unix)]
            let terminate = async {
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to install signal handler")
                    .recv()
                    .await;
            };

            #[cfg(not(unix))]
            let terminate = std::future::pending::<()>();

            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate => {},
            }
        };

        // Wait for the first transport to finish or fail, OR a signal.
        let result = tokio::select! {
            res = set.join_next() => {
                if let Some(res) = res {
                    match res {
                        Ok(Err(e)) => {
                            error!("Transport error: {}", e);
                            Err(e)
                        }
                        Ok(Ok(())) => {
                            // One transport finished (e.g. stdio EOF).
                            Ok(())
                        }
                        Err(e) => {
                            error!("Task join error: {}", e);
                            Err(FastMCPError::new(e.to_string()))
                        }
                    }
                } else {
                    Ok(())
                }
            },
            _ = shutdown_signal => {
                tracing::info!("Shutdown signal received");
                Ok(())
            }
        };

        // 2. Run shutdown hooks
        // We attempt to run shutdown hooks even if transport failed, to cleanup.
        if let Err(e) = self.core.run_shutdown().await {
            error!("Shutdown hook failed: {}", e);
            // If the main run was successful, return the shutdown error.
            // If it was already an error, keep the original error but log this one.
            if result.is_ok() {
                return Err(e);
            }
        }

        result
    }
}
