pub mod json;
pub mod table;

use serde::Serialize;
use std::process::ExitCode;

pub use json::ChainOutput;
pub use table::TableRenderable;

use crate::cli::Cli;
use crate::error::ChainError;
use colored::Colorize;

pub enum OutputMode {
    Json,
    Human,
    Quiet,
}

#[derive(Debug, Clone, Copy)]
pub struct OutputContext {
    pub chain_id: u64,
    pub dry_run: bool,
}

impl OutputContext {
    pub fn new(chain_id: u64, dry_run: bool) -> Self {
        Self { chain_id, dry_run }
    }
}

impl From<&Cli> for OutputMode {
    fn from(cli: &Cli) -> Self {
        if cli.quiet {
            OutputMode::Quiet
        } else if cli.json {
            OutputMode::Json
        } else {
            OutputMode::Human
        }
    }
}

pub fn print_output<T: Serialize + TableRenderable>(
    data: Result<T, ChainError>,
    command: &str,
    mode: OutputMode,
    context: OutputContext,
) -> ExitCode {
    let meta = json::OutputMeta {
        chain_id: context.chain_id,
        dry_run: context.dry_run,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    match mode {
        OutputMode::Json => match data {
            Ok(val) => {
                let out = ChainOutput::success(command, val, meta);
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
                ExitCode::SUCCESS
            }
            Err(e) => {
                let out = ChainOutput::<()>::error(command, &e, meta);
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
                ExitCode::FAILURE
            }
        },
        OutputMode::Human => match data {
            Ok(val) => {
                val.render_table();
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{}: {}", "Error".red().bold(), e);
                ExitCode::FAILURE
            }
        },
        OutputMode::Quiet => match data {
            Ok(_) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        },
    }
}
