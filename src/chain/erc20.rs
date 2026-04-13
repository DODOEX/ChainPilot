//! ERC-20 token interactions via alloy sol! macro + Provider.

use alloy::primitives::Address;
use alloy_sol_types::sol;

use crate::chain::OnChainClient;
use crate::error::{ChainError, Result};
use crate::models::token::TokenInfo;

sol! {
    #[sol(rpc)]
    contract ERC20 {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
        function balanceOf(address owner) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
    }
}

pub async fn get_token_info(client: &OnChainClient, token_address: Address) -> Result<TokenInfo> {
    let erc20 = ERC20::new(token_address, &client.provider);

    let name = erc20
        .name()
        .call()
        .await
        .map_err(|e| ChainError::Rpc(format!("name failed: {:?}", e)))?;
    let symbol = erc20
        .symbol()
        .call()
        .await
        .map_err(|e| ChainError::Rpc(format!("symbol failed: {:?}", e)))?;
    let decimals = erc20
        .decimals()
        .call()
        .await
        .map_err(|e| ChainError::Rpc(format!("decimals failed: {:?}", e)))?;
    let total_supply = erc20
        .totalSupply()
        .call()
        .await
        .map_err(|e| ChainError::Rpc(format!("totalSupply failed: {:?}", e)))?;

    let total_supply_str = total_supply.to_string();
    let decimals_u64 = decimals as u64;
    let total_supply_display = parse_token_amount(&total_supply_str, decimals_u64);

    Ok(TokenInfo {
        address: token_address.to_string(),
        symbol,
        name,
        decimals,
        chain_id: client.chain_id,
        total_supply: total_supply_str,
        total_supply_display,
        source: "on-chain".to_string(),
    })
}

pub async fn get_balance(
    client: &OnChainClient,
    token_address: Address,
    wallet_address: Address,
) -> Result<(String, u8)> {
    let erc20 = ERC20::new(token_address, &client.provider);

    let balance = erc20
        .balanceOf(wallet_address)
        .call()
        .await
        .map_err(|e| ChainError::Rpc(format!("balanceOf failed: {:?}", e)))?;
    let decimals = erc20
        .decimals()
        .call()
        .await
        .map_err(|e| ChainError::Rpc(format!("decimals failed: {:?}", e)))?;

    Ok((balance.to_string(), decimals))
}

pub async fn get_allowance(
    client: &OnChainClient,
    token_address: Address,
    owner: Address,
    spender: Address,
) -> Result<String> {
    let erc20 = ERC20::new(token_address, &client.provider);

    let allowance = erc20
        .allowance(owner, spender)
        .call()
        .await
        .map_err(|e| ChainError::Rpc(format!("allowance failed: {:?}", e)))?;

    Ok(allowance.to_string())
}

fn parse_token_amount(raw: &str, decimals: u64) -> f64 {
    let raw_uint: u128 = raw.parse().unwrap_or(0);
    let divisor = 10u128.pow(decimals as u32) as f64;
    raw_uint as f64 / divisor
}
