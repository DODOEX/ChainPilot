use chrono::Utc;
use std::future::Future;
use std::pin::Pin;
use std::process::ExitCode;

use alloy_signer_local::PrivateKeySigner;

use crate::api::ApiClients;
use crate::chain::OnChainClient;
use crate::cli::swap::{
    ApproveArgs, ExecuteArgs, HistoryArgs, QuoteArgs, RevokeArgs, SimulateArgs, StatusArgs,
    SwapAction, SwapCmd,
};
use crate::commands::{parse_display_amount, resolve_token, to_raw_amount};
use crate::config::AppConfig;
use crate::error::{ChainError, Result};
use crate::models::quote::{Quote, QuoteRequest, TokenRef};
use crate::models::swap::{ExecutionResult, ExecutionStatus, SimulationResult};
use crate::output::{OutputContext, OutputMode};
use crate::store::QuoteStore;
use alloy::primitives::Address;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

trait QuoteDeps {
    fn get_route<'a>(
        &'a self,
        req: &'a QuoteRequest,
        from_token: &'a TokenRef,
        to_token: &'a TokenRef,
        user_addr: &'a str,
        estimate_gas: bool,
        quote_ttl_secs: u64,
    ) -> BoxFuture<'a, Result<Quote>>;

    fn get_eth_balance<'a>(&'a self, wallet_addr: Address) -> BoxFuture<'a, Result<(String, f64)>>;

    fn get_balance<'a>(
        &'a self,
        token_addr: Address,
        wallet_addr: Address,
    ) -> BoxFuture<'a, Result<(String, u8)>>;

    fn get_allowance<'a>(
        &'a self,
        token_addr: Address,
        wallet_addr: Address,
        spender: Address,
    ) -> BoxFuture<'a, Result<String>>;
}

struct LiveQuoteDeps<'a> {
    api: &'a ApiClients,
    onchain: &'a OnChainClient,
}

impl QuoteDeps for LiveQuoteDeps<'_> {
    fn get_route<'a>(
        &'a self,
        req: &'a QuoteRequest,
        from_token: &'a TokenRef,
        to_token: &'a TokenRef,
        user_addr: &'a str,
        estimate_gas: bool,
        quote_ttl_secs: u64,
    ) -> BoxFuture<'a, Result<Quote>> {
        Box::pin(async move {
            self.api
                .dodo
                .get_route(
                    req,
                    from_token,
                    to_token,
                    user_addr,
                    estimate_gas,
                    quote_ttl_secs,
                )
                .await
        })
    }

    fn get_eth_balance<'a>(&'a self, wallet_addr: Address) -> BoxFuture<'a, Result<(String, f64)>> {
        Box::pin(async move { crate::chain::get_eth_balance(self.onchain, wallet_addr).await })
    }

    fn get_balance<'a>(
        &'a self,
        token_addr: Address,
        wallet_addr: Address,
    ) -> BoxFuture<'a, Result<(String, u8)>> {
        Box::pin(
            async move { crate::chain::get_balance(self.onchain, token_addr, wallet_addr).await },
        )
    }

    fn get_allowance<'a>(
        &'a self,
        token_addr: Address,
        wallet_addr: Address,
        spender: Address,
    ) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            crate::chain::get_allowance(self.onchain, token_addr, wallet_addr, spender).await
        })
    }
}

trait ExecuteDeps {
    fn estimate_gas<'a>(
        &'a self,
        from: Address,
        to: Address,
        data: &'a str,
        value: &'a str,
    ) -> BoxFuture<'a, Result<u64>>;

    fn get_nonce<'a>(&'a self, address: Address) -> BoxFuture<'a, Result<u64>>;

    fn send_tx<'a>(
        &'a self,
        chain_id: u64,
        signer: PrivateKeySigner,
        to: Address,
        data: &'a str,
        value_hex: &'a str,
        gas_limit: Option<u64>,
        max_fee_gwei: Option<f64>,
    ) -> BoxFuture<'a, Result<(Address, String)>>;

    fn get_tx_receipt<'a>(
        &'a self,
        tx_hash: &'a str,
    ) -> BoxFuture<'a, Result<Option<crate::chain::TxStatus>>>;
}

struct LiveExecuteDeps<'a> {
    onchain: &'a OnChainClient,
    exec_rpc_url: &'a str,
}

impl ExecuteDeps for LiveExecuteDeps<'_> {
    fn estimate_gas<'a>(
        &'a self,
        from: Address,
        to: Address,
        data: &'a str,
        value: &'a str,
    ) -> BoxFuture<'a, Result<u64>> {
        Box::pin(
            async move { crate::chain::estimate_gas(self.onchain, from, to, data, value).await },
        )
    }

    fn get_nonce<'a>(&'a self, address: Address) -> BoxFuture<'a, Result<u64>> {
        Box::pin(async move { crate::chain::get_nonce(self.onchain, address).await })
    }

    fn send_tx<'a>(
        &'a self,
        chain_id: u64,
        signer: PrivateKeySigner,
        to: Address,
        data: &'a str,
        value_hex: &'a str,
        gas_limit: Option<u64>,
        max_fee_gwei: Option<f64>,
    ) -> BoxFuture<'a, Result<(Address, String)>> {
        Box::pin(async move {
            crate::chain::send_tx(
                self.exec_rpc_url,
                chain_id,
                signer,
                to,
                data,
                value_hex,
                gas_limit,
                max_fee_gwei,
            )
            .await
        })
    }

    fn get_tx_receipt<'a>(
        &'a self,
        tx_hash: &'a str,
    ) -> BoxFuture<'a, Result<Option<crate::chain::TxStatus>>> {
        Box::pin(async move { crate::chain::get_tx_receipt(self.onchain, tx_hash).await })
    }
}

trait ApprovalDeps {
    fn send_tx<'a>(
        &'a self,
        chain_id: u64,
        signer: PrivateKeySigner,
        to: Address,
        data: &'a str,
        value_hex: &'a str,
    ) -> BoxFuture<'a, Result<(Address, String)>>;
}

struct LiveApprovalDeps<'a> {
    chain_rpc: &'a str,
}

impl ApprovalDeps for LiveApprovalDeps<'_> {
    fn send_tx<'a>(
        &'a self,
        chain_id: u64,
        signer: PrivateKeySigner,
        to: Address,
        data: &'a str,
        value_hex: &'a str,
    ) -> BoxFuture<'a, Result<(Address, String)>> {
        Box::pin(async move {
            crate::chain::send_tx(
                self.chain_rpc,
                chain_id,
                signer,
                to,
                data,
                value_hex,
                None,
                None,
            )
            .await
        })
    }
}

pub async fn handle(
    cmd: SwapCmd,
    config: &AppConfig,
    store: &QuoteStore,
    api: &ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match cmd.action {
        SwapAction::Quote(args) => quote(args, config, store, api, output_mode).await,
        SwapAction::Simulate(args) => simulate(args, config, store, output_mode).await,
        SwapAction::Execute(args) => execute(args, config, store, output_mode).await,
        SwapAction::Status(args) => status(args, config, output_mode).await,
        SwapAction::History(args) => history(args, config, store, output_mode).await,
        SwapAction::Approve(args) => approve(args, config, store, output_mode).await,
        SwapAction::Revoke(args) => revoke(args, config, output_mode).await,
    }
}

