use alloy::primitives::Address;
use chrono::Utc;
use std::process::ExitCode;

use crate::api::ApiClients;
use crate::chain::{get_allowance, get_eth_balance, OnChainClient};
use crate::cli::risk::{RiskAction, RiskCmd};
use crate::commands::resolve_token;
use crate::config::AppConfig;
use crate::error::{ChainError, Result};
use crate::models::risk::{ApprovalRisk, RiskLevel, RiskReport, RiskSignal};
use crate::output::{OutputMode, TableRenderable};
use crate::store::QuoteStore;

pub async fn handle(
    cmd: RiskCmd,
    config: &AppConfig,
    _store: &QuoteStore,
    api: &ApiClients,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match cmd.action {
        RiskAction::Token(args) => token_risk(args, api, config, onchain, output_mode).await,
        RiskAction::Wallet(args) => wallet_risk(args, config, onchain, output_mode).await,
        RiskAction::Approval(args) => approval_risk(args, api, config, onchain, output_mode).await,
    }
}

async fn token_risk(
    args: crate::cli::risk::TokenRiskArgs,
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
            return Ok(crate::output::print_output::<RiskReport>(
                Err(e),
                "risk.token",
                output_mode,
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
            ));
        }
    };

    let mut signals = Vec::new();
    let mut overall_risk = RiskLevel::Low;

    if info.total_supply_display < 1_000_000.0 {
        signals.push(RiskSignal {
            signal: "low_total_supply".to_string(),
            description: "Total supply is very low, may indicate thin liquidity".to_string(),
            severity: RiskLevel::High,
            value: serde_json::json!({ "supply": info.total_supply_display }),
        });
        overall_risk = RiskLevel::High;
    }

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
    ))
}

async fn wallet_risk(
    args: crate::cli::risk::WalletRiskArgs,
    config: &AppConfig,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_client = OnChainClient::for_chain(config, args.chain_id).await?;
    let onchain = &chain_client;
    let addr: Address = match args.address.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<RiskReport>(
                Err(ChainError::InvalidAddress(args.address.clone())),
                "risk.wallet",
                output_mode,
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
    ))
}

async fn approval_risk(
    args: crate::cli::risk::ApprovalRiskArgs,
    api: &ApiClients,
    config: &AppConfig,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_client = OnChainClient::for_chain(config, args.chain_id).await?;
    let onchain = &chain_client;
    let owner: Address = match args.address.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<ApprovalRisk>(
                Err(ChainError::InvalidAddress(args.address.clone())),
                "risk.approval",
                output_mode,
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
            ));
        }
    };

    let token_ref = match resolve_token(&args.token, args.chain_id, onchain, api, config).await {
        Ok(t) => t,
        Err(e) => {
            return Ok(crate::output::print_output::<ApprovalRisk>(
                Err(e),
                "risk.approval",
                output_mode,
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
    ))
}
