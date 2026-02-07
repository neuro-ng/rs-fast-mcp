use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub debug: bool,
    pub log_level: LogLevel,
    pub client_init_timeout: Option<u64>, // in milliseconds
    pub secrets: HashMap<String, String>,
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            debug: false,
            log_level: LogLevel::Info,
            client_init_timeout: None,
            secrets: HashMap::new(),
            include_tags: Vec::new(),
            exclude_tags: Vec::new(),
        }
    }
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load settings from environment variables with `FASTMCP_` prefix.
    /// Priority: Env Vars (including .env) > Defaults.
    pub fn load() -> Self {
        // Load .env file if it exists
        dotenvy::dotenv().ok();

        let mut settings = Self::default();

        if let Ok(val) = env::var("FASTMCP_HOST") {
            settings.host = val;
        }

        if let Ok(val) = env::var("FASTMCP_PORT")
            && let Ok(port) = val.parse()
        {
            settings.port = port;
        }

        if let Ok(val) = env::var("FASTMCP_DEBUG") {
            settings.debug = val.to_lowercase() == "true" || val == "1";
        }

        if let Ok(val) = env::var("FASTMCP_LOG_LEVEL")
            && let Ok(level) = LogLevel::from_str(&val)
        {
            settings.log_level = level;
        }

        if let Ok(val) = env::var("FASTMCP_CLIENT_INIT_TIMEOUT")
            && let Ok(timeout) = val.parse()
        {
            settings.client_init_timeout = Some(timeout);
        }

        // Load filtering tags (comma separated)
        if let Ok(val) = env::var("FASTMCP_INCLUDE_TAGS") {
            settings.include_tags = val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        if let Ok(val) = env::var("FASTMCP_EXCLUDE_TAGS") {
            settings.exclude_tags = val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // Load secrets
        let secrets_dir =
            env::var("FASTMCP_SECRETS_DIR").unwrap_or_else(|_| "/run/secrets".to_string());
        let secrets_path = Path::new(&secrets_dir);

        if secrets_path.exists()
            && secrets_path.is_dir()
            && let Ok(entries) = fs::read_dir(secrets_path)
        {
            for entry in entries.flatten() {
                if let Ok(path) = entry.path().canonicalize()
                    && path.is_file()
                    && let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                    && let Ok(content) = fs::read_to_string(&path)
                {
                    settings
                        .secrets
                        .insert(file_name.to_string(), content.trim().to_string());
                }
            }
        }

        settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to enforce serial execution of tests that modify/read environment variables.
    // This prevents race conditions where test_load_from_env pollutes the env for test_load_defaults.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_load_defaults() {
        let _lock = ENV_LOCK.lock().unwrap();
        // Ensure clean slate just in case
        unsafe {
            env::remove_var("FASTMCP_HOST");
            env::remove_var("FASTMCP_PORT");
            env::remove_var("FASTMCP_DEBUG");
            env::remove_var("FASTMCP_LOG_LEVEL");
            env::remove_var("FASTMCP_CLIENT_INIT_TIMEOUT");
        }

        let settings = Settings::load();
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, 3000);
        assert!(!settings.debug);
    }

    #[test]
    fn test_load_from_env() {
        let _lock = ENV_LOCK.lock().unwrap();

        unsafe {
            env::set_var("FASTMCP_HOST", "0.0.0.0");
            env::set_var("FASTMCP_PORT", "8080");
            env::set_var("FASTMCP_DEBUG", "true");
            env::set_var("FASTMCP_LOG_LEVEL", "debug");
        }

        let settings = Settings::load();
        assert_eq!(settings.host, "0.0.0.0");
        assert_eq!(settings.port, 8080);
        assert!(settings.debug);
        assert!(matches!(settings.log_level, LogLevel::Debug));

        // Cleanup
        unsafe {
            env::remove_var("FASTMCP_HOST");
            env::remove_var("FASTMCP_PORT");
            env::remove_var("FASTMCP_DEBUG");
            env::remove_var("FASTMCP_LOG_LEVEL");
        }
    }

    #[test]
    fn test_load_secrets() {
        use std::io::Write;
        let _lock = ENV_LOCK.lock().unwrap(); // Use lock here too as it sets FASTMCP_SECRETS_DIR

        // Setup temp secrets dir
        let temp_dir = std::env::temp_dir().join("fastmcp_secrets_test");
        fs::create_dir_all(&temp_dir).unwrap();

        let secret_file = temp_dir.join("api_key");
        let mut f = fs::File::create(&secret_file).unwrap();
        f.write_all(b"super_secret_value").unwrap();

        unsafe {
            env::set_var("FASTMCP_SECRETS_DIR", temp_dir.to_str().unwrap());
        }

        let settings = Settings::load();
        assert_eq!(
            settings.secrets.get("api_key").map(|s| s.as_str()),
            Some("super_secret_value")
        );

        // Cleanup
        unsafe {
            env::remove_var("FASTMCP_SECRETS_DIR");
        }
        fs::remove_dir_all(temp_dir).unwrap();
    }
}
