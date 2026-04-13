use chrono::Utc;
use std::process::ExitCode;

use crate::api::ApiClients;
use crate::chain::OnChainClient;
use crate::cli::swap::{
    ApproveArgs, ExecuteArgs, HistoryArgs, QuoteArgs, RevokeArgs, SimulateArgs, StatusArgs,
    SwapAction, SwapCmd,
};
use crate::commands::{resolve_token, to_raw_amount};
use crate::config::AppConfig;
use crate::error::{ChainError, Result};
use crate::models::quote::QuoteRequest;
use crate::models::swap::{ExecutionResult, ExecutionStatus, SimulationResult};
use crate::output::{OutputMode, TableRenderable};
use crate::store::QuoteStore;

pub async fn handle(
    cmd: SwapCmd,
    config: &AppConfig,
    store: &QuoteStore,
    api: &ApiClients,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match cmd.action {
        SwapAction::Quote(args) => quote(args, config, store, api, onchain, output_mode).await,
        SwapAction::Simulate(args) => simulate(args, config, store, onchain, output_mode).await,
        SwapAction::Execute(args) => execute(args, config, store, onchain, output_mode).await,
        SwapAction::Status(args) => status(args, config, onchain, output_mode).await,
        SwapAction::History(args) => history(args, config, store, output_mode).await,
        SwapAction::Approve(args) => approve(args, config, store, onchain, output_mode).await,
        SwapAction::Revoke(args) => revoke(args, config, onchain, output_mode).await,
    }
}

async fn quote(
    args: QuoteArgs,
    config: &AppConfig,
    store: &QuoteStore,
    api: &ApiClients,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    use crate::chain::{get_allowance, get_balance, get_eth_balance, OnChainClient};
    use alloy::primitives::Address;

    let chain_onchain = OnChainClient::for_chain(config, args.chain_id).await?;
    let onchain = &chain_onchain;

    let from_token = resolve_token(&args.from, args.chain_id, onchain, api, config).await?;
    let to_token = resolve_token(&args.to, args.chain_id, onchain, api, config).await?;
    let user_addr = match config.wallet_address.as_deref() {
        Some(addr) => match addr.parse::<Address>() {
            Ok(parsed) => parsed.to_string(),
            Err(_) => return Err(ChainError::InvalidAddress(addr.to_string()).into()),
        },
        None => Address::ZERO.to_string(),
    };

    let req = QuoteRequest {
        from: from_token.address.clone(),
        to: to_token.address.clone(),
        amount: args.amount,
        chain_id: args.chain_id,
        slippage: args.slippage,
    };

    let estimate_gas = user_addr != Address::ZERO.to_string();
    let quote = match api
        .dodo
        .get_route(
            &req,
            &from_token,
            &to_token,
            &user_addr,
            estimate_gas,
            config.quote_ttl_secs,
        )
        .await
    {
        Ok(quote) => quote,
        Err(err) if estimate_gas => {
            let wallet_addr = user_addr
                .parse::<Address>()
                .map_err(|_| ChainError::InvalidAddress(user_addr.clone()))?;
            let amount_raw = to_raw_amount(args.amount, from_token.decimals);
            let need_raw: u128 = amount_raw.parse().unwrap_or(0);
            let is_native = from_token
                .address
                .eq_ignore_ascii_case(crate::config::chains::NATIVE_ADDR);

            if is_native {
                let (have_raw, _) = get_eth_balance(onchain, wallet_addr).await?;
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
                let (have_raw, _) = get_balance(onchain, token_addr, wallet_addr).await?;
                let have: u128 = have_raw.parse().unwrap_or(0);
                if have < need_raw {
                    return Err(ChainError::InsufficientBalance {
                        have: have_raw,
                        need: amount_raw,
                        token: from_token.symbol.clone(),
                    }
                    .into());
                }

                if let Some(chain_cfg) = config.chain_config() {
                    let spender = chain_cfg
                        .contracts
                        .dodo_approve
                        .parse::<Address>()
                        .map_err(|_| {
                            ChainError::InvalidAddress(chain_cfg.contracts.dodo_approve.to_string())
                        })?;
                    let allowance_raw =
                        get_allowance(onchain, token_addr, wallet_addr, spender).await?;
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

            match api
                .dodo
                .get_route(
                    &req,
                    &from_token,
                    &to_token,
                    &user_addr,
                    false,
                    config.quote_ttl_secs,
                )
                .await
            {
                Ok(quote) => quote,
                Err(_) => return Err(err.into()),
            }
        }
        Err(err) => return Err(err.into()),
    };
    store.save_quote(&quote)?;

    Ok(crate::output::print_output::<crate::models::quote::Quote>(
        Ok(quote),
        "swap.quote",
        output_mode,
    ))
}

async fn simulate(
    args: SimulateArgs,
    _config: &AppConfig,
    store: &QuoteStore,
    onchain: &OnChainClient,
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
            ));
        }
    };

    if Utc::now() > quote_data.expires_at {
        return Ok(crate::output::print_output::<SimulationResult>(
            Err(ChainError::QuoteExpired(args.quote_id)),
            "swap.simulate",
            output_mode,
        ));
    }

    let mut warnings = Vec::new();
    let mut is_valid = true;

    let mut estimated_gas = quote_data.estimated_gas.or(quote_data.gas_limit);
    if estimated_gas.is_none() {
        warnings.push("Quote did not include gas estimate or gas limit.".to_string());
    }

    if quote_data.route_summary.is_empty() {
        warnings.push("Route summary is empty in the quote response.".to_string());
    }

    let gas_price_gwei = match crate::chain::get_gas_price_gwei(onchain).await {
        Ok(price) => price,
        Err(e) => {
            warnings.push(format!("Failed to fetch current gas price: {}", e));
            0.0
        }
    };

    let mut total_gas_cost_eth = estimated_gas
        .map(|gas| gas as f64 * gas_price_gwei / 1e9)
        .unwrap_or(0.0);

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
                let from_amount_raw = (quote_data.from_amount_display
                    * 10f64.powi(quote_data.from_token.decimals as i32))
                    as u128;

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
                                total_gas_cost_eth = gas as f64 * gas_price_gwei / 1e9;
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
    ))
}

