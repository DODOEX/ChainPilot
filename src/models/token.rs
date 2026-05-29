use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub chain_id: u64,
    pub total_supply: String,
    pub total_supply_display: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenContract {
    pub address: String,
    pub chain_id: u64,
    pub is_proxy: bool,
    pub proxy_implementation: Option<String>,
    pub owner: Option<String>,
    pub deployer: Option<String>,
    pub deployed_at_block: Option<u64>,
    pub is_verified: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomTokenRecord {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub chain_id: u64,
    pub added_at: DateTime<Utc>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCreateFee {
    pub chain_id: u64,
    pub factory: String,
    pub fee_raw: String,
    pub fee_display: f64,
    pub fee_symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCreateResult {
    pub chain_id: u64,
    pub dry_run: bool,
    pub factory: String,
    pub method: String,
    pub token_name: String,
    pub token_symbol: String,
    pub decimals: u8,
    pub supply_raw: String,
    pub supply_display: String,
    pub calldata: String,
    pub value: String,
    pub from_address: Option<String>,
    pub estimated_gas: Option<u64>,
    pub tx_hash: Option<String>,
    pub new_token_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMintResult {
    pub chain_id: u64,
    pub dry_run: bool,
    pub token: String,
    pub to: String,
    pub amount_raw: String,
    pub amount_display: String,
    pub calldata: String,
    pub from_address: Option<String>,
    pub estimated_gas: Option<u64>,
    pub tx_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenOwnershipActionResult {
    pub chain_id: u64,
    pub dry_run: bool,
    pub action: String,
    pub token: String,
    pub calldata: String,
    pub from_address: Option<String>,
    pub estimated_gas: Option<u64>,
    pub tx_hash: Option<String>,
}

impl CustomTokenRecord {
    pub fn from_token_info(info: &TokenInfo) -> Self {
        Self {
            address: info.address.clone(),
            symbol: info.symbol.clone(),
            name: info.name.clone(),
            decimals: info.decimals,
            chain_id: info.chain_id,
            added_at: Utc::now(),
            source: "custom".to_string(),
        }
    }
}
