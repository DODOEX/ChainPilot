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
