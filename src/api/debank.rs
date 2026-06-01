use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::api::send_retrying;
use crate::error::{ChainError, Result};

#[derive(Clone)]
pub struct DebankClient {
    client: Client,
    base_url: String,
    api_key: String,
}

#[derive(Debug, Deserialize)]
struct TotalBalanceResp {
    total_usd_value: Option<f64>,
    #[serde(default)]
    chain_list: Vec<TotalBalanceChain>,
}

#[derive(Debug, Deserialize)]
struct TotalBalanceChain {
    id: Option<String>,
    community_id: Option<u64>,
    name: Option<String>,
    usd_value: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct DebankToken {
    id: Option<String>,
    chain: Option<String>,
    name: Option<String>,
    symbol: Option<String>,
    amount: Option<f64>,
    price: Option<f64>,
    #[serde(default)]
    is_verified: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct DebankProtocol {
    name: Option<String>,
    chain: Option<String>,
    site_url: Option<String>,
    #[serde(default)]
    portfolio_item_list: Vec<DebankPortfolioItem>,
    #[serde(default)]
    asset_usd_value: Option<f64>,
    #[serde(default)]
    net_usd_value: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct DebankPortfolioItem {
    name: Option<String>,
    chain: Option<String>,
    #[serde(default)]
    stats: Option<DebankPortfolioStats>,
    #[serde(default)]
    detail: Option<DebankPortfolioDetail>,
}

#[derive(Debug, Deserialize)]
struct DebankPortfolioStats {
    net_usd_value: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DebankPortfolioDetail {
    #[serde(default)]
    supply_token_list: Vec<DebankDetailToken>,
    #[serde(default)]
    reward_token_list: Vec<DebankDetailToken>,
    #[serde(default)]
    borrow_token_list: Vec<DebankDetailToken>,
    #[serde(default)]
    token_pair: Option<DebankDetailTokenPair>,
}

#[derive(Debug, Deserialize)]
struct DebankDetailToken {
    amount: Option<f64>,
    #[serde(default)]
    token: Option<DebankDetailTokenInfo>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DebankDetailTokenInfo {
    symbol: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DebankDetailTokenPair {
    #[serde(default)]
    tokens: Vec<DebankDetailTokenInfo>,
}

#[derive(Debug, Clone)]
pub struct DebankChainSummary {
    pub id: String,
    pub community_id: Option<u64>,
    pub name: String,
    pub usd_value: f64,
}

#[derive(Debug, Clone)]
pub struct DebankAssetRecord {
    pub chain: String,
    pub chain_id: Option<u64>,
    pub symbol: String,
    pub name: String,
    pub address: String,
    pub amount: f64,
    pub price_usd: Option<f64>,
    pub value_usd: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct DebankProtocolRecord {
    pub name: String,
    pub chain: String,
    pub net_usd_value: Option<f64>,
    pub site_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeFiPositionRecord {
    pub protocol: String,
    pub position_name: String,
    pub chain: String,
    pub value_usd: Option<f64>,
    pub tokens: Vec<DeFiPositionToken>,
    pub position_type: String,
    pub site_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeFiPositionToken {
    pub symbol: String,
    pub amount: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct DebankTotalBalance {
    pub total_usd_value: Option<f64>,
    pub chains: Vec<DebankChainSummary>,
}

// ── transaction history ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct HistoryListResp {
    #[serde(default)]
    history_list: Vec<HistoryItem>,
    #[serde(default)]
    token_dict: std::collections::HashMap<String, HistoryTokenInfo>,
}

#[derive(Debug, Deserialize)]
struct HistoryTokenInfo {
    symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HistoryItem {
    id: Option<String>,
    time_at: Option<u64>,
    cate_id: Option<String>,
    tx: Option<HistoryTx>,
    #[serde(default)]
    sends: Vec<HistoryTransfer>,
    #[serde(default)]
    receives: Vec<HistoryTransfer>,
}

#[derive(Debug, Deserialize)]
struct HistoryTx {
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HistoryTransfer {
    amount: Option<f64>,
    token_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DebankHistoryRecord {
    pub tx_hash: String,
    pub time: String,
    pub action: String,
    pub token_in: Option<String>,
    pub token_out: Option<String>,
    pub value_usd: Option<f64>,
    pub amount: Option<f64>,
    pub success: Option<bool>,
}

// ── wallet labels ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct LabelListResp {
    #[serde(default)]
    labels: Vec<LabelItem>,
}

#[derive(Debug, Deserialize)]
struct LabelItem {
    name: Option<String>,
    #[serde(default)]
    tags: Vec<LabelTag>,
}

#[derive(Debug, Deserialize)]
struct LabelTag {
    name: Option<String>,
    score: Option<f64>,
    reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DebankLabelRecord {
    pub label: String,
    pub score: Option<f64>,
    pub reason: Option<String>,
}

impl DebankClient {
    pub fn new(client: Client, base_url: &str, api_key: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    pub async fn total_balance(&self, address: &str) -> Result<DebankTotalBalance> {
        self.require_key()?;
        let url = format!("{}/user/total_balance", self.base_url);
        let req = self
            .client
            .get(&url)
            .header("AccessKey", &self.api_key)
            .header("accept", "application/json")
            .timeout(Duration::from_secs(10))
            .query(&[("id", address)]);
        let resp: TotalBalanceResp = send_retrying(req, "debank.total_balance")
            .await?
            .error_for_status()
            .map_err(map_http_err)?
            .json()
            .await
            .map_err(map_http_err)?;

        let chains = resp
            .chain_list
            .into_iter()
            .filter_map(|c| {
                let id = c.id?;
                let name = c.name.unwrap_or_else(|| id.clone());
                let usd_value = c.usd_value.unwrap_or(0.0);
                Some(DebankChainSummary {
                    id,
                    community_id: c.community_id,
                    name,
                    usd_value,
                })
            })
            .collect();

        Ok(DebankTotalBalance {
            total_usd_value: resp.total_usd_value,
            chains,
        })
    }

    pub async fn all_token_list(&self, address: &str) -> Result<Vec<DebankAssetRecord>> {
        self.require_key()?;
        let url = format!("{}/user/all_token_list", self.base_url);
        let req = self
            .client
            .get(&url)
            .header("AccessKey", &self.api_key)
            .header("accept", "application/json")
            .timeout(Duration::from_secs(15))
            .query(&[("id", address), ("is_all", "true")]);
        let resp: Vec<DebankToken> = send_retrying(req, "debank.all_token_list")
            .await?
            .error_for_status()
            .map_err(map_http_err)?
            .json()
            .await
            .map_err(map_http_err)?;

        Ok(resp
            .into_iter()
            .filter(|t| t.is_verified.unwrap_or(true))
            .filter_map(|t| {
                let amount = t.amount.unwrap_or(0.0);
                if amount <= 0.0 {
                    return None;
                }
                let chain = t.chain.unwrap_or_default();
                let chain_id = debank_chain_to_id(&chain);
                let symbol = t.symbol.unwrap_or_default();
                let name = t.name.unwrap_or_else(|| symbol.clone());
                let address = t.id.unwrap_or_default();
                let price_usd = t.price;
                let value_usd = price_usd.map(|p| p * amount);
                Some(DebankAssetRecord {
                    chain,
                    chain_id,
                    symbol,
                    name,
                    address,
                    amount,
                    price_usd,
                    value_usd,
                })
            })
            .collect())
    }

    pub async fn all_complex_protocol_list(
        &self,
        address: &str,
    ) -> Result<Vec<DebankProtocolRecord>> {
        self.require_key()?;
        let url = format!("{}/user/all_complex_protocol_list", self.base_url);
        let req = self
            .client
            .get(&url)
            .header("AccessKey", &self.api_key)
            .header("accept", "application/json")
            .timeout(Duration::from_secs(15))
            .query(&[("id", address)]);
        let resp: Vec<DebankProtocol> = send_retrying(req, "debank.all_complex_protocol_list")
            .await?
            .error_for_status()
            .map_err(map_http_err)?
            .json()
            .await
            .map_err(map_http_err)?;

        Ok(resp
            .into_iter()
            .filter_map(|p| {
                let name = p.name?;
                let chain = p.chain.unwrap_or_default();
                let net_usd_value = p.net_usd_value.or_else(|| {
                    let sum: f64 = p
                        .portfolio_item_list
                        .iter()
                        .filter_map(|i| i.stats.as_ref().and_then(|s| s.net_usd_value))
                        .sum();
                    if sum > 0.0 { Some(sum) } else { p.asset_usd_value }
                });
                Some(DebankProtocolRecord {
                    name,
                    chain,
                    net_usd_value,
                    site_url: p.site_url,
                })
            })
            .collect())
    }

    pub async fn defi_positions(&self, address: &str) -> Result<Vec<DeFiPositionRecord>> {
        self.require_key()?;
        let url = format!("{}/user/all_complex_protocol_list", self.base_url);
        let req = self
            .client
            .get(&url)
            .header("AccessKey", &self.api_key)
            .header("accept", "application/json")
            .timeout(Duration::from_secs(15))
            .query(&[("id", address)]);
        let resp: Vec<DebankProtocol> = send_retrying(req, "debank.defi_positions")
            .await?
            .error_for_status()
            .map_err(map_http_err)?
            .json()
            .await
            .map_err(map_http_err)?;

        let mut positions = Vec::new();
        for p in resp {
            let protocol = match p.name {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            };
            let chain = p.chain.clone().unwrap_or_default();
            let site_url = p.site_url.clone();

            if p.portfolio_item_list.is_empty() {
                // No sub-items — treat the protocol itself as a single position.
                let value_usd = p.net_usd_value.or(p.asset_usd_value);
                if value_usd.unwrap_or(0.0) <= 0.0 {
                    continue;
                }
                positions.push(DeFiPositionRecord {
                    position_name: protocol.clone(),
                    protocol,
                    chain,
                    value_usd,
                    tokens: Vec::new(),
                    position_type: "protocol".to_string(),
                    site_url,
                });
                continue;
            }

            for item in &p.portfolio_item_list {
                let value_usd = item.stats.as_ref().and_then(|s| s.net_usd_value);
                if value_usd.unwrap_or(0.0) <= 0.0 {
                    continue;
                }
                let position_name = item
                    .name
                    .clone()
                    .unwrap_or_else(|| protocol.clone());
                let item_chain = item.chain.clone().unwrap_or_else(|| chain.clone());

                let mut tokens = Vec::new();
                if let Some(ref detail) = item.detail {
                    for t in &detail.supply_token_list {
                        if let Some(ref info) = t.token {
                            tokens.push(DeFiPositionToken {
                                symbol: info
                                    .symbol
                                    .clone()
                                    .unwrap_or_else(|| "unknown".to_string()),
                                amount: t.amount,
                            });
                        }
                    }
                    for t in &detail.reward_token_list {
                        if let Some(ref info) = t.token {
                            tokens.push(DeFiPositionToken {
                                symbol: info
                                    .symbol
                                    .clone()
                                    .unwrap_or_else(|| "unknown".to_string()),
                                amount: t.amount,
                            });
                        }
                    }
                    if let Some(ref pair) = detail.token_pair {
                        let symbols: Vec<String> = pair
                            .tokens
                            .iter()
                            .filter_map(|t| t.symbol.clone())
                            .collect();
                        if !symbols.is_empty() {
                            tokens.push(DeFiPositionToken {
                                symbol: symbols.join("/"),
                                amount: None,
                            });
                        }
                    }
                }

                let position_type = classify_position(&item.name, &tokens);

                positions.push(DeFiPositionRecord {
                    protocol: protocol.clone(),
                    position_name,
                    chain: item_chain,
                    value_usd,
                    tokens,
                    position_type,
                    site_url: site_url.clone(),
                });
            }
        }
        Ok(positions)
    }

    pub async fn wallet_labels(&self, address: &str) -> Result<Vec<DebankLabelRecord>> {
        self.require_key()?;
        let url = format!("{}/user/label", self.base_url);
        let req = self
            .client
            .get(&url)
            .header("AccessKey", &self.api_key)
            .header("accept", "application/json")
            .timeout(Duration::from_secs(10))
            .query(&[("id", address)]);
        let resp: LabelListResp = send_retrying(req, "debank.wallet_labels")
            .await?
            .error_for_status()
            .map_err(map_http_err)?
            .json()
            .await
            .map_err(map_http_err)?;

        let mut records = Vec::new();
        for item in resp.labels {
            if let Some(label_name) = item.name {
                if !label_name.is_empty() {
                    records.push(DebankLabelRecord {
                        label: label_name,
                        score: None,
                        reason: None,
                    });
                }
            }
            for tag in item.tags {
                if let Some(tag_name) = tag.name {
                    if !tag_name.is_empty() {
                        records.push(DebankLabelRecord {
                            label: tag_name,
                            score: tag.score,
                            reason: tag.reason,
                        });
                    }
                }
            }
        }
        Ok(records)
    }

    pub async fn all_history_list(
        &self,
        address: &str,
        page_count: u32,
    ) -> Result<Vec<DebankHistoryRecord>> {
        self.require_key()?;
        let url = format!("{}/user/all_history_list", self.base_url);
        let req = self
            .client
            .get(&url)
            .header("AccessKey", &self.api_key)
            .header("accept", "application/json")
            .timeout(Duration::from_secs(15))
            .query(&[
                ("id", address.to_string()),
                ("page_count", page_count.to_string()),
            ]);
        let resp: HistoryListResp = send_retrying(req, "debank.all_history_list")
            .await?
            .error_for_status()
            .map_err(map_http_err)?
            .json()
            .await
            .map_err(map_http_err)?;

        let token_dict = resp.token_dict;

        Ok(resp
            .history_list
            .into_iter()
            .filter_map(|h| {
                let tx_hash = h.id?;
                let time = h
                    .time_at
                    .map(|ts| {
                        chrono::DateTime::from_timestamp(ts as i64, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_else(|| ts.to_string())
                    })
                    .unwrap_or_default();
                let action = debank_cate_to_action(h.cate_id.as_deref());

                let token_out = h.receives.first().and_then(|t| {
                    t.token_id.as_ref().and_then(|id| {
                        token_dict
                            .get(id)
                            .and_then(|info| info.symbol.clone())
                    })
                });
                let token_in = h.sends.first().and_then(|t| {
                    t.token_id.as_ref().and_then(|id| {
                        token_dict
                            .get(id)
                            .and_then(|info| info.symbol.clone())
                    })
                });
                let amount = h.sends.first().and_then(|t| t.amount);

                let success = h.tx.as_ref().and_then(|tx| {
                    tx.status.as_deref().map(|s| s == "ok")
                });

                Some(DebankHistoryRecord {
                    tx_hash,
                    time,
                    action,
                    token_in,
                    token_out,
                    value_usd: None,
                    amount,
                    success,
                })
            })
            .collect())
    }

    fn require_key(&self) -> Result<()> {
        if self.api_key.is_empty() {
            Err(ChainError::Config(
                "DEBANK_API_KEY not set. Run: chainpilot config set debank_api_key <key>"
                    .to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

fn map_http_err(e: reqwest::Error) -> ChainError {
    ChainError::Http(e)
}

fn debank_cate_to_action(cate_id: Option<&str>) -> String {
    match cate_id {
        Some("send") => "send".to_string(),
        Some("receive") => "receive".to_string(),
        Some("approve") => "approve".to_string(),
        Some("swap") => "swap".to_string(),
        Some("bridge") => "bridge".to_string(),
        Some("deposit") => "deposit".to_string(),
        Some("withdraw") => "withdraw".to_string(),
        Some("claim") => "claim".to_string(),
        Some(other) => other.to_string(),
        None => "unknown".to_string(),
    }
}

/// Map a Debank chain slug (e.g. "eth", "bsc", "matic") to an EVM chain ID.
pub fn debank_chain_to_id(chain: &str) -> Option<u64> {
    match chain.to_ascii_lowercase().as_str() {
        "eth" => Some(1),
        "bsc" => Some(56),
        "matic" | "polygon" => Some(137),
        "arb" | "arbitrum" => Some(42161),
        "op" | "optimism" => Some(10),
        "avax" | "avalanche" => Some(43114),
        "base" => Some(8453),
        "linea" => Some(59144),
        "scrl" | "scroll" => Some(534352),
        "mnt" | "mantle" => Some(5000),
        "aurora" => Some(1313161554),
        "okt" => Some(66),
        "cfx" => Some(1030),
        "manta" => Some(169),
        "plume" => Some(98866),
        "taiko" => Some(167000),
        _ => None,
    }
}

fn classify_position(name: &Option<String>, tokens: &[DeFiPositionToken]) -> String {
    let name_lower = name
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    if name_lower.contains("deposit") || name_lower.contains("supply") || name_lower.contains("lend")
    {
        return "deposit".to_string();
    }
    if name_lower.contains("borrow") || name_lower.contains("debt") {
        return "borrow".to_string();
    }
    if name_lower.contains("stake") || name_lower.contains("staking") {
        return "stake".to_string();
    }
    if name_lower.contains("liquidity") || name_lower.contains(" lp") || name_lower.contains("pool")
    {
        return "liquidity".to_string();
    }
    if name_lower.contains("farm") || name_lower.contains("yield") {
        return "yield".to_string();
    }
    if name_lower.contains("vault") {
        return "vault".to_string();
    }
    // If there are two tokens named together (LP pair), treat as liquidity.
    if tokens.len() == 2 {
        return "liquidity".to_string();
    }
    "position".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debank_chain_to_id_maps_common_chains() {
        assert_eq!(debank_chain_to_id("eth"), Some(1));
        assert_eq!(debank_chain_to_id("ETH"), Some(1));
        assert_eq!(debank_chain_to_id("base"), Some(8453));
        assert_eq!(debank_chain_to_id("matic"), Some(137));
        assert_eq!(debank_chain_to_id("polygon"), Some(137));
        assert_eq!(debank_chain_to_id("unknown"), None);
    }

    #[test]
    fn client_without_key_reports_unconfigured() {
        let http = Client::new();
        let client = DebankClient::new(http, "https://pro-openapi.debank.com/v1", "");
        assert!(!client.is_configured());
        assert!(client.require_key().is_err());
    }

    #[test]
    fn client_with_key_is_configured() {
        let http = Client::new();
        let client = DebankClient::new(http, "https://pro-openapi.debank.com/v1", "test-key");
        assert!(client.is_configured());
        assert!(client.require_key().is_ok());
    }
}
