use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use std::process::ExitCode;
use std::time::Duration;

use crate::api::ApiClients;
use crate::chain::OnChainClient;
use crate::cli::token::{
    TokenAction, TokenCmd, TokenCreateAction, TokenCreateCustomArgs, TokenCreateMintableArgs,
    TokenCreateStdArgs, TokenFeeArgs, TokenIdentArg, TokenMintArgs, TokenOwnershipArgs,
};
use crate::commands::resolve_token;
use crate::config::AppConfig;
use crate::error::{ChainError, Result};
use crate::models::token::{
    TokenCreateFee, TokenCreateResult, TokenMintResult, TokenOwnershipActionResult,
};
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
        TokenAction::Create(cmd) => match cmd.action {
            TokenCreateAction::Std(args) => create_std(args, config, store, output_mode).await,
            TokenCreateAction::Custom(args) => {
                create_custom(args, config, store, output_mode).await
            }
            TokenCreateAction::Mintable(args) => {
                create_mintable(args, config, store, output_mode).await
            }
        },
        TokenAction::Fee(args) => fee(args, config, output_mode).await,
        TokenAction::Mint(args) => mint(args, config, output_mode).await,
        TokenAction::RenounceOwnership(args) => renounce_ownership(args, config, output_mode).await,
    }
}

