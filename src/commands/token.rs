use alloy::primitives::Address;
use std::process::ExitCode;

use crate::api::ApiClients;
use crate::chain::OnChainClient;
use crate::cli::token::{TokenAction, TokenCmd};
use crate::commands::resolve_token;
use crate::config::AppConfig;
use crate::error::{ChainError, Result};
use crate::output::{OutputMode, TableRenderable};
use crate::store::QuoteStore;

pub async fn handle(
    cmd: TokenCmd,
    config: &AppConfig,
    _store: &QuoteStore,
    api: &ApiClients,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match cmd.action {
        TokenAction::Info(args) => info(args, api, config, onchain, output_mode).await,
        TokenAction::Contract(args) => contract(args, onchain, output_mode).await,
    }
}

async fn info(
    args: crate::cli::token::TokenIdentArg,
    api: &ApiClients,
    config: &AppConfig,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_client = OnChainClient::for_chain(config, args.chain_id).await?;
    let onchain = &chain_client;
    let token_ref = match resolve_token(&args.token, args.chain_id, onchain, api, config).await {
        Ok(t) => t,
        Err(e) => {
            return Ok(
                crate::output::print_output::<crate::models::token::TokenInfo>(
                    Err(e),
                    "token.info",
                    output_mode,
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
                ),
            );
        }
    };
    Ok(
        crate::output::print_output::<crate::models::token::TokenInfo>(
            Ok(info),
            "token.info",
            output_mode,
        ),
    )
}

async fn contract(
    args: crate::cli::token::TokenIdentArg,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let addr: Address = match args.token.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<
                crate::models::token::TokenContract,
            >(
                Err(ChainError::InvalidAddress(args.token.clone())),
                "token.contract",
                output_mode,
            ));
        }
    };
    match crate::chain::get_token_info(onchain, addr).await {
        Ok(_) => {
            let tc = crate::models::token::TokenContract {
                address: args.token.clone(),
                is_proxy: false,
                proxy_implementation: None,
                owner: None,
                deployer: None,
                deployed_at_block: None,
                is_verified: None,
            };
            Ok(crate::output::print_output::<
                crate::models::token::TokenContract,
            >(Ok(tc), "token.contract", output_mode))
        }
        Err(e) => Ok(crate::output::print_output::<
            crate::models::token::TokenContract,
        >(Err(e), "token.contract", output_mode)),
    }
}
