use std::process::ExitCode;

use crate::api::ApiClients;
use crate::cli::{Cli, Commands};
use crate::config::AppConfig;
use crate::error::Result;
use crate::output::OutputMode;
use crate::store::QuoteStore;

pub mod risk;
pub mod swap;
pub mod token;
pub mod wallet;

use crate::chain::OnChainClient;

pub async fn dispatch(
    cmd: Commands,
    config: AppConfig,
    store: QuoteStore,
    api_clients: ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let onchain = OnChainClient::new(&config).await?;

    match cmd {
        Commands::Swap(cmd) => {
            swap::handle(cmd, &config, &store, &api_clients, &onchain, output_mode).await
        }
        Commands::Token(cmd) => {
            token::handle(cmd, &config, &store, &api_clients, &onchain, output_mode).await
        }
        Commands::Wallet(cmd) => {
            wallet::handle(cmd, &config, &store, &api_clients, &onchain, output_mode).await
        }
        Commands::Risk(cmd) => {
            risk::handle(cmd, &config, &store, &api_clients, &onchain, output_mode).await
        }
    }
}

pub async fn resolve_token(
    input: &str,
    chain_id: u64,
    onchain: &OnChainClient,
    api: &ApiClients,
    config: &crate::config::AppConfig,
) -> Result<crate::models::quote::TokenRef> {
    use alloy::primitives::Address;

    // 0. Reject unsupported chains up front for a clear error message.
    if crate::config::chain_config(chain_id).is_none() {
        return Err(crate::error::ChainError::UnsupportedChain(chain_id).into());
    }

    // 1. Address input → native sentinel or on-chain lookup.
    if let Ok(addr) = input.parse::<Address>() {
        let addr_lower = addr.to_string().to_lowercase();
        if addr_lower == crate::config::chains::NATIVE_ADDR.to_lowercase() {
            let (symbol, decimals) = crate::config::chain_config(chain_id)
                .map(|cc| (cc.native_token.symbol, cc.native_token.decimals))
                .unwrap_or(("ETH", 18));
            return Ok(crate::models::quote::TokenRef {
                symbol: symbol.to_string(),
                address: crate::config::chains::NATIVE_ADDR.to_string(),
                decimals,
                chain_id,
            });
        }
        let info = crate::chain::get_token_info(onchain, addr).await?;
        return Ok(crate::models::quote::TokenRef {
            symbol: info.symbol.clone(),
            address: info.address.clone(),
            decimals: info.decimals,
            chain_id,
        });
    }

    let upper = input.to_uppercase();

    // 2. Native token symbol (e.g. "ETH", "BNB", "MATIC").
    if let Some(cc) = crate::config::chain_config(chain_id) {
        if upper == cc.native_token.symbol.to_uppercase() {
            return Ok(crate::models::quote::TokenRef {
                symbol: cc.native_token.symbol.to_string(),
                address: crate::config::chains::NATIVE_ADDR.to_string(),
                decimals: cc.native_token.decimals,
                chain_id,
            });
        }

        // 2.5 Wrapped native token symbol (e.g. WETH, WBNB, WMATIC).
        if upper == cc.native_token.wrapped_symbol.to_uppercase() {
            return Ok(crate::models::quote::TokenRef {
                symbol: cc.native_token.wrapped_symbol.to_string(),
                address: cc.native_token.wrapped_address.to_string(),
                decimals: cc.native_token.decimals,
                chain_id,
            });
        }
    }

    // 3. ERC-20 symbol → tokenlist API (with disk cache).
    if let Some(token) = api
        .dodo
        .find_token_by_symbol(chain_id, &upper, &config.tokenlist_cache_path())
        .await?
    {
        return Ok(token);
    }

    Err(crate::error::ChainError::TokenNotFound(input.to_string()).into())
}

pub fn to_raw_amount(amount: f64, decimals: u8) -> String {
    let multiplier = 10u128.pow(decimals as u32) as f64;
    let raw = (amount * multiplier) as u128;
    raw.to_string()
}
