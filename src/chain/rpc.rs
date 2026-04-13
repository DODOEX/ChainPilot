//! Ethereum RPC calls via alloy Provider.

use alloy::network::TransactionBuilder;
use alloy::primitives::{Address, Bytes, U256};
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use url::Url;

use crate::chain::OnChainClient;
use crate::error::{ChainError, Result};

#[derive(Debug, serde::Serialize)]
pub struct TxStatus {
    pub confirmed: bool,
    pub success: bool,
    pub block_number: Option<u64>,
    pub gas_used: Option<u64>,
    pub effective_gas_price: Option<f64>,
}

pub async fn get_eth_balance(client: &OnChainClient, address: Address) -> Result<(String, f64)> {
    let balance = client
        .provider
        .get_balance(address)
        .await
        .map_err(|e| ChainError::Rpc(format!("get_balance failed: {:?}", e)))?;

    let balance_str = balance.to_string();
    let balance_eth = parse_wei(&balance_str);
    Ok((balance_str, balance_eth))
}

pub async fn get_gas_price_gwei(client: &OnChainClient) -> Result<f64> {
    let gas_price = client
        .provider
        .get_gas_price()
        .await
        .map_err(|e| ChainError::Rpc(format!("get_gas_price failed: {:?}", e)))?;
    Ok(gas_price as f64 / 1e9)
}

pub async fn get_tx_receipt(client: &OnChainClient, tx_hash: &str) -> Result<Option<TxStatus>> {
    let tx_hash = tx_hash
        .parse()
        .map_err(|_| ChainError::Rpc(format!("invalid tx hash: {}", tx_hash)))?;

    let receipt = client
        .provider
        .get_transaction_receipt(tx_hash)
        .await
        .map_err(|e| ChainError::Rpc(format!("get_transaction_receipt failed: {:?}", e)))?;

    Ok(receipt.map(|r| {
        let success = r.inner.status();
        TxStatus {
            confirmed: true,
            success,
            block_number: r.block_number,
            gas_used: Some(r.gas_used),
            effective_gas_price: Some(r.effective_gas_price as f64 / 1e9),
        }
    }))
}

pub async fn estimate_gas(
    client: &OnChainClient,
    from: Address,
    to: Address,
    data: &str,
    value: &str,
) -> Result<u64> {
    let data_bytes = if data.starts_with("0x") {
        &data[2..]
    } else {
        data
    };
    let value_u256 = parse_value_u256(value)?;

    let input_bytes =
        hex::decode(data_bytes).map_err(|_| ChainError::Rpc("invalid data hex".to_string()))?;

    let tx = TransactionRequest::default()
        .with_from(from)
        .with_to(to)
        .with_input(input_bytes)
        .with_value(value_u256);

    let gas = client
        .provider
        .estimate_gas(tx)
        .await
        .map_err(|e| ChainError::Rpc(format!("estimate_gas failed: {:?}", e)))?;

    Ok(gas)
}

/// Return the number of confirmed transactions sent by `address` (i.e. the confirmed nonce).
pub async fn get_nonce(client: &OnChainClient, address: Address) -> Result<u64> {
    client
        .provider
        .get_transaction_count(address)
        .await
        .map_err(|e| ChainError::Rpc(format!("get_transaction_count failed: {:?}", e)))
}

/// Derive the wallet address from a hex private key without sending anything.
pub fn address_from_private_key(private_key: &str) -> Result<Address> {
    use alloy::signers::local::PrivateKeySigner;
    let pk = if private_key.starts_with("0x") {
        &private_key[2..]
    } else {
        private_key
    };
    let signer: PrivateKeySigner = pk
        .parse()
        .map_err(|_| ChainError::InvalidPrivateKey(private_key.to_string()))?;
    Ok(signer.address())
}

/// Sign and broadcast a swap transaction. Returns `(from_address, tx_hash)`.
pub async fn send_tx(
    rpc_url: &str,
    chain_id: u64,
    private_key: &str,
    to: Address,
    data: &str,
    value_hex: &str,
    gas_limit: Option<u64>,
    max_fee_gwei: Option<f64>,
) -> Result<(Address, String)> {
    use alloy::network::EthereumWallet;
    use alloy::providers::ProviderBuilder;
    use alloy::signers::local::PrivateKeySigner;

    let pk = if private_key.starts_with("0x") {
        &private_key[2..]
    } else {
        private_key
    };
    let signer: PrivateKeySigner = pk
        .parse()
        .map_err(|_| ChainError::InvalidPrivateKey(private_key.to_string()))?;
    let from = signer.address();
    let wallet = EthereumWallet::from(signer);

    let url: Url = rpc_url
        .parse()
        .map_err(|e| ChainError::Rpc(format!("invalid rpc url: {}", e)))?;
    // In alloy 1.8, recommended fillers (gas, nonce, chain_id) are included by default.
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(url);

    let value_u256 = parse_value_u256(value_hex)?;

    let data_hex = if data.starts_with("0x") {
        &data[2..]
    } else {
        data
    };
    let data_bytes =
        hex::decode(data_hex).map_err(|_| ChainError::Rpc("invalid calldata hex".to_string()))?;

    let mut tx = TransactionRequest::default()
        .with_from(from)
        .with_to(to)
        .with_input(data_bytes)
        .with_value(value_u256)
        .with_chain_id(chain_id);

    if let Some(gas) = gas_limit {
        tx = tx.with_gas_limit(gas);
    }
    if let Some(fee_gwei) = max_fee_gwei {
        tx = tx.with_max_fee_per_gas((fee_gwei * 1e9) as u128);
    }

    let pending: alloy::providers::PendingTransactionBuilder<alloy::network::Ethereum> = provider
        .send_transaction(tx)
        .await
        .map_err(|e| ChainError::Rpc(format!("send_transaction failed: {:?}", e)))?;

    Ok((from, pending.tx_hash().to_string()))
}

fn parse_wei(raw: &str) -> f64 {
    let wei: u128 = raw.parse().unwrap_or(0);
    wei as f64 / 1e18
}

fn parse_value_u256(value: &str) -> Result<U256> {
    if value.is_empty() || value == "0" || value == "0x0" {
        return Ok(U256::ZERO);
    }

    if let Some(hex) = value.strip_prefix("0x") {
        U256::from_str_radix(hex, 16).map_err(|_| ChainError::Rpc("invalid hex value".to_string()))
    } else {
        U256::from_str_radix(value, 10)
            .map_err(|_| ChainError::Rpc("invalid decimal value".to_string()))
    }
}