async fn quote(
    args: QuoteArgs,
    config: &AppConfig,
    store: &QuoteStore,
    api: &ApiClients,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let chain_onchain = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_onchain;

    let from_token = resolve_token(&args.from, chain_id, onchain, api, config, store).await?;
    let to_token = resolve_token(&args.to, chain_id, onchain, api, config, store).await?;
    let user_addr = match config.wallet_address.as_deref() {
        Some(addr) => match addr.parse::<Address>() {
            Ok(parsed) => parsed.to_string(),
            Err(_) => return Err(ChainError::InvalidAddress(addr.to_string()).into()),
        },
        None => Address::ZERO.to_string(),
    };

    let amount_display = parse_display_amount(&args.amount)?;
    let req = QuoteRequest {
        from: from_token.address.clone(),
        to: to_token.address.clone(),
        amount: args.amount.clone(),
        amount_display,
        chain_id,
        slippage: args.slippage,
    };

    let deps = LiveQuoteDeps { api, onchain };
    let quote =
        fetch_quote_with_fallback(&deps, &req, &from_token, &to_token, &user_addr, config).await?;
    store.save_quote(&quote)?;
    record_custom_tokens_from_quote_inputs(&args, &quote, onchain, store).await;

    Ok(crate::output::print_output::<crate::models::quote::Quote>(
        Ok(quote),
        "swap.quote",
        output_mode,
        OutputContext::new(chain_id, false),
    ))
}

async fn record_custom_tokens_from_quote_inputs(
    args: &QuoteArgs,
    quote: &Quote,
    onchain: &OnChainClient,
    store: &QuoteStore,
) {
    for (input, token) in [(&args.from, &quote.from_token), (&args.to, &quote.to_token)] {
        if input.parse::<Address>().is_err()
            || token
                .address
                .eq_ignore_ascii_case(crate::config::chains::NATIVE_ADDR)
        {
            continue;
        }

        let token_addr = match token.address.parse::<Address>() {
            Ok(addr) => addr,
            Err(_) => continue,
        };

        match crate::chain::get_token_info(onchain, token_addr).await {
            Ok(info) => {
                if let Err(err) = store.save_custom_token_info(&info) {
                    eprintln!(
                        "Warning: failed to persist custom token {} on chain {}: {}",
                        info.address, info.chain_id, err
                    );
                }
            }
            Err(err) => {
                eprintln!(
                    "Warning: failed to fetch token metadata for {} on chain {}: {}",
                    token.address, token.chain_id, err
                );
            }
        }
    }
}

async fn fetch_quote_with_fallback<D: QuoteDeps>(
    deps: &D,
    req: &QuoteRequest,
    from_token: &TokenRef,
    to_token: &TokenRef,
    user_addr: &str,
    config: &AppConfig,
) -> Result<Quote> {
    let estimate_gas = user_addr != Address::ZERO.to_string();
    match deps
        .get_route(
            req,
            from_token,
            to_token,
            user_addr,
            estimate_gas,
            crate::config::DEFAULT_QUOTE_TTL_SECS,
        )
        .await
    {
        Ok(quote) => Ok(quote),
        Err(err) if estimate_gas => {
            let wallet_addr = user_addr
                .parse::<Address>()
                .map_err(|_| ChainError::InvalidAddress(user_addr.to_string()))?;
            let amount_raw = to_raw_amount(&req.amount, from_token.decimals)?;
            let need_raw: u128 = amount_raw.parse().unwrap_or(0);
            let is_native = from_token
                .address
                .eq_ignore_ascii_case(crate::config::chains::NATIVE_ADDR);

            if is_native {
                let (have_raw, _) = deps.get_eth_balance(wallet_addr).await?;
                let have: u128 = have_raw.parse().unwrap_or(0);
                if have < need_raw {
                    return Err(ChainError::InsufficientBalance {
                        have: have_raw,
                        need: amount_raw,
                        token: from_token.symbol.clone(),
                    }
                    .into());
                }
            } else {
                let token_addr = from_token
                    .address
                    .parse::<Address>()
                    .map_err(|_| ChainError::InvalidAddress(from_token.address.clone()))?;
                let (have_raw, _) = deps.get_balance(token_addr, wallet_addr).await?;
                let have: u128 = have_raw.parse().unwrap_or(0);
                if have < need_raw {
                    return Err(ChainError::InsufficientBalance {
                        have: have_raw,
                        need: amount_raw,
                        token: from_token.symbol.clone(),
                    }
                    .into());
                }

                if let Some(chain_cfg) = config.chain_config_for(req.chain_id) {
                    let spender = chain_cfg
                        .contracts
                        .dodo_approve
                        .parse::<Address>()
                        .map_err(|_| {
                            ChainError::InvalidAddress(chain_cfg.contracts.dodo_approve.to_string())
                        })?;
                    let allowance_raw =
                        deps.get_allowance(token_addr, wallet_addr, spender).await?;
                    let allowance: u128 = allowance_raw.parse().unwrap_or(0);
                    if allowance < need_raw {
                        return Err(ChainError::NotApproved {
                            token: from_token.address.clone(),
                            spender: chain_cfg.contracts.dodo_approve.to_string(),
                        }
                        .into());
                    }
                }
            }

            match deps
                .get_route(
                    req,
                    from_token,
                    to_token,
                    user_addr,
                    false,
                    crate::config::DEFAULT_QUOTE_TTL_SECS,
                )
                .await
            {
                Ok(quote) => Ok(quote),
                Err(_) => Err(err),
            }
        }
        Err(err) => Err(err),
    }
}

