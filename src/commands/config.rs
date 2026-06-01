use std::process::ExitCode;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::cli::config::{ConfigAction, ConfigCmd, ConfigKeyArg, ConfigSetArgs};
use crate::config::AppConfig;
use crate::error::{ChainError, Result};
use crate::models::config::{ConfigEntry, ConfigStatus};
use crate::output::{OutputContext, OutputMode};

const CONFIGURABLE_KEYS: &[(&str, &str, bool)] = &[
    // (user-facing key, env var name, is_sensitive)
    ("dodo_api_key", "DODO_API_KEY", true),
    ("dodo_project_id", "DODO_PROJECT_ID", false),
    ("coingecko_api_key", "COINGECKO_API_KEY", true),
    ("debank_api_key", "DEBANK_API_KEY", true),
    ("zerion_api_key", "ZERION_API_KEY", true),
    ("goldrush_api_key", "GOLDRUSH_API_KEY", true),
    ("dune_api_key", "DUNE_API_KEY", true),
];

fn find_key(key: &str) -> Option<(&'static str, &'static str, bool)> {
    let lower = key.to_lowercase();
    CONFIGURABLE_KEYS
        .iter()
        .find(|(name, _, _)| *name == lower)
        .map(|&(name, env, sensitive)| (name, env, sensitive))
}

fn mask_value(value: &str) -> String {
    if value.len() <= 8 {
        return "*".repeat(value.len());
    }
    format!("{}...{}", &value[..4], &value[value.len() - 4..])
}

fn read_config_file(path: &std::path::Path) -> Vec<(String, String)> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn write_config_file(path: &std::path::Path, entries: &[(String, String)]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content: String = entries
        .iter()
        .map(|(k, v)| format!("{}={}\n", k, v))
        .collect();
    std::fs::write(path, content)?;
    set_config_file_permissions(path)
}

#[cfg(unix)]
fn set_config_file_permissions(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_config_file_permissions(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

pub async fn handle(
    cmd: ConfigCmd,
    config: &AppConfig,
    _output_mode: OutputMode,
) -> Result<ExitCode> {
    let env_path = config.config_env_path();
    match cmd.action {
        ConfigAction::Set(args) => set(args, &env_path, _output_mode, config),
        ConfigAction::Get(args) => get(args, &env_path, _output_mode, config),
        ConfigAction::List => list(&env_path, _output_mode, config),
        ConfigAction::Unset(args) => unset(args, &env_path, _output_mode, config),
    }
}

fn set(
    args: ConfigSetArgs,
    env_path: &std::path::Path,
    output_mode: OutputMode,
    config: &AppConfig,
) -> Result<ExitCode> {
    let (key, env_var, _sensitive) = find_key(&args.key).ok_or_else(|| {
        ChainError::Config(format!(
            "Unknown config key '{}'. Valid keys: {}",
            args.key,
            CONFIGURABLE_KEYS
                .iter()
                .map(|(k, _, _)| *k)
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;

    let mut entries = read_config_file(env_path);
    if let Some(entry) = entries.iter_mut().find(|(k, _)| k == env_var) {
        entry.1 = args.value.clone();
    } else {
        entries.push((env_var.to_string(), args.value.clone()));
    }
    write_config_file(env_path, &entries).map_err(|e| ChainError::Config(e.to_string()))?;

    // Also set in current process so subsequent commands in the same session can use it.
    std::env::set_var(env_var, &args.value);

    let status = ConfigStatus {
        key: key.to_string(),
        action: "set".to_string(),
        message: "Saved successfully".to_string(),
    };
    Ok(crate::output::print_output::<ConfigStatus>(
        Ok(status),
        "config.set",
        output_mode,
        OutputContext::new(config.chain_id, false),
    ))
}

fn get(
    args: ConfigKeyArg,
    env_path: &std::path::Path,
    output_mode: OutputMode,
    config: &AppConfig,
) -> Result<ExitCode> {
    let (key, env_var, sensitive) = find_key(&args.key).ok_or_else(|| {
        ChainError::Config(format!(
            "Unknown config key '{}'. Valid keys: {}",
            args.key,
            CONFIGURABLE_KEYS
                .iter()
                .map(|(k, _, _)| *k)
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;

    // Read from config file first, then fall back to env var.
    let entries = read_config_file(env_path);
    let raw_value = entries
        .iter()
        .find(|(k, _)| k == env_var)
        .map(|(_, v)| v.clone())
        .or_else(|| std::env::var(env_var).ok());

    let display_value = if sensitive {
        raw_value.map(|v| mask_value(&v))
    } else {
        raw_value
    };

    let entry = ConfigEntry {
        key: key.to_string(),
        value: display_value,
        masked: sensitive,
    };
    Ok(crate::output::print_output::<ConfigEntry>(
        Ok(entry),
        "config.get",
        output_mode,
        OutputContext::new(config.chain_id, false),
    ))
}

fn list(
    env_path: &std::path::Path,
    output_mode: OutputMode,
    config: &AppConfig,
) -> Result<ExitCode> {
    let entries = read_config_file(env_path);
    let result: Vec<ConfigEntry> = CONFIGURABLE_KEYS
        .iter()
        .map(|&(key, env_var, sensitive)| {
            let raw_value = entries
                .iter()
                .find(|(k, _)| k == env_var)
                .map(|(_, v)| v.clone())
                .or_else(|| std::env::var(env_var).ok());

            let display_value = if sensitive {
                raw_value.map(|v| mask_value(&v))
            } else {
                raw_value
            };

            ConfigEntry {
                key: key.to_string(),
                value: display_value,
                masked: sensitive,
            }
        })
        .collect();

    Ok(crate::output::print_output::<Vec<ConfigEntry>>(
        Ok(result),
        "config.list",
        output_mode,
        OutputContext::new(config.chain_id, false),
    ))
}

fn unset(
    args: ConfigKeyArg,
    env_path: &std::path::Path,
    output_mode: OutputMode,
    config: &AppConfig,
) -> Result<ExitCode> {
    let (key, env_var, _sensitive) = find_key(&args.key).ok_or_else(|| {
        ChainError::Config(format!(
            "Unknown config key '{}'. Valid keys: {}",
            args.key,
            CONFIGURABLE_KEYS
                .iter()
                .map(|(k, _, _)| *k)
                .collect::<Vec<_>>()
                .join(", ")
        ))
    })?;

    let mut entries = read_config_file(env_path);
    let before_len = entries.len();
    entries.retain(|(k, _)| k != env_var);

    if entries.len() == before_len {
        let status = ConfigStatus {
            key: key.to_string(),
            action: "unset".to_string(),
            message: "Key was not set in config file".to_string(),
        };
        return Ok(crate::output::print_output::<ConfigStatus>(
            Ok(status),
            "config.unset",
            output_mode,
            OutputContext::new(config.chain_id, false),
        ));
    }

    write_config_file(env_path, &entries).map_err(|e| ChainError::Config(e.to_string()))?;

    // Remove from current process env so it falls back to default.
    std::env::remove_var(env_var);

    let status = ConfigStatus {
        key: key.to_string(),
        action: "unset".to_string(),
        message: "Removed successfully".to_string(),
    };
    Ok(crate::output::print_output::<ConfigStatus>(
        Ok(status),
        "config.unset",
        output_mode,
        OutputContext::new(config.chain_id, false),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn write_config_file_uses_owner_only_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.env");

        write_config_file(&path, &[("DODO_API_KEY".to_string(), "secret".to_string())]).unwrap();

        let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
