use alloy::primitives::Address;
use chrono::Utc;
use std::process::ExitCode;

use crate::api::ApiClients;
use crate::chain::{get_allowance, get_eth_balance, AddressRef, OnChainClient};
use crate::cli::risk::{RiskAction, RiskCmd};
use crate::commands::resolve_token;
use crate::config::AppConfig;
use crate::error::{ChainError, Result};
use crate::models::risk::{ApprovalRisk, RiskLevel, RiskReport, RiskSignal};
use crate::output::{OutputContext, OutputMode};
use crate::store::QuoteStore;

fn unsupported_on(vm: &str, command: &str) -> ChainError {
    ChainError::Config(format!(
        "{command} is not supported on {vm} — no comparable data source"
    ))
}

pub async fn handle(
    cmd: RiskCmd,
    config: &AppConfig,
    store: &QuoteStore,
    api: &ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match cmd.action {
        RiskAction::Token(args) => token_risk(args, api, config, store, output_mode).await,
        RiskAction::Wallet(args) => wallet_risk(args, config, output_mode).await,
        RiskAction::Approval(args) => approval_risk(args, api, config, store, output_mode).await,
    }
}

async fn token_risk(
    args: crate::cli::risk::TokenRiskArgs,
    api: &ApiClients,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match AddressRef::parse(&args.token) {
        Ok(AddressRef::Svm(mint)) => return token_risk_svm(&mint, api, output_mode).await,
        Ok(AddressRef::Bvm(_)) => {
            return Ok(crate::output::print_output::<RiskReport>(
                Err(unsupported_on("Bitcoin mainnet", "risk token")),
                "risk.token",
                output_mode,
                OutputContext::new(config.chain_id, false),
            ));
        }
        _ => {}
    }

    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_client;
    let token_ref = match resolve_token(&args.token, chain_id, onchain, api, config, store).await {
        Ok(t) => t,
        Err(e) => {
            return Ok(crate::output::print_output::<RiskReport>(
                Err(e),
                "risk.token",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };
    let addr: Address = match token_ref.address.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<RiskReport>(
                Err(ChainError::InvalidAddress(token_ref.address.clone())),
                "risk.token",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };
    let info = match crate::chain::get_token_info(onchain, addr).await {
        Ok(i) => i,
        Err(e) => {
            return Ok(crate::output::print_output::<RiskReport>(
                Err(e),
                "risk.token",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let mut signals = Vec::new();
    let mut overall_risk = RiskLevel::Low;

    if info.decimals == 0 {
        signals.push(RiskSignal {
            signal: "zero_decimals".to_string(),
            description: "Token has 0 decimals, unusual for fungible tokens".to_string(),
            severity: RiskLevel::Medium,
            value: serde_json::json!({ "decimals": 0 }),
        });
        if matches!(overall_risk, RiskLevel::Low) {
            overall_risk = RiskLevel::Medium;
        }
    }

    let report = RiskReport {
        subject: token_ref.address,
        subject_type: "token".to_string(),
        overall_risk,
        signals,
        metadata: serde_json::json!({
            "symbol": info.symbol,
            "name": info.name,
            "decimals": info.decimals,
        }),
        analyzed_at: Utc::now(),
    };

    Ok(crate::output::print_output::<RiskReport>(
        Ok(report),
        "risk.token",
        output_mode,
        OutputContext::new(chain_id, false),
    ))
}

/// SVM token risk: GoPlus Solana token_security. Produces a `RiskReport`
/// from the per-authority signals (mintable/freezable/closable/transfer
/// fee/hook) the EVM version derives from honeypot/blacklist fields.
async fn token_risk_svm(
    mint: &str,
    api: &ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let symbol = api
        .jupiter
        .token(mint)
        .await
        .ok()
        .flatten()
        .map(|t| t.symbol)
        .unwrap_or_default();
    let token_risk = api.token_metadata.fetch_risk_svm(mint, &symbol).await;

    let overall_risk = match token_risk.risk_level.as_deref() {
        Some("high") => RiskLevel::High,
        Some("medium") => RiskLevel::Medium,
        Some("critical") => RiskLevel::Critical,
        Some("low") => RiskLevel::Low,
        _ => RiskLevel::Low,
    };

    let mut signals = Vec::new();
    if token_risk.mintable == Some(true) {
        signals.push(RiskSignal {
            signal: "mint_authority_set".to_string(),
            description: "SPL mint authority is active; supply can be inflated".to_string(),
            severity: RiskLevel::Medium,
            value: serde_json::json!({"mintable": true}),
        });
    }
    if token_risk.owner_privileged == Some(true) {
        signals.push(RiskSignal {
            signal: "freeze_or_close_authority_set".to_string(),
            description:
                "Token has an active freeze or close authority; issuer can disrupt holders"
                    .to_string(),
            severity: RiskLevel::Medium,
            value: serde_json::json!({"owner_privileged": true}),
        });
    }
    if token_risk.transfer_restricted == Some(true) {
        signals.push(RiskSignal {
            signal: "transfer_restricted".to_string(),
            description:
                "Token enforces a transfer hook, fee, or non-transferable flag".to_string(),
            severity: RiskLevel::High,
            value: serde_json::json!({
                "transfer_restricted": true,
                "tax_buy_pct": token_risk.tax_buy,
            }),
        });
    }

    let report = RiskReport {
        subject: mint.to_string(),
        subject_type: "token".to_string(),
        overall_risk,
        signals,
        metadata: serde_json::json!({
            "symbol": symbol,
            "chain": "Solana",
            "source": "goplus",
        }),
        analyzed_at: Utc::now(),
    };

    Ok(crate::output::print_output::<RiskReport>(
        Ok(report),
        "risk.token",
        output_mode,
        OutputContext::new(0, false),
    ))
}

async fn wallet_risk(
    args: crate::cli::risk::WalletRiskArgs,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let ctx = OutputContext::new(chain_id, false);

    match AddressRef::parse(&args.address) {
        Ok(AddressRef::Svm(_)) => return wallet_risk_svm(&args.address, output_mode).await,
        Ok(AddressRef::Bvm(_)) => {
            return Ok(crate::output::print_output::<RiskReport>(
                Err(unsupported_on("Bitcoin mainnet", "risk wallet")),
                "risk.wallet",
                output_mode,
                ctx,
            ));
        }
        Ok(AddressRef::Evm(_)) => {}
        Err(e) => {
            return Ok(crate::output::print_output::<RiskReport>(
                Err(e),
                "risk.wallet",
                output_mode,
                ctx,
            ));
        }
    }

    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_client;
    let addr: Address = match args.address.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<RiskReport>(
                Err(ChainError::InvalidAddress(args.address.clone())),
                "risk.wallet",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let (eth_balance_raw, eth_balance_display) = match get_eth_balance(onchain, addr).await {
        Ok(pair) => pair,
        Err(e) => {
            return Ok(crate::output::print_output::<RiskReport>(
                Err(e),
                "risk.wallet",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let overall_risk = if eth_balance_display < 0.01 {
        RiskLevel::High
    } else if eth_balance_display < 0.1 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    };

    let report = RiskReport {
        subject: args.address.clone(),
        subject_type: "wallet".to_string(),
        overall_risk,
        signals: vec![],
        metadata: serde_json::json!({
            "eth_balance": eth_balance_raw,
            "eth_balance_display": eth_balance_display,
        }),
        analyzed_at: Utc::now(),
    };

    Ok(crate::output::print_output::<RiskReport>(
        Ok(report),
        "risk.wallet",
        output_mode,
        OutputContext::new(chain_id, false),
    ))
}

/// SVM wallet risk: GoPlus and other providers don't index per-address
/// behavioral risk on Solana the way they do on EVM, and the EVM "ETH
/// balance < 0.01" heuristic doesn't translate. We return a low-severity
/// report with no signals plus an informational metadata field so the
/// command stays callable without inventing fake signals.
async fn wallet_risk_svm(address: &str, output_mode: OutputMode) -> Result<ExitCode> {
    let report = RiskReport {
        subject: address.to_string(),
        subject_type: "wallet".to_string(),
        overall_risk: RiskLevel::Low,
        signals: vec![],
        metadata: serde_json::json!({
            "chain": "Solana",
            "note": "wallet risk on Solana is metadata-only; no behavioral signals available",
        }),
        analyzed_at: Utc::now(),
    };
    Ok(crate::output::print_output::<RiskReport>(
        Ok(report),
        "risk.wallet",
        output_mode,
        OutputContext::new(0, false),
    ))
}

async fn approval_risk(
    args: crate::cli::risk::ApprovalRiskArgs,
    api: &ApiClients,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let ctx = OutputContext::new(chain_id, false);

    // ERC-20 approvals are an EVM-only concept; SPL tokens use delegate
    // accounts and Bitcoin has no approval primitive. Reject both early
    // before doing any address parsing or RPC work.
    for (raw, label) in [(args.address.as_str(), "owner"), (args.spender.as_str(), "spender")] {
        match AddressRef::parse(raw) {
            Ok(AddressRef::Svm(_)) => {
                return Ok(crate::output::print_output::<ApprovalRisk>(
                    Err(unsupported_on(
                        "Solana",
                        &format!("risk approval ({label} is a Solana address)"),
                    )),
                    "risk.approval",
                    output_mode,
                    ctx,
                ));
            }
            Ok(AddressRef::Bvm(_)) => {
                return Ok(crate::output::print_output::<ApprovalRisk>(
                    Err(unsupported_on(
                        "Bitcoin mainnet",
                        &format!("risk approval ({label} is a Bitcoin address)"),
                    )),
                    "risk.approval",
                    output_mode,
                    ctx,
                ));
            }
            _ => {}
        }
    }

    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_client;
    let owner: Address = match args.address.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<ApprovalRisk>(
                Err(ChainError::InvalidAddress(args.address.clone())),
                "risk.approval",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };
    let spender: Address = match args.spender.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<ApprovalRisk>(
                Err(ChainError::InvalidAddress(args.spender.clone())),
                "risk.approval",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let token_ref = match resolve_token(&args.token, chain_id, onchain, api, config, store).await {
        Ok(t) => t,
        Err(e) => {
            return Ok(crate::output::print_output::<ApprovalRisk>(
                Err(e),
                "risk.approval",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let token_addr: Address = match token_ref.address.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<ApprovalRisk>(
                Err(ChainError::InvalidAddress(token_ref.address.clone())),
                "risk.approval",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let allowance_raw = match get_allowance(onchain, token_addr, owner, spender).await {
        Ok(a) => a,
        Err(e) => {
            return Ok(crate::output::print_output::<ApprovalRisk>(
                Err(e),
                "risk.approval",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let is_unlimited = allowance_raw
        .parse::<u128>()
        .map(|a| a >= u128::MAX / 2)
        .unwrap_or(false);

    let risk = if is_unlimited {
        RiskLevel::Critical
    } else {
        RiskLevel::Low
    };

    let signals = vec![RiskSignal {
        signal: if is_unlimited {
            "unlimited_approval"
        } else {
            "has_approval"
        }
        .to_string(),
        description: if is_unlimited {
            "Token approval is set to unlimited - spender can spend all tokens".to_string()
        } else {
            "Approval amount is set".to_string()
        },
        severity: risk.clone(),
        value: serde_json::json!({ "allowance": allowance_raw, "is_unlimited": is_unlimited }),
    }];

    let approval_risk = ApprovalRisk {
        address: args.address.clone(),
        spender: args.spender.clone(),
        token_address: token_ref.address,
        token_symbol: token_ref.symbol,
        current_allowance: allowance_raw,
        is_unlimited,
        risk,
        signals,
        analyzed_at: Utc::now(),
    };

    Ok(crate::output::print_output::<ApprovalRisk>(
        Ok(approval_risk),
        "risk.approval",
        output_mode,
        OutputContext::new(chain_id, false),
    ))
}
