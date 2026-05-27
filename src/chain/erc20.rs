//! ERC-20 token interactions via alloy sol! macro + Provider.

use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy_sol_types::sol;

use crate::chain::OnChainClient;
use crate::error::{ChainError, Result};
use crate::models::token::{TokenContract, TokenInfo, TokenMetadataSources, TokenSocialLinks};

sol! {
    #[sol(rpc)]
    contract ERC20 {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
        function balanceOf(address owner) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
    }

    #[sol(rpc)]
    contract Ownable {
        function owner() external view returns (address);
        function getOwner() external view returns (address);
    }
}

const EIP1967_IMPLEMENTATION_SLOT: U256 = U256::from_be_bytes([
    0x36, 0x08, 0x94, 0xa1, 0x3b, 0xa1, 0xa3, 0x21, 0x06, 0x67, 0xc8, 0x28, 0x49, 0x2d, 0xb9, 0x8d,
    0xca, 0x3e, 0x20, 0x76, 0xcc, 0x37, 0x35, 0xa9, 0x20, 0xa3, 0xca, 0x50, 0x5d, 0x38, 0x2b, 0xbc,
]);
const EIP1967_ADMIN_SLOT: U256 = U256::from_be_bytes([
    0xb5, 0x31, 0x27, 0x68, 0x4a, 0x56, 0x8b, 0x31, 0x73, 0xae, 0x13, 0xb9, 0xf8, 0xa6, 0x01, 0x6e,
    0x24, 0x3e, 0x63, 0xb6, 0xe8, 0xee, 0x11, 0x78, 0xd6, 0xa7, 0x17, 0x85, 0x0b, 0x5d, 0x61, 0x03,
]);

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

    Ok(TokenInfo {
        address: token_address.to_string(),
        symbol,
        name,
        decimals,
        chain_id: client.chain_id,
        chain: crate::config::chain_config(client.chain_id).map(|c| c.name.to_string()),
        website: None,
        social_links: TokenSocialLinks::default(),
        price: None,
        market_cap: None,
        fdv: None,
        primary_liquidity: None,
        volume_24h: None,
        price_change_24h: None,
        risk_level: None,
        metadata_sources: TokenMetadataSources {
            identity: Some("on-chain".to_string()),
            address: Some("resolver".to_string()),
            chain: crate::config::chain_config(client.chain_id).map(|_| "chain-config".to_string()),
            ..TokenMetadataSources::default()
        },
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

pub async fn inspect_token_contract(
    client: &OnChainClient,
    token_address: Address,
) -> Result<TokenContract> {
    let code = client
        .provider
        .get_code_at(token_address)
        .await
        .map_err(|e| ChainError::Rpc(format!("get_code_at failed: {:?}", e)))?;
    if code.is_empty() {
        return Err(ChainError::Rpc("address has no contract code".to_string()));
    }

    let implementation_storage = client
        .provider
        .get_storage_at(token_address, EIP1967_IMPLEMENTATION_SLOT)
        .await
        .map_err(|e| ChainError::Rpc(format!("get_storage_at failed: {:?}", e)))?;
    let implementation = address_from_storage(implementation_storage);
    let proxy_implementation = if implementation == Address::ZERO {
        None
    } else {
        Some(implementation.to_string())
    };

    let admin_storage = client
        .provider
        .get_storage_at(token_address, EIP1967_ADMIN_SLOT)
        .await
        .map_err(|e| ChainError::Rpc(format!("get_storage_at failed: {:?}", e)))?;
    let admin = address_from_storage(admin_storage);

    let owner = read_owner(client, token_address)
        .await?
        .or_else(|| (admin != Address::ZERO).then(|| admin.to_string()));

    Ok(TokenContract {
        address: token_address.to_string(),
        chain_id: client.chain_id,
        is_proxy: proxy_implementation.is_some(),
        proxy_implementation,
        owner,
        deployer: None,
        deployed_at_block: None,
        is_verified: None,
    })
}

fn address_from_storage(storage: U256) -> Address {
    let bytes = storage.to_be_bytes::<32>();
    Address::from_slice(&bytes[12..])
}

async fn read_owner(client: &OnChainClient, token_address: Address) -> Result<Option<String>> {
    let ownable = Ownable::new(token_address, &client.provider);
    match ownable.owner().call().await {
        Ok(owner) if owner != Address::ZERO => return Ok(Some(owner.to_string())),
        Ok(_) => return Ok(None),
        Err(_) => {}
    }

    match ownable.getOwner().call().await {
        Ok(owner) if owner != Address::ZERO => Ok(Some(owner.to_string())),
        Ok(_) => Ok(None),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_from_storage_extracts_lower_20_bytes() {
        let addr = Address::from([0x11; 20]);
        let mut word = [0u8; 32];
        word[12..].copy_from_slice(addr.as_slice());
        let storage = U256::from_be_bytes(word);
        assert_eq!(address_from_storage(storage), addr);
    }
}
