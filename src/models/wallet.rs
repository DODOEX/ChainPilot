use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletBalance {
    pub address: String,
    pub chain_id: u64,
    pub eth_balance: String,
    pub eth_balance_display: f64,
    pub eth_balance_usd: Option<f64>,
    pub token_balances: Vec<TokenBalance>,
    pub total_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBalance {
    pub token_address: String,
    pub symbol: String,
    pub name: String,
    pub balance: String,
    pub balance_display: f64,
    pub balance_usd: Option<f64>,
    pub decimals: u8,
}
