use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolInfo {
    pub name: String,
    pub category: Option<String>,
    pub chain: Option<String>,
    pub website: Option<String>,
    pub description: Option<String>,
    pub tvl: Option<f64>,
    pub revenue: Option<f64>,
    pub fee: Option<f64>,
    pub sources: ProtocolInfoSources,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolInfoSources {
    pub name: Option<String>,
    pub category: Option<String>,
    pub chain: Option<String>,
    pub website: Option<String>,
    pub description: Option<String>,
    pub tvl: Option<String>,
    pub revenue: Option<String>,
    pub fee: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolTvl {
    pub protocol: String,
    pub current_tvl: Option<f64>,
    pub tvl_change_24h: Option<f64>,
    pub tvl_change_7d: Option<f64>,
    pub tvl_change_30d: Option<f64>,
    pub tvl_history_total: usize,
    pub tvl_history_limit: usize,
    pub tvl_history_offset: usize,
    pub tvl_history: Vec<ProtocolTvlPoint>,
    pub sources: ProtocolTvlSources,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolTvlSources {
    pub current_tvl: Option<String>,
    pub tvl_change_24h: Option<String>,
    pub tvl_change_7d: Option<String>,
    pub tvl_change_30d: Option<String>,
    pub tvl_history: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolTvlPoint {
    pub date: i64,
    pub tvl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRevenue {
    pub protocol: String,
    pub revenue_24h: Option<f64>,
    pub revenue_7d: Option<f64>,
    pub revenue_30d: Option<f64>,
    pub fees_24h: Option<f64>,
    pub fees_7d: Option<f64>,
    pub sources: ProtocolRevenueSources,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolRevenueSources {
    pub revenue_24h: Option<String>,
    pub revenue_7d: Option<String>,
    pub revenue_30d: Option<String>,
    pub fees_24h: Option<String>,
    pub fees_7d: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolChains {
    pub protocol: String,
    pub chains: Vec<ProtocolChainMetrics>,
    pub sources: ProtocolChainsSources,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolChainsSources {
    pub chains: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolChainMetrics {
    pub chain: String,
    pub tvl: Option<f64>,
    pub revenue: Option<f64>,
    pub sources: ProtocolChainMetricSources,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtocolChainMetricSources {
    pub tvl: Option<String>,
    pub revenue: Option<String>,
}
