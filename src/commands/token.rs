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
    store: &QuoteStore,
    api: &ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match cmd.action {
        TokenAction::Info(args) => info(args, api, config, store, output_mode).await,
        TokenAction::Contract(args) => contract(args, api, config, store, output_mode).await,
        TokenAction::Price(args) => price(args, api, config, store, output_mode).await,
        TokenAction::Add(args) => add(args, api, config, store, output_mode).await,
    }
}

async fn info(
    args: crate::cli::token::TokenIdentArg,
    api: &ApiClients,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_client;
    let token_ref = match resolve_token(&args.token, chain_id, onchain, api, config, store).await {
        Ok(t) => t,
        Err(ChainError::TokenNotFound(_)) => {
            let search = api
                .token_metadata
                .search_symbol(&args.token, chain_id)
                .await;
            if search.candidates.is_empty() {
                return Ok(
                    crate::output::print_output::<crate::models::token::TokenInfo>(
                        Err(ChainError::TokenNotFound(args.token)),
                        "token.info",
                        output_mode,
                        OutputContext::new(chain_id, false),
                    ),
                );
            }
            return Ok(crate::output::print_output::<
                crate::models::token::TokenSearchResult,
            >(
                Ok(search),
                "token.search",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
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
    let info = api.token_metadata.enrich(info).await;
    Ok(
        crate::output::print_output::<crate::models::token::TokenInfo>(
            Ok(info),
            "token.info",
            output_mode,
            OutputContext::new(chain_id, false),
        ),
    )
}

async fn price(
    args: crate::cli::token::TokenIdentArg,
    api: &ApiClients,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_client;
    let token_ref = match resolve_token(&args.token, chain_id, onchain, api, config, store).await {
        Ok(t) => t,
        Err(ChainError::TokenNotFound(_)) => {
            let search = api
                .token_metadata
                .search_symbol(&args.token, chain_id)
                .await;
            if search.candidates.is_empty() {
                return Ok(crate::output::print_output::<
                    crate::models::token::TokenPrice,
                >(
                    Err(ChainError::TokenNotFound(args.token)),
                    "token.price",
                    output_mode,
                    OutputContext::new(chain_id, false),
                ));
            }
            return Ok(crate::output::print_output::<
                crate::models::token::TokenSearchResult,
            >(
                Ok(search),
                "token.search",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
        Err(e) => {
            return Ok(crate::output::print_output::<
                crate::models::token::TokenPrice,
            >(
                Err(e),
                "token.price",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };
    let price = api
        .token_metadata
        .fetch_price(chain_id, &token_ref.address, &token_ref.symbol)
        .await;
    Ok(crate::output::print_output::<
        crate::models::token::TokenPrice,
    >(
        Ok(price),
        "token.price",
        output_mode,
        OutputContext::new(chain_id, false),
    ))
}

async fn contract(
    args: crate::cli::token::TokenIdentArg,
    api: &ApiClients,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_client;
    let token_ref = match resolve_token(&args.token, chain_id, onchain, api, config, store).await {
        Ok(t) => t,
        Err(e) => {
            return Ok(crate::output::print_output::<
                crate::models::token::TokenContract,
            >(
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
            return Ok(crate::output::print_output::<
                crate::models::token::TokenContract,
            >(
                Err(ChainError::InvalidAddress(token_ref.address.clone())),
                "token.contract",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };
    match crate::chain::inspect_token_contract(onchain, addr).await {
        Ok(contract) => Ok(crate::output::print_output::<
            crate::models::token::TokenContract,
        >(
            Ok(contract),
            "token.contract",
            output_mode,
            OutputContext::new(chain_id, false),
        )),
        Err(e) => Ok(crate::output::print_output::<
            crate::models::token::TokenContract,
        >(
            Err(e),
            "token.contract",
            output_mode,
            OutputContext::new(chain_id, false),
        )),
    }
}

async fn add(
    args: crate::cli::token::TokenAddArgs,
    _api: &ApiClients,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_client;

    let addr: Address = match args.address.parse() {
        Ok(addr) => addr,
        Err(_) => {
            return Ok(crate::output::print_output::<
                crate::models::token::CustomTokenRecord,
            >(
                Err(ChainError::InvalidAddress(args.address)),
                "token.add",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let info = match crate::chain::get_token_info(onchain, addr).await {
        Ok(info) => info,
        Err(e) => {
            return Ok(crate::output::print_output::<
                crate::models::token::CustomTokenRecord,
            >(
                Err(e),
                "token.add",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let record = match store.save_custom_token_info(&info) {
        Ok(record) => record,
        Err(e) => {
            return Ok(crate::output::print_output::<
                crate::models::token::CustomTokenRecord,
            >(
                Err(e),
                "token.add",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    Ok(crate::output::print_output::<
        crate::models::token::CustomTokenRecord,
    >(
        Ok(record),
        "token.add",
        output_mode,
        OutputContext::new(chain_id, false),
    ))
}
