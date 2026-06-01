use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletBalance {
    pub wallet: String,
    pub total_balance_usd: Option<f64>,
    pub assets: Vec<WalletAsset>,
    pub chain_allocation: Vec<ChainAllocation>,
    pub sources: WalletBalanceSources,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WalletBalanceSources {
    pub total_balance_usd: Option<String>,
    pub assets: Option<String>,
    pub chain_allocation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletAsset {
    pub chain: String,
    pub chain_id: Option<u64>,
    pub symbol: String,
    pub name: String,
    pub address: String,
    pub amount: f64,
    pub price_usd: Option<f64>,
    pub value_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainAllocation {
    pub chain: String,
    pub chain_id: Option<u64>,
    pub balance_usd: f64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletOverview {
    pub wallet: String,
    pub total_balance_usd: Option<f64>,
    pub chain_allocation: Vec<ChainAllocation>,
    pub token_allocation: Vec<TokenAllocation>,
    pub active_protocols: Vec<ActiveProtocol>,
    pub top_holdings: Vec<TopHolding>,
    pub sources: WalletOverviewSources,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WalletOverviewSources {
    pub total_balance_usd: Option<String>,
    pub chain_allocation: Option<String>,
    pub token_allocation: Option<String>,
    pub active_protocols: Option<String>,
    pub top_holdings: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAllocation {
    pub symbol: String,
    pub name: String,
    pub balance_usd: f64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveProtocol {
    pub name: String,
    pub chain: String,
    pub net_usd_value: Option<f64>,
    pub site_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopHolding {
    pub symbol: String,
    pub name: String,
    pub chain: String,
    pub amount: f64,
    pub value_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletPnl {
    pub wallet: String,
    pub realized_pnl: Option<f64>,
    pub unrealized_pnl: Option<f64>,
    pub total_pnl: Option<f64>,
    pub roi: Option<f64>,
    pub win_rate: Option<f64>,
    pub total_invested: Option<f64>,
    pub total_fee: Option<f64>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletHistory {
    pub wallet: String,
    pub transactions: Vec<WalletTransaction>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletTransaction {
    pub tx_hash: String,
    pub time: String,
    pub action: String,
    pub status: Option<String>,
    pub fee_usd: Option<f64>,
    pub token_in: Option<String>,
    pub token_out: Option<String>,
    pub value_usd: Option<f64>,
    pub amount: Option<f64>,
    pub success: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletLabels {
    pub wallet: String,
    pub labels: Vec<String>,
    pub label_scores: Vec<LabelScore>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelScore {
    pub label: String,
    pub score: Option<f64>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletDefi {
    pub wallet: String,
    pub total_value_usd: Option<f64>,
    pub positions: Vec<DefiPosition>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefiPosition {
    pub protocol: String,
    pub position_name: String,
    pub chain: String,
    pub value_usd: Option<f64>,
    pub tokens: Vec<DefiPositionToken>,
    pub position_type: String,
    pub site_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefiPositionToken {
    pub symbol: String,
    pub amount: Option<f64>,
}
