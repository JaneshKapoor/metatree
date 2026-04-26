//! Config resolution: CLI overrides > env vars > `~/.ometa/config.toml`.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR: &str = ".ometa";
const CONFIG_FILE: &str = "config.toml";
const ENV_HOST: &str = "OPENMETADATA_HOST";
const ENV_TOKEN: &str = "OPENMETADATA_JWT_TOKEN";

#[derive(Debug, Default, Clone)]
pub struct Overrides {
    pub host: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OnDisk {
    pub host: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Resolved {
    pub host: String,
    pub token: String,
}

pub fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not determine home directory"))?;
    Ok(home.join(CONFIG_DIR).join(CONFIG_FILE))
}

pub fn load_disk() -> OnDisk {
    let Ok(path) = config_path() else {
        return OnDisk::default();
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return OnDisk::default();
    };
    toml::from_str(&text).unwrap_or_default()
}

pub fn save_disk(d: &OnDisk) -> Result<PathBuf> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let body = toml::to_string_pretty(d).context("serializing config")?;
    fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// Resolve into a fully-populated `Resolved`, returning a friendly error if
/// either field is missing.
pub fn resolve(overrides: Overrides) -> Result<Resolved> {
    let disk = load_disk();
    let host = overrides
        .host
        .or_else(|| env::var(ENV_HOST).ok())
        .or(disk.host)
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "OPENMETADATA_HOST is not set. Pass --host, set the env var, or run \
                 `ometa configure`."
            )
        })?;
    let token = overrides
        .token
        .or_else(|| env::var(ENV_TOKEN).ok())
        .or(disk.token)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "OPENMETADATA_JWT_TOKEN is not set. Pass --token, set the env var, or run \
                 `ometa configure`."
            )
        })?;
    Ok(Resolved { host, token })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    // Serialize env-mutating tests so cargo test's parallel runner doesn't
    // interleave set_var/remove_var between cases.
    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn clear_env() {
        env::remove_var(ENV_HOST);
        env::remove_var(ENV_TOKEN);
    }

    #[test]
    fn cli_overrides_win() {
        let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        let r = resolve(Overrides {
            host: Some("https://h/api/".into()),
            token: Some("tok".into()),
        })
        .unwrap();
        assert_eq!(r.host, "https://h/api");
        assert_eq!(r.token, "tok");
    }

    #[test]
    fn env_used_when_no_overrides() {
        let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        env::set_var(ENV_HOST, "https://envhost/api");
        env::set_var(ENV_TOKEN, "envtok");
        let r = resolve(Overrides::default()).unwrap();
        assert_eq!(r.host, "https://envhost/api");
        assert_eq!(r.token, "envtok");
        clear_env();
    }

    #[test]
    fn missing_host_is_a_clear_error() {
        let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        let err = resolve(Overrides::default()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("OPENMETADATA_HOST"));
    }
}
