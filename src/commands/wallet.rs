use alloy::primitives::Address;
use std::process::ExitCode;

use crate::api::ApiClients;
use crate::chain::{get_balance, get_eth_balance, OnChainClient};
use crate::cli::wallet::{BalanceArgs, WalletAction, WalletCmd};
use crate::commands::resolve_token;
use crate::config::AppConfig;
use crate::error::{ChainError, Result};
use crate::models::wallet::{TokenBalance, WalletBalance};
use crate::output::{OutputContext, OutputMode};
use crate::store::QuoteStore;

pub async fn handle(
    cmd: WalletCmd,
    config: &AppConfig,
    store: &QuoteStore,
    api: &ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match cmd.action {
        WalletAction::Balance(args) => balance(args, api, config, store, output_mode).await,
    }
}

async fn balance(
    args: BalanceArgs,
    api: &ApiClients,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_client;
    let addr: Address = match args.address.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<WalletBalance>(
                Err(ChainError::InvalidAddress(args.address.clone())),
                "wallet.balance",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let (eth_balance_raw, eth_balance_display) = match get_eth_balance(onchain, addr).await {
        Ok(pair) => pair,
        Err(e) => {
            return Ok(crate::output::print_output::<WalletBalance>(
                Err(e),
                "wallet.balance",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let mut token_balances = Vec::new();

    if let Some(tokens_str) = &args.tokens {
        for token_input in tokens_str.split(',') {
            let token_ref = match resolve_token(
                token_input.trim(),
                chain_id,
                onchain,
                api,
                config,
                store,
            )
            .await
            {
                Ok(t) => t,
                Err(e) => {
                    return Ok(crate::output::print_output::<WalletBalance>(
                        Err(e),
                        "wallet.balance",
                        output_mode,
                        OutputContext::new(chain_id, false),
                    ));
                }
            };
            let token_addr: Address = match token_ref.address.parse() {
                Ok(a) => a,
                Err(_) => {
                    return Ok(crate::output::print_output::<WalletBalance>(
                        Err(ChainError::InvalidAddress(token_ref.address.clone())),
                        "wallet.balance",
                        output_mode,
                        OutputContext::new(chain_id, false),
                    ));
                }
            };
            let (balance_raw, decimals) = match get_balance(onchain, token_addr, addr).await {
                Ok(pair) => pair,
                Err(e) => {
                    return Ok(crate::output::print_output::<WalletBalance>(
                        Err(e),
                        "wallet.balance",
                        output_mode,
                        OutputContext::new(chain_id, false),
                    ));
                }
            };
            let balance_display = parse_token_amount(&balance_raw, decimals);
            token_balances.push(TokenBalance {
                token_address: token_ref.address,
                symbol: token_ref.symbol.clone(),
                name: token_ref.symbol,
                balance: balance_raw,
                balance_display,
                balance_usd: None,
                decimals,
            });
        }
    }

    let wallet_balance = WalletBalance {
        address: args.address.clone(),
        chain_id,
        eth_balance: eth_balance_raw,
        eth_balance_display,
        eth_balance_usd: None,
        token_balances,
        total_usd: None,
    };

    Ok(crate::output::print_output::<WalletBalance>(
        Ok(wallet_balance),
        "wallet.balance",
        output_mode,
        OutputContext::new(chain_id, false),
    ))
}

fn parse_token_amount(raw: &str, decimals: u8) -> f64 {
    let raw_uint: u128 = raw.parse().unwrap_or(0);
    let divisor = 10u128.pow(decimals as u32) as f64;
    raw_uint as f64 / divisor
}

#[cfg(test)]
mod tests {
    use super::parse_token_amount;

    #[test]
    fn parse_token_amount_cases_are_table_driven() {
        for (raw, decimals, expected) in [
            ("0", 18, 0.0),
            ("1234567", 6, 1.234567),
            ("42", 0, 42.0),
            ("not-a-number", 18, 0.0),
        ] {
            assert_eq!(parse_token_amount(raw, decimals), expected);
        }
    }
}
