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

/// A config key resolved to how it is stored.
enum ResolvedKey {
    /// A fixed API-key style entry: (user-facing key, env var, sensitive).
    Simple(&'static str, &'static str, bool),
    /// A per-chain RPC entry. `Some(id)` targets one chain via a `.` suffix;
    /// `None` means the key carried no chain, so each action decides the scope.
    Rpc { chain: Option<u64> },
}

/// Parse a user-facing config key into its storage form.
/// Accepts the fixed API keys, plus `rpc_url` / `rpc` optionally suffixed with
/// a chain id (`rpc_url.56`).
fn resolve_key(key: &str) -> Result<ResolvedKey> {
    let lower = key.trim().to_lowercase();
    if let Some((name, env, sensitive)) = find_key(&lower) {
        return Ok(ResolvedKey::Simple(name, env, sensitive));
    }

    let (head, chain) = match lower.split_once('.') {
        Some((head, suffix)) => {
            let id: u64 = suffix.parse().map_err(|_| {
                ChainError::Config(format!("Invalid chain id '{}' in key '{}'", suffix, key))
            })?;
            (head, Some(id))
        }
        None => (lower.as_str(), None),
    };

    if head == "rpc_url" || head == "rpc" {
        return Ok(ResolvedKey::Rpc { chain });
    }

    Err(unknown_key_error(key))
}

fn unknown_key_error(key: &str) -> ChainError {
    ChainError::Config(format!(
        "Unknown config key '{}'. Valid keys: {}, rpc_url[.<chainId>]",
        key,
        CONFIGURABLE_KEYS
            .iter()
            .map(|(k, _, _)| *k)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Env var name that persists the RPC URL for a chain, e.g. `RPC_URL_56`.
fn rpc_env_var(chain_id: u64) -> String {
    format!("{}{}", crate::config::RPC_URL_ENV_PREFIX, chain_id)
}

/// If `key` is an `RPC_URL_<id>` env var, return the chain id.
fn rpc_key_chain_id(key: &str) -> Option<u64> {
    key.strip_prefix(crate::config::RPC_URL_ENV_PREFIX)
        .and_then(|suffix| suffix.parse().ok())
}

/// Reject anything that is not a valid http(s) RPC URL, matching the parse
/// `OnChainClient::new` performs so misconfiguration fails here, not at first use.
fn validate_rpc_url(value: &str) -> Result<()> {
    let url = url::Url::parse(value)
        .map_err(|e| ChainError::Config(format!("Invalid RPC URL '{}': {}", value, e)))?;
    match url.scheme() {
        "http" | "https" => Ok(()),
        other => Err(ChainError::Config(format!(
            "Invalid RPC URL '{}': scheme '{}' is not http or https",
            value, other
        ))),
    }
}

/// Insert or overwrite `key` in the parsed config-file entries.
fn upsert(entries: &mut Vec<(String, String)>, key: &str, value: &str) {
    if let Some(entry) = entries.iter_mut().find(|(k, _)| k == key) {
        entry.1 = value.to_string();
    } else {
        entries.push((key.to_string(), value.to_string()));
    }
}

/// Look up a key's value in the parsed config-file entries.
fn lookup<'a>(entries: &'a [(String, String)], key: &str) -> Option<&'a str> {
    entries
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// The effective value for a simple key: env var wins over the config file.
fn effective_simple_value(env_path: &std::path::Path, env_var: &str) -> Option<String> {
    std::env::var(env_var)
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| lookup(&read_config_file(env_path), env_var).map(str::to_string))
}

/// The effective per-chain RPC map: config-file entries as the base, with
/// `RPC_URL_<chainId>` environment variables taking precedence (matching the
/// runtime precedence the app itself resolves). Sorted by chain id.
fn effective_rpc_overrides(env_path: &std::path::Path) -> std::collections::BTreeMap<u64, String> {
    let mut map: std::collections::BTreeMap<u64, String> = read_config_file(env_path)
        .into_iter()
        .filter_map(|(k, v)| rpc_key_chain_id(&k).map(|id| (id, v)))
        .collect();
    // Environment overrides the file (env wins at runtime).
    for (k, v) in std::env::vars() {
        if let Some(id) = rpc_key_chain_id(&k) {
            if !v.trim().is_empty() {
                map.insert(id, v);
            }
        }
    }
    map
}

/// The effective configured RPC URL for a single chain (env over file).
fn effective_rpc_for(env_path: &std::path::Path, chain_id: u64) -> Option<String> {
    let env_var = rpc_env_var(chain_id);
    std::env::var(&env_var)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| lookup(&read_config_file(env_path), &env_var).map(str::to_string))
}

