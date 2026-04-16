use std::process::ExitCode;

use crate::api::ApiClients;
use crate::cli::Commands;
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
    match cmd {
        Commands::Swap(cmd) => swap::handle(cmd, &config, &store, &api_clients, output_mode).await,
        Commands::Token(cmd) => {
            token::handle(cmd, &config, &store, &api_clients, output_mode).await
        }
        Commands::Wallet(cmd) => {
            wallet::handle(cmd, &config, &store, &api_clients, output_mode).await
        }
        Commands::Risk(cmd) => risk::handle(cmd, &config, &store, &api_clients, output_mode).await,
    }
}

pub async fn resolve_token(
    input: &str,
    chain_id: u64,
    onchain: &OnChainClient,
    api: &ApiClients,
    config: &crate::config::AppConfig,
    store: &QuoteStore,
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

    // 4. Custom token store fallback when tokenlist does not contain the symbol.
    if let Some(token) = store.find_custom_token_by_symbol(chain_id, &upper)? {
        return Ok(token);
    }

    Err(crate::error::ChainError::TokenNotFound(input.to_string()).into())
}

pub fn parse_display_amount(amount: &str) -> Result<f64> {
    amount
        .trim()
        .parse::<f64>()
        .map_err(|_| crate::error::ChainError::InvalidAmount(amount.to_string()))
}

pub fn to_raw_amount(amount: &str, decimals: u8) -> Result<String> {
    let amount = amount.trim();
    if amount.is_empty() {
        return Err(
            crate::error::ChainError::InvalidAmount("amount cannot be empty".to_string()).into(),
        );
    }
    if amount.starts_with('-') {
        return Err(crate::error::ChainError::InvalidAmount(
            "amount cannot be negative".to_string(),
        )
        .into());
    }

    let mut parts = amount.split('.');
    let int_part = parts.next().unwrap_or_default();
    let frac_part = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return Err(crate::error::ChainError::InvalidAmount(amount.to_string()).into());
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(crate::error::ChainError::InvalidAmount(amount.to_string()).into());
    }
    if frac_part.len() > decimals as usize {
        return Err(crate::error::ChainError::InvalidAmount(format!(
            "too many decimal places for token precision {}",
            decimals
        ))
        .into());
    }

    let int_normalized = int_part.trim_start_matches('0');
    let frac_padded = format!("{:0<width$}", frac_part, width = decimals as usize);
    let combined = format!(
        "{}{}",
        if int_normalized.is_empty() {
            "0"
        } else {
            int_normalized
        },
        frac_padded
    );
    let normalized = combined.trim_start_matches('0');
    let raw = if normalized.is_empty() {
        "0"
    } else {
        normalized
    };
    raw.parse::<u128>()
        .map_err(|_| crate::error::ChainError::InvalidAmount(amount.to_string()))?;
    Ok(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_raw_amount_parses_decimal_strings_exactly() {
        assert_eq!(to_raw_amount("1", 18).unwrap(), "1000000000000000000");
        assert_eq!(to_raw_amount("0.25", 18).unwrap(), "250000000000000000");
        assert_eq!(to_raw_amount("12.34", 6).unwrap(), "12340000");
    }

    #[test]
    fn to_raw_amount_rejects_excess_precision() {
        let err = to_raw_amount("0.1234567", 6).unwrap_err();
        assert!(err.to_string().contains("too many decimal places"));
    }

    #[test]
    fn amount_parsing_cases_are_table_driven() {
        struct Case<'a> {
            input: &'a str,
            decimals: u8,
            expected_raw: Option<&'a str>,
            expected_err: Option<&'a str>,
        }

        for case in [
            Case {
                input: "0001.2300",
                decimals: 6,
                expected_raw: Some("1230000"),
                expected_err: None,
            },
            Case {
                input: ".5",
                decimals: 18,
                expected_raw: Some("500000000000000000"),
                expected_err: None,
            },
            Case {
                input: " 42 ",
                decimals: 0,
                expected_raw: Some("42"),
                expected_err: None,
            },
            Case {
                input: "",
                decimals: 18,
                expected_raw: None,
                expected_err: Some("amount cannot be empty"),
            },
            Case {
                input: "-1",
                decimals: 18,
                expected_raw: None,
                expected_err: Some("amount cannot be negative"),
            },
            Case {
                input: "1.2.3",
                decimals: 18,
                expected_raw: None,
                expected_err: Some("Invalid amount"),
            },
        ] {
            let result = to_raw_amount(case.input, case.decimals);
            match (result, case.expected_raw, case.expected_err) {
                (Ok(raw), Some(expected), None) => assert_eq!(raw, expected),
                (Err(err), None, Some(expected)) => {
                    assert!(err.to_string().contains(expected), "{}", err)
                }
                other => panic!("unexpected result shape: {:?}", other),
            }
        }
    }

    #[test]
    fn parse_display_amount_trims_valid_input_and_rejects_invalid() {
        assert_eq!(parse_display_amount(" 0.25 ").unwrap(), 0.25);
        let err = parse_display_amount("abc").unwrap_err();
        assert!(err.to_string().contains("Invalid amount"));
    }
}
