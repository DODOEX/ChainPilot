//! On-chain data access via alloy Provider.

mod erc20;
mod rpc;

pub use erc20::{get_allowance, get_balance, get_token_info};
pub use rpc::{
    address_from_private_key, estimate_gas, get_eth_balance, get_gas_price_gwei, get_nonce,
    get_tx_receipt, send_tx, TxStatus,
};

use alloy::network::Ethereum;
use alloy::providers::RootProvider;
use url::Url;

use crate::config::AppConfig;
use crate::error::Result;

pub struct OnChainClient {
    pub provider: RootProvider<Ethereum>,
    pub chain_id: u64,
}

impl OnChainClient {
    pub async fn new(config: &AppConfig) -> Result<Self> {
        let rpc_url = config.rpc_url.clone();
        let url: Url = rpc_url
            .parse()
            .map_err(|e| crate::error::ChainError::Rpc(format!("invalid rpc url: {}", e)))?;
        let provider = RootProvider::new_http(url);
        Ok(Self {
            provider,
            chain_id: config.chain_id,
        })
    }

    /// Build a client for `chain_id`, using the chain's built-in RPC URL as default.
    /// Falls back to `config.rpc_url` when no static config exists for the chain.
    pub async fn for_chain(config: &AppConfig, chain_id: u64) -> Result<Self> {
        if chain_id == config.chain_id {
            return Self::new(config).await;
        }
        let rpc_url = crate::config::chain_config(chain_id)
            .and_then(|c| c.rpc_urls.first().copied())
            .unwrap_or(config.rpc_url.as_str())
            .to_string();
        let mut cfg = config.clone();
        cfg.rpc_url = rpc_url;
        cfg.chain_id = chain_id;
        Self::new(&cfg).await
    }
}