async fn execute(
    args: ExecuteArgs,
    config: &AppConfig,
    store: &QuoteStore,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    use crate::chain::{
        address_from_private_key, estimate_gas, get_nonce, get_tx_receipt, send_tx,
    };
    use alloy::primitives::Address;
    use std::time::Duration;

    let quote_data = match store.load_quote(&args.quote_id)? {
        Some(q) => q,
        None => {
            return Ok(crate::output::print_output::<ExecutionResult>(
                Err(ChainError::QuoteNotFound(args.quote_id.clone())),
                "swap.execute",
                output_mode,
            ));
        }
    };

    if Utc::now() > quote_data.expires_at {
        return Ok(crate::output::print_output::<ExecutionResult>(
            Err(ChainError::QuoteExpired(args.quote_id.clone())),
            "swap.execute",
            output_mode,
        ));
    }

    let chain_onchain = OnChainClient::for_chain(config, quote_data.chain_id).await?;
    let onchain = &chain_onchain;
    let exec_rpc_url = crate::config::chain_config(quote_data.chain_id)
        .and_then(|c| c.rpc_urls.first().copied())
        .unwrap_or(config.rpc_url.as_str())
        .to_string();

    // Resolve private key: CLI arg takes precedence over env (already handled by clap)
    let private_key = args.private_key.as_deref();

    if !args.dry_run && private_key.is_none() {
        return Ok(crate::output::print_output::<ExecutionResult>(
            Err(ChainError::NoWallet),
            "swap.execute",
            output_mode,
        ));
    }

    let (from_address, tx_hash, status, gas_used, effective_gas_price_gwei) = if args.dry_run {
        // In dry-run, derive from_address from private key if available,
        // else from the subcommand wallet, else from the global wallet address.
        let addr = private_key
            .and_then(|pk| address_from_private_key(pk).ok())
            .map(|a| a.to_string())
            .or_else(|| args.wallet.clone())
            .or_else(|| config.wallet_address.clone());
        (addr, None, ExecutionStatus::DryRun, None, None)
    } else {
        let pk = private_key.unwrap(); // safe: checked above
        let to_addr: Address = match quote_data.router_to.parse() {
            Ok(a) => a,
            Err(_) => {
                return Ok(crate::output::print_output::<ExecutionResult>(
                    Err(ChainError::InvalidAddress(quote_data.router_to.clone())),
                    "swap.execute",
                    output_mode,
                ));
            }
        };

        // Derive from_address early; needed for eth_estimateGas and populating the result.
        let from_addr = match address_from_private_key(pk) {
            Ok(a) => a,
            Err(e) => {
                return Ok(crate::output::print_output::<ExecutionResult>(
                    Err(e),
                    "swap.execute",
                    output_mode,
                ));
            }
        };

        // eth_estimateGas: more accurate than the quote's stale estimate, and catches
        // reverts before we pay gas to broadcast a doomed transaction.
        // Precedence: --gas-limit > eth_estimateGas (unless --skip-estimate) > quote gas > None (node estimates)
        let effective_gas_limit = if let Some(user_gas) = args.gas_limit {
            Some(user_gas)
        } else if args.skip_estimate {
            quote_data.estimated_gas.or(quote_data.gas_limit)
        } else {
            match estimate_gas(
                onchain,
                from_addr,
                to_addr,
                &quote_data.calldata,
                &quote_data.value,
            )
            .await
            {
                Ok(gas) => Some(if let Some(pct) = args.gas_buffer_pct {
                    gas + gas * pct / 100
                } else {
                    gas
                }),
                Err(e) => {
                    return Ok(crate::output::print_output::<ExecutionResult>(
                        Err(e),
                        "swap.execute",
                        output_mode,
                    ));
                }
            }
        };

        // Snapshot confirmed nonce before broadcasting; used to detect cancellation during --wait.
        let tx_nonce = get_nonce(onchain, from_addr).await.unwrap_or(0);

        match send_tx(
            &exec_rpc_url,
            quote_data.chain_id,
            pk,
            to_addr,
            &quote_data.calldata,
            &quote_data.value,
            effective_gas_limit,
            args.max_fee_gwei,
        )
        .await
        {
            Err(e) => {
                return Ok(crate::output::print_output::<ExecutionResult>(
                    Err(e),
                    "swap.execute",
                    output_mode,
                ));
            }
            Ok((_from_addr, hash)) => {
                let from_str = Some(from_addr.to_string());
                let tx_hash_str = hash;

                // Optionally wait for the tx to be mined and report the final on-chain status.
                // Possible outcomes:
                //   Confirmed  — mined, EVM execution succeeded (receipt.status = 1)
                //   Failed     — mined, EVM execution reverted (receipt.status = 0)
                //   Cancelled  — nonce advanced with no receipt: tx was replaced or dropped
                //   Submitted  — still in mempool after timeout (rare; tx not yet mined)
                let (final_status, gas_used, effective_gas_price_gwei) = if args.wait {
                    const POLL_INTERVAL: Duration = Duration::from_secs(3);
                    const MAX_POLLS: u32 = 100; // ~5 minutes
                    let mut polls = 0u32;
                    let mut outcome = (ExecutionStatus::Submitted, None, None);
                    loop {
                        tokio::time::sleep(POLL_INTERVAL).await;
                        // 1. Check for receipt first.
                        match get_tx_receipt(onchain, &tx_hash_str).await {
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
                        // 2. No receipt yet — check if nonce advanced (tx replaced/dropped).
                        if let Ok(current_nonce) = get_nonce(onchain, from_addr).await {
                            if current_nonce > tx_nonce {
                                outcome = (ExecutionStatus::Cancelled, None, None);
                                break;
                            }
                        }
                        polls += 1;
                        if polls >= MAX_POLLS {
                            eprintln!(
                                "Warning: timed out waiting for receipt; tx may still be pending."
                            );
                            break;
                        }
                    }
                    outcome
                } else {
                    (ExecutionStatus::Submitted, None, None)
                };

                (
                    from_str,
                    Some(tx_hash_str),
                    final_status,
                    gas_used,
                    effective_gas_price_gwei,
                )
            }
        }
    };

    let result = ExecutionResult {
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
    ))
}

