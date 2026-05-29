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
    #[serde(default)]
    stats: Option<DebankPortfolioStats>,
}

#[derive(Debug, Deserialize)]
struct DebankPortfolioStats {
    net_usd_value: Option<f64>,
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
pub struct DebankTotalBalance {
    pub total_usd_value: Option<f64>,
    pub chains: Vec<DebankChainSummary>,
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
