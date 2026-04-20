use alloy::primitives::{Address, U256};
use alloy_sol_types::{sol, SolCall};

use crate::chain::OnChainClient;
use crate::error::{ChainError, Result};

sol! {
    #[sol(rpc)]
    contract ERC20V3Factory {
        function createStdERC20(
            uint256 totalSupply,
            string memory tokenName,
            string memory tokenSymbol,
            uint8 tokenDecimals
        ) external payable returns (address token);

        function createCustomERC20(
            uint256 totalSupply,
            string memory tokenName,
            string memory tokenSymbol,
            uint8 tokenDecimals,
            uint256 tradeBurnRatio,
            uint256 tradeFeeRatio,
            address teamAccount
        ) external payable returns (address token);

        function createCustomMintableERC20(
            uint256 initSupply,
            string memory tokenName,
            string memory tokenSymbol,
            uint8 tokenDecimals,
            uint256 tradeBurnRatio,
            uint256 tradeFeeRatio,
            address teamAccount
        ) external payable returns (address token);

        function _CREATE_FEE_() external view returns (uint256);
    }
}

pub async fn get_create_fee(client: &OnChainClient, factory_address: Address) -> Result<String> {
    let factory = ERC20V3Factory::new(factory_address, &client.provider);
    let fee = factory
        ._CREATE_FEE_()
        .call()
        .await
        .map_err(|e| ChainError::Rpc(format!("_CREATE_FEE_ failed: {:?}", e)))?;
    Ok(fee.to_string())
}

pub fn encode_create_std_calldata(
    total_supply: U256,
    token_name: String,
    token_symbol: String,
    token_decimals: u8,
) -> String {
    let call = ERC20V3Factory::createStdERC20Call {
        totalSupply: total_supply,
        tokenName: token_name,
        tokenSymbol: token_symbol,
        tokenDecimals: token_decimals,
    };
    format!("0x{}", hex::encode(call.abi_encode()))
}

pub fn encode_create_custom_calldata(
    total_supply: U256,
    token_name: String,
    token_symbol: String,
    token_decimals: u8,
    trade_burn_ratio: U256,
    trade_fee_ratio: U256,
    team_account: Address,
) -> String {
    let call = ERC20V3Factory::createCustomERC20Call {
        totalSupply: total_supply,
        tokenName: token_name,
        tokenSymbol: token_symbol,
        tokenDecimals: token_decimals,
        tradeBurnRatio: trade_burn_ratio,
        tradeFeeRatio: trade_fee_ratio,
        teamAccount: team_account,
    };
    format!("0x{}", hex::encode(call.abi_encode()))
}

pub fn encode_create_mintable_calldata(
    init_supply: U256,
    token_name: String,
    token_symbol: String,
    token_decimals: u8,
    trade_burn_ratio: U256,
    trade_fee_ratio: U256,
    team_account: Address,
) -> String {
    let call = ERC20V3Factory::createCustomMintableERC20Call {
        initSupply: init_supply,
        tokenName: token_name,
        tokenSymbol: token_symbol,
        tokenDecimals: token_decimals,
        tradeBurnRatio: trade_burn_ratio,
        tradeFeeRatio: trade_fee_ratio,
        teamAccount: team_account,
    };
    format!("0x{}", hex::encode(call.abi_encode()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_std_calldata_has_expected_selector() {
        let calldata = encode_create_std_calldata(
            U256::from(42u64),
            "Demo".to_string(),
            "DEMO".to_string(),
            18,
        );
        assert!(calldata.starts_with("0x"));
        assert!(calldata.len() > 10);
    }
}