async fn status(
    args: StatusArgs,
    config: &AppConfig,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    match crate::chain::get_tx_receipt(onchain, &args.tx_hash).await {
        Ok(Some(status)) => {
            println!("{:?}", status);
            Ok(ExitCode::SUCCESS)
        }
        Ok(None) => {
            eprintln!("Transaction not found or pending");
            Ok(ExitCode::FAILURE)
        }
        Err(e) => Ok(crate::output::print_output::<crate::chain::TxStatus>(
            Err(e),
            "swap.status",
            output_mode,
        )),
    }
}

async fn history(
    args: HistoryArgs,
    _config: &AppConfig,
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
    >(Ok(records), "swap.history", output_mode))
}

async fn approve(
    args: ApproveArgs,
    config: &AppConfig,
    store: &QuoteStore,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    use crate::chain::{address_from_private_key, get_token_info, send_tx, OnChainClient};
    use crate::models::swap::ApprovalResult;
    use alloy::primitives::{Address, U256};

    let chain_client = OnChainClient::for_chain(config, args.chain_id).await?;
    let onchain = &chain_client;
    let chain_rpc = crate::config::chain_config(args.chain_id)
        .and_then(|c| c.rpc_urls.first().copied())
        .unwrap_or(config.rpc_url.as_str())
        .to_string();

    // Load quote if provided; used as fallback for token and spender.
    let quote = match &args.quote_id {
        Some(id) => store.load_quote(id)?,
        None => None,
    };

    // Resolve token address: explicit arg > quote's from-token.
    let token_str = args
        .token
        .as_deref()
        .map(|s| s.to_string())
        .or_else(|| quote.as_ref().map(|q| q.from_token.address.clone()))
        .ok_or_else(|| ChainError::Config("--token or --quote-id is required".to_string()))?;

    // Resolve spender: explicit --spender takes highest precedence.
    // When --quote-id is given without --spender, use the chain's DODOApprove contract.
    let spender_str = if let Some(s) = args.spender.as_deref() {
        s.to_string()
    } else if quote.is_some() {
        crate::config::chain_config(args.chain_id)
            .map(|c| c.contracts.dodo_approve.to_string())
            .ok_or_else(|| {
                ChainError::Config(format!(
                    "no chain config for chain_id {}; use --spender to set the approve address",
                    args.chain_id
                ))
            })?
    } else {
        return Err(ChainError::Config(
            "--spender or --quote-id is required".to_string(),
        ));
    };

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
            let raw = (human * 10f64.powi(decimals as i32)) as u128;
            (U256::from(raw), raw.to_string())
        }
    };

    // ABI-encode approve(address spender, uint256 amount):
    //   selector  = keccak256("approve(address,uint256)")[0..4] = 0x095ea7b3
    //   arg[0]    = spender padded to 32 bytes (12 zeros + 20-byte address)
    //   arg[1]    = amount as 32-byte big-endian
    let selector = [0x09u8, 0x5e, 0xa7, 0xb3];
    let mut calldata = Vec::with_capacity(68);
    calldata.extend_from_slice(&selector);
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(spender_addr.as_slice());
    calldata.extend_from_slice(&amount_u256.to_be_bytes::<32>());
    let calldata_hex = format!("0x{}", hex::encode(&calldata));

    let private_key = args.private_key.as_deref();

    if args.dry_run || private_key.is_none() {
        let from_address = private_key
            .and_then(|pk| address_from_private_key(pk).ok())
            .map(|a| a.to_string());
        let result = ApprovalResult {
            token: token_addr.to_string(),
            spender: spender_addr.to_string(),
            raw_amount: raw_amount_str,
            dry_run: true,
            tx_hash: None,
            from_address,
        };
        return Ok(crate::output::print_output::<ApprovalResult>(
            Ok(result),
            "swap.approve",
            output_mode,
        ));
    }

    let pk = private_key.unwrap();
    let from_addr = match address_from_private_key(pk) {
        Ok(a) => a,
        Err(e) => {
            return Ok(crate::output::print_output::<ApprovalResult>(
                Err(e),
                "swap.approve",
                output_mode,
            ));
        }
    };

    match send_tx(
        &chain_rpc,
        args.chain_id,
        pk,
        token_addr,
        &calldata_hex,
        "0x0",
        None,
        None,
    )
    .await
    {
        Err(e) => Ok(crate::output::print_output::<ApprovalResult>(
            Err(e),
            "swap.approve",
            output_mode,
        )),
        Ok((_addr, tx_hash)) => {
            let result = ApprovalResult {
                token: token_addr.to_string(),
                spender: spender_addr.to_string(),
                raw_amount: raw_amount_str,
                dry_run: false,
                tx_hash: Some(tx_hash),
                from_address: Some(from_addr.to_string()),
            };
            Ok(crate::output::print_output::<ApprovalResult>(
                Ok(result),
                "swap.approve",
                output_mode,
            ))
        }
    }
}

