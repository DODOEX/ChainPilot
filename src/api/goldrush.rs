use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::api::send_retrying;
use crate::error::{ChainError, Result};

#[derive(Clone)]
pub struct GoldrushClient {
    client: Client,
    base_url: String,
    api_key: String,
}

#[derive(Debug, Deserialize)]
struct BalancesResponse {
    data: Option<BalancesData>,
}

#[derive(Debug, Deserialize)]
struct ActivityResponse {
    data: Option<ActivityData>,
}

#[derive(Debug, Deserialize)]
struct ActivityData {
    #[serde(default)]
    items: Vec<ActivityItem>,
}

#[derive(Debug, Deserialize)]
struct ActivityItem {
    /// Numeric chain ID, returned as a JSON string ("1", "56", …) by Goldrush.
    chain_id: Option<String>,
    is_testnet: Option<bool>,
    last_seen_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BalancesData {
    chain_id: Option<u64>,
    chain_name: Option<String>,
    #[serde(default)]
    items: Vec<BalancesItem>,
}

#[derive(Debug, Deserialize)]
struct BalancesItem {
    contract_address: Option<String>,
    contract_name: Option<String>,
    contract_ticker_symbol: Option<String>,
    contract_decimals: Option<u8>,
    balance: Option<String>,
    quote_rate: Option<f64>,
    quote: Option<f64>,
    #[serde(rename = "type")]
    item_type: Option<String>,
    native_token: Option<bool>,
}

/// One token balance, normalized into our wallet schema.
#[derive(Debug, Clone)]
pub struct GoldrushAssetRecord {
    pub chain: String,
    pub chain_id: u64,
    pub symbol: String,
    pub name: String,
    pub address: String,
    pub amount: f64,
    pub price_usd: Option<f64>,
    pub value_usd: Option<f64>,
    pub is_native: bool,
}

/// All token balances on a single chain.
#[derive(Debug, Clone)]
pub struct GoldrushChainBalance {
    pub chain_id: u64,
    pub chain_name: String,
    pub items: Vec<GoldrushAssetRecord>,
}

impl GoldrushChainBalance {
    pub fn total_usd(&self) -> f64 {
        self.items.iter().filter_map(|i| i.value_usd).sum()
    }
}

impl GoldrushClient {
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

    /// Discover which chains an address has been active on. Returns numeric
    /// mainnet chain IDs in last-seen-first order. Testnet entries are dropped.
    /// The endpoint costs 1 credit regardless of how many chains it surfaces,
    /// which is dramatically cheaper than blindly hitting balances_v2 on every
    /// supported chain.
    pub async fn active_chains(&self, address: &str) -> Result<Vec<u64>> {
        self.require_key()?;
        let url = format!("{}/address/{}/activity/", self.base_url, address);

        let req = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(15))
            .query(&[("key", self.api_key.as_str())]);
        let resp: ActivityResponse = send_retrying(req, "goldrush.activity")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .json()
            .await
            .map_err(ChainError::Http)?;

        let mut items = resp.data.map(|d| d.items).unwrap_or_default();
        // Most-recent activity first, so chains with stale dust sink down if
        // the caller later truncates the list.
        items.sort_by(|a, b| b.last_seen_at.cmp(&a.last_seen_at));

        let chains = items
            .into_iter()
            .filter(|i| !i.is_testnet.unwrap_or(false))
            .filter_map(|i| i.chain_id.and_then(|s| s.parse::<u64>().ok()))
            .collect();
        Ok(chains)
    }

    pub async fn balances_v2(
        &self,
        chain_id: u64,
        address: &str,
    ) -> Result<GoldrushChainBalance> {
        self.require_key()?;
        let url = format!(
            "{}/{}/address/{}/balances_v2/",
            self.base_url, chain_id, address
        );

        let req = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(15))
            .query(&[("key", self.api_key.as_str()), ("nft", "false")]);
        let resp: BalancesResponse = send_retrying(req, "goldrush.balances_v2")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .json()
            .await
            .map_err(ChainError::Http)?;

        let data = resp.data.ok_or_else(|| ChainError::Config(
            format!("Goldrush returned no data for chain {chain_id}"),
        ))?;

        let chain_name = data
            .chain_name
            .clone()
            .unwrap_or_else(|| format!("chain-{chain_id}"));
        let resolved_chain_id = data.chain_id.unwrap_or(chain_id);

        let items = data
            .items
            .into_iter()
            .filter(|i| i.item_type.as_deref() != Some("nft"))
            .filter_map(|i| {
                let decimals = i.contract_decimals.unwrap_or(18);
                let amount = parse_token_amount(i.balance.as_deref()?, decimals)?;
                if amount <= 0.0 {
                    return None;
                }
                let symbol = i.contract_ticker_symbol.unwrap_or_default();
                let name = i.contract_name.unwrap_or_else(|| symbol.clone());
                let address = i.contract_address.unwrap_or_default();
                Some(GoldrushAssetRecord {
                    chain: chain_name.clone(),
                    chain_id: resolved_chain_id,
                    symbol,
                    name,
                    address,
                    amount,
                    price_usd: i.quote_rate,
                    value_usd: i.quote,
                    is_native: i.native_token.unwrap_or(false),
                })
            })
            .collect();

        Ok(GoldrushChainBalance {
            chain_id: resolved_chain_id,
            chain_name,
            items,
        })
    }

    fn require_key(&self) -> Result<()> {
        if self.api_key.is_empty() {
            Err(ChainError::Config(
                "GOLDRUSH_API_KEY not set. Run: chainpilot config set goldrush_api_key <key>"
                    .to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Parse a raw integer balance string into a human-readable float, returning None on garbage.
fn parse_token_amount(raw: &str, decimals: u8) -> Option<f64> {
    let raw_uint: u128 = raw.parse().ok()?;
    let divisor = 10u128.pow(decimals as u32) as f64;
    Some(raw_uint as f64 / divisor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_token_amount_handles_common_cases() {
        assert_eq!(parse_token_amount("0", 18), Some(0.0));
        assert_eq!(parse_token_amount("1234567", 6), Some(1.234567));
        assert_eq!(parse_token_amount("42", 0), Some(42.0));
        assert!(parse_token_amount("not-a-number", 18).is_none());
    }

    #[test]
    fn client_without_key_reports_unconfigured() {
        let http = Client::new();
        let client = GoldrushClient::new(http, "https://api.covalenthq.com/v1", "");
        assert!(!client.is_configured());
        assert!(client.require_key().is_err());
    }

    #[test]
    fn client_with_key_is_configured() {
        let http = Client::new();
        let client = GoldrushClient::new(http, "https://api.covalenthq.com/v1", "test-key");
        assert!(client.is_configured());
        assert!(client.require_key().is_ok());
    }
}
