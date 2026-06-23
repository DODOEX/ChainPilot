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
        RiskAction::Wallet(args) => wallet_risk(args, api, config, output_mode).await,
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
    api: &ApiClients,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let ctx = OutputContext::new(chain_id, false);

    match AddressRef::parse(&args.address) {
        Ok(AddressRef::Svm(_)) => return wallet_risk_svm(&args.address, api, output_mode).await,
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

    let balance_level = if eth_balance_display < 0.01 {
        RiskLevel::High
    } else if eth_balance_display < 0.1 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    };

    // Augment the native-balance heuristic with GoPlus address reputation
    // (sanctions, phishing, drainer, mixer, …). `Some(chain_id)` scopes the
    // lookup to this EVM chain; `None` when GoPlus has no record.
    let reputation = api
        .token_metadata
        .fetch_address_security(&args.address, Some(chain_id))
        .await;
    let signals = reputation
        .as_ref()
        .map(address_reputation_signals)
        .unwrap_or_default();

    // Overall is the more severe of the balance heuristic and any reputation
    // signal, so a funded-but-flagged wallet isn't mislabeled `LOW`.
    let overall_risk =
        std::cmp::max_by_key(balance_level, overall_from_signals(&signals), severity_rank);

    let report = RiskReport {
        subject: args.address.clone(),
        subject_type: "wallet".to_string(),
        overall_risk,
        signals,
        metadata: serde_json::json!({
            "eth_balance": eth_balance_raw,
            "eth_balance_display": eth_balance_display,
            "reputation_source": reputation.as_ref().map(|_| "goplus"),
            "flagged": reputation.as_ref().map(|r| !r.is_clean()),
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

/// SVM wallet risk: GoPlus's malicious-address library is chain-agnostic, so
/// it flags Solana addresses it has seen tied to sanctions, phishing, drainer
/// (stealing) attacks, mixers, etc. We map those reputation flags to signals.
/// The EVM "ETH balance < 0.01" heuristic doesn't translate and is dropped.
/// When GoPlus has no record (or the request fails) we still return a callable
/// low-severity report rather than erroring.
async fn wallet_risk_svm(
    address: &str,
    api: &ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    // `None` chain_id: the malicious-address library keys on the raw address,
    // so a Solana base58 pubkey resolves without an EVM chain context.
    let security = api.token_metadata.fetch_address_security(address, None).await;

    let report = match &security {
        Some(sec) => {
            let signals = address_reputation_signals(sec);
            let overall_risk = overall_from_signals(&signals);
            RiskReport {
                subject: address.to_string(),
                subject_type: "wallet".to_string(),
                overall_risk,
                signals,
                metadata: serde_json::json!({
                    "chain": "Solana",
                    "source": "goplus",
                    "flagged": !sec.is_clean(),
                }),
                analyzed_at: Utc::now(),
            }
        }
        None => RiskReport {
            subject: address.to_string(),
            subject_type: "wallet".to_string(),
            overall_risk: RiskLevel::Low,
            signals: vec![],
            metadata: serde_json::json!({
                "chain": "Solana",
                "source": "goplus",
                "note": "GoPlus has no malicious-address record for this address",
            }),
            analyzed_at: Utc::now(),
        },
    };

    Ok(crate::output::print_output::<RiskReport>(
        Ok(report),
        "risk.wallet",
        output_mode,
        OutputContext::new(0, false),
    ))
}

/// Numeric rank for picking the most severe level across signals. `RiskLevel`
/// intentionally doesn't derive `Ord` (its serialization is the contract), so
/// we rank explicitly here.
fn severity_rank(level: &RiskLevel) -> u8 {
    match level {
        RiskLevel::Low => 0,
        RiskLevel::Medium => 1,
        RiskLevel::High => 2,
        RiskLevel::Critical => 3,
    }
}

/// Overall risk is the highest-severity signal present, or `Low` when the
/// address is clean (GoPlus had a record but flagged nothing).
fn overall_from_signals(signals: &[RiskSignal]) -> RiskLevel {
    signals
        .iter()
        .max_by_key(|s| severity_rank(&s.severity))
        .map(|s| s.severity.clone())
        .unwrap_or(RiskLevel::Low)
}

/// Map GoPlus address-reputation flags to risk signals. Chain-neutral (the
/// malicious-address library is keyed on the raw address), so it serves both
/// EVM and SVM wallet risk. Pure (no I/O) so the severity classification is
/// unit-testable. Sanctions, theft/drainer, and honeypot ties are treated as
/// the gravest; financial-crime categories as high; softer suspicions (fake
/// KYC, generic blacklist doubt) as medium.
fn address_reputation_signals(sec: &crate::api::AddressSecurity) -> Vec<RiskSignal> {
    // (flag, signal id, description, severity)
    let table: &[(bool, &str, &str, RiskLevel)] = &[
        (
            sec.sanctioned,
            "sanctioned",
            "Address appears on a sanctions list",
            RiskLevel::Critical,
        ),
        (
            sec.stealing_attack,
            "stealing_attack",
            "Address is associated with token-stealing (drainer) attacks",
            RiskLevel::Critical,
        ),
        (
            sec.honeypot_related_address,
            "honeypot_related_address",
            "Address is linked to honeypot tokens",
            RiskLevel::Critical,
        ),
        (
            sec.phishing_activities,
            "phishing_activities",
            "Address has been involved in phishing activities",
            RiskLevel::High,
        ),
        (
            sec.blackmail_activities,
            "blackmail_activities",
            "Address has been involved in blackmail activities",
            RiskLevel::High,
        ),
        (
            sec.cybercrime,
            "cybercrime",
            "Address is associated with cybercrime",
            RiskLevel::High,
        ),
        (
            sec.money_laundering,
            "money_laundering",
            "Address is associated with money laundering",
            RiskLevel::High,
        ),
        (
            sec.financial_crime,
            "financial_crime",
            "Address is associated with financial crime",
            RiskLevel::High,
        ),
        (
            sec.darkweb_transactions,
            "darkweb_transactions",
            "Address has transacted with darkweb services",
            RiskLevel::High,
        ),
        (
            sec.malicious_mining_activities,
            "malicious_mining_activities",
            "Address is associated with malicious mining activities",
            RiskLevel::High,
        ),
        (
            sec.mixer,
            "mixer",
            "Address is associated with a coin mixer",
            RiskLevel::High,
        ),
        (
            sec.fake_kyc,
            "fake_kyc",
            "Address is associated with fake KYC",
            RiskLevel::Medium,
        ),
        (
            sec.blacklist_doubt,
            "blacklist_doubt",
            "Address is suspected of malicious behavior (blacklist doubt)",
            RiskLevel::Medium,
        ),
    ];

    table
        .iter()
        .filter(|(flagged, _, _, _)| *flagged)
        .map(|(_, id, desc, sev)| RiskSignal {
            signal: (*id).to_string(),
            description: (*desc).to_string(),
            severity: sev.clone(),
            value: serde_json::json!({ "flagged": true }),
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::AddressSecurity;

    #[test]
    fn severity_rank_is_monotonic() {
        // The EVM path merges balance level and reputation level via
        // `max_by_key(severity_rank)`, so this ordering is load-bearing.
        assert!(severity_rank(&RiskLevel::Low) < severity_rank(&RiskLevel::Medium));
        assert!(severity_rank(&RiskLevel::Medium) < severity_rank(&RiskLevel::High));
        assert!(severity_rank(&RiskLevel::High) < severity_rank(&RiskLevel::Critical));
    }

    #[test]
    fn clean_address_has_no_signals_and_low_overall() {
        let sec = AddressSecurity::default();
        let signals = address_reputation_signals(&sec);
        assert!(signals.is_empty());
        assert!(sec.is_clean());
        assert!(matches!(overall_from_signals(&signals), RiskLevel::Low));
    }

    #[test]
    fn sanctioned_address_is_critical() {
        let sec = AddressSecurity {
            sanctioned: true,
            ..Default::default()
        };
        let signals = address_reputation_signals(&sec);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].signal, "sanctioned");
        assert!(!sec.is_clean());
        assert!(matches!(overall_from_signals(&signals), RiskLevel::Critical));
    }

    #[test]
    fn overall_picks_highest_severity_across_mixed_flags() {
        // phishing (High) + fake_kyc (Medium) -> overall High.
        let sec = AddressSecurity {
            phishing_activities: true,
            fake_kyc: true,
            ..Default::default()
        };
        let signals = address_reputation_signals(&sec);
        assert_eq!(signals.len(), 2);
        assert!(matches!(overall_from_signals(&signals), RiskLevel::High));
    }
}