async fn simulate(
    args: SimulateArgs,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    use crate::chain::{estimate_gas, get_allowance, get_balance, get_eth_balance};
    use alloy::primitives::Address;

    let quote_data = match store.load_quote(&args.quote_id)? {
        Some(q) => q,
        None => {
            return Ok(crate::output::print_output::<SimulationResult>(
                Err(ChainError::QuoteNotFound(args.quote_id)),
                "swap.simulate",
                output_mode,
                OutputContext::new(config.chain_id, false),
            ));
        }
    };
    let chain_id = quote_data.chain_id;

    let chain_onchain = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_onchain;

    let mut warnings = simulation_base_warnings(&quote_data);
    let mut is_valid = true;

    let mut estimated_gas = quote_data.estimated_gas.or(quote_data.gas_limit);

    let gas_price_gwei = match crate::chain::get_gas_price_gwei(onchain).await {
        Ok(price) => price,
        Err(e) => {
            warnings.push(format!("Failed to fetch current gas price: {}", e));
            0.0
        }
    };

    let mut total_gas_cost_eth = simulation_gas_cost_eth(estimated_gas, gas_price_gwei);

    // Wallet-specific checks (balance + allowance + eth_estimateGas pre-execution)
    let mut wallet_balance: Option<String> = None;
    let mut has_sufficient_balance: Option<bool> = None;
    let mut current_allowance: Option<String> = None;
    let mut needs_approval: Option<bool> = None;
    let mut suggested_approve_amount: Option<String> = None;

    if let Some(wallet_str) = args.wallet.as_deref() {
        match wallet_str.parse::<Address>() {
            Err(_) => warnings.push(format!("Invalid wallet address: {}", wallet_str)),
            Ok(wallet_addr) => {
                const ETH_ADDR: &str = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
                let is_native_eth = quote_data.from_token.address.to_lowercase() == ETH_ADDR;

                // Raw from_amount in token's smallest unit
                let from_amount_raw = crate::commands::to_raw_amount(
                    &quote_data.from_amount,
                    quote_data.from_token.decimals,
                )?
                .parse::<u128>()
                .unwrap_or(0);

                if is_native_eth {
                    match get_eth_balance(onchain, wallet_addr).await {
                        Ok((raw_balance, _)) => {
                            let bal: u128 = raw_balance.parse().unwrap_or(0);
                            has_sufficient_balance = Some(bal >= from_amount_raw);
                            wallet_balance = Some(raw_balance);
                            needs_approval = Some(false);
                        }
                        Err(e) => warnings.push(format!("Failed to fetch ETH balance: {}", e)),
                    }
                } else {
                    match quote_data.from_token.address.parse::<Address>() {
                        Err(_) => warnings.push("Invalid from_token address in quote.".to_string()),
                        Ok(token_addr) => {
                            match get_balance(onchain, token_addr, wallet_addr).await {
                                Err(e) => {
                                    warnings.push(format!("Failed to fetch token balance: {}", e))
                                }
                                Ok((raw_balance, _)) => {
                                    let bal: u128 = raw_balance.parse().unwrap_or(0);
                                    has_sufficient_balance = Some(bal >= from_amount_raw);
                                    wallet_balance = Some(raw_balance);

                                    match quote_data.router_to.parse::<Address>() {
                                        Err(_) => warnings
                                            .push("Invalid router address in quote.".to_string()),
                                        Ok(spender_addr) => {
                                            match get_allowance(
                                                onchain,
                                                token_addr,
                                                wallet_addr,
                                                spender_addr,
                                            )
                                            .await
                                            {
                                                Err(e) => warnings.push(format!(
                                                    "Failed to fetch allowance: {}",
                                                    e
                                                )),
                                                Ok(allowance_str) => {
                                                    let allowance: u128 =
                                                        allowance_str.parse().unwrap_or(0);
                                                    let needs = allowance < from_amount_raw;
                                                    current_allowance = Some(allowance_str);
                                                    needs_approval = Some(needs);
                                                    if needs {
                                                        suggested_approve_amount =
                                                            Some(from_amount_raw.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // eth_estimateGas: pre-execute on the node to catch reverts and get accurate gas.
                // Runs regardless of token type; failure means the tx would revert.
                match quote_data.router_to.parse::<Address>() {
                    Err(_) => {} // already warned above if invalid
                    Ok(router_addr) => {
                        match estimate_gas(
                            onchain,
                            wallet_addr,
                            router_addr,
                            &quote_data.calldata,
                            &quote_data.value,
                        )
                        .await
                        {
                            Ok(gas) => {
                                estimated_gas = Some(gas);
                                total_gas_cost_eth =
                                    simulation_gas_cost_eth(Some(gas), gas_price_gwei);
                            }
                            Err(e) => {
                                is_valid = false;
                                warnings.push(format!("Transaction would revert: {}", e));
                            }
                        }
                    }
                }
            }
        }
    }

    let result = SimulationResult {
        quote_id: quote_data.quote_id,
        simulated_at: Utc::now(),
        is_valid,
        warnings,
        expected_out: quote_data.to_amount.clone(),
        min_out: quote_data.to_amount_min.clone(),
        current_price_impact_pct: quote_data.price_impact_pct,
        wallet_balance,
        has_sufficient_balance,
        current_allowance,
        needs_approval,
        suggested_approve_amount,
        estimated_gas,
        gas_price_gwei,
        total_gas_cost_eth,
        total_gas_cost_usd: None,
        calldata: quote_data.calldata.clone(),
        to_contract: quote_data.router_to.clone(),
        value_eth: quote_data.value.clone(),
    };

    Ok(crate::output::print_output::<SimulationResult>(
        Ok(result),
        "swap.simulate",
        output_mode,
        OutputContext::new(chain_id, false),
    ))
}

fn simulation_base_warnings(quote: &Quote) -> Vec<String> {
    let mut warnings = Vec::new();
    if quote.estimated_gas.is_none() && quote.gas_limit.is_none() {
        warnings.push("Quote did not include gas estimate or gas limit.".to_string());
    }
    if quote.route_summary.is_empty() {
        warnings.push("Route summary is empty in the quote response.".to_string());
    }
    warnings
}

fn simulation_gas_cost_eth(estimated_gas: Option<u64>, gas_price_gwei: f64) -> f64 {
    estimated_gas
        .map(|gas| gas as f64 * gas_price_gwei / 1e9)
        .unwrap_or(0.0)
}

fn dry_run_from_address(
    derived_from_private_key: Option<String>,
    subcommand_wallet: Option<String>,
    global_wallet: Option<String>,
) -> Option<String> {
    derived_from_private_key
        .or(subcommand_wallet)
        .or(global_wallet)
}

fn resolve_effective_gas_limit(
    user_gas_limit: Option<u64>,
    skip_estimate: bool,
    quote_estimated_gas: Option<u64>,
    quote_gas_limit: Option<u64>,
    estimated_gas_from_node: Option<u64>,
    gas_buffer_pct: Option<u64>,
) -> Option<u64> {
    if let Some(user_gas) = user_gas_limit {
        return Some(user_gas);
    }
    if skip_estimate {
        return quote_estimated_gas.or(quote_gas_limit);
    }
    estimated_gas_from_node.map(|gas| {
        if let Some(pct) = gas_buffer_pct {
            gas + gas * pct / 100
        } else {
            gas
        }
    })
}

fn resolve_approve_targets(
    explicit_token: Option<&str>,
    quote: Option<&Quote>,
    explicit_spender: Option<&str>,
    chain_id: u64,
) -> Result<(String, String)> {
    let token = explicit_token
        .map(str::to_string)
        .or_else(|| quote.map(|q| q.from_token.address.clone()))
        .ok_or_else(|| ChainError::Config("--token or --quote-id is required".to_string()))?;

    let spender = if let Some(s) = explicit_spender {
        s.to_string()
    } else if quote.is_some() {
        crate::config::chain_config(chain_id)
            .map(|c| c.contracts.dodo_approve.to_string())
            .ok_or_else(|| {
                ChainError::Config(format!(
                    "no chain config for chain_id {}; use --spender to set the approve address",
                    chain_id
                ))
            })?
    } else {
        return Err(ChainError::Config(
            "--spender or --quote-id is required".to_string(),
        ));
    };

    Ok((token, spender))
}

fn approve_calldata(spender_addr: Address, amount_u256: alloy::primitives::U256) -> String {
    let selector = [0x09u8, 0x5e, 0xa7, 0xb3];
    let mut calldata = Vec::with_capacity(68);
    calldata.extend_from_slice(&selector);
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(spender_addr.as_slice());
    calldata.extend_from_slice(&amount_u256.to_be_bytes::<32>());
    format!("0x{}", hex::encode(&calldata))
}

fn revoke_calldata(spender_addr: Address) -> String {
    let selector = [0x09u8, 0x5e, 0xa7, 0xb3];
    let mut calldata = Vec::with_capacity(68);
    calldata.extend_from_slice(&selector);
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(spender_addr.as_slice());
    calldata.extend_from_slice(&[0u8; 32]);
    format!("0x{}", hex::encode(&calldata))
}

async fn send_approval_with_deps<D: ApprovalDeps>(
    deps: &D,
    chain_id: u64,
    signer: PrivateKeySigner,
    token_addr: Address,
    spender_addr: Address,
    raw_amount: String,
    calldata_hex: &str,
) -> Result<crate::models::swap::ApprovalResult> {
    let from_addr = signer.address();
    let (_from, tx_hash) = deps
        .send_tx(chain_id, signer, token_addr, calldata_hex, "0x0")
        .await?;

    Ok(crate::models::swap::ApprovalResult {
        token: token_addr.to_string(),
        spender: spender_addr.to_string(),
        raw_amount,
        dry_run: false,
        tx_hash: Some(tx_hash),
        from_address: Some(from_addr.to_string()),
    })
}

async fn execute(
    args: ExecuteArgs,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let quote_data = match store.load_quote(&args.quote_id)? {
        Some(q) => q,
        None => {
            return Ok(crate::output::print_output::<ExecutionResult>(
                Err(ChainError::QuoteNotFound(args.quote_id.clone())),
                "swap.execute",
                output_mode,
                OutputContext::new(config.chain_id, args.dry_run),
            ));
        }
    };
    let chain_id = quote_data.chain_id;

    let chain_onchain = OnChainClient::for_chain(config, chain_id).await?;
    let exec_rpc_url = config.rpc_url_for_chain(chain_id);
    let deps = LiveExecuteDeps {
        onchain: &chain_onchain,
        exec_rpc_url: &exec_rpc_url,
    };
    let result = match execute_quote_with_deps(&deps, &args, config, &quote_data).await {
        Ok(result) => result,
        Err(e) => {
            return Ok(crate::output::print_output::<ExecutionResult>(
                Err(e),
                "swap.execute",
                output_mode,
                OutputContext::new(chain_id, args.dry_run),
            ));
        }
    };

    // Persist to history
    let history_record = crate::models::swap::SwapHistoryRecord::from_execution(
        &quote_data,
        &result,
        uuid::Uuid::new_v4().to_string(),
    );
    if let Err(e) = store.save_history(&history_record) {
        eprintln!("Warning: failed to save swap history: {}", e);
    }

    Ok(crate::output::print_output::<ExecutionResult>(
        Ok(result),
        "swap.execute",
        output_mode,
        OutputContext::new(chain_id, args.dry_run),
    ))
}

async fn execute_quote_with_deps<D: ExecuteDeps>(
    deps: &D,
    args: &ExecuteArgs,
    config: &AppConfig,
    quote_data: &Quote,
) -> Result<ExecutionResult> {
    use alloy::primitives::Address;
    use std::time::Duration;

    let (from_address, tx_hash, status, gas_used, effective_gas_price_gwei) = if args.dry_run {
        let addr = dry_run_from_address(
            crate::chain::resolve_signer(config)
                .ok()
                .map(|signer| signer.address().to_string()),
            args.wallet.clone(),
            config.wallet_address.clone(),
        );
        (addr, None, ExecutionStatus::DryRun, None, None)
    } else {
        let signer = crate::chain::resolve_signer(config)?;
        let to_addr: Address = quote_data
            .router_to
            .parse()
            .map_err(|_| ChainError::InvalidAddress(quote_data.router_to.clone()))?;
        let from_addr = signer.address();

        let estimated_gas_from_node = if args.gas_limit.is_none() && !args.skip_estimate {
            Some(
                deps.estimate_gas(from_addr, to_addr, &quote_data.calldata, &quote_data.value)
                    .await?,
            )
        } else {
            None
        };
        let effective_gas_limit = resolve_effective_gas_limit(
            args.gas_limit,
            args.skip_estimate,
            quote_data.estimated_gas,
            quote_data.gas_limit,
            estimated_gas_from_node,
            args.gas_buffer_pct,
        );

        let tx_nonce = deps.get_nonce(from_addr).await.unwrap_or(0);
        let (_sent_from, tx_hash_str) = deps
            .send_tx(
                quote_data.chain_id,
                signer,
                to_addr,
                &quote_data.calldata,
                &quote_data.value,
                effective_gas_limit,
                args.max_fee_gwei,
            )
            .await?;

        let (final_status, gas_used, effective_gas_price_gwei) = if args.wait {
            const POLL_INTERVAL: Duration = Duration::from_secs(3);
            const MAX_POLLS: u32 = 100;
            let mut polls = 0u32;
            let mut outcome = (ExecutionStatus::Submitted, None, None);
            loop {
                tokio::time::sleep(POLL_INTERVAL).await;
                match deps.get_tx_receipt(&tx_hash_str).await {
                    Ok(Some(receipt)) => {
                        let status = if receipt.success {
                            ExecutionStatus::Confirmed
                        } else {
                            ExecutionStatus::Failed
                        };
                        outcome = (status, receipt.gas_used, receipt.effective_gas_price);
                        break;
                    }
                    Err(e) => {
                        eprintln!("Warning: error polling receipt: {}", e);
                        break;
                    }
                    Ok(None) => {}
                }
                if let Ok(current_nonce) = deps.get_nonce(from_addr).await {
                    if current_nonce > tx_nonce {
                        outcome = (ExecutionStatus::Cancelled, None, None);
                        break;
                    }
                }
                polls += 1;
                if polls >= MAX_POLLS {
                    eprintln!("Warning: timed out waiting for receipt; tx may still be pending.");
                    break;
                }
            }
            outcome
        } else {
            (ExecutionStatus::Submitted, None, None)
        };

        (
            Some(from_addr.to_string()),
            Some(tx_hash_str),
            final_status,
            gas_used,
            effective_gas_price_gwei,
        )
    };

    Ok(ExecutionResult {
        quote_id: quote_data.quote_id,
        executed_at: Utc::now(),
        dry_run: args.dry_run,
        tx_hash,
        status,
        calldata: quote_data.calldata.clone(),
        to_contract: quote_data.router_to.clone(),
        value_eth: quote_data.value.clone(),
        from_address,
        gas_used,
        effective_gas_price_gwei,
    })
}

async fn status(args: StatusArgs, config: &AppConfig, output_mode: OutputMode) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let chain_onchain = OnChainClient::for_chain(config, chain_id).await?;
    match crate::chain::get_tx_receipt(&chain_onchain, &args.tx_hash).await {
        Ok(Some(status)) => Ok(crate::output::print_output::<crate::chain::TxStatus>(
            Ok(status),
            "swap.status",
            output_mode,
            OutputContext::new(chain_id, false),
        )),
        Ok(None) => Ok(crate::output::print_output::<crate::chain::TxStatus>(
            Err(ChainError::Config(
                "Transaction not found or pending".to_string(),
            )),
            "swap.status",
            output_mode,
            OutputContext::new(chain_id, false),
        )),
        Err(e) => Ok(crate::output::print_output::<crate::chain::TxStatus>(
            Err(e),
            "swap.status",
            output_mode,
            OutputContext::new(chain_id, false),
        )),
    }
}

async fn history(
    args: HistoryArgs,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let mut records = store.load_history(args.limit)?;
    if let Some(status) = args.status.as_deref() {
        let status = status.to_lowercase();
        records.retain(|record| match status.as_str() {
            "dry_run" => matches!(record.status, ExecutionStatus::DryRun),
            "pending" | "submitted" => matches!(record.status, ExecutionStatus::Submitted),
            "success" | "confirmed" => matches!(record.status, ExecutionStatus::Confirmed),
            "failed" => matches!(record.status, ExecutionStatus::Failed),
            "cancelled" | "canceled" => matches!(record.status, ExecutionStatus::Cancelled),
            _ => false,
        });
    }
    Ok(crate::output::print_output::<
        Vec<crate::models::swap::SwapHistoryRecord>,
    >(
        Ok(records),
        "swap.history",
        output_mode,
        OutputContext::new(config.chain_id, false),
    ))
}

async fn approve(
    args: ApproveArgs,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    use crate::chain::{get_token_info, OnChainClient};
    use crate::models::swap::ApprovalResult;
    use alloy::primitives::{Address, U256};

    // Load quote if provided; used as fallback for token and spender.
    let quote = match &args.quote_id {
        Some(id) => store.load_quote(id)?,
        None => None,
    };

    let chain_id = quote
        .as_ref()
        .map(|q| q.chain_id)
        .unwrap_or(config.chain_id);
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let onchain = &chain_client;
    let chain_rpc = config.rpc_url_for_chain(chain_id);

    let (token_str, spender_str) = resolve_approve_targets(
        args.token.as_deref(),
        quote.as_ref(),
        args.spender.as_deref(),
        chain_id,
    )?;

    let token_addr: Address = token_str
        .parse()
        .map_err(|_| ChainError::InvalidAddress(token_str.clone()))?;
    let spender_addr: Address = spender_str
        .parse()
        .map_err(|_| ChainError::InvalidAddress(spender_str.clone()))?;

    // Get token decimals (for amount conversion): quote is authoritative, else on-chain.
    let decimals: u8 = if let Some(q) = &quote {
        if q.from_token.address.to_lowercase() == token_str.to_lowercase() {
            q.from_token.decimals
        } else {
            get_token_info(onchain, token_addr).await?.decimals
        }
    } else {
        get_token_info(onchain, token_addr).await?.decimals
    };

    // Compute raw approval amount.
    let (amount_u256, raw_amount_str) = match args.amount {
        None => (U256::MAX, "unlimited".to_string()),
        Some(human) => {
            let raw = crate::commands::to_raw_amount(&human, decimals)?;
            let raw_u128 = raw
                .parse::<u128>()
                .map_err(|_| ChainError::InvalidAmount(human.clone()))?;
            (U256::from(raw_u128), raw)
        }
    };

    // ABI-encode approve(address spender, uint256 amount):
    //   selector  = keccak256("approve(address,uint256)")[0..4] = 0x095ea7b3
    //   arg[0]    = spender padded to 32 bytes (12 zeros + 20-byte address)
    //   arg[1]    = amount as 32-byte big-endian
    let calldata_hex = approve_calldata(spender_addr, amount_u256);

    let signer = match crate::chain::resolve_signer(config) {
        Ok(signer) => Some(signer),
        Err(ChainError::NoWallet) if args.dry_run => None,
        Err(ChainError::NoWallet) => None,
        Err(e) if args.dry_run => {
            return Ok(crate::output::print_output::<ApprovalResult>(
                Err(e),
                "swap.approve",
                output_mode,
                OutputContext::new(chain_id, true),
            ));
        }
        Err(e) => {
            return Ok(crate::output::print_output::<ApprovalResult>(
                Err(e),
                "swap.approve",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    if args.dry_run || signer.is_none() {
        let result = ApprovalResult {
            token: token_addr.to_string(),
            spender: spender_addr.to_string(),
            raw_amount: raw_amount_str,
            dry_run: true,
            tx_hash: None,
            from_address: signer.as_ref().map(|signer| signer.address().to_string()),
        };
        let dry_run = result.dry_run;
        return Ok(crate::output::print_output::<ApprovalResult>(
            Ok(result),
            "swap.approve",
            output_mode,
            OutputContext::new(chain_id, dry_run),
        ));
    }

    match send_approval_with_deps(
        &LiveApprovalDeps {
            chain_rpc: &chain_rpc,
        },
        chain_id,
        signer.unwrap(),
        token_addr,
        spender_addr,
        raw_amount_str,
        &calldata_hex,
    )
    .await
    {
        Ok(result) => Ok(crate::output::print_output::<ApprovalResult>(
            Ok(result),
            "swap.approve",
            output_mode,
            OutputContext::new(chain_id, false),
        )),
        Err(e) => Ok(crate::output::print_output::<ApprovalResult>(
            Err(e),
            "swap.approve",
            output_mode,
            OutputContext::new(chain_id, false),
        )),
    }
}

async fn revoke(args: RevokeArgs, config: &AppConfig, output_mode: OutputMode) -> Result<ExitCode> {
    use crate::models::swap::ApprovalResult;
    use alloy::primitives::Address;

    let chain_id = config.chain_id;
    let chain_rpc = config.rpc_url_for_chain(chain_id);

    let token_addr: Address = match args.token.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<ApprovalResult>(
                Err(ChainError::InvalidAddress(args.token.clone())),
                "swap.revoke",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };
    let spender_addr: Address = match args.spender.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<ApprovalResult>(
                Err(ChainError::InvalidAddress(args.spender.clone())),
                "swap.revoke",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    // approve(address,uint256) with amount 0 revokes ERC-20 allowance.
    let calldata_hex = revoke_calldata(spender_addr);

    let signer = match crate::chain::resolve_signer(config) {
        Ok(signer) => Some(signer),
        Err(ChainError::NoWallet) if args.dry_run => None,
        Err(ChainError::NoWallet) => None,
        Err(e) if args.dry_run => {
            return Ok(crate::output::print_output::<ApprovalResult>(
                Err(e),
                "swap.revoke",
                output_mode,
                OutputContext::new(chain_id, true),
            ));
        }
        Err(e) => {
            return Ok(crate::output::print_output::<ApprovalResult>(
                Err(e),
                "swap.revoke",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    if args.dry_run || signer.is_none() {
        let result = ApprovalResult {
            token: token_addr.to_string(),
            spender: spender_addr.to_string(),
            raw_amount: "0".to_string(),
            dry_run: true,
            tx_hash: None,
            from_address: signer.as_ref().map(|signer| signer.address().to_string()),
        };
        let dry_run = result.dry_run;
        return Ok(crate::output::print_output::<ApprovalResult>(
            Ok(result),
            "swap.revoke",
            output_mode,
            OutputContext::new(chain_id, dry_run),
        ));
    }
    match send_approval_with_deps(
        &LiveApprovalDeps {
            chain_rpc: &chain_rpc,
        },
        chain_id,
        signer.unwrap(),
        token_addr,
        spender_addr,
        "0".to_string(),
        &calldata_hex,
    )
    .await
    {
        Ok(result) => Ok(crate::output::print_output::<ApprovalResult>(
            Ok(result),
            "swap.revoke",
            output_mode,
            OutputContext::new(chain_id, false),
        )),
        Err(e) => Ok(crate::output::print_output::<ApprovalResult>(
            Err(e),
            "swap.revoke",
            output_mode,
            OutputContext::new(chain_id, false),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use std::cell::RefCell;
    use uuid::Uuid;

    struct MockQuoteDeps {
        route_results: RefCell<Vec<Result<Quote>>>,
        eth_balance: (String, f64),
        token_balance: (String, u8),
        allowance: String,
    }

    impl QuoteDeps for MockQuoteDeps {
        fn get_route<'a>(
            &'a self,
            _req: &'a QuoteRequest,
            _from_token: &'a TokenRef,
            _to_token: &'a TokenRef,
            _user_addr: &'a str,
            _estimate_gas: bool,
            _quote_ttl_secs: u64,
        ) -> BoxFuture<'a, Result<Quote>> {
            Box::pin(async move { self.route_results.borrow_mut().remove(0) })
        }

        fn get_eth_balance<'a>(
            &'a self,
            _wallet_addr: Address,
        ) -> BoxFuture<'a, Result<(String, f64)>> {
            let response = self.eth_balance.clone();
            Box::pin(async move { Ok(response) })
        }

        fn get_balance<'a>(
            &'a self,
            _token_addr: Address,
            _wallet_addr: Address,
        ) -> BoxFuture<'a, Result<(String, u8)>> {
            let response = self.token_balance.clone();
            Box::pin(async move { Ok(response) })
        }

        fn get_allowance<'a>(
            &'a self,
            _token_addr: Address,
            _wallet_addr: Address,
            _spender: Address,
        ) -> BoxFuture<'a, Result<String>> {
            let response = self.allowance.clone();
            Box::pin(async move { Ok(response) })
        }
    }

    struct MockExecuteDeps {
        estimated_gas: Result<u64>,
        nonce: Result<u64>,
        send_tx_result: Result<(Address, String)>,
        receipts: RefCell<Vec<Result<Option<crate::chain::TxStatus>>>>,
    }

    impl ExecuteDeps for MockExecuteDeps {
        fn estimate_gas<'a>(
            &'a self,
            _from: Address,
            _to: Address,
            _data: &'a str,
            _value: &'a str,
        ) -> BoxFuture<'a, Result<u64>> {
            Box::pin(async move {
                match &self.estimated_gas {
                    Ok(v) => Ok(*v),
                    Err(e) => Err(anyhow::anyhow!(e.to_string()).into()),
                }
            })
        }

        fn get_nonce<'a>(&'a self, _address: Address) -> BoxFuture<'a, Result<u64>> {
            Box::pin(async move {
                match &self.nonce {
                    Ok(v) => Ok(*v),
                    Err(e) => Err(anyhow::anyhow!(e.to_string()).into()),
                }
            })
        }

        fn send_tx<'a>(
            &'a self,
            _chain_id: u64,
            _signer: PrivateKeySigner,
            _to: Address,
            _data: &'a str,
            _value_hex: &'a str,
            _gas_limit: Option<u64>,
            _max_fee_gwei: Option<f64>,
        ) -> BoxFuture<'a, Result<(Address, String)>> {
            Box::pin(async move {
                match &self.send_tx_result {
                    Ok(v) => Ok(v.clone()),
                    Err(e) => Err(anyhow::anyhow!(e.to_string()).into()),
                }
            })
        }

        fn get_tx_receipt<'a>(
            &'a self,
            _tx_hash: &'a str,
        ) -> BoxFuture<'a, Result<Option<crate::chain::TxStatus>>> {
            Box::pin(async move {
                match self.receipts.borrow_mut().remove(0) {
                    Ok(v) => Ok(v),
                    Err(e) => Err(anyhow::anyhow!(e.to_string()).into()),
                }
            })
        }
    }

    struct MockApprovalDeps {
        send_tx_result: Result<(Address, String)>,
    }

    impl ApprovalDeps for MockApprovalDeps {
        fn send_tx<'a>(
            &'a self,
            _chain_id: u64,
            _signer: PrivateKeySigner,
            _to: Address,
            _data: &'a str,
            _value_hex: &'a str,
        ) -> BoxFuture<'a, Result<(Address, String)>> {
            Box::pin(async move {
                match &self.send_tx_result {
                    Ok(v) => Ok(v.clone()),
                    Err(e) => Err(anyhow::anyhow!(e.to_string()).into()),
                }
            })
        }
    }

    fn test_config(chain_id: u64) -> AppConfig {
        let (
            coingecko_api_url,
            coingecko_api_key,
            dexscreener_api_url,
            okx_dex_api_url,
            okx_api_key,
            okx_api_secret,
            okx_api_passphrase,
            okx_project_id,
        ) = crate::config::test_metadata_config_fields();
        AppConfig {
            rpc_url: "https://rpc.example.com".to_string(),
            rpc_url_overridden: false,
            chain_id,
            private_key: None,
            keystore_path: None,
            keystore_password_file: None,
            keystore_password_env: None,
            wallet_address: None,
            dodo_api_url: "https://api.example.com".to_string(),
            dodo_api_key: String::new(),
            dodo_project_id: String::new(),
            coingecko_api_url,
            coingecko_api_key,
            dexscreener_api_url,
            okx_dex_api_url,
            okx_api_key,
            okx_api_secret,
            okx_api_passphrase,
            okx_project_id,
            data_dir: std::env::temp_dir().join(format!("chainpilot_test_{}", Uuid::new_v4())),
        }
    }

    fn native_token(chain_id: u64) -> TokenRef {
        TokenRef {
            symbol: "ETH".to_string(),
            address: crate::config::chains::NATIVE_ADDR.to_string(),
            decimals: 18,
            chain_id,
        }
    }

    fn erc20_token(chain_id: u64) -> TokenRef {
        TokenRef {
            symbol: "USDC".to_string(),
            address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
            decimals: 6,
            chain_id,
        }
    }

    fn quote_request(chain_id: u64, amount: &str) -> QuoteRequest {
        QuoteRequest {
            from: native_token(chain_id).address,
            to: erc20_token(chain_id).address,
            amount: amount.to_string(),
            amount_display: amount.parse().unwrap(),
            chain_id,
            slippage: 0.5,
        }
    }

    fn sample_quote(chain_id: u64) -> Quote {
        Quote {
            quote_id: Uuid::new_v4(),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::minutes(5),
            from_token: native_token(chain_id),
            to_token: erc20_token(chain_id),
            from_amount: "1".to_string(),
            from_amount_display: 1.0,
            to_amount: "3000".to_string(),
            to_amount_display: 3000.0,
            to_amount_min: "2970".to_string(),
            price_impact_pct: 0.1,
            exchange_rate: 3000.0,
            route_summary: vec![],
            dex_sources: vec!["DODO".to_string()],
            route_id: Some("route-1".to_string()),
            router_to: "0x1111111111111111111111111111111111111111".to_string(),
            calldata: "0xdeadbeef".to_string(),
            value: "0".to_string(),
            gas_limit: Some(200_000),
            estimated_gas: Some(180_000),
            estimated_gas_usd: Some(5.0),
            raw_dodo_response: serde_json::json!({}),
            chain_id,
            slippage: 0.5,
        }
    }

    fn execute_args() -> ExecuteArgs {
        ExecuteArgs {
            quote_id: "quote-1".to_string(),
            dry_run: false,
            gas_limit: None,
            max_fee_gwei: None,
            wallet: None,
            wait: false,
            skip_estimate: false,
            gas_buffer_pct: None,
        }
    }

    fn sample_quote_for_approve(chain_id: u64) -> Quote {
        let mut quote = sample_quote(chain_id);
        quote.from_token = erc20_token(chain_id);
        quote
    }

    #[tokio::test]
    async fn fetch_quote_without_wallet_does_not_retry() {
        let chain_id = 1;
        let req = quote_request(chain_id, "1");
        let from_token = native_token(chain_id);
        let to_token = erc20_token(chain_id);
        let deps = MockQuoteDeps {
            route_results: RefCell::new(vec![Err(ChainError::DodoApi {
                code: 500,
                message: "upstream".to_string(),
            })]),
            eth_balance: ("0".to_string(), 0.0),
            token_balance: ("0".to_string(), 6),
            allowance: "0".to_string(),
        };

        let err = fetch_quote_with_fallback(
            &deps,
            &req,
            &from_token,
            &to_token,
            &Address::ZERO.to_string(),
            &test_config(chain_id),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, ChainError::DodoApi { .. }));
        assert_eq!(deps.route_results.borrow().len(), 0);
    }

    #[tokio::test]
    async fn fetch_quote_native_balance_failures_are_table_driven() {
        let chain_id = 1;
        let req = quote_request(chain_id, "1");
        let to_token = erc20_token(chain_id);
        let wallet = "0x1111111111111111111111111111111111111111";

        for case in [("0", "ETH"), ("999999999999999999", "ETH")] {
            let deps = MockQuoteDeps {
                route_results: RefCell::new(vec![Err(ChainError::DodoApi {
                    code: 500,
                    message: "estimate failed".to_string(),
                })]),
                eth_balance: (case.0.to_string(), 0.0),
                token_balance: ("0".to_string(), 6),
                allowance: "0".to_string(),
            };

            let err = fetch_quote_with_fallback(
                &deps,
                &req,
                &native_token(chain_id),
                &to_token,
                wallet,
                &test_config(chain_id),
            )
            .await
            .unwrap_err();

            match err {
                ChainError::InsufficientBalance { token, .. } => assert_eq!(token, case.1),
                other => panic!("unexpected error: {other}"),
            }
        }
    }

    #[tokio::test]
    async fn fetch_quote_erc20_not_approved_after_balance_check() {
        let chain_id = 1;
        let from_token = erc20_token(chain_id);
        let to_token = native_token(chain_id);
        let req = QuoteRequest {
            from: from_token.address.clone(),
            to: to_token.address.clone(),
            amount: "1".to_string(),
            amount_display: 1.0,
            chain_id,
            slippage: 0.5,
        };
        let deps = MockQuoteDeps {
            route_results: RefCell::new(vec![Err(ChainError::DodoApi {
                code: 500,
                message: "estimate failed".to_string(),
            })]),
            eth_balance: ("0".to_string(), 0.0),
            token_balance: ("1000000".to_string(), 6),
            allowance: "999999".to_string(),
        };

        let err = fetch_quote_with_fallback(
            &deps,
            &req,
            &from_token,
            &to_token,
            "0x1111111111111111111111111111111111111111",
            &test_config(chain_id),
        )
        .await
        .unwrap_err();

        match err {
            ChainError::NotApproved { token, .. } => assert_eq!(token, from_token.address),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[tokio::test]
    async fn fetch_quote_retries_without_estimate_after_checks_pass() {
        let chain_id = 1;
        let from_token = erc20_token(chain_id);
        let to_token = native_token(chain_id);
        let req = QuoteRequest {
            from: from_token.address.clone(),
            to: to_token.address.clone(),
            amount: "1".to_string(),
            amount_display: 1.0,
            chain_id,
            slippage: 0.5,
        };
        let expected = sample_quote(chain_id);
        let deps = MockQuoteDeps {
            route_results: RefCell::new(vec![
                Err(ChainError::DodoApi {
                    code: 500,
                    message: "estimate failed".to_string(),
                }),
                Ok(expected.clone()),
            ]),
            eth_balance: ("0".to_string(), 0.0),
            token_balance: ("1000000".to_string(), 6),
            allowance: "1000000".to_string(),
        };

        let quote = fetch_quote_with_fallback(
            &deps,
            &req,
            &from_token,
            &to_token,
            "0x1111111111111111111111111111111111111111",
            &test_config(chain_id),
        )
        .await
        .unwrap();

        assert_eq!(quote.quote_id, expected.quote_id);
        assert!(deps.route_results.borrow().is_empty());
    }

    #[test]
    fn simulation_base_warnings_are_table_driven() {
        let chain_id = 1;
        for (estimated_gas, gas_limit, route_summary_len, expected) in [
            (None, None, 0usize, 2usize),
            (Some(21000), None, 0usize, 1usize),
            (None, Some(25000), 1usize, 0usize),
        ] {
            let mut quote = sample_quote(chain_id);
            quote.estimated_gas = estimated_gas;
            quote.gas_limit = gas_limit;
            quote.route_summary = (0..route_summary_len)
                .map(|_| crate::models::quote::RouteHop {
                    pool_address: "0xpool".to_string(),
                    dex_name: "DODO".to_string(),
                    from_token: "USDC".to_string(),
                    to_token: "ETH".to_string(),
                    percent: 100.0,
                })
                .collect();

            let warnings = simulation_base_warnings(&quote);
            assert_eq!(warnings.len(), expected, "{warnings:?}");
        }
    }

    #[test]
    fn simulation_gas_cost_eth_handles_missing_and_present_estimates() {
        assert_eq!(simulation_gas_cost_eth(None, 20.0), 0.0);
        assert_eq!(simulation_gas_cost_eth(Some(21_000), 20.0), 0.00042);
    }

    #[test]
    fn dry_run_from_address_prefers_private_key_then_subcommand_then_global_wallet() {
        for (derived, subcommand, global, expected) in [
            (
                Some("0xpk".to_string()),
                Some("0xsub".to_string()),
                Some("0xglobal".to_string()),
                Some("0xpk".to_string()),
            ),
            (
                None,
                Some("0xsub".to_string()),
                Some("0xglobal".to_string()),
                Some("0xsub".to_string()),
            ),
            (
                None,
                None,
                Some("0xglobal".to_string()),
                Some("0xglobal".to_string()),
            ),
            (None, None, None, None),
        ] {
            assert_eq!(dry_run_from_address(derived, subcommand, global), expected);
        }
    }

    #[test]
    fn resolve_effective_gas_limit_obeys_precedence_rules() {
        struct Case {
            user_gas_limit: Option<u64>,
            skip_estimate: bool,
            quote_estimated_gas: Option<u64>,
            quote_gas_limit: Option<u64>,
            estimated_gas_from_node: Option<u64>,
            gas_buffer_pct: Option<u64>,
            expected: Option<u64>,
        }

        for case in [
            Case {
                user_gas_limit: Some(123_000),
                skip_estimate: false,
                quote_estimated_gas: Some(100_000),
                quote_gas_limit: Some(110_000),
                estimated_gas_from_node: Some(120_000),
                gas_buffer_pct: Some(20),
                expected: Some(123_000),
            },
            Case {
                user_gas_limit: None,
                skip_estimate: true,
                quote_estimated_gas: Some(100_000),
                quote_gas_limit: Some(110_000),
                estimated_gas_from_node: Some(120_000),
                gas_buffer_pct: Some(20),
                expected: Some(100_000),
            },
            Case {
                user_gas_limit: None,
                skip_estimate: true,
                quote_estimated_gas: None,
                quote_gas_limit: Some(110_000),
                estimated_gas_from_node: Some(120_000),
                gas_buffer_pct: Some(20),
                expected: Some(110_000),
            },
            Case {
                user_gas_limit: None,
                skip_estimate: false,
                quote_estimated_gas: Some(100_000),
                quote_gas_limit: Some(110_000),
                estimated_gas_from_node: Some(120_000),
                gas_buffer_pct: Some(25),
                expected: Some(150_000),
            },
            Case {
                user_gas_limit: None,
                skip_estimate: false,
                quote_estimated_gas: Some(100_000),
                quote_gas_limit: Some(110_000),
                estimated_gas_from_node: None,
                gas_buffer_pct: Some(25),
                expected: None,
            },
        ] {
            assert_eq!(
                resolve_effective_gas_limit(
                    case.user_gas_limit,
                    case.skip_estimate,
                    case.quote_estimated_gas,
                    case.quote_gas_limit,
                    case.estimated_gas_from_node,
                    case.gas_buffer_pct,
                ),
                case.expected
            );
        }
    }

    #[tokio::test]
    async fn execute_quote_returns_no_wallet_when_not_dry_run_and_no_private_key() {
        let deps = MockExecuteDeps {
            estimated_gas: Ok(21_000),
            nonce: Ok(7),
            send_tx_result: Ok((Address::ZERO, "0xtx".to_string())),
            receipts: RefCell::new(vec![]),
        };
        let args = execute_args();
        let err = execute_quote_with_deps(&deps, &args, &test_config(1), &sample_quote(1))
            .await
            .unwrap_err();
        assert!(matches!(err, ChainError::NoWallet));
    }

    #[tokio::test]
    async fn execute_quote_dry_run_uses_wallet_fallbacks_without_rpc_calls() {
        let deps = MockExecuteDeps {
            estimated_gas: Ok(21_000),
            nonce: Ok(7),
            send_tx_result: Ok((Address::ZERO, "0xtx".to_string())),
            receipts: RefCell::new(vec![]),
        };
        let mut args = execute_args();
        args.dry_run = true;
        args.wallet = Some("0x2222222222222222222222222222222222222222".to_string());
        let result = execute_quote_with_deps(&deps, &args, &test_config(1), &sample_quote(1))
            .await
            .unwrap();
        assert!(matches!(result.status, ExecutionStatus::DryRun));
        assert_eq!(
            result.from_address.as_deref(),
            Some("0x2222222222222222222222222222222222222222")
        );
        assert!(result.tx_hash.is_none());
    }

    #[tokio::test]
    async fn execute_quote_rejects_invalid_router_before_sending() {
        let deps = MockExecuteDeps {
            estimated_gas: Ok(21_000),
            nonce: Ok(7),
            send_tx_result: Ok((Address::ZERO, "0xtx".to_string())),
            receipts: RefCell::new(vec![]),
        };
        let args = execute_args();
        let mut config = test_config(1);
        config.private_key =
            Some("0x59c6995e998f97a5a0044966f0945382dbf7f50a3f2f72f5f7a0b7d7d4f5e5f1".to_string());
        let mut quote = sample_quote(1);
        quote.router_to = "not-an-address".to_string();

        let err = execute_quote_with_deps(&deps, &args, &config, &quote)
            .await
            .unwrap_err();
        match err {
            ChainError::InvalidAddress(addr) => assert_eq!(addr, "not-an-address"),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[tokio::test]
    async fn execute_quote_submits_transaction_with_estimated_gas() {
        let signer = "0x59c6995e998f97a5a0044966f0945382dbf7f50a3f2f72f5f7a0b7d7d4f5e5f1";
        let expected_from = crate::chain::address_from_private_key(signer).unwrap();
        let deps = MockExecuteDeps {
            estimated_gas: Ok(50_000),
            nonce: Ok(7),
            send_tx_result: Ok((expected_from, "0xabc".to_string())),
            receipts: RefCell::new(vec![]),
        };
        let mut args = execute_args();
        let mut config = test_config(1);
        config.private_key = Some(signer.to_string());
        args.gas_buffer_pct = Some(20);
        let result = execute_quote_with_deps(&deps, &args, &config, &sample_quote(1))
            .await
            .unwrap();

        assert!(matches!(result.status, ExecutionStatus::Submitted));
        assert_eq!(result.tx_hash.as_deref(), Some("0xabc"));
        let expected_from_str = expected_from.to_string();
        assert_eq!(
            result.from_address.as_deref(),
            Some(expected_from_str.as_str())
        );
    }

    #[test]
    fn resolve_approve_targets_prefers_explicit_values_and_quote_fallbacks() {
        let quote = sample_quote_for_approve(1);
        let explicit_token = "0x2222222222222222222222222222222222222222";
        let explicit_spender = "0x3333333333333333333333333333333333333333";

        let resolved = resolve_approve_targets(
            Some(explicit_token),
            Some(&quote),
            Some(explicit_spender),
            1,
        )
        .unwrap();
        assert_eq!(
            resolved,
            (explicit_token.to_string(), explicit_spender.to_string())
        );

        let fallback = resolve_approve_targets(None, Some(&quote), None, 1).unwrap();
        assert_eq!(fallback.0, quote.from_token.address);
        assert!(fallback.1.starts_with("0x"));
    }

    #[test]
    fn resolve_approve_targets_errors_without_required_inputs() {
        let err = resolve_approve_targets(None, None, None, 1).unwrap_err();
        assert!(err.to_string().contains("--token or --quote-id"));
    }

    #[test]
    fn approve_uses_quote_chain_id_when_present() {
        let mut config = test_config(1);
        config.chain_id = 1;

        let quote = sample_quote_for_approve(42161);
        let effective_chain_id = quote.chain_id;

        let resolved =
            resolve_approve_targets(None, Some(&quote), None, effective_chain_id).unwrap();
        assert_eq!(effective_chain_id, 42161);
        assert_eq!(resolved.0, quote.from_token.address);
        assert!(resolved.1.starts_with("0x"));
    }

    #[test]
    fn calldata_builders_encode_expected_selector() {
        let spender: Address = "0x1111111111111111111111111111111111111111"
            .parse()
            .unwrap();
        let approve = approve_calldata(spender, alloy::primitives::U256::from(42u64));
        let revoke = revoke_calldata(spender);
        assert!(approve.starts_with("0x095ea7b3"));
        assert!(revoke.starts_with("0x095ea7b3"));
        assert!(approve.len() > revoke.len() - 1);
    }

    #[tokio::test]
    async fn send_approval_with_deps_returns_tx_hash_and_sender() {
        let private_key = "0x59c6995e998f97a5a0044966f0945382dbf7f50a3f2f72f5f7a0b7d7d4f5e5f1";
        let (
            coingecko_api_url,
            coingecko_api_key,
            dexscreener_api_url,
            okx_dex_api_url,
            okx_api_key,
            okx_api_secret,
            okx_api_passphrase,
            okx_project_id,
        ) = crate::config::test_metadata_config_fields();
        let signer = crate::chain::resolve_signer(&AppConfig {
            rpc_url: String::new(),
            rpc_url_overridden: false,
            chain_id: 1,
            private_key: Some(private_key.to_string()),
            keystore_path: None,
            keystore_password_file: None,
            keystore_password_env: None,
            wallet_address: None,
            dodo_api_url: String::new(),
            dodo_api_key: String::new(),
            dodo_project_id: String::new(),
            coingecko_api_url,
            coingecko_api_key,
            dexscreener_api_url,
            okx_dex_api_url,
            okx_api_key,
            okx_api_secret,
            okx_api_passphrase,
            okx_project_id,
            data_dir: std::env::temp_dir(),
        })
        .unwrap();
        let expected_from = signer.address();
        let deps = MockApprovalDeps {
            send_tx_result: Ok((expected_from, "0xapprove".to_string())),
        };
        let token: Address = "0x2222222222222222222222222222222222222222"
            .parse()
            .unwrap();
        let spender: Address = "0x3333333333333333333333333333333333333333"
            .parse()
            .unwrap();
        let result = send_approval_with_deps(
            &deps,
            1,
            signer,
            token,
            spender,
            "1000".to_string(),
            &approve_calldata(spender, alloy::primitives::U256::from(1000u64)),
        )
        .await
        .unwrap();

        assert!(!result.dry_run);
        assert_eq!(result.tx_hash.as_deref(), Some("0xapprove"));
        assert_eq!(
            result.from_address.as_deref(),
            Some(expected_from.to_string().as_str())
        );
    }
}
