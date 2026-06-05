use std::process::ExitCode;

use crate::api::ApiClients;
use crate::cli::protocol::{ProtocolAction, ProtocolCmd};
use crate::config::AppConfig;
use crate::error::Result;
use crate::models::protocol::{ProtocolChains, ProtocolInfo, ProtocolRevenue, ProtocolTvl};
use crate::output::{OutputContext, OutputMode};
use crate::store::QuoteStore;

pub async fn handle(
    cmd: ProtocolCmd,
    config: &AppConfig,
    _store: &QuoteStore,
    api: &ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match cmd.action {
        ProtocolAction::Info(args) => {
            let data = api.defillama.info(&args.protocol).await;
            Ok(crate::output::print_output::<ProtocolInfo>(
                data,
                "protocol.info",
                output_mode,
                OutputContext::new(config.chain_id, false),
            ))
        }
        ProtocolAction::Tvl(args) => {
            let data = api
                .defillama
                .tvl(&args.protocol, args.limit, args.offset)
                .await;
            Ok(crate::output::print_output::<ProtocolTvl>(
                data,
                "protocol.tvl",
                output_mode,
                OutputContext::new(config.chain_id, false),
            ))
        }
        ProtocolAction::Revenue(args) => {
            let data = api.defillama.revenue(&args.protocol).await;
            Ok(crate::output::print_output::<ProtocolRevenue>(
                data,
                "protocol.revenue",
                output_mode,
                OutputContext::new(config.chain_id, false),
            ))
        }
        ProtocolAction::Chains(args) => {
            let data = api.defillama.chains(&args.protocol).await;
            Ok(crate::output::print_output::<ProtocolChains>(
                data,
                "protocol.chains",
                output_mode,
                OutputContext::new(config.chain_id, false),
            ))
        }
    }
}
