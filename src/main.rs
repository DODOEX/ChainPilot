use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use std::process::ExitCode;

mod api;
mod chain;
mod cli;
mod commands;
mod config;
mod error;
mod models;
mod output;
mod store;

use crate::chain::address_from_private_key;
use crate::cli::Cli;
use crate::config::AppConfig;
use crate::output::OutputMode;

/// Load the persistent config file (`config.env` in the data directory).
/// Unlike `dotenvy::dotenv()`, this always sets env vars so config file
/// values take precedence over the CWD `.env` file.
fn load_config_env() {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("chain");
    let config_path = data_dir.join("config.env");
    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            std::env::set_var(key.trim(), value.trim());
        }
    }
}

fn apply_cli_overrides(config: &mut AppConfig, cli: &Cli) {
    // CLI args take highest precedence (above runtime env vars and compile-time defaults).
    if let Some(rpc_url) = cli.rpc_url.clone() {
        config.rpc_url = rpc_url;
        config.rpc_url_overridden = true;
    }
    if let Some(chain_id) = cli.chain_id {
        config.set_chain_id(chain_id);
    }
    if let Some(private_key) = cli.private_key.clone() {
        config.private_key = Some(private_key);
        config.keystore_path = None;
        config.keystore_password_file = None;
        config.keystore_password_env = None;
    }
    if let Some(keystore_path) = cli.keystore_path.clone() {
        config.private_key = None;
        config.keystore_path = Some(keystore_path);
    }
    if let Some(password_file) = cli.password_file.clone() {
        config.keystore_password_file = Some(password_file);
    }
    if let Some(password_env) = cli.password_env.clone() {
        config.keystore_password_env = Some(password_env);
    }
    if let Some(wallet_address) = cli.wallet_address.clone() {
        config.wallet_address = Some(wallet_address);
    } else if config.wallet_address.is_none() {
        config.wallet_address = config
            .private_key
            .as_deref()
            .and_then(|pk| address_from_private_key(pk).ok())
            .map(|addr| addr.to_string());
    }
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    dotenvy::dotenv().ok();
    load_config_env();

    if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(std::io::stderr)
            .init();
    }

    let cli = Cli::parse();
    let mut config = AppConfig::load()?;
    apply_cli_overrides(&mut config, &cli);

    let store = store::QuoteStore::new(&config)?;
    let api_clients = api::ApiClients::new(&config)?;

    let output_mode = OutputMode::from(&cli);
    let cli_json = cli.json;
    let default_chain_id = config.chain_id;

    let result = commands::dispatch(cli.command, config, store, api_clients, output_mode).await;

    match result {
        Ok(exit_code) => Ok(exit_code),
        Err(e) => {
            if cli_json {
                let err_output = output::ChainOutput::<()>::error(
                    "dispatch",
                    &e,
                    output::json::OutputMeta {
                        chain_id: default_chain_id,
                        dry_run: false,
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                );
                println!("{}", serde_json::to_string_pretty(&err_output).unwrap());
            } else {
                eprintln!("{}: {}", "Error".red().bold(), e);
            }
            Ok(ExitCode::FAILURE)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved: Vec<(&str, Option<String>)> = vars
            .iter()
            .map(|(k, _)| (*k, std::env::var(k).ok()))
            .collect();
        for (k, v) in vars {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
        f();
        for (k, v) in &saved {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }

    #[test]
    fn cli_chain_id_override_updates_default_rpc() {
        with_env(&[("CHAIN_ID", Some("1"))], || {
            let cli = Cli::parse_from([
                "chainpilot",
                "--chain-id",
                "11155111",
                "wallet",
                "balance",
                "0x54393f2288b4F3da2ac247A48d48eFF3Ac52F6d6",
            ]);
            let mut config = AppConfig::load().unwrap();

            apply_cli_overrides(&mut config, &cli);

            assert_eq!(config.chain_id, 11155111);
            assert_eq!(
                config.rpc_url,
                "https://ethereum-sepolia-rpc.publicnode.com"
            );
            assert!(!config.rpc_url_overridden);
        });
    }

    #[test]
    fn cli_explicit_rpc_override_takes_precedence_over_chain_default() {
        with_env(&[("CHAIN_ID", Some("1"))], || {
            let cli = Cli::parse_from([
                "chainpilot",
                "--rpc-url",
                "https://override.example.com",
                "--chain-id",
                "11155111",
                "wallet",
                "balance",
                "0x54393f2288b4F3da2ac247A48d48eFF3Ac52F6d6",
            ]);
            let mut config = AppConfig::load().unwrap();

            apply_cli_overrides(&mut config, &cli);

            assert_eq!(config.chain_id, 11155111);
            assert_eq!(config.rpc_url, "https://override.example.com");
            assert!(config.rpc_url_overridden);
        });
    }

    #[test]
    fn cli_keystore_path_overrides_env_private_key() {
        with_env(
            &[
                (
                    "PRIVATE_KEY",
                    Some("0x59c6995e998f97a5a0044966f0945382dbf7f50a3f2f72f5f7a0b7d7d4f5e5f1"),
                ),
                ("KEYSTORE_PATH", None),
            ],
            || {
                let cli = Cli::parse_from([
                    "chainpilot",
                    "--keystore-path",
                    "/tmp/test-keystore.json",
                    "wallet",
                    "balance",
                    "0x54393f2288b4F3da2ac247A48d48eFF3Ac52F6d6",
                ]);
                let mut config = AppConfig::load().unwrap();

                apply_cli_overrides(&mut config, &cli);

                assert_eq!(
                    config.keystore_path.as_deref(),
                    Some("/tmp/test-keystore.json")
                );
                assert!(config.private_key.is_none());
            },
        );
    }
}
