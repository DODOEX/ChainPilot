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

#[tokio::main]
async fn main() -> Result<ExitCode> {
    dotenvy::dotenv().ok();

    if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(std::io::stderr)
            .init();
    }

    let cli = Cli::parse();
    let mut config = AppConfig::load()?;

    // CLI args take highest precedence (above runtime env vars and compile-time defaults).
    if let Some(key) = cli.dodo_api_key.clone() {
        config.dodo_api_key = key;
    }
    if let Some(project_id) = cli.dodo_project_id.clone() {
        config.dodo_project_id = project_id;
    }
    if let Some(wallet_address) = cli.wallet_address.clone() {
        config.wallet_address = Some(wallet_address);
    } else if config.wallet_address.is_none() {
        config.wallet_address = cli
            .private_key
            .as_deref()
            .and_then(|pk| address_from_private_key(pk).ok())
            .map(|addr| addr.to_string());
    }

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
