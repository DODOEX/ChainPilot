use std::process::ExitCode;

use crate::api::ApiClients;
use crate::cli::chain::{ChainAction, ChainCmd};
use crate::config::AppConfig;
use crate::error::Result;
use crate::models::chain::{ChainFlows, ChainInfo, ChainProtocols, ChainStablecoins};
use crate::output::{OutputContext, OutputMode};
use crate::store::QuoteStore;

pub async fn handle(
    cmd: ChainCmd,
    config: &AppConfig,
    _store: &QuoteStore,
    api: &ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match cmd.action {
        ChainAction::Info(args) => {
            let data = api.chain.chain_info(&args.chain).await;
            Ok(crate::output::print_output::<ChainInfo>(
                data,
                "chain.info",
                output_mode,
                OutputContext::new(config.chain_id, false),
            ))
        }
        ChainAction::Flows(args) => {
            let data = api.chain.chain_flows(&args.chain).await;
            Ok(crate::output::print_output::<ChainFlows>(
                data,
                "chain.flows",
                output_mode,
                OutputContext::new(config.chain_id, false),
            ))
        }
        ChainAction::Stablecoins(args) => {
            let data = api.chain.chain_stablecoins(&args.chain).await;
            Ok(crate::output::print_output::<ChainStablecoins>(
                data,
                "chain.stablecoins",
                output_mode,
                OutputContext::new(config.chain_id, false),
            ))
        }
        ChainAction::Protocols(args) => {
            let data = api
                .chain
                .chain_protocols(&args.chain, args.limit)
                .await;
            Ok(crate::output::print_output::<ChainProtocols>(
                data,
                "chain.protocols",
                output_mode,
                OutputContext::new(config.chain_id, false),
            ))
        }
    }
}