async fn revoke(
    args: RevokeArgs,
    config: &AppConfig,
    onchain: &OnChainClient,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    use crate::chain::{address_from_private_key, send_tx};
    use crate::models::swap::ApprovalResult;
    use alloy::primitives::Address;

    let chain_rpc = crate::config::chain_config(args.chain_id)
        .and_then(|c| c.rpc_urls.first().copied())
        .unwrap_or(config.rpc_url.as_str())
        .to_string();

    let token_addr: Address = match args.token.parse() {
        Ok(a) => a,
        Err(_) => {
            return Ok(crate::output::print_output::<ApprovalResult>(
                Err(ChainError::InvalidAddress(args.token.clone())),
                "swap.revoke",
                output_mode,
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
            ));
        }
    };

    // approve(address,uint256) with amount 0 revokes ERC-20 allowance.
    let selector = [0x09u8, 0x5e, 0xa7, 0xb3];
    let mut calldata = Vec::with_capacity(68);
    calldata.extend_from_slice(&selector);
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(spender_addr.as_slice());
    calldata.extend_from_slice(&[0u8; 32]);
    let calldata_hex = format!("0x{}", hex::encode(&calldata));

    let private_key = args.private_key.as_deref();

    if args.dry_run || private_key.is_none() {
        let from_address = private_key
            .and_then(|pk| address_from_private_key(pk).ok())
            .map(|a| a.to_string());
        let result = ApprovalResult {
            token: token_addr.to_string(),
            spender: spender_addr.to_string(),
            raw_amount: "0".to_string(),
            dry_run: true,
            tx_hash: None,
            from_address,
        };
        return Ok(crate::output::print_output::<ApprovalResult>(
            Ok(result),
            "swap.revoke",
            output_mode,
        ));
    }

    let pk = private_key.unwrap();
    let from_addr = match address_from_private_key(pk) {
        Ok(a) => a,
        Err(e) => {
            return Ok(crate::output::print_output::<ApprovalResult>(
                Err(e),
                "swap.revoke",
                output_mode,
            ));
        }
    };

    match send_tx(
        &chain_rpc,
        args.chain_id,
        pk,
        token_addr,
        &calldata_hex,
        "0x0",
        None,
        None,
    )
    .await
    {
        Err(e) => Ok(crate::output::print_output::<ApprovalResult>(
            Err(e),
            "swap.revoke",
            output_mode,
        )),
        Ok((_addr, tx_hash)) => {
            let result = ApprovalResult {
                token: token_addr.to_string(),
                spender: spender_addr.to_string(),
                raw_amount: "0".to_string(),
                dry_run: false,
                tx_hash: Some(tx_hash),
                from_address: Some(from_addr.to_string()),
            };
            Ok(crate::output::print_output::<ApprovalResult>(
                Ok(result),
                "swap.revoke",
                output_mode,
            ))
        }
    }
}