async fn info(
    args: TokenIdentArg,
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
    let is_native = token_ref
        .address
        .eq_ignore_ascii_case(crate::config::chains::NATIVE_ADDR);
    let info = if is_native {
        crate::config::chain_config(chain_id)
            .map(|cc| crate::models::token::TokenInfo {
                address: token_ref.address.clone(),
                symbol: cc.native_token.symbol.to_string(),
                name: cc.native_token.name.to_string(),
                decimals: cc.native_token.decimals,
                chain_id,
                chain: Some(cc.name.to_string()),
                website: None,
                social_links: crate::models::token::TokenSocialLinks::default(),
                price: None,
                market_cap: None,
                fdv: None,
                primary_liquidity: None,
                volume_24h: None,
                price_change_24h: None,
                risk_level: None,
                sources: crate::models::token::TokenInfoSources {
                    identity: Some("chain-config".to_string()),
                    chain: Some("chain-config".to_string()),
                    ..crate::models::token::TokenInfoSources::default()
                },
            })
            .ok_or(ChainError::UnsupportedChain(chain_id))?
    } else {
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
        match crate::chain::get_token_info(onchain, addr).await {
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
    args: TokenIdentArg,
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

async fn fee(_args: TokenFeeArgs, config: &AppConfig, output_mode: OutputMode) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let factory = match factory_address(config, chain_id) {
        Ok(factory) => factory,
        Err(e) => {
            return Ok(crate::output::print_output::<TokenCreateFee>(
                Err(e),
                "token.fee",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let fee_raw = match crate::chain::get_create_fee(&chain_client, factory).await {
        Ok(fee) => fee,
        Err(e) => {
            return Ok(crate::output::print_output::<TokenCreateFee>(
                Err(e),
                "token.fee",
                output_mode,
                OutputContext::new(chain_id, false),
            ));
        }
    };

    let chain_cfg = config.chain_config_for(chain_id);
    let native_symbol = chain_cfg
        .map(|cfg| cfg.native_token.symbol)
        .unwrap_or("ETH");
    let native_decimals = chain_cfg
        .map(|cfg| cfg.native_token.decimals)
        .unwrap_or(18);
    let result = TokenCreateFee {
        chain_id,
        factory: factory.to_string(),
        fee_display: format_units_to_f64(&fee_raw, native_decimals),
        fee_raw,
        fee_symbol: native_symbol.to_string(),
    };

    Ok(crate::output::print_output::<TokenCreateFee>(
        Ok(result),
        "token.fee",
        output_mode,
        OutputContext::new(chain_id, false),
    ))
}

async fn create_custom(
    args: TokenCreateCustomArgs,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let dry_run = args.dry_run;
    let result = create_custom_impl(&args, config, store).await;
    Ok(crate::output::print_output::<TokenCreateResult>(
        result,
        "token.create.custom",
        output_mode,
        OutputContext::new(chain_id, dry_run),
    ))
}

async fn create_mintable(
    args: TokenCreateMintableArgs,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let dry_run = args.dry_run;
    let result = create_mintable_impl(&args, config, store).await;
    Ok(crate::output::print_output::<TokenCreateResult>(
        result,
        "token.create.mintable",
        output_mode,
        OutputContext::new(chain_id, dry_run),
    ))
}

async fn create_std(
    args: TokenCreateStdArgs,
    config: &AppConfig,
    store: &QuoteStore,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let dry_run = args.dry_run;
    let result = create_std_impl(&args, config, store).await;
    Ok(crate::output::print_output::<TokenCreateResult>(
        result,
        "token.create.std",
        output_mode,
        OutputContext::new(chain_id, dry_run),
    ))
}

async fn create_std_impl(
    args: &TokenCreateStdArgs,
    config: &AppConfig,
    store: &QuoteStore,
) -> Result<TokenCreateResult> {
    validate_create_std_args(args)?;

    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let factory = factory_address(config, chain_id)?;
    let fee_raw = crate::chain::get_create_fee(&chain_client, factory).await?;
    let supply_raw = crate::commands::to_raw_amount(&args.supply, args.decimals)?;
    let supply_u256 = U256::from_str_radix(&supply_raw, 10)
        .map_err(|_| ChainError::InvalidAmount(args.supply.clone()))?;
    let calldata = crate::chain::encode_create_std_calldata(
        supply_u256,
        args.name.trim().to_string(),
        args.symbol.trim().to_string(),
        args.decimals,
    );
    let from_address = resolve_sender_address(config)?;
    let estimated_gas = match from_address {
        Some(from) => crate::chain::estimate_gas(&chain_client, from, factory, &calldata, &fee_raw)
            .await
            .ok(),
        None => None,
    };

    if args.dry_run {
        return Ok(TokenCreateResult {
            chain_id,
            dry_run: true,
            factory: factory.to_string(),
            method: "createStdERC20".to_string(),
            token_name: args.name.trim().to_string(),
            token_symbol: args.symbol.trim().to_string(),
            decimals: args.decimals,
            supply_raw,
            supply_display: args.supply.trim().to_string(),
            calldata,
            value: fee_raw,
            from_address: from_address.map(|addr| addr.to_string()),
            estimated_gas,
            tx_hash: None,
            new_token_address: None,
        });
    }

    let signer = crate::chain::resolve_signer(config)?;
    let signer_address = signer.address().to_string();
    let (_from, tx_hash) = crate::chain::send_tx(
        &config.rpc_url_for_chain(chain_id),
        chain_id,
        signer,
        factory,
        &calldata,
        &fee_raw,
        estimated_gas,
        None,
    )
    .await?;
    let new_token_address =
        wait_for_created_token_address(&chain_client, &tx_hash, factory).await?;
    persist_created_token(store, &chain_client, new_token_address.as_deref()).await;

    Ok(TokenCreateResult {
        chain_id,
        dry_run: false,
        factory: factory.to_string(),
        method: "createStdERC20".to_string(),
        token_name: args.name.trim().to_string(),
        token_symbol: args.symbol.trim().to_string(),
        decimals: args.decimals,
        supply_raw,
        supply_display: args.supply.trim().to_string(),
        calldata,
        value: fee_raw,
        from_address: Some(signer_address),
        estimated_gas,
        tx_hash: Some(tx_hash),
        new_token_address,
    })
}

async fn create_mintable_impl(
    args: &TokenCreateMintableArgs,
    config: &AppConfig,
    store: &QuoteStore,
) -> Result<TokenCreateResult> {
    validate_create_params(&args.name, &args.symbol, &args.supply, args.decimals)?;

    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let factory = factory_address(config, chain_id)?;
    let fee_raw = crate::chain::get_create_fee(&chain_client, factory).await?;
    let supply_raw = crate::commands::to_raw_amount(&args.supply, args.decimals)?;
    let supply_u256 = U256::from_str_radix(&supply_raw, 10)
        .map_err(|_| ChainError::InvalidAmount(args.supply.clone()))?;
    let burn_ratio = parse_ratio_hundredths(&args.burn_pct)?;
    let fee_ratio = parse_ratio_hundredths(&args.fee_pct)?;
    let owner = resolve_create_owner_address(config, args.owner.as_deref())?;
    let calldata = crate::chain::encode_create_mintable_calldata(
        supply_u256,
        args.name.trim().to_string(),
        args.symbol.trim().to_string(),
        args.decimals,
        burn_ratio,
        fee_ratio,
        owner,
    );
    let from_address = resolve_sender_address(config)?;
    let estimated_gas = match from_address {
        Some(from) => crate::chain::estimate_gas(&chain_client, from, factory, &calldata, &fee_raw)
            .await
            .ok(),
        None => None,
    };

    if args.dry_run {
        return Ok(TokenCreateResult {
            chain_id,
            dry_run: true,
            factory: factory.to_string(),
            method: "createCustomMintableERC20".to_string(),
            token_name: args.name.trim().to_string(),
            token_symbol: args.symbol.trim().to_string(),
            decimals: args.decimals,
            supply_raw,
            supply_display: args.supply.trim().to_string(),
            calldata,
            value: fee_raw,
            from_address: from_address.map(|addr| addr.to_string()),
            estimated_gas,
            tx_hash: None,
            new_token_address: None,
        });
    }

    let signer = crate::chain::resolve_signer(config)?;
    let signer_address = signer.address().to_string();
    let (_from, tx_hash) = crate::chain::send_tx(
        &config.rpc_url_for_chain(chain_id),
        chain_id,
        signer,
        factory,
        &calldata,
        &fee_raw,
        estimated_gas,
        None,
    )
    .await?;
    let new_token_address =
        wait_for_created_token_address(&chain_client, &tx_hash, factory).await?;
    persist_created_token(store, &chain_client, new_token_address.as_deref()).await;

    Ok(TokenCreateResult {
        chain_id,
        dry_run: false,
        factory: factory.to_string(),
        method: "createCustomMintableERC20".to_string(),
        token_name: args.name.trim().to_string(),
        token_symbol: args.symbol.trim().to_string(),
        decimals: args.decimals,
        supply_raw,
        supply_display: args.supply.trim().to_string(),
        calldata,
        value: fee_raw,
        from_address: Some(signer_address),
        estimated_gas,
        tx_hash: Some(tx_hash),
        new_token_address,
    })
}

async fn create_custom_impl(
    args: &TokenCreateCustomArgs,
    config: &AppConfig,
    store: &QuoteStore,
) -> Result<TokenCreateResult> {
    validate_create_params(&args.name, &args.symbol, &args.supply, args.decimals)?;

    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let factory = factory_address(config, chain_id)?;
    let fee_raw = crate::chain::get_create_fee(&chain_client, factory).await?;
    let supply_raw = crate::commands::to_raw_amount(&args.supply, args.decimals)?;
    let supply_u256 = U256::from_str_radix(&supply_raw, 10)
        .map_err(|_| ChainError::InvalidAmount(args.supply.clone()))?;
    let burn_ratio = parse_ratio_hundredths(&args.burn_pct)?;
    let fee_ratio = parse_ratio_hundredths(&args.fee_pct)?;
    let team_account = resolve_create_owner_address(config, args.team_account.as_deref())?;
    let calldata = crate::chain::encode_create_custom_calldata(
        supply_u256,
        args.name.trim().to_string(),
        args.symbol.trim().to_string(),
        args.decimals,
        burn_ratio,
        fee_ratio,
        team_account,
    );
    let from_address = resolve_sender_address(config)?;
    let estimated_gas = match from_address {
        Some(from) => crate::chain::estimate_gas(&chain_client, from, factory, &calldata, &fee_raw)
            .await
            .ok(),
        None => None,
    };

    if args.dry_run {
        return Ok(TokenCreateResult {
            chain_id,
            dry_run: true,
            factory: factory.to_string(),
            method: "createCustomERC20".to_string(),
            token_name: args.name.trim().to_string(),
            token_symbol: args.symbol.trim().to_string(),
            decimals: args.decimals,
            supply_raw,
            supply_display: args.supply.trim().to_string(),
            calldata,
            value: fee_raw,
            from_address: from_address.map(|addr| addr.to_string()),
            estimated_gas,
            tx_hash: None,
            new_token_address: None,
        });
    }

    let signer = crate::chain::resolve_signer(config)?;
    let signer_address = signer.address().to_string();
    let (_from, tx_hash) = crate::chain::send_tx(
        &config.rpc_url_for_chain(chain_id),
        chain_id,
        signer,
        factory,
        &calldata,
        &fee_raw,
        estimated_gas,
        None,
    )
    .await?;
    let new_token_address =
        wait_for_created_token_address(&chain_client, &tx_hash, factory).await?;
    persist_created_token(store, &chain_client, new_token_address.as_deref()).await;

    Ok(TokenCreateResult {
        chain_id,
        dry_run: false,
        factory: factory.to_string(),
        method: "createCustomERC20".to_string(),
        token_name: args.name.trim().to_string(),
        token_symbol: args.symbol.trim().to_string(),
        decimals: args.decimals,
        supply_raw,
        supply_display: args.supply.trim().to_string(),
        calldata,
        value: fee_raw,
        from_address: Some(signer_address),
        estimated_gas,
        tx_hash: Some(tx_hash),
        new_token_address,
    })
}

async fn renounce_ownership(
    args: TokenOwnershipArgs,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let dry_run = args.dry_run;
    let result = renounce_ownership_impl(&args, config).await;
    Ok(crate::output::print_output::<TokenOwnershipActionResult>(
        result,
        "token.renounce-ownership",
        output_mode,
        OutputContext::new(chain_id, dry_run),
    ))
}

async fn renounce_ownership_impl(
    args: &TokenOwnershipArgs,
    config: &AppConfig,
) -> Result<TokenOwnershipActionResult> {
    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;
    let token: Address = args
        .token
        .parse()
        .map_err(|_| ChainError::InvalidAddress(args.token.clone()))?;
    let calldata = crate::chain::encode_abandon_ownership_calldata();
    let from_address = resolve_sender_address(config)?;
    let estimated_gas = match from_address {
        Some(from) => crate::chain::estimate_gas(&chain_client, from, token, &calldata, "0")
            .await
            .ok(),
        None => None,
    };

    if args.dry_run {
        return Ok(TokenOwnershipActionResult {
            chain_id,
            dry_run: true,
            action: "renounce_ownership".to_string(),
            token: token.to_string(),
            calldata,
            from_address: from_address.map(|addr| addr.to_string()),
            estimated_gas,
            tx_hash: None,
        });
    }

    let signer = crate::chain::resolve_signer(config)?;
    let signer_address = signer.address().to_string();
    let (_from, tx_hash) = crate::chain::send_tx(
        &config.rpc_url_for_chain(chain_id),
        chain_id,
        signer,
        token,
        &calldata,
        "0",
        estimated_gas,
        None,
    )
    .await?;

    Ok(TokenOwnershipActionResult {
        chain_id,
        dry_run: false,
        action: "renounce_ownership".to_string(),
        token: token.to_string(),
        calldata,
        from_address: Some(signer_address),
        estimated_gas,
        tx_hash: Some(tx_hash),
    })
}

async fn mint(
    args: TokenMintArgs,
    config: &AppConfig,
    output_mode: OutputMode,
) -> Result<ExitCode> {
    let chain_id = config.chain_id;
    let dry_run = args.dry_run;
    let result = mint_impl(&args, config).await;
    Ok(crate::output::print_output::<TokenMintResult>(
        result,
        "token.mint",
        output_mode,
        OutputContext::new(chain_id, dry_run),
    ))
}

async fn mint_impl(args: &TokenMintArgs, config: &AppConfig) -> Result<TokenMintResult> {
    let chain_id = config.chain_id;
    let chain_client = OnChainClient::for_chain(config, chain_id).await?;

    let token: Address = args
        .token
        .parse()
        .map_err(|_| ChainError::InvalidAddress(args.token.clone()))?;
    let recipient: Address = args
        .to
        .parse()
        .map_err(|_| ChainError::InvalidAddress(args.to.clone()))?;
    let token_info = crate::chain::get_token_info(&chain_client, token).await?;
    let amount_raw = crate::commands::to_raw_amount(&args.amount, token_info.decimals)?;
    let amount_u256 = U256::from_str_radix(&amount_raw, 10)
        .map_err(|_| ChainError::InvalidAmount(args.amount.clone()))?;
    let calldata = crate::chain::encode_mint_calldata(recipient, amount_u256);
    let from_address = resolve_sender_address(config)?;
    let estimated_gas = match from_address {
        Some(from) => crate::chain::estimate_gas(&chain_client, from, token, &calldata, "0")
            .await
            .ok(),
        None => None,
    };

    if args.dry_run {
        return Ok(TokenMintResult {
            chain_id,
            dry_run: true,
            token: token.to_string(),
            to: recipient.to_string(),
            amount_raw,
            amount_display: args.amount.trim().to_string(),
            calldata,
            from_address: from_address.map(|addr| addr.to_string()),
            estimated_gas,
            tx_hash: None,
        });
    }

    let signer = crate::chain::resolve_signer(config)?;
    let signer_address = signer.address().to_string();
    let (_from, tx_hash) = crate::chain::send_tx(
        &config.rpc_url_for_chain(chain_id),
        chain_id,
        signer,
        token,
        &calldata,
        "0",
        estimated_gas,
        None,
    )
    .await?;

    Ok(TokenMintResult {
        chain_id,
        dry_run: false,
        token: token.to_string(),
        to: recipient.to_string(),
        amount_raw,
        amount_display: args.amount.trim().to_string(),
        calldata,
        from_address: Some(signer_address),
        estimated_gas,
        tx_hash: Some(tx_hash),
    })
}

fn factory_address(config: &AppConfig, chain_id: u64) -> Result<Address> {
    let chain_config = config
        .chain_config_for(chain_id)
        .ok_or(ChainError::UnsupportedTokenFactoryChain(chain_id))?;
    let factory = chain_config
        .contracts
        .erc20_v3_factory
        .ok_or(ChainError::MissingFactoryAddress(chain_id))?;
    factory
        .parse()
        .map_err(|_| ChainError::InvalidAddress(factory.to_string()))
}

fn resolve_sender_address(config: &AppConfig) -> Result<Option<Address>> {
    if let Some(private_key) = config.private_key.as_deref() {
        return Ok(Some(crate::chain::address_from_private_key(private_key)?));
    }
    if let Some(wallet_address) = config.wallet_address.as_deref() {
        return wallet_address
            .parse()
            .map(Some)
            .map_err(|_| ChainError::InvalidAddress(wallet_address.to_string()));
    }
    Ok(None)
}

fn resolve_create_owner_address(
    config: &AppConfig,
    explicit_owner: Option<&str>,
) -> Result<Address> {
    if let Some(owner) = explicit_owner {
        return owner
            .parse()
            .map_err(|_| ChainError::InvalidAddress(owner.to_string()));
    }
    if let Some(private_key) = config.private_key.as_deref() {
        return crate::chain::address_from_private_key(private_key);
    }
    if let Some(wallet_address) = config.wallet_address.as_deref() {
        return wallet_address
            .parse()
            .map_err(|_| ChainError::InvalidAddress(wallet_address.to_string()));
    }
    Err(ChainError::NoWallet)
}

fn validate_create_std_args(args: &TokenCreateStdArgs) -> Result<()> {
    validate_create_params(&args.name, &args.symbol, &args.supply, args.decimals)
}

fn validate_create_params(name: &str, symbol: &str, supply: &str, decimals: u8) -> Result<()> {
    let name = name.trim();
    let symbol = symbol.trim();
    if name.is_empty() {
        return Err(ChainError::InvalidTokenCreateParams(
            "token name cannot be empty".to_string(),
        ));
    }
    if symbol.is_empty() {
        return Err(ChainError::InvalidTokenCreateParams(
            "token symbol cannot be empty".to_string(),
        ));
    }
    if decimals > 18 {
        return Err(ChainError::InvalidTokenCreateParams(
            "decimals must be <= 18".to_string(),
        ));
    }
    if name.len() > 64 {
        return Err(ChainError::InvalidTokenCreateParams(
            "token name must be <= 64 characters".to_string(),
        ));
    }
    if symbol.len() > 16 {
        return Err(ChainError::InvalidTokenCreateParams(
            "token symbol must be <= 16 characters".to_string(),
        ));
    }
    if !symbol.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(ChainError::InvalidTokenCreateParams(
            "token symbol must be alphanumeric".to_string(),
        ));
    }
    if crate::commands::to_raw_amount(supply, decimals).is_err() {
        return Err(ChainError::InvalidAmount(supply.to_string()));
    }
    Ok(())
}

fn parse_ratio_hundredths(input: &str) -> Result<U256> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ChainError::InvalidTokenCreateParams(
            "ratio cannot be empty".to_string(),
        ));
    }
    if trimmed.starts_with('-') {
        return Err(ChainError::InvalidTokenCreateParams(
            "ratio cannot be negative".to_string(),
        ));
    }

    let mut parts = trimmed.split('.');
    let int_part = parts.next().unwrap_or_default();
    let frac_part = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return Err(ChainError::InvalidTokenCreateParams(format!(
            "invalid ratio: {}",
            input
        )));
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(ChainError::InvalidTokenCreateParams(format!(
            "invalid ratio: {}",
            input
        )));
    }
    if frac_part.len() > 2 {
        return Err(ChainError::InvalidTokenCreateParams(
            "ratio supports at most 2 decimal places".to_string(),
        ));
    }

    let int_value: u64 = if int_part.is_empty() {
        0
    } else {
        int_part.parse().map_err(|_| {
            ChainError::InvalidTokenCreateParams(format!("invalid ratio: {}", input))
        })?
    };
    let frac_value: u64 = if frac_part.is_empty() {
        0
    } else if frac_part.len() == 1 {
        frac_part.parse::<u64>().map_err(|_| {
            ChainError::InvalidTokenCreateParams(format!("invalid ratio: {}", input))
        })? * 10
    } else {
        frac_part.parse::<u64>().map_err(|_| {
            ChainError::InvalidTokenCreateParams(format!("invalid ratio: {}", input))
        })?
    };

    let hundredths = int_value
        .checked_mul(100)
        .and_then(|v| v.checked_add(frac_value))
        .ok_or_else(|| ChainError::InvalidTokenCreateParams(format!("invalid ratio: {}", input)))?;
    if hundredths > 5000 {
        return Err(ChainError::InvalidTokenCreateParams(
            "ratio must be between 0 and 50".to_string(),
        ));
    }
    Ok(U256::from(hundredths))
}

fn format_units_to_f64(raw: &str, decimals: u8) -> f64 {
    match U256::from_str_radix(raw, 10) {
        Ok(value) => {
            let divisor = 10f64.powi(decimals as i32);
            value.to_string().parse::<f64>().unwrap_or(0.0) / divisor
        }
        Err(_) => 0.0,
    }
}

async fn persist_created_token(
    store: &QuoteStore,
    client: &OnChainClient,
    maybe_address: Option<&str>,
) {
    let Some(address) = maybe_address else {
        return;
    };
    let Ok(token_address) = address.parse::<Address>() else {
        return;
    };
    match crate::chain::get_token_info(client, token_address).await {
        Ok(info) => {
            if let Err(err) = store.save_custom_token_info(&info) {
                eprintln!(
                    "Warning: failed to persist created token {}: {}",
                    address, err
                );
            }
        }
        Err(err) => {
            eprintln!(
                "Warning: failed to fetch created token metadata for {}: {}",
                address, err
            );
        }
    }
}

async fn wait_for_created_token_address(
    client: &OnChainClient,
    tx_hash: &str,
    factory: Address,
) -> Result<Option<String>> {
    let parsed_hash = tx_hash
        .parse()
        .map_err(|_| ChainError::Rpc(format!("invalid tx hash: {}", tx_hash)))?;

    // Token creation sends a tx and then needs the mined receipt to recover the
    // emitted token address from factory logs. On slower RPCs or blocks, 15s is
    // not enough and we would return before the receipt exists.
    for _ in 0..60 {
        let receipt = client
            .provider
            .get_transaction_receipt(parsed_hash)
            .await
            .map_err(|e| ChainError::Rpc(format!("get_transaction_receipt failed: {:?}", e)))?;

        if let Some(receipt) = receipt {
            return Ok(
                extract_created_token_address(receipt.logs(), factory).map(|addr| addr.to_string())
            );
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    Ok(None)
}

fn extract_created_token_address<L>(logs: &[L], factory: Address) -> Option<Address>
where
    L: AsRef<alloy::primitives::Log>,
{
    logs.iter().rev().find_map(|log| {
        let log = log.as_ref();
        if log.address != factory {
            return None;
        }
        let data = log.data.data.as_ref();
        if data.len() < 32 {
            return None;
        }
        let address = Address::from_slice(&data[12..32]);
        (address != Address::ZERO).then_some(address)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Bytes, Log, LogData, B256};

    fn base_config() -> AppConfig {
        AppConfig {
            rpc_url: "https://ethereum-rpc.publicnode.com".to_string(),
            rpc_url_overridden: false,
            chain_id: 1,
            private_key: None,
            keystore_path: None,
            keystore_password_file: None,
            keystore_password_env: None,
            wallet_address: None,
            dodo_api_url: "https://api.dodoex.io".to_string(),
            dodo_api_key: String::new(),
            dodo_project_id: String::new(),
            data_dir: std::env::temp_dir(),
        }
    }

    #[test]
    fn factory_address_reads_configured_value() {
        let cfg = base_config();
        let factory = factory_address(&cfg, 1).unwrap();
        assert_eq!(
            factory.to_string().to_lowercase(),
            "0x6a3b1cc74019e252a857abbe9ee1b2f03ee1009f"
        );
    }

    #[test]
    fn validate_create_std_args_rejects_invalid_symbol() {
        let err = validate_create_std_args(&TokenCreateStdArgs {
            name: "Demo".to_string(),
            symbol: "BAD!".to_string(),
            supply: "1000".to_string(),
            decimals: 18,
            dry_run: true,
        })
        .unwrap_err();
        assert!(err.to_string().contains("alphanumeric"));
    }

    #[test]
    fn parse_ratio_hundredths_handles_common_inputs() {
        assert_eq!(parse_ratio_hundredths("0").unwrap(), U256::from(0u64));
        assert_eq!(parse_ratio_hundredths("0.1").unwrap(), U256::from(10u64));
        assert_eq!(parse_ratio_hundredths("1.25").unwrap(), U256::from(125u64));
        assert!(parse_ratio_hundredths("50.01").is_err());
    }

    #[test]
    fn resolve_sender_address_prefers_private_key() {
        let mut cfg = base_config();
        cfg.private_key =
            Some("0x59c6995e998f97a5a0044966f0945382dbf7f50a3f2f72f5f7a0b7d7d4f5e5f1".to_string());
        let sender = resolve_sender_address(&cfg).unwrap().unwrap();
        assert_eq!(
            sender,
            crate::chain::address_from_private_key(cfg.private_key.as_deref().unwrap()).unwrap()
        );
    }

    #[test]
    fn extract_created_token_address_reads_factory_log_data() {
        let factory: Address = "0x3450dBC7094bB20065f430D98087e37708C1ddfE"
            .parse()
            .unwrap();
        let token: Address = "0x070e4485DA80050837d55fC7Af04fDaBBB67dBcF"
            .parse()
            .unwrap();
        let mut data = vec![0u8; 32];
        data[12..32].copy_from_slice(token.as_slice());
        let logs = vec![
            Log {
                address: Address::ZERO,
                data: LogData::new_unchecked(vec![B256::ZERO], Bytes::new()),
            },
            Log {
                address: factory,
                data: LogData::new_unchecked(vec![B256::ZERO], Bytes::from(data)),
            },
        ];

        assert_eq!(extract_created_token_address(&logs, factory), Some(token));
    }
}
