use alloy::primitives::Address;
use std::process::ExitCode;

use crate::api::ApiClients;
use crate::chain::OnChainClient;
use crate::cli::token::{TokenAction, TokenCmd};
use crate::commands::resolve_token;
use crate::config::AppConfig;
use crate::error::{ChainError, Result};
use crate::output::{OutputContext, OutputMode};
use crate::store::QuoteStore;

pub async fn handle(
    cmd: TokenCmd,
    config: &AppConfig,
    _store: &QuoteStore,
    api: &ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match cmd.action {
        TokenAction::Info(args) => info(args, api, config, output_mode).await,
        TokenAction::Contract(args) => contract(args, api, config, output_mode).await,
    }
}

async fn info(
    args: crate::cli::token::TokenIdentArg,
    api: &ApiClients,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.effective_chain_id(args.chain_id);
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_client;
    let token_ref = match resolve_token(&args.token, chain_id, onchain, api, config).await {
        Ok(t) => t,
        Err(e) => {
            return Ok(
                crate::output::print_output::<crate::models::token::TokenInfo>(
                    Err(e),
                    "token.info",
                    output_mode,
                    OutputContext::new(chain_id, false),
                ),
            );
        }
    };
    let addr: Address = match token_ref.address.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(
                crate::output::print_output::<crate::models::token::TokenInfo>(
                    Err(ChainError::InvalidAddress(token_ref.address.clone())),
                    "token.info",
                    output_mode,
                    OutputContext::new(chain_id, false),
                ),
            );
        }
    };
    let info = match crate::chain::get_token_info(onchain, addr).await {
        Ok(i) => i,
        Err(e) => {
            return Ok(
                crate::output::print_output::<crate::models::token::TokenInfo>(
                    Err(e),
                    "token.info",
                    output_mode,
                    OutputContext::new(chain_id, false),
                ),
            );
        }
    };
    Ok(
        crate::output::print_output::<crate::models::token::TokenInfo>(
            Ok(info),
            "token.info",
            output_mode,
            OutputContext::new(chain_id, false),
        ),
    )
}

async fn contract(
    args: crate::cli::token::TokenIdentArg,
    api: &ApiClients,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.effective_chain_id(args.chain_id);
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_client;
    let token_ref = match resolve_token(&args.token, chain_id, onchain, api, config).await {
        Ok(t) => t,
        Err(e) => {
            return Ok(crate::output::print_output::<crate::models::token::TokenContract>(
                Err(e),
                "token.contract",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };
    let addr: Address = match token_ref.address.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<crate::models::token::TokenContract>(
                Err(ChainError::InvalidAddress(token_ref.address.clone())),
                "token.contract",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };
    match crate::chain::inspect_token_contract(onchain, addr).await {
        Ok(contract) => Ok(crate::output::print_output::<crate::models::token::TokenContract>(
            Ok(contract),
            "token.contract",
            output_mode,
            OutputContext::new(chain_id, false),
        )),
        Err(e) => Ok(crate::output::print_output::<crate::models::token::TokenContract>(
            Err(e),
            "token.contract",
            output_mode,
            OutputContext::new(chain_id, false),
        )),
    }
}