/// All configured `rpc_url.<id>` rows (effective: env over file), sorted by chain id.
fn rpc_entries(env_path: &std::path::Path) -> Vec<ConfigEntry> {
    effective_rpc_overrides(env_path)
        .into_iter()
        .map(|(id, url)| ConfigEntry {
            key: format!("rpc_url.{}", id),
            value: Some(url),
            masked: false,
        })
        .collect()
}

/// Emit a `ConfigStatus` result through the standard output path.
fn status_output(
    key: &str,
    action: &str,
    message: &str,
    output_mode: OutputMode,
    config: &AppConfig,
) -> ExitCode {
    let status = ConfigStatus {
        key: key.to_string(),
        action: action.to_string(),
        message: message.to_string(),
    };
    crate::output::print_output::<ConfigStatus>(
        Ok(status),
        &format!("config.{}", action),
        output_mode,
        OutputContext::new(config.chain_id, false),
    )
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
    match resolve_key(&args.key)? {
        ResolvedKey::Simple(key, env_var, _sensitive) => {
            let mut entries = read_config_file(env_path);
            upsert(&mut entries, env_var, &args.value);
            write_config_file(env_path, &entries).map_err(|e| ChainError::Config(e.to_string()))?;
            // Also set in current process so subsequent commands in the same session can use it.
            std::env::set_var(env_var, &args.value);
            Ok(status_output(
                key,
                "set",
                "Saved successfully",
                output_mode,
                config,
            ))
        }
        ResolvedKey::Rpc { chain } => set_rpc(chain, &args.value, env_path, output_mode, config),
    }
}

/// Handle `config set rpc_url[.<id>] <url|json-map>`.
fn set_rpc(
    chain: Option<u64>,
    value: &str,
    env_path: &std::path::Path,
    output_mode: OutputMode,
    config: &AppConfig,
) -> Result<ExitCode> {
    let value = value.trim();
    let mut entries = read_config_file(env_path);

    // A leading `{` selects the batch JSON-map form: {"1":"https://...","56":"..."}.
    if value.starts_with('{') {
        if chain.is_some() {
            return Err(ChainError::Config(
                "A JSON map cannot be combined with a chain-specific key; use `rpc_url` with the map or `rpc_url.<id>` with a single URL".to_string(),
            ));
        }
        let map: std::collections::BTreeMap<String, String> = serde_json::from_str(value)
            .map_err(|e| ChainError::Config(format!("Invalid JSON RPC map: {}", e)))?;
        if map.is_empty() {
            return Err(ChainError::Config("JSON RPC map is empty".to_string()));
        }
        // Validate everything before mutating so a bad entry leaves config untouched.
        let mut parsed: Vec<(u64, String)> = Vec::with_capacity(map.len());
        for (k, url) in &map {
            let id: u64 = k
                .parse()
                .map_err(|_| ChainError::Config(format!("Invalid chain id '{}' in JSON map", k)))?;
            validate_rpc_url(url)?;
            parsed.push((id, url.clone()));
        }
        for (id, url) in &parsed {
            let env_var = rpc_env_var(*id);
            upsert(&mut entries, &env_var, url);
            std::env::set_var(&env_var, url);
        }
        write_config_file(env_path, &entries).map_err(|e| ChainError::Config(e.to_string()))?;
        let mut ids: Vec<u64> = parsed.iter().map(|(id, _)| *id).collect();
        ids.sort_unstable();
        let ids = ids
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Ok(status_output(
            "rpc_url",
            "set",
            &format!("Saved RPC endpoints for chain(s): {}", ids),
            output_mode,
            config,
        ));
    }

    // Single URL form: target chain from the key suffix, else the active chain.
    validate_rpc_url(value)?;
    let id = chain.unwrap_or(config.chain_id);
    let env_var = rpc_env_var(id);
    upsert(&mut entries, &env_var, value);
    write_config_file(env_path, &entries).map_err(|e| ChainError::Config(e.to_string()))?;
    std::env::set_var(&env_var, value);
    Ok(status_output(
        &format!("rpc_url.{}", id),
        "set",
        "Saved successfully",
        output_mode,
        config,
    ))
}

