use alloy::primitives::Address;
use std::process::ExitCode;

use crate::api::{
    debank_chain_to_id, ApiClients, GoldrushChainBalance, ZerionPortfolio, ZerionPositionRecord,
};
use crate::chain::{get_eth_balance, OnChainClient};
use crate::cli::wallet::{
    BalanceArgs, DefiArgs, HistoryArgs, LabelsArgs, OverviewArgs, PnlArgs, WalletAction, WalletCmd,
};
use crate::config::AppConfig;
use crate::error::{ChainError, Result};
use crate::models::wallet::{
    ActiveProtocol, ChainAllocation, DefiPosition, DefiPositionToken, LabelScore, TokenAllocation,
    TopHolding, WalletAsset, WalletBalance, WalletBalanceSources, WalletDefi, WalletHistory,
    WalletLabels, WalletOverview, WalletOverviewSources, WalletPnl, WalletTransaction,
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// Cap on concurrent per-chain Goldrush `balances_v2` requests. Goldrush
/// free-tier accounts get rate-limited (429) well below 17 simultaneous
/// requests, so we keep the burst small even though each token still finishes
/// the job — fan-out latency degrades gracefully instead of failing wholesale.
const GOLDRUSH_FAN_OUT_CONCURRENCY: usize = 5;
use crate::output::{OutputContext, OutputMode};
use crate::store::QuoteStore;

pub async fn handle(
    cmd: WalletCmd,
    config: &AppConfig,
    _store: &QuoteStore,
    api: &ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match cmd.action {
        WalletAction::Balance(args) => balance(args, api, config, output_mode).await,
        WalletAction::Overview(args) => overview(args, api, config, output_mode).await,
        WalletAction::Pnl(args) => pnl(args, api, config, output_mode).await,
        WalletAction::History(args) => history(args, api, config, output_mode).await,
        WalletAction::Labels(args) => labels(args, api, config, output_mode).await,
        WalletAction::Defi(args) => defi(args, api, config, output_mode).await,
    }
}

async fn balance(
    args: BalanceArgs,
    api: &ApiClients,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let ctx = OutputContext::new(chain_id, false);

    if args.address.parse::<Address>().is_err() {
        return Ok(crate::output::print_output::<WalletBalance>(
            Err(ChainError::InvalidAddress(args.address.clone())),
            "wallet.balance",
            output_mode,
            ctx,
        ));
    }

    let result = build_balance(&args, api, config).await;
    Ok(crate::output::print_output::<WalletBalance>(
        result,
        "wallet.balance",
        output_mode,
        ctx,
    ))
}

async fn build_balance(
    args: &BalanceArgs,
    api: &ApiClients,
    config: &AppConfig,
) -> Result<WalletBalance> {
    // When set, --chain-id scopes the *entire* response (assets, chain_allocation,
    // total_balance_usd) to that single chain. All data sources honor this.
    let chain_filter = config.chain_id_overridden.then_some(config.chain_id);
    let mut last_err: Option<ChainError> = None;
    if api.debank.is_configured() {
        match fetch_balance_from_debank(args, api, chain_filter).await {
            Ok(balance) => return Ok(balance),
            Err(e) => {
                tracing::warn!("Debank balance lookup failed: {e}. Falling back to Zerion.");
                last_err = Some(e);
            }
        }
    }
    if api.zerion.is_configured() {
        match fetch_balance_from_zerion(args, api, chain_filter).await {
            Ok(balance) => return Ok(balance),
            Err(e) => {
                tracing::warn!("Zerion balance lookup failed: {e}. Falling back to Goldrush.");
                last_err = Some(e);
            }
        }
    }
    if api.goldrush.is_configured() {
        match fetch_balance_from_goldrush(args, api, config).await {
            Ok(balance) => return Ok(balance),
            Err(e) => {
                tracing::warn!("Goldrush balance lookup failed: {e}. Falling back to on-chain.");
                last_err = Some(e);
            }
        }
    }
    // On-chain only surfaces native-token amount; if every aggregator failed,
    // it is more useful to bubble the real upstream error than to silently
    // hide it behind a partial on-chain answer.
    match fetch_balance_from_onchain(&args.address, config).await {
        Ok(balance) => Ok(balance),
        Err(onchain_err) => Err(last_err.unwrap_or(onchain_err)),
    }
}

async fn fetch_balance_from_debank(
    args: &BalanceArgs,
    api: &ApiClients,
    chain_filter: Option<u64>,
) -> Result<WalletBalance> {
    let total = api.debank.total_balance(&args.address).await?;
    let tokens = api.debank.all_token_list(&args.address).await?;

    let assets: Vec<WalletAsset> = tokens
        .into_iter()
        .filter(|t| chain_filter.is_none_or(|id| t.chain_id == Some(id)))
        .filter(|t| t.value_usd.unwrap_or(0.0) >= args.min_usd)
        .map(|t| WalletAsset {
            chain: t.chain,
            chain_id: t.chain_id,
            symbol: t.symbol,
            name: t.name,
            address: t.address,
            amount: t.amount,
            price_usd: t.price_usd,
            value_usd: t.value_usd,
        })
        .collect();

    let filtered_chains: Vec<_> = total
        .chains
        .into_iter()
        .filter(|c| debank_chain_matches(&c.id, c.community_id, chain_filter))
        .filter(|c| c.usd_value > 0.0)
        .collect();

    // When a chain filter is active, the upstream total_usd_value covers all
    // chains and is no longer meaningful; recompute from the filtered subset.
    let chain_total: f64 = if chain_filter.is_some() {
        filtered_chains.iter().map(|c| c.usd_value).sum()
    } else {
        total
            .total_usd_value
            .unwrap_or_else(|| filtered_chains.iter().map(|c| c.usd_value).sum())
    };

    let chain_allocation: Vec<ChainAllocation> = filtered_chains
        .into_iter()
        .map(|c| ChainAllocation {
            chain: c.name,
            chain_id: c.community_id.or_else(|| debank_chain_to_id(&c.id)),
            balance_usd: c.usd_value,
            percentage: percentage(c.usd_value, chain_total),
        })
        .collect();

    let total_balance_usd = if chain_filter.is_some() {
        Some(chain_total)
    } else {
        total.total_usd_value
    };

    Ok(WalletBalance {
        wallet: args.address.clone(),
        total_balance_usd,
        assets,
        chain_allocation,
        sources: WalletBalanceSources {
            total_balance_usd: Some("debank".to_string()),
            assets: Some("debank".to_string()),
            chain_allocation: Some("debank".to_string()),
        },
    })
}

async fn fetch_balance_from_goldrush(
    args: &BalanceArgs,
    api: &ApiClients,
    config: &AppConfig,
) -> Result<WalletBalance> {
    // resolve_goldrush_chains already honors --chain-id by returning a single-chain
    // set, so every field below naturally scopes to that one chain — no extra
    // filtering needed here.
    let chains = resolve_goldrush_chains(api, config, &args.address).await;
    let queried = chains.len();
    let chain_balances = goldrush_fan_out(&api.goldrush, &args.address, chains).await;

    if chain_balances.is_empty() {
        return Err(ChainError::Config(format!(
            "Goldrush returned no data across {queried} queried chain(s)"
        )));
    }

    let chain_total: f64 = chain_balances
        .iter()
        .map(GoldrushChainBalance::total_usd)
        .sum();

    let assets: Vec<WalletAsset> = chain_balances
        .iter()
        .flat_map(|cb| cb.items.iter().map(goldrush_asset_to_wallet_asset))
        .filter(|a| a.value_usd.unwrap_or(0.0) >= args.min_usd)
        .collect();

    let chain_allocation: Vec<ChainAllocation> = chain_balances
        .iter()
        .map(|cb| {
            let usd = cb.total_usd();
            ChainAllocation {
                chain: cb.chain_name.clone(),
                chain_id: Some(cb.chain_id),
                balance_usd: usd,
                percentage: percentage(usd, chain_total),
            }
        })
        .filter(|c| c.balance_usd > 0.0)
        .collect();

    Ok(WalletBalance {
        wallet: args.address.clone(),
        total_balance_usd: Some(chain_total),
        assets,
        chain_allocation,
        sources: WalletBalanceSources {
            total_balance_usd: Some("goldrush".to_string()),
            assets: Some("goldrush".to_string()),
            chain_allocation: Some("goldrush".to_string()),
        },
    })
}

async fn fetch_balance_from_zerion(
    args: &BalanceArgs,
    api: &ApiClients,
    chain_filter: Option<u64>,
) -> Result<WalletBalance> {
    // Fetch portfolio + positions concurrently — they hit independent endpoints
    // and together drive every field below.
    let (portfolio, positions) = tokio::join!(
        api.zerion.portfolio(&args.address),
        api.zerion.positions(&args.address, true, chain_filter),
    );
    let portfolio = portfolio?;
    let positions = positions?;

    let chains = portfolio_chains_for_filter(&portfolio, chain_filter);
    let chain_total = chains_total(&chains, &portfolio, chain_filter);

    let assets: Vec<WalletAsset> = positions
        .into_iter()
        .filter(|p| p.position_type == "wallet" || p.protocol.is_none())
        .filter(|p| p.value_usd.unwrap_or(0.0) >= args.min_usd)
        .map(zerion_position_to_asset)
        .collect();

    let chain_allocation = chains_to_allocation(&chains, chain_total);

    let total_balance_usd = if chain_filter.is_some() {
        Some(chain_total)
    } else {
        portfolio.total_usd
    };

    Ok(WalletBalance {
        wallet: args.address.clone(),
        total_balance_usd,
        assets,
        chain_allocation,
        sources: WalletBalanceSources {
            total_balance_usd: Some("zerion".to_string()),
            assets: Some("zerion".to_string()),
            chain_allocation: Some("zerion".to_string()),
        },
    })
}

async fn fetch_balance_from_onchain(address: &str, config: &AppConfig) -> Result<WalletBalance> {
    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let addr: Address = address
        .parse()
        .map_err(|_| ChainError::InvalidAddress(address.to_string()))?;
    let (_eth_raw, eth_display) = get_eth_balance(&chain_client, addr).await?;

    let chain_name = crate::config::chain_config(chain_id)
        .map(|c| c.name.to_string())
        .unwrap_or_else(|| format!("chain-{chain_id}"));
    let native_symbol = crate::config::chain_config(chain_id)
        .map(|c| c.native_token.symbol.to_string())
        .unwrap_or_else(|| "ETH".to_string());

    let mut assets = Vec::new();
    if eth_display > 0.0 {
        assets.push(WalletAsset {
            chain: chain_name,
            chain_id: Some(chain_id),
            symbol: native_symbol.clone(),
            name: native_symbol,
            address: crate::config::chains::NATIVE_ADDR.to_string(),
            amount: eth_display,
            price_usd: None,
            value_usd: None,
        });
    }

    // Without a price oracle we cannot fill USD values, so chain_allocation
    // and total_balance_usd stay empty/None rather than reporting zeros.
    Ok(WalletBalance {
        wallet: address.to_string(),
        total_balance_usd: None,
        assets,
        chain_allocation: Vec::new(),
        sources: WalletBalanceSources {
            total_balance_usd: None,
            assets: Some("onchain".to_string()),
            chain_allocation: None,
        },
    })
}

async fn overview(
    args: OverviewArgs,
    api: &ApiClients,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let ctx = OutputContext::new(chain_id, false);

    if args.address.parse::<Address>().is_err() {
        return Ok(crate::output::print_output::<WalletOverview>(
            Err(ChainError::InvalidAddress(args.address.clone())),
            "wallet.overview",
            output_mode,
            ctx,
        ));
    }

    let result = build_overview(&args, api, config).await;
    Ok(crate::output::print_output::<WalletOverview>(
        result,
        "wallet.overview",
        output_mode,
        ctx,
    ))
}

async fn build_overview(
    args: &OverviewArgs,
    api: &ApiClients,
    config: &AppConfig,
) -> Result<WalletOverview> {
    let chain_filter = config.chain_id_overridden.then_some(config.chain_id);
    let mut last_err: Option<ChainError> = None;
    if api.debank.is_configured() {
        match build_overview_from_debank(args, api, chain_filter).await {
            Ok(overview) => return Ok(overview),
            Err(e) => {
                tracing::warn!("Debank overview lookup failed: {e}. Falling back to Zerion.");
                last_err = Some(e);
            }
        }
    }
    if api.zerion.is_configured() {
        match build_overview_from_zerion(args, api, chain_filter).await {
            Ok(overview) => return Ok(overview),
            Err(e) => {
                tracing::warn!("Zerion overview lookup failed: {e}. Falling back to Goldrush.");
                last_err = Some(e);
            }
        }
    }
    if api.goldrush.is_configured() {
        match build_overview_from_goldrush(args, api, config).await {
            Ok(overview) => return Ok(overview),
            Err(e) => {
                tracing::warn!("Goldrush overview lookup failed: {e}.");
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        ChainError::Config(
            "wallet overview requires Debank, Zerion, or Goldrush. Run: \
         chainpilot config set debank_api_key <key> (preferred), \
         chainpilot config set zerion_api_key <key>, or \
         chainpilot config set goldrush_api_key <key>"
                .to_string(),
        )
    }))
}

async fn build_overview_from_debank(
    args: &OverviewArgs,
    api: &ApiClients,
    chain_filter: Option<u64>,
) -> Result<WalletOverview> {
    let total = api.debank.total_balance(&args.address).await?;
    let tokens = api.debank.all_token_list(&args.address).await?;
    let protocols = api
        .debank
        .all_complex_protocol_list(&args.address)
        .await
        .unwrap_or_default();

    // --chain-id scopes every aggregation below to one chain. Filter the raw
    // Debank lists up-front so each derived field stays consistent.
    let filtered_chain_summaries: Vec<_> = total
        .chains
        .into_iter()
        .filter(|c| debank_chain_matches(&c.id, c.community_id, chain_filter))
        .collect();
    let filtered_tokens: Vec<_> = tokens
        .into_iter()
        .filter(|t| chain_filter.is_none_or(|id| t.chain_id == Some(id)))
        .collect();
    let filtered_protocols: Vec<_> = protocols
        .into_iter()
        .filter(|p| debank_chain_matches(&p.chain, None, chain_filter))
        .collect();

    let total_usd: f64 = if chain_filter.is_some() {
        filtered_chain_summaries.iter().map(|c| c.usd_value).sum()
    } else {
        total
            .total_usd_value
            .unwrap_or_else(|| filtered_chain_summaries.iter().map(|c| c.usd_value).sum())
    };

    let chain_allocation: Vec<ChainAllocation> = filtered_chain_summaries
        .into_iter()
        .filter(|c| c.usd_value > 0.0)
        .map(|c| ChainAllocation {
            chain: c.name,
            chain_id: c.community_id.or_else(|| debank_chain_to_id(&c.id)),
            balance_usd: c.usd_value,
            percentage: percentage(c.usd_value, total_usd),
        })
        .collect();

    let token_allocation = aggregate_token_allocation(&filtered_tokens, total_usd);

    let top_holdings = build_top_holdings(&filtered_tokens, args.top);

    let mut active_protocols: Vec<ActiveProtocol> = filtered_protocols
        .into_iter()
        .filter(|p| p.net_usd_value.unwrap_or(0.0) > 0.0)
        .map(|p| ActiveProtocol {
            name: p.name,
            chain: p.chain,
            net_usd_value: p.net_usd_value,
            site_url: p.site_url,
        })
        .collect();
    active_protocols.sort_by(|a, b| {
        b.net_usd_value
            .unwrap_or(0.0)
            .partial_cmp(&a.net_usd_value.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total_balance_usd = if chain_filter.is_some() {
        Some(total_usd)
    } else {
        total.total_usd_value
    };

    Ok(WalletOverview {
        wallet: args.address.clone(),
        total_balance_usd,
        chain_allocation,
        token_allocation,
        active_protocols,
        top_holdings,
        sources: WalletOverviewSources {
            total_balance_usd: Some("debank".to_string()),
            chain_allocation: Some("debank".to_string()),
            token_allocation: Some("debank".to_string()),
            active_protocols: Some("debank".to_string()),
            top_holdings: Some("debank".to_string()),
        },
    })
}

async fn build_overview_from_goldrush(
    args: &OverviewArgs,
    api: &ApiClients,
    config: &AppConfig,
) -> Result<WalletOverview> {
    let chains = resolve_goldrush_chains(api, config, &args.address).await;
    let queried = chains.len();
    let chain_balances = goldrush_fan_out(&api.goldrush, &args.address, chains).await;

    if chain_balances.is_empty() {
        return Err(ChainError::Config(format!(
            "Goldrush returned no data across {queried} queried chain(s)"
        )));
    }

    let chain_total: f64 = chain_balances
        .iter()
        .map(GoldrushChainBalance::total_usd)
        .sum();

    let chain_allocation: Vec<ChainAllocation> = chain_balances
        .iter()
        .map(|cb| {
            let usd = cb.total_usd();
            ChainAllocation {
                chain: cb.chain_name.clone(),
                chain_id: Some(cb.chain_id),
                balance_usd: usd,
                percentage: percentage(usd, chain_total),
            }
        })
        .filter(|c| c.balance_usd > 0.0)
        .collect();

    let all_assets: Vec<WalletAsset> = chain_balances
        .iter()
        .flat_map(|cb| cb.items.iter().map(goldrush_asset_to_wallet_asset))
        .collect();
    let token_allocation = aggregate_token_allocation_from_assets(&all_assets, chain_total);
    let top_holdings = build_top_holdings_from_assets(&all_assets, args.top);

    Ok(WalletOverview {
        wallet: args.address.clone(),
        total_balance_usd: Some(chain_total),
        chain_allocation,
        token_allocation,
        active_protocols: Vec::new(),
        top_holdings,
        sources: WalletOverviewSources {
            total_balance_usd: Some("goldrush".to_string()),
            chain_allocation: Some("goldrush".to_string()),
            token_allocation: Some("goldrush".to_string()),
            // Goldrush has no protocol-interaction index, so this stays unsourced.
            active_protocols: None,
            top_holdings: Some("goldrush".to_string()),
        },
    })
}

async fn build_overview_from_zerion(
    args: &OverviewArgs,
    api: &ApiClients,
    chain_filter: Option<u64>,
) -> Result<WalletOverview> {
    // `no_filter` returns DeFi positions too, which is what `active_protocols`
    // is built from. Portfolio gives us total + per-chain breakdown.
    let (portfolio, positions) = tokio::join!(
        api.zerion.portfolio(&args.address),
        api.zerion.positions(&args.address, false, chain_filter),
    );
    let portfolio = portfolio?;
    let positions = positions?;

    let chains = portfolio_chains_for_filter(&portfolio, chain_filter);
    let chain_total = chains_total(&chains, &portfolio, chain_filter);

    let chain_allocation = chains_to_allocation(&chains, chain_total);

    // Wallet positions feed token_allocation / top_holdings; non-wallet feed protocols.
    // Partition strictly on position_type: a DeFi position with a missing
    // protocol slug must still appear in `active_protocols` (we'll fall back
    // to the position name when bucketing).
    let (wallet_positions, defi_positions): (Vec<_>, Vec<_>) = positions
        .into_iter()
        .partition(|p| p.position_type == "wallet");

    let wallet_assets: Vec<WalletAsset> = wallet_positions
        .into_iter()
        .map(zerion_position_to_asset)
        .collect();
    let token_allocation = aggregate_token_allocation_from_assets(&wallet_assets, chain_total);
    let top_holdings = build_top_holdings_from_assets(&wallet_assets, args.top);

    let active_protocols = aggregate_zerion_protocols(defi_positions);

    let total_balance_usd = if chain_filter.is_some() {
        Some(chain_total)
    } else {
        portfolio.total_usd
    };

    Ok(WalletOverview {
        wallet: args.address.clone(),
        total_balance_usd,
        chain_allocation,
        token_allocation,
        active_protocols,
        top_holdings,
        sources: WalletOverviewSources {
            total_balance_usd: Some("zerion".to_string()),
            chain_allocation: Some("zerion".to_string()),
            token_allocation: Some("zerion".to_string()),
            active_protocols: Some("zerion".to_string()),
            top_holdings: Some("zerion".to_string()),
        },
    })
}

fn aggregate_token_allocation_from_assets(
    assets: &[WalletAsset],
    total_usd: f64,
) -> Vec<TokenAllocation> {
    use std::collections::HashMap;
    let mut bucket: HashMap<String, (String, f64)> = HashMap::new();
    for a in assets {
        let Some(value) = a.value_usd else { continue };
        if value <= 0.0 {
            continue;
        }
        let entry = bucket
            .entry(a.symbol.to_uppercase())
            .or_insert_with(|| (a.name.clone(), 0.0));
        entry.1 += value;
    }
    let mut allocs: Vec<TokenAllocation> = bucket
        .into_iter()
        .map(|(symbol, (name, value))| TokenAllocation {
            symbol,
            name,
            balance_usd: value,
            percentage: percentage(value, total_usd),
        })
        .collect();
    allocs.sort_by(|a, b| {
        b.balance_usd
            .partial_cmp(&a.balance_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    allocs
}

fn build_top_holdings_from_assets(assets: &[WalletAsset], limit: usize) -> Vec<TopHolding> {
    let mut holdings: Vec<TopHolding> = assets
        .iter()
        .filter_map(|a| {
            let value = a.value_usd?;
            if value <= 0.0 {
                return None;
            }
            Some(TopHolding {
                symbol: a.symbol.clone(),
                name: a.name.clone(),
                chain: a.chain.clone(),
                amount: a.amount,
                value_usd: value,
            })
        })
        .collect();
    holdings.sort_by(|a, b| {
        b.value_usd
            .partial_cmp(&a.value_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    holdings.truncate(limit.max(1));
    holdings
}

fn aggregate_token_allocation(
    tokens: &[crate::api::DebankAssetRecord],
    total_usd: f64,
) -> Vec<TokenAllocation> {
    use std::collections::HashMap;
    let mut bucket: HashMap<String, (String, f64)> = HashMap::new();
    for t in tokens {
        let Some(value) = t.value_usd else { continue };
        if value <= 0.0 {
            continue;
        }
        let entry = bucket
            .entry(t.symbol.to_uppercase())
            .or_insert_with(|| (t.name.clone(), 0.0));
        entry.1 += value;
    }
    let mut allocs: Vec<TokenAllocation> = bucket
        .into_iter()
        .map(|(symbol, (name, value))| TokenAllocation {
            symbol,
            name,
            balance_usd: value,
            percentage: percentage(value, total_usd),
        })
        .collect();
    allocs.sort_by(|a, b| {
        b.balance_usd
            .partial_cmp(&a.balance_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    allocs
}

fn build_top_holdings(tokens: &[crate::api::DebankAssetRecord], limit: usize) -> Vec<TopHolding> {
    let mut holdings: Vec<TopHolding> = tokens
        .iter()
        .filter_map(|t| {
            let value = t.value_usd?;
            if value <= 0.0 {
                return None;
            }
            Some(TopHolding {
                symbol: t.symbol.clone(),
                name: t.name.clone(),
                chain: t.chain.clone(),
                amount: t.amount,
                value_usd: value,
            })
        })
        .collect();
    holdings.sort_by(|a, b| {
        b.value_usd
            .partial_cmp(&a.value_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    holdings.truncate(limit.max(1));
    holdings
}

fn percentage(value: f64, total: f64) -> f64 {
    if total > 0.0 {
        (value / total) * 100.0
    } else {
        0.0
    }
}

/// True when a Debank-side entry matches the active `--chain-id` filter.
/// `community_id` is the EVM chain id when Debank surfaces it; we fall back
/// to mapping the chain slug (`eth`, `bsc`, …) for older response shapes.
fn debank_chain_matches(slug: &str, community_id: Option<u64>, filter: Option<u64>) -> bool {
    match filter {
        None => true,
        Some(id) => community_id == Some(id) || debank_chain_to_id(slug) == Some(id),
    }
}

/// Decide which chain IDs to query through Goldrush.
///
/// Precedence:
/// 1. Explicit `--chain-id` → only that chain. `--chain-id` is a global
///    "scope everything to this chain" flag; downstream callers rely on the
///    returned set to also drive `chain_allocation`, `total_balance_usd`, etc.
/// 2. `/address/{addr}/activity/` → intersect the returned chains with our
///    supported set. One extra HTTP call usually saves many no-op
///    balances_v2 requests on chains the wallet has never touched.
/// 3. If activity is empty or fails, fall back to every supported chain so
///    we still return data when the cheap path is unavailable.
async fn resolve_goldrush_chains(api: &ApiClients, config: &AppConfig, address: &str) -> Vec<u64> {
    if config.chain_id_overridden {
        return vec![config.chain_id];
    }

    let supported: std::collections::HashSet<u64> =
        crate::config::chains::all_chain_ids().collect();

    match api.goldrush.active_chains(address).await {
        Ok(active) => {
            let scoped: Vec<u64> = active
                .into_iter()
                .filter(|c| supported.contains(c))
                .collect();
            if !scoped.is_empty() {
                return scoped;
            }
            tracing::debug!(
                "Goldrush activity returned no supported chains for {address}; \
                 falling back to all supported chains"
            );
            supported.into_iter().collect()
        }
        Err(e) => {
            tracing::warn!(
                "Goldrush activity lookup failed: {e}. Falling back to all supported chains."
            );
            supported.into_iter().collect()
        }
    }
}

/// Issue a Goldrush balances_v2 request per chain in parallel; failed chains
/// are dropped silently so a single unsupported chain does not fail the call.
/// A semaphore caps in-flight requests to [`GOLDRUSH_FAN_OUT_CONCURRENCY`] so
/// large chain sets don't burst-trigger Goldrush's per-second rate limit.
async fn goldrush_fan_out(
    client: &crate::api::GoldrushClient,
    address: &str,
    chains: Vec<u64>,
) -> Vec<GoldrushChainBalance> {
    let semaphore = Arc::new(Semaphore::new(GOLDRUSH_FAN_OUT_CONCURRENCY));
    let mut set: JoinSet<Result<GoldrushChainBalance>> = JoinSet::new();
    for chain_id in chains {
        let client = client.clone();
        let address = address.to_string();
        let permit = semaphore.clone();
        set.spawn(async move {
            // Hold the permit for the entire request; dropped on task exit.
            let _permit = permit
                .acquire_owned()
                .await
                .expect("goldrush semaphore was never closed");
            client.balances_v2(chain_id, &address).await
        });
    }
    let mut out = Vec::new();
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(balance)) => out.push(balance),
            Ok(Err(e)) => tracing::debug!("Goldrush per-chain request failed: {e}"),
            Err(e) => tracing::debug!("Goldrush task panicked: {e}"),
        }
    }
    out
}

/// Filter Zerion's per-chain distribution by `--chain-id`, keeping the
/// original ordering. Returns owned slices so the caller can borrow freely.
fn portfolio_chains_for_filter(
    portfolio: &ZerionPortfolio,
    chain_filter: Option<u64>,
) -> Vec<&crate::api::ZerionChainBalance> {
    portfolio
        .chains
        .iter()
        .filter(|c| chain_filter.is_none_or(|id| c.chain_id == Some(id)))
        .collect()
}

/// Pick the total USD value to use for percentage math. When `--chain-id` is
/// active, the upstream `portfolio.total_usd` covers every chain so we recompute
/// from the filtered subset instead.
fn chains_total(
    filtered: &[&crate::api::ZerionChainBalance],
    portfolio: &ZerionPortfolio,
    chain_filter: Option<u64>,
) -> f64 {
    if chain_filter.is_some() {
        filtered.iter().map(|c| c.usd_value).sum()
    } else {
        portfolio
            .total_usd
            .unwrap_or_else(|| filtered.iter().map(|c| c.usd_value).sum())
    }
}

fn chains_to_allocation(
    chains: &[&crate::api::ZerionChainBalance],
    total: f64,
) -> Vec<ChainAllocation> {
    chains
        .iter()
        .map(|c| ChainAllocation {
            chain: c.slug.clone(),
            chain_id: c.chain_id,
            balance_usd: c.usd_value,
            percentage: percentage(c.usd_value, total),
        })
        .collect()
}

fn zerion_position_to_asset(p: ZerionPositionRecord) -> WalletAsset {
    WalletAsset {
        chain: p.chain_slug,
        chain_id: p.chain_id,
        symbol: p.symbol,
        name: p.name,
        address: p.address,
        amount: p.amount,
        price_usd: p.price_usd,
        value_usd: p.value_usd,
    }
}

/// Roll multiple per-chain positions of the same protocol into one entry,
/// sorted by net USD value descending. Positions with non-positive net value
/// are dropped; positions without a recognized protocol slug fall back to
/// `display_name` (Zerion's `attributes.name`, e.g. "Aave V3 USDC Deposit").
fn aggregate_zerion_protocols(positions: Vec<ZerionPositionRecord>) -> Vec<ActiveProtocol> {
    use std::collections::HashMap;
    let mut bucket: HashMap<String, (String, Option<String>, f64)> = HashMap::new();
    for p in positions {
        let value = p.value_usd.unwrap_or(0.0);
        if value <= 0.0 {
            continue;
        }
        let Some(label) = p.protocol.clone().or_else(|| p.display_name.clone()) else {
            continue;
        };
        let entry = bucket
            .entry(label)
            .or_insert_with(|| (p.chain_slug.clone(), p.protocol_url.clone(), 0.0));
        entry.2 += value;
    }
    let mut out: Vec<ActiveProtocol> = bucket
        .into_iter()
        .map(|(name, (chain, site_url, net))| ActiveProtocol {
            name,
            chain,
            net_usd_value: Some(net),
            site_url,
        })
        .collect();
    out.sort_by(|a, b| {
        b.net_usd_value
            .unwrap_or(0.0)
            .partial_cmp(&a.net_usd_value.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

fn goldrush_asset_to_wallet_asset(item: &crate::api::GoldrushAssetRecord) -> WalletAsset {
    let address = if item.is_native {
        crate::config::chains::NATIVE_ADDR.to_string()
    } else {
        item.address.clone()
    };
    WalletAsset {
        chain: item.chain.clone(),
        chain_id: Some(item.chain_id),
        symbol: item.symbol.clone(),
        name: item.name.clone(),
        address,
        amount: item.amount,
        price_usd: item.price_usd,
        value_usd: item.value_usd,
    }
}

async fn pnl(
    args: PnlArgs,
    api: &ApiClients,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let ctx = OutputContext::new(chain_id, false);

    if args.address.parse::<Address>().is_err() {
        return Ok(crate::output::print_output::<WalletPnl>(
            Err(ChainError::InvalidAddress(args.address.clone())),
            "wallet.pnl",
            output_mode,
            ctx,
        ));
    }

    let result = build_pnl(&args, api).await;
    Ok(crate::output::print_output::<WalletPnl>(
        result,
        "wallet.pnl",
        output_mode,
        ctx,
    ))
}

async fn build_pnl(args: &PnlArgs, api: &ApiClients) -> Result<WalletPnl> {
    if !api.zerion.is_configured() {
        return Err(ChainError::Config(
            "PnL requires ZERION_API_KEY. Run: chainpilot config set zerion_api_key <key>"
                .to_string(),
        ));
    }

    let record = api.zerion.pnl(&args.address).await?;
    Ok(WalletPnl {
        wallet: args.address.clone(),
        realized_pnl: record.realized_pnl,
        unrealized_pnl: record.unrealized_pnl,
        total_pnl: record.total_pnl,
        roi: record.roi,
        win_rate: record.win_rate,
        total_invested: record.total_invested,
        total_fee: record.total_fee,
        source: "zerion".to_string(),
    })
}

async fn history(
    args: HistoryArgs,
    api: &ApiClients,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let ctx = OutputContext::new(chain_id, false);

    if args.address.parse::<Address>().is_err() {
        return Ok(crate::output::print_output::<WalletHistory>(
            Err(ChainError::InvalidAddress(args.address.clone())),
            "wallet.history",
            output_mode,
            ctx,
        ));
    }

    let result = build_history(&args, api).await;
    Ok(crate::output::print_output::<WalletHistory>(
        result,
        "wallet.history",
        output_mode,
        ctx,
    ))
}

async fn build_history(args: &HistoryArgs, api: &ApiClients) -> Result<WalletHistory> {
    let limit = args.limit.clamp(1, 100);

    if api.zerion.is_configured() {
        match api.zerion.transactions(&args.address, limit).await {
            Ok(txs) => {
                return Ok(WalletHistory {
                    wallet: args.address.clone(),
                    transactions: txs
                        .into_iter()
                        .map(|t| WalletTransaction {
                            tx_hash: t.tx_hash,
                            time: t.time,
                            action: t.action,
                            status: t.status,
                            fee_usd: t.fee_usd,
                            token_in: t.token_in,
                            token_out: t.token_out,
                            value_usd: t.value_usd,
                            amount: t.amount,
                            success: t.success,
                        })
                        .collect(),
                    source: "zerion".to_string(),
                });
            }
            Err(e) => {
                tracing::warn!("Zerion transactions failed: {e}. Falling back to Debank.");
            }
        }
    }

    if api.debank.is_configured() {
        match api.debank.all_history_list(&args.address, limit).await {
            Ok(txs) => {
                return Ok(WalletHistory {
                    wallet: args.address.clone(),
                    transactions: txs
                        .into_iter()
                        .map(|t| WalletTransaction {
                            tx_hash: t.tx_hash,
                            time: t.time,
                            action: t.action,
                            status: None,
                            fee_usd: None,
                            token_in: t.token_in,
                            token_out: t.token_out,
                            value_usd: t.value_usd,
                            amount: t.amount,
                            success: t.success,
                        })
                        .collect(),
                    source: "debank".to_string(),
                });
            }
            Err(e) => {
                tracing::warn!("Debank history failed: {e}");
            }
        }
    }

    Err(ChainError::Config(
        "Transaction history requires ZERION_API_KEY or DEBANK_API_KEY. \
         Run: chainpilot config set zerion_api_key <key>"
            .to_string(),
    ))
}

async fn labels(
    args: LabelsArgs,
    api: &ApiClients,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let ctx = OutputContext::new(chain_id, false);

    if args.address.parse::<Address>().is_err() {
        return Ok(crate::output::print_output::<WalletLabels>(
            Err(ChainError::InvalidAddress(args.address.clone())),
            "wallet.labels",
            output_mode,
            ctx,
        ));
    }

    let result = build_labels(&args, api).await;
    Ok(crate::output::print_output::<WalletLabels>(
        result,
        "wallet.labels",
        output_mode,
        ctx,
    ))
}

async fn build_labels(args: &LabelsArgs, api: &ApiClients) -> Result<WalletLabels> {
    let mut last_err: Option<ChainError> = None;

    if api.zerion.is_configured() {
        match build_labels_from_zerion(args, api).await {
            Ok(labels) => return Ok(labels),
            Err(e) => {
                tracing::warn!("Zerion labels lookup failed: {e}. Falling back to Dune.");
                last_err = Some(e);
            }
        }
    }

    if api.dune.is_configured() {
        match build_labels_from_dune(args, api).await {
            Ok(labels) => return Ok(labels),
            Err(e) => {
                tracing::warn!("Dune labels lookup failed: {e}. Falling back to Zerion.");
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        ChainError::Config(
            "Wallet labels requires ZERION_API_KEY or DUNE_API_KEY. Run: \
         chainpilot config set zerion_api_key <key> (preferred), \
         or chainpilot config set dune_api_key <key>"
                .to_string(),
        )
    }))
}

async fn build_labels_from_dune(args: &LabelsArgs, api: &ApiClients) -> Result<WalletLabels> {
    let records = api.dune.wallet_labels(&args.address).await?;

    let labels: Vec<String> = records.iter().map(|r| r.label.clone()).collect();
    let label_scores: Vec<LabelScore> = records
        .into_iter()
        .map(|r| LabelScore {
            label: r.label,
            score: r.score,
            reason: r.reason,
        })
        .collect();

    Ok(WalletLabels {
        wallet: args.address.clone(),
        labels,
        label_scores,
        source: "dune".to_string(),
    })
}

async fn build_labels_from_zerion(args: &LabelsArgs, api: &ApiClients) -> Result<WalletLabels> {
    let records = api.zerion.wallet_labels(&args.address).await?;

    let labels: Vec<String> = records.iter().map(|r| r.label.clone()).collect();
    let label_scores: Vec<LabelScore> = records
        .into_iter()
        .map(|r| LabelScore {
            label: r.label,
            score: r.score,
            reason: r.reason,
        })
        .collect();

    Ok(WalletLabels {
        wallet: args.address.clone(),
        labels,
        label_scores,
        source: "zerion".to_string(),
    })
}

async fn defi(
    args: DefiArgs,
    api: &ApiClients,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let ctx = OutputContext::new(chain_id, false);

    if args.address.parse::<Address>().is_err() {
        return Ok(crate::output::print_output::<WalletDefi>(
            Err(ChainError::InvalidAddress(args.address.clone())),
            "wallet.defi",
            output_mode,
            ctx,
        ));
    }

    let result = build_defi(&args, api, config).await;
    Ok(crate::output::print_output::<WalletDefi>(
        result,
        "wallet.defi",
        output_mode,
        ctx,
    ))
}

async fn build_defi(args: &DefiArgs, api: &ApiClients, config: &AppConfig) -> Result<WalletDefi> {
    let chain_filter = config.chain_id_overridden.then_some(config.chain_id);
    let mut last_err: Option<ChainError> = None;

    if api.debank.is_configured() {
        match build_defi_from_debank(args, api, chain_filter).await {
            Ok(defi) => return Ok(defi),
            Err(e) => {
                tracing::warn!("Debank DeFi lookup failed: {e}. Falling back to Zerion.");
                last_err = Some(e);
            }
        }
    }

    if api.zerion.is_configured() {
        match build_defi_from_zerion(args, api, chain_filter).await {
            Ok(defi) => return Ok(defi),
            Err(e) => {
                tracing::warn!("Zerion DeFi lookup failed: {e}.");
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        ChainError::Config(
            "DeFi positions requires DEBANK_API_KEY or ZERION_API_KEY. Run: \
         chainpilot config set debank_api_key <key> (preferred) or \
         chainpilot config set zerion_api_key <key>"
                .to_string(),
        )
    }))
}

async fn build_defi_from_debank(
    args: &DefiArgs,
    api: &ApiClients,
    chain_filter: Option<u64>,
) -> Result<WalletDefi> {
    let raw = api.debank.defi_positions(&args.address).await?;

    let positions: Vec<DefiPosition> = raw
        .into_iter()
        .filter(|p| {
            chain_filter.is_none_or(|id| crate::api::debank_chain_to_id(&p.chain) == Some(id))
        })
        .filter(|p| p.value_usd.unwrap_or(0.0) >= args.min_usd)
        .map(|p| DefiPosition {
            protocol: p.protocol,
            position_name: p.position_name,
            chain: p.chain,
            value_usd: p.value_usd,
            tokens: p
                .tokens
                .into_iter()
                .map(|t| DefiPositionToken {
                    symbol: t.symbol,
                    amount: t.amount,
                })
                .collect(),
            position_type: p.position_type,
            site_url: p.site_url,
        })
        .collect();

    let total_value_usd = Some(positions.iter().map(|p| p.value_usd.unwrap_or(0.0)).sum());

    Ok(WalletDefi {
        wallet: args.address.clone(),
        total_value_usd,
        positions,
        source: "debank".to_string(),
    })
}

async fn build_defi_from_zerion(
    args: &DefiArgs,
    api: &ApiClients,
    chain_filter: Option<u64>,
) -> Result<WalletDefi> {
    // `no_filter` returns DeFi positions too (deposit, stake, lend, etc.).
    let positions = api
        .zerion
        .positions(&args.address, false, chain_filter)
        .await?;

    let positions = build_zerion_defi_positions(positions, args.min_usd);

    let total_value_usd = Some(positions.iter().map(|p| p.value_usd.unwrap_or(0.0)).sum());

    Ok(WalletDefi {
        wallet: args.address.clone(),
        total_value_usd,
        positions,
        source: "zerion".to_string(),
    })
}

fn build_zerion_defi_positions(
    positions: Vec<ZerionPositionRecord>,
    min_usd: f64,
) -> Vec<DefiPosition> {
    use std::collections::HashMap;

    #[derive(Default)]
    struct GroupedLp {
        protocol: Option<String>,
        position_name: Option<String>,
        chain: Option<String>,
        value_usd: f64,
        tokens: Vec<DefiPositionToken>,
        position_type: Option<String>,
        site_url: Option<String>,
    }

    let mut grouped_lp: HashMap<String, GroupedLp> = HashMap::new();
    let mut out = Vec::new();

    for p in positions.into_iter().filter(|p| p.position_type != "wallet") {
        if is_groupable_zerion_lp(&p) {
            let key = p.group_id.clone().unwrap_or_default();
            let entry = grouped_lp.entry(key).or_default();
            if entry.protocol.is_none() {
                entry.protocol = p.protocol.clone().or_else(|| p.display_name.clone());
            }
            if entry.position_name.is_none() {
                entry.position_name = p.display_name.clone().or(Some(p.name.clone()));
            }
            if entry.chain.is_none() {
                entry.chain = Some(p.chain_slug.clone());
            }
            entry.value_usd += p.value_usd.unwrap_or(0.0);
            if !p.symbol.is_empty() {
                entry.tokens.push(DefiPositionToken {
                    symbol: p.symbol.clone(),
                    amount: Some(p.amount),
                });
            }
            if entry.position_type.is_none() {
                entry.position_type = Some(p.position_type.clone());
            }
            if entry.site_url.is_none() {
                entry.site_url = p.protocol_url.clone();
            }
            continue;
        }

        let value = p.value_usd.unwrap_or(0.0);
        if value < min_usd {
            continue;
        }

        let mut tokens = Vec::new();
        if !p.symbol.is_empty() {
            tokens.push(DefiPositionToken {
                symbol: p.symbol.clone(),
                amount: Some(p.amount),
            });
        }
        out.push(DefiPosition {
            protocol: p
                .protocol
                .clone()
                .or_else(|| p.display_name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            position_name: p.display_name.clone().unwrap_or_else(|| p.name.clone()),
            chain: p.chain_slug,
            value_usd: p.value_usd,
            tokens,
            position_type: p.position_type,
            site_url: p.protocol_url,
        });
    }

    for lp in grouped_lp.into_values() {
        if lp.value_usd < min_usd {
            continue;
        }
        out.push(DefiPosition {
            protocol: lp.protocol.unwrap_or_else(|| "unknown".to_string()),
            position_name: lp.position_name.unwrap_or_else(|| "liquidity position".to_string()),
            chain: lp.chain.unwrap_or_default(),
            value_usd: Some(lp.value_usd),
            tokens: lp.tokens,
            position_type: lp.position_type.unwrap_or_else(|| "liquidity".to_string()),
            site_url: lp.site_url,
        });
    }

    out
}

fn is_groupable_zerion_lp(p: &ZerionPositionRecord) -> bool {
    if p.group_id.as_deref().is_none_or(str::is_empty) {
        return false;
    }

    matches!(
        p.position_type.as_str(),
        "liquidity" | "lp" | "liquidity_pool"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::DebankAssetRecord;

    fn asset(symbol: &str, chain: &str, amount: f64, value: f64) -> DebankAssetRecord {
        DebankAssetRecord {
            chain: chain.to_string(),
            chain_id: None,
            symbol: symbol.to_string(),
            name: symbol.to_string(),
            address: format!("0x{}", symbol),
            amount,
            price_usd: if amount > 0.0 {
                Some(value / amount)
            } else {
                None
            },
            value_usd: Some(value),
        }
    }

    #[test]
    fn percentage_handles_zero_total() {
        assert_eq!(percentage(0.0, 0.0), 0.0);
        assert_eq!(percentage(10.0, 0.0), 0.0);
        assert_eq!(percentage(25.0, 100.0), 25.0);
    }

    #[test]
    fn token_allocation_buckets_same_symbol_across_chains() {
        let tokens = vec![
            asset("USDC", "eth", 100.0, 100.0),
            asset("USDC", "base", 50.0, 50.0),
            asset("ETH", "eth", 1.0, 3000.0),
        ];
        let allocs = aggregate_token_allocation(&tokens, 3150.0);
        assert_eq!(allocs.len(), 2);
        assert_eq!(allocs[0].symbol, "ETH");
        assert!((allocs[0].balance_usd - 3000.0).abs() < 1e-9);
        assert_eq!(allocs[1].symbol, "USDC");
        assert!((allocs[1].balance_usd - 150.0).abs() < 1e-9);
    }

    #[test]
    fn top_holdings_sorted_by_value_and_truncated() {
        let tokens = vec![
            asset("A", "eth", 1.0, 10.0),
            asset("B", "eth", 1.0, 50.0),
            asset("C", "eth", 1.0, 30.0),
        ];
        let holdings = build_top_holdings(&tokens, 2);
        assert_eq!(holdings.len(), 2);
        assert_eq!(holdings[0].symbol, "B");
        assert_eq!(holdings[1].symbol, "C");
    }

    #[test]
    fn top_holdings_skips_zero_value() {
        let tokens = vec![asset("Z", "eth", 1.0, 0.0), asset("A", "eth", 1.0, 5.0)];
        let holdings = build_top_holdings(&tokens, 5);
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].symbol, "A");
    }

    #[test]
    fn debank_chain_matches_handles_filter_modes() {
        // No filter → everything matches regardless of slug/community_id.
        assert!(debank_chain_matches("eth", Some(1), None));
        assert!(debank_chain_matches("unknown", None, None));

        // Filter via community_id (the modern Debank shape).
        assert!(debank_chain_matches("base", Some(8453), Some(8453)));
        assert!(!debank_chain_matches("base", Some(8453), Some(1)));

        // Filter via slug fallback (older response shape lacking community_id).
        assert!(debank_chain_matches("matic", None, Some(137)));
        assert!(!debank_chain_matches("matic", None, Some(1)));

        // Neither slug nor community_id resolves → no match.
        assert!(!debank_chain_matches("rune", None, Some(1)));
    }

    fn zerion_position(
        symbol: &str,
        chain_slug: &str,
        value: f64,
        position_type: &str,
        protocol: Option<&str>,
    ) -> ZerionPositionRecord {
        ZerionPositionRecord {
            chain_slug: chain_slug.to_string(),
            chain_id: None,
            symbol: symbol.to_string(),
            name: symbol.to_string(),
            display_name: None,
            address: format!("0x{symbol}"),
            amount: value.max(0.0),
            price_usd: Some(1.0),
            value_usd: Some(value),
            group_id: None,
            position_type: position_type.to_string(),
            protocol: protocol.map(str::to_string),
            protocol_url: None,
        }
    }

    fn zerion_position_with_display_name(
        symbol: &str,
        chain_slug: &str,
        value: f64,
        position_type: &str,
        display_name: &str,
    ) -> ZerionPositionRecord {
        let mut p = zerion_position(symbol, chain_slug, value, position_type, None);
        p.display_name = Some(display_name.to_string());
        p
    }

    fn zerion_lp_position(
        symbol: &str,
        chain_slug: &str,
        value: f64,
        amount: f64,
        group_id: &str,
        protocol: Option<&str>,
    ) -> ZerionPositionRecord {
        let mut p = zerion_position(symbol, chain_slug, value, "liquidity", protocol);
        p.amount = amount;
        p.group_id = Some(group_id.to_string());
        p
    }

    #[test]
    fn aggregate_zerion_protocols_groups_by_name_and_sorts_by_value() {
        let positions = vec![
            zerion_position("USDC", "ethereum", 100.0, "deposit", Some("aave-v3")),
            zerion_position("USDC", "base", 200.0, "deposit", Some("aave-v3")),
            zerion_position("ETH", "ethereum", 500.0, "staked", Some("lido")),
            zerion_position("UNI", "ethereum", 50.0, "wallet", None),
        ];
        let protocols = aggregate_zerion_protocols(positions);
        assert_eq!(protocols.len(), 2);
        assert_eq!(protocols[0].name, "lido");
        assert_eq!(protocols[0].net_usd_value, Some(500.0));
        assert_eq!(protocols[1].name, "aave-v3");
        assert_eq!(protocols[1].net_usd_value, Some(300.0));
    }

    #[test]
    fn aggregate_zerion_protocols_drops_zero_value_and_unprotocoled() {
        let positions = vec![
            zerion_position("X", "ethereum", 0.0, "deposit", Some("p1")),
            zerion_position("Y", "ethereum", -1.0, "deposit", Some("p2")),
            zerion_position("Z", "ethereum", 10.0, "wallet", None),
        ];
        let protocols = aggregate_zerion_protocols(positions);
        assert!(protocols.is_empty());
    }

    #[test]
    fn aggregate_zerion_protocols_falls_back_to_display_name_when_protocol_missing() {
        // Real-world Zerion responses sometimes omit the `protocol` slug for
        // DeFi positions but always set `attributes.name`. The aggregator
        // should keep these so active_protocols isn't silently empty.
        let positions = vec![
            zerion_position_with_display_name(
                "USDC",
                "arbitrum",
                250.0,
                "deposit",
                "Aave V3 USDC Deposit",
            ),
            zerion_position("AURA", "ethereum", 100.0, "staked", Some("aura-finance")),
        ];
        let protocols = aggregate_zerion_protocols(positions);
        assert_eq!(protocols.len(), 2);
        assert!(protocols
            .iter()
            .any(|p| p.name == "Aave V3 USDC Deposit" && p.net_usd_value == Some(250.0)));
        assert!(protocols
            .iter()
            .any(|p| p.name == "aura-finance" && p.net_usd_value == Some(100.0)));
    }

    #[test]
    fn build_zerion_defi_positions_groups_lp_legs_by_group_id() {
        let positions = vec![
            zerion_lp_position("USDC", "base", 120.0, 100.0, "pool-1", Some("uniswap-v3")),
            zerion_lp_position("ETH", "base", 80.0, 0.05, "pool-1", Some("uniswap-v3")),
            zerion_position("USDC", "base", 50.0, "deposit", Some("aave-v3")),
        ];

        let defi = build_zerion_defi_positions(positions, 1.0);
        assert_eq!(defi.len(), 2);

        let lp = defi
            .iter()
            .find(|p| p.position_type == "liquidity")
            .expect("expected grouped liquidity position");
        assert_eq!(lp.protocol, "uniswap-v3");
        assert_eq!(lp.chain, "base");
        assert_eq!(lp.value_usd, Some(200.0));
        assert_eq!(lp.tokens.len(), 2);
        assert!(lp.tokens.iter().any(|t| t.symbol == "USDC" && t.amount == Some(100.0)));
        assert!(lp.tokens.iter().any(|t| t.symbol == "ETH" && t.amount == Some(0.05)));
    }

    #[test]
    fn build_zerion_defi_positions_does_not_group_non_lp_positions() {
        let mut p1 = zerion_position("USDC", "ethereum", 100.0, "deposit", Some("aave-v3"));
        p1.group_id = Some("shared".to_string());
        let mut p2 = zerion_position("DAI", "ethereum", 75.0, "borrow", Some("aave-v3"));
        p2.group_id = Some("shared".to_string());

        let defi = build_zerion_defi_positions(vec![p1, p2], 1.0);
        assert_eq!(defi.len(), 2);
        assert!(defi.iter().any(|p| p.position_type == "deposit" && p.value_usd == Some(100.0)));
        assert!(defi.iter().any(|p| p.position_type == "borrow" && p.value_usd == Some(75.0)));
    }

    #[test]
    fn token_allocation_with_pre_filter_scopes_to_one_chain() {
        // Mirrors what fetch_balance_from_debank does: filter tokens by
        // chain_id BEFORE handing them to aggregate_token_allocation.
        let tokens = vec![
            DebankAssetRecord {
                chain: "eth".to_string(),
                chain_id: Some(1),
                symbol: "USDC".to_string(),
                name: "USDC".to_string(),
                address: "0xa".to_string(),
                amount: 100.0,
                price_usd: Some(1.0),
                value_usd: Some(100.0),
            },
            DebankAssetRecord {
                chain: "base".to_string(),
                chain_id: Some(8453),
                symbol: "USDC".to_string(),
                name: "USDC".to_string(),
                address: "0xb".to_string(),
                amount: 50.0,
                price_usd: Some(1.0),
                value_usd: Some(50.0),
            },
        ];
        let filter: Option<u64> = Some(8453);
        let scoped: Vec<_> = tokens
            .into_iter()
            .filter(|t| filter.is_none_or(|id| t.chain_id == Some(id)))
            .collect();
        let allocs = aggregate_token_allocation(&scoped, 50.0);
        assert_eq!(allocs.len(), 1);
        assert_eq!(allocs[0].symbol, "USDC");
        assert!((allocs[0].balance_usd - 50.0).abs() < 1e-9);
    }
}
