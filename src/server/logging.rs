use std::sync::OnceLock;
use tracing_subscriber::{
    Registry, filter::EnvFilter, fmt, layer::SubscriberExt, reload, util::SubscriberInitExt,
};

/// Handle to reload the log filter globally.
/// This allows changing the log level at runtime.
pub type LogFilterHandle = reload::Handle<EnvFilter, Registry>;

/// Global handle for the logging filter.
static LOG_FILTER_HANDLE: OnceLock<LogFilterHandle> = OnceLock::new();

/// Initializes the logging system with a default filter and stderr output.
///
/// # Arguments
///
/// * `default_level` - The default log level to use if `RUST_LOG` is not set (e.g., "info", "debug").
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if logging is already initialized.
pub fn init_logging(default_level: &str) -> anyhow::Result<()> {
    // 1. Create the filter layer (EnvFilter).
    // It tries to read from RUST_LOG env var, otherwise falls back to `default_level`.
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    // 2. Wrap the filter in a reload layer to allow dynamic updates.
    let (filter_layer, reload_handle) = reload::Layer::new(filter);

    // 3. Store the handle globally.
    // If init is called twice, we just ignore the second attempt's handle (or error out).
    // For simplicity, we only allow one init.
    if LOG_FILTER_HANDLE.set(reload_handle).is_err() {
        return Err(anyhow::anyhow!("Logging already initialized"));
    }

    // 4. Create the formatting layer.
    // We write to stderr to keep stdout clean for Stdio transport.
    let fmt_layer = fmt::Layer::default()
        .with_writer(std::io::stderr)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(false)
        .with_line_number(false);

    // 5. Combine and init.
    // valid subscriber needs Registry + Layers.
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .try_init()?;

    Ok(())
}

/// Sets the global log level dynamically.
///
/// # Arguments
///
/// * `level` - The new log level directive (e.g., "debug", "error", "my_crate=trace").
pub fn set_global_log_level(level: &str) -> anyhow::Result<()> {
    if let Some(handle) = LOG_FILTER_HANDLE.get() {
        let new_filter = EnvFilter::new(level);
        handle.reload(new_filter)?;
        Ok(())
    } else {
        Err(anyhow::anyhow!("Logging not initialized"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Tests run in parallel, so initializing the global logger in one test
    // might affect others or fail if already initialized.
    // We can't easily test "init succeeds" then "init fails" in parallel without synchronization.
    // But we can test that it initializes and we can change levels.

    #[test]
    fn test_logging_init_and_level_change() {
        // This test might conflict with other tests if they also init logging.
        // We try to init, if it fails because it's already init (by another test), that's fine for this context,
        // but we should assert that we can change the level.
        let init_result = init_logging("info");

        // If it returns Err, it must be "Logging already initialized"
        if let Err(e) = &init_result {
            assert_eq!(e.to_string(), "Logging already initialized");
        }

        // Now try to change level
        let change_result = set_global_log_level("debug");
        assert!(change_result.is_ok(), "Should be able to change log level");

        let change_result_2 = set_global_log_level("warn");
        assert!(
            change_result_2.is_ok(),
            "Should be able to change log level again"
        );
    }
}