fn get(
    args: ConfigKeyArg,
    env_path: &std::path::Path,
    output_mode: OutputMode,
    config: &AppConfig,
) -> Result<ExitCode> {
    match resolve_key(&args.key)? {
        ResolvedKey::Simple(key, env_var, sensitive) => {
            // Report the effective value: env var wins over the config file,
            // matching the precedence the app resolves at runtime.
            let raw_value = effective_simple_value(env_path, env_var);

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
        ResolvedKey::Rpc { chain } => {
            // Explicit chain = a `.` suffix on the key, or an explicit `--chain-id`.
            // Otherwise show the whole configured map.
            let target = chain.or_else(|| config.chain_id_overridden.then_some(config.chain_id));
            match target {
                Some(id) => {
                    let entry = ConfigEntry {
                        key: format!("rpc_url.{}", id),
                        value: effective_rpc_for(env_path, id),
                        masked: false,
                    };
                    Ok(crate::output::print_output::<ConfigEntry>(
                        Ok(entry),
                        "config.get",
                        output_mode,
                        OutputContext::new(config.chain_id, false),
                    ))
                }
                None => Ok(crate::output::print_output::<Vec<ConfigEntry>>(
                    Ok(rpc_entries(env_path)),
                    "config.get",
                    output_mode,
                    OutputContext::new(config.chain_id, false),
                )),
            }
        }
    }
}

fn list(
    env_path: &std::path::Path,
    output_mode: OutputMode,
    config: &AppConfig,
) -> Result<ExitCode> {
    let mut result: Vec<ConfigEntry> = CONFIGURABLE_KEYS
        .iter()
        .map(|&(key, env_var, sensitive)| {
            let raw_value = effective_simple_value(env_path, env_var);

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

    // Append configured per-chain RPC endpoints (sorted by chain id).
    result.extend(rpc_entries(env_path));

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
    match resolve_key(&args.key)? {
        ResolvedKey::Simple(key, env_var, _sensitive) => {
            let mut entries = read_config_file(env_path);
            let before_len = entries.len();
            entries.retain(|(k, _)| k != env_var);

            if entries.len() == before_len {
                return Ok(status_output(
                    key,
                    "unset",
                    "Key was not set in config file",
                    output_mode,
                    config,
                ));
            }

            write_config_file(env_path, &entries).map_err(|e| ChainError::Config(e.to_string()))?;
            // Remove from current process env so it falls back to default.
            std::env::remove_var(env_var);
            Ok(status_output(
                key,
                "unset",
                "Removed successfully",
                output_mode,
                config,
            ))
        }
        ResolvedKey::Rpc { chain } => {
            // Explicit chain removes that one; otherwise clear every configured RPC.
            let target = chain.or_else(|| config.chain_id_overridden.then_some(config.chain_id));
            let mut entries = read_config_file(env_path);

            match target {
                Some(id) => {
                    let env_var = rpc_env_var(id);
                    let key = format!("rpc_url.{}", id);
                    let before_len = entries.len();
                    entries.retain(|(k, _)| k != &env_var);
                    if entries.len() == before_len {
                        return Ok(status_output(
                            &key,
                            "unset",
                            "Key was not set in config file",
                            output_mode,
                            config,
                        ));
                    }
                    write_config_file(env_path, &entries)
                        .map_err(|e| ChainError::Config(e.to_string()))?;
                    std::env::remove_var(&env_var);
                    Ok(status_output(&key, "unset", "Removed successfully", output_mode, config))
                }
                None => {
                    let removed: Vec<String> = entries
                        .iter()
                        .filter(|(k, _)| rpc_key_chain_id(k).is_some())
                        .map(|(k, _)| k.clone())
                        .collect();
                    if removed.is_empty() {
                        return Ok(status_output(
                            "rpc_url",
                            "unset",
                            "No RPC endpoints were set",
                            output_mode,
                            config,
                        ));
                    }
                    entries.retain(|(k, _)| rpc_key_chain_id(k).is_none());
                    write_config_file(env_path, &entries)
                        .map_err(|e| ChainError::Config(e.to_string()))?;
                    for env_var in &removed {
                        std::env::remove_var(env_var);
                    }
                    Ok(status_output(
                        "rpc_url",
                        "unset",
                        &format!("Removed {} RPC endpoint(s)", removed.len()),
                        output_mode,
                        config,
                    ))
                }
            }
        }
    }
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

    // Config-command tests use high, otherwise-unused chain ids so the process-env
    // side effects of set/unset cannot collide with `config::mod` env tests.
    fn tmp_path() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.env");
        (dir, path)
    }

    fn test_cfg(chain_id: u64, overridden: bool) -> AppConfig {
        let mut c = AppConfig::load().unwrap();
        c.chain_id = chain_id;
        c.chain_id_overridden = overridden;
        c
    }

    #[test]
    fn resolve_key_parses_simple_and_rpc_forms() {
        assert!(matches!(
            resolve_key("dodo_api_key").unwrap(),
            ResolvedKey::Simple(..)
        ));
        assert!(matches!(
            resolve_key("rpc_url").unwrap(),
            ResolvedKey::Rpc { chain: None }
        ));
        assert!(matches!(
            resolve_key("rpc_url.56").unwrap(),
            ResolvedKey::Rpc { chain: Some(56) }
        ));
        assert!(matches!(
            resolve_key("RPC.42").unwrap(),
            ResolvedKey::Rpc { chain: Some(42) }
        ));
        assert!(resolve_key("rpc_url.abc").is_err());
        assert!(resolve_key("totally_unknown").is_err());
    }

    #[test]
    fn validate_rpc_url_accepts_http_rejects_others() {
        assert!(validate_rpc_url("https://node.example").is_ok());
        assert!(validate_rpc_url("http://localhost:8545").is_ok());
        assert!(validate_rpc_url("ftp://node.example").is_err());
        assert!(validate_rpc_url("not a url").is_err());
    }

    #[test]
    fn set_single_writes_active_chain_rpc() {
        let (_d, path) = tmp_path();
        let cfg = test_cfg(700001, true);
        set_rpc(None, "https://active-node", &path, OutputMode::Quiet, &cfg).unwrap();
        let entries = read_config_file(&path);
        assert_eq!(lookup(&entries, "RPC_URL_700001"), Some("https://active-node"));
        std::env::remove_var("RPC_URL_700001");
    }

    #[test]
    fn set_suffixed_key_writes_that_chain() {
        let (_d, path) = tmp_path();
        let cfg = test_cfg(1, false);
        set(
            ConfigSetArgs {
                key: "rpc_url.700002".to_string(),
                value: "https://n2".to_string(),
            },
            &path,
            OutputMode::Quiet,
            &cfg,
        )
        .unwrap();
        let entries = read_config_file(&path);
        assert_eq!(lookup(&entries, "RPC_URL_700002"), Some("https://n2"));
        std::env::remove_var("RPC_URL_700002");
    }

    #[test]
    fn set_json_map_merges_without_dropping_existing() {
        let (_d, path) = tmp_path();
        let cfg = test_cfg(1, false);
        // Seed one chain, then merge a map that adds another.
        set_rpc(Some(700003), "https://keep", &path, OutputMode::Quiet, &cfg).unwrap();
        set_rpc(
            None,
            "{\"700004\":\"https://added\"}",
            &path,
            OutputMode::Quiet,
            &cfg,
        )
        .unwrap();
        let entries = read_config_file(&path);
        assert_eq!(lookup(&entries, "RPC_URL_700003"), Some("https://keep"));
        assert_eq!(lookup(&entries, "RPC_URL_700004"), Some("https://added"));
        std::env::remove_var("RPC_URL_700003");
        std::env::remove_var("RPC_URL_700004");
    }

    #[test]
    fn set_rejects_invalid_url_and_bad_json() {
        let (_d, path) = tmp_path();
        let cfg = test_cfg(700005, true);
        assert!(set_rpc(None, "ftp://bad", &path, OutputMode::Quiet, &cfg).is_err());
        assert!(set_rpc(Some(1), "{\"1\":\"https://x\"}", &path, OutputMode::Quiet, &cfg).is_err());
        // Nothing was written.
        assert!(read_config_file(&path).is_empty());
    }

    #[test]
    fn unset_all_clears_every_rpc_entry() {
        let (_d, path) = tmp_path();
        let cfg = test_cfg(700006, false);
        set_rpc(Some(700006), "https://a", &path, OutputMode::Quiet, &cfg).unwrap();
        set_rpc(Some(700007), "https://b", &path, OutputMode::Quiet, &cfg).unwrap();
        // chain not overridden and no suffix -> clears all.
        unset(
            ConfigKeyArg {
                key: "rpc_url".to_string(),
            },
            &path,
            OutputMode::Quiet,
            &cfg,
        )
        .unwrap();
        // unset edits the file; assert the file (not the env-merged view, which
        // other parallel tests may pollute) has no RPC entries left.
        assert!(read_config_file(&path)
            .iter()
            .all(|(k, _)| rpc_key_chain_id(k).is_none()));
        std::env::remove_var("RPC_URL_700006");
        std::env::remove_var("RPC_URL_700007");
    }

    #[test]
    fn rpc_entries_are_sorted_by_chain_id() {
        let (_d, path) = tmp_path();
        write_config_file(
            &path,
            &[
                ("RPC_URL_700009".to_string(), "https://b".to_string()),
                ("DODO_API_KEY".to_string(), "secret".to_string()),
                ("RPC_URL_700008".to_string(), "https://a".to_string()),
            ],
        )
        .unwrap();
        // rpc_entries merges the environment, so other RPC_URL_* vars may exist;
        // assert the two file ids appear in ascending relative order.
        let keys: Vec<String> = rpc_entries(&path).into_iter().map(|e| e.key).collect();
        let a = keys.iter().position(|k| k == "rpc_url.700008");
        let b = keys.iter().position(|k| k == "rpc_url.700009");
        assert!(a.is_some() && b.is_some(), "both file ids present");
        assert!(a < b, "sorted by chain id");
    }

    #[test]
    fn inspection_reports_env_over_file() {
        let (_d, path) = tmp_path();
        write_config_file(&path, &[("RPC_URL_700010".to_string(), "https://from-file".to_string())])
            .unwrap();
        std::env::set_var("RPC_URL_700010", "https://from-env");

        // Single-chain get and the full map both prefer the env value.
        assert_eq!(
            effective_rpc_for(&path, 700010).as_deref(),
            Some("https://from-env")
        );
        let map = effective_rpc_overrides(&path);
        assert_eq!(map.get(&700010).map(String::as_str), Some("https://from-env"));

        std::env::remove_var("RPC_URL_700010");
    }

    #[test]
    fn inspection_includes_env_only_override() {
        let (_d, path) = tmp_path();
        // Nothing in the file; the override exists only in the environment.
        std::env::set_var("RPC_URL_700011", "https://env-only");

        assert!(rpc_entries(&path).iter().any(|e| e.key == "rpc_url.700011"
            && e.value.as_deref() == Some("https://env-only")));

        std::env::remove_var("RPC_URL_700011");
    }

    #[test]
    fn effective_simple_value_prefers_env_over_file() {
        // Use a unique, non-config env var name so this cannot race with other
        // tests that read real config keys via AppConfig::load().
        const KEY: &str = "CHAINPILOT_TEST_SIMPLE_KEY_700100";
        let (_d, path) = tmp_path();
        write_config_file(&path, &[(KEY.to_string(), "file-key".to_string())]).unwrap();
        std::env::set_var(KEY, "env-key");
        assert_eq!(effective_simple_value(&path, KEY).as_deref(), Some("env-key"));
        std::env::remove_var(KEY);
        // With no env var, falls back to the file.
        assert_eq!(effective_simple_value(&path, KEY).as_deref(), Some("file-key"));
    }
}
