use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ChainError, Result};
use crate::models::quote::{Quote, QuoteRequest, RouteHop, TokenRef};

pub struct DodoClient {
    client: Client,
    base_url: String,
    api_key: String,
    project_id: String,
}

// ── tokenlist cache ──────────────────────────────────────────────────────────

const TOKENLIST_CACHE_TTL_SECS: i64 = 3600; // 1 hour

#[derive(Debug, Serialize, Deserialize)]
struct TokenListCache {
    fetched_at: DateTime<Utc>,
    /// chain_id (string) → token list
    tokens_by_chain: HashMap<String, Vec<CachedToken>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedToken {
    symbol: String,
    address: String,
    decimals: u8,
}

impl TokenListCache {
    fn is_fresh(&self) -> bool {
        Utc::now()
            .signed_duration_since(self.fetched_at)
            .num_seconds()
            < TOKENLIST_CACHE_TTL_SECS
    }

    fn find(&self, chain_id: u64, symbol_upper: &str) -> Option<TokenRef> {
        self.tokens_by_chain
            .get(&chain_id.to_string())?
            .iter()
            .find(|t| t.symbol.to_uppercase() == symbol_upper)
            .map(|t| TokenRef {
                symbol: t.symbol.clone(),
                address: t.address.clone(),
                decimals: t.decimals,
                chain_id,
            })
    }
}

fn load_cache(path: &Path) -> Option<TokenListCache> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_cache(path: &Path, by_chain: HashMap<String, Vec<CachedToken>>) {
    let cache = TokenListCache {
        fetched_at: Utc::now(),
        tokens_by_chain: by_chain,
    };
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = std::fs::write(path, json);
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum DodoStatus {
    Code(i64),
    Text(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DodoResponse {
    status: DodoStatus,
    data: Option<DodoRouteData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DodoRouteData {
    #[serde(rename = "resAmount")]
    res_amount: f64,
    #[serde(rename = "baseFeeAmount")]
    base_fee_amount: Option<f64>,
    #[serde(rename = "baseFeeRate")]
    base_fee_rate: Option<f64>,
    #[serde(rename = "resPricePerToToken")]
    res_price_per_to_token: Option<f64>,
    #[serde(rename = "resPricePerFromToken")]
    res_price_per_from_token: Option<f64>,
    #[serde(rename = "priceImpact")]
    price_impact: f64,
    #[serde(rename = "useSource")]
    use_source: String,
    #[serde(rename = "targetSymbol")]
    target_symbol: Option<String>,
    #[serde(rename = "targetDecimals")]
    target_decimals: Option<u8>,
    to: String,
    data: String,
    #[serde(rename = "minReturnAmount")]
    min_return_amount: String,
    #[serde(rename = "gasLimit")]
    gas_limit: Option<FlexibleU64>,
    #[serde(rename = "estimatedGas")]
    estimated_gas: Option<FlexibleU64>,
    #[serde(rename = "routeInfo")]
    route_info: Option<DodoRouteInfo>,
    value: String,
    id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DodoRouteInfo {
    #[serde(rename = "subRouteTotalPart")]
    sub_route_total_part: Option<f64>,
    #[serde(rename = "subRoute", default)]
    sub_route: Vec<DodoSubRoute>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DodoSubRoute {
    #[serde(rename = "midPathPart")]
    mid_path_part: f64,
    #[serde(rename = "midPath", default)]
    mid_path: Vec<DodoMidPath>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DodoMidPath {
    #[serde(rename = "fromToken")]
    from_token: String,
    #[serde(rename = "toToken")]
    to_token: String,
    #[serde(rename = "poolDetails", default)]
    pool_details: Vec<DodoPoolDetail>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DodoPoolDetail {
    #[serde(rename = "poolName")]
    pool_name: String,
    #[serde(rename = "poolPart")]
    pool_part: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum FlexibleU64 {
    Int(u64),
    StrNumString(String),
}

impl FlexibleU64 {
    fn into_u64(self) -> Result<u64> {
        match self {
            Self::Int(value) => Ok(value),
            Self::StrNumString(value) => value.parse::<u64>().map_err(|e| ChainError::DodoApi {
                code: 0,
                message: format!("invalid integer field from DODO API: {}", e),
            }),
        }
    }
}

impl DodoStatus {
    fn is_success(&self) -> bool {
        match self {
            Self::Code(code) => *code == 200,
            Self::Text(text) => text == "success",
        }
    }

    fn as_code(&self) -> i64 {
        match self {
            Self::Code(code) => *code,
            Self::Text(_) => 0,
        }
    }
}

impl DodoClient {
    pub fn new(client: Client, base_url: &str, api_key: &str, project_id: &str) -> Self {
        Self {
            client,
            base_url: base_url.to_string(),
            api_key: api_key.to_string(),
            project_id: project_id.to_string(),
        }
    }

    /// Look up a token by symbol on the given chain.
    ///
    /// Strategy:
    /// 1. If the on-disk cache is fresh (< 1 h), search it and return.
    /// 2. Otherwise fetch the tokenlist from the DODO API and refresh the cache.
    /// 3. If the API call fails (network error, timeout, missing project_id),
    ///    fall back to the stale cache.
    /// 4. Returns `Ok(None)` if the symbol is not found in any source.
    /// 5. Returns `Err(...)` when the tokenlist API request itself fails.
    pub async fn find_token_by_symbol(
        &self,
        chain_id: u64,
        symbol: &str,
        cache_path: &Path,
    ) -> Result<Option<TokenRef>> {
        let upper = symbol.to_uppercase();
        let stale_cache = load_cache(cache_path);

        // Fresh cache → use directly.
        if let Some(ref c) = stale_cache {
            if c.is_fresh() {
                return Ok(c.find(chain_id, &upper));
            }
        }

        // Try fetching from API (skip if no project_id configured).
        if !self.project_id.is_empty() {
            match self.fetch_tokenlist_by_chain().await {
                Ok(by_chain) => {
                    let result = by_chain
                        .get(&chain_id.to_string())
                        .and_then(|tokens| tokens.iter().find(|t| t.symbol.to_uppercase() == upper))
                        .map(|t| TokenRef {
                            symbol: t.symbol.clone(),
                            address: t.address.clone(),
                            decimals: t.decimals,
                            chain_id,
                        });
                    save_cache(cache_path, by_chain);
                    return Ok(result);
                }
                Err(e) => {
                    eprintln!(
                        "Warning: tokenlist API unavailable ({}), falling back to cache",
                        e
                    );
                }
            }
        }

        // API unavailable → fall back to stale cache.
        Ok(stale_cache.and_then(|c| c.find(chain_id, &upper)))
    }

    async fn fetch_tokenlist_by_chain(&self) -> Result<HashMap<String, Vec<CachedToken>>> {
        let url = "https://api.dodoex.io/config-center/user/tokenlist/v2";
        let resp = self
            .client
            .get(url)
            .timeout(Duration::from_secs(5))
            .query(&[("project", &self.project_id), ("apikey", &self.api_key)])
            .send()
            .await
            .map_err(|e| {
                let reason = if e.is_timeout() {
                    "timed out".to_string()
                } else if e.is_connect() {
                    format!("connection refused or unreachable: {}", e.without_url())
                } else {
                    e.without_url().to_string()
                };
                ChainError::DodoApi {
                    code: 0,
                    message: format!("tokenlist fetch failed: {}", reason),
                }
            })?;

        let text = resp.text().await.map_err(|e| ChainError::DodoApi {
            code: 0,
            message: format!(
                "failed to read tokenlist response body: {}",
                e.without_url()
            ),
        })?;
        let val: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| ChainError::DodoApi {
                code: 0,
                message: format!(
                    "tokenlist response is not valid JSON ({}): {}",
                    e,
                    text.chars().take(120).collect::<String>()
                ),
            })?;

        // Response may be wrapped: { data: { chains: [...] } } or { chains: [...] }
        let chains = val["data"]["chains"]
            .as_array()
            .or_else(|| val["chains"].as_array())
            .ok_or_else(|| ChainError::DodoApi {
                code: 0,
                message: "missing chains in tokenlist response".to_string(),
            })?;

        let mut by_chain: HashMap<String, Vec<CachedToken>> = HashMap::new();
        for chain in chains {
            let chain_id_str = chain["chainId"]
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| chain["chainId"].as_u64().map(|n| n.to_string()))
                .unwrap_or_default();
            if chain_id_str.is_empty() {
                continue;
            }
            if let Some(tokens) = chain["tokens"].as_array() {
                let entry = by_chain.entry(chain_id_str).or_default();
                for t in tokens {
                    let symbol = t["symbol"].as_str().unwrap_or("").to_string();
                    let address = t["address"].as_str().unwrap_or("").to_string();
                    let decimals = t["decimals"].as_u64().unwrap_or(18) as u8;
                    if symbol.is_empty() || address.is_empty() {
                        continue;
                    }
                    entry.push(CachedToken {
                        symbol,
                        address,
                        decimals,
                    });
                }
            }
        }
        Ok(by_chain)
    }

    pub async fn get_route(
        &self,
        req: &QuoteRequest,
        from_token: &TokenRef,
        to_token: &TokenRef,
        user_addr: &str,
        estimate_gas: bool,
        quote_ttl_secs: u64,
    ) -> Result<Quote> {
        let deadline = (Utc::now() + Duration::from_secs(1200)).timestamp() as u64;

        let amount_str = crate::commands::to_raw_amount(&req.amount, from_token.decimals)?;

        let query = vec![
            ("chainId", req.chain_id.to_string()),
            ("deadLine", deadline.to_string()),
            ("apikey", self.api_key.clone()),
            ("slippage", req.slippage.to_string()),
            ("source", "dodoV2AndMixWasm".to_string()),
            ("toTokenAddress", req.to.to_lowercase()),
            ("fromTokenAddress", req.from.to_lowercase()),
            ("fromAmount", amount_str),
            ("userAddr", user_addr.to_string()),
            ("estimateGas", estimate_gas.to_string()),
        ];
        let resp = self.client.get(&self.base_url).query(&query).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(ChainError::DodoApi {
                code: status.as_u16() as i64,
                message: text,
            });
        }

        let text = resp.text().await?;
        let raw_response: serde_json::Value = serde_json::from_str(&text)?;
        let dodo_resp: DodoResponse =
            serde_json::from_str(&text).map_err(|e| ChainError::DodoApi {
                code: 0,
                message: format!(
                    "json parse error at pos {}: {} - text len {}",
                    e.column(),
                    e,
                    text.len()
                ),
            })?;

        if !dodo_resp.status.is_success() {
            return Err(ChainError::DodoApi {
                code: dodo_resp.status.as_code(),
                message: "API returned error".to_string(),
            });
        }

        let data = dodo_resp.data.ok_or_else(|| ChainError::DodoApi {
            code: dodo_resp.status.as_code(),
            message: "DODO API response missing data".to_string(),
        })?;

        let now = Utc::now();
        let expires_at = now + Duration::from_secs(quote_ttl_secs);
        let route_summary = parse_route_summary(&data.route_info);

        let target_decimals = data.target_decimals.unwrap_or(to_token.decimals);

        Ok(Quote {
            quote_id: Uuid::new_v4(),
            created_at: now,
            expires_at,
            from_token: from_token.clone(),
            to_token: TokenRef {
                symbol: data
                    .target_symbol
                    .unwrap_or_else(|| to_token.symbol.clone()),
                address: to_token.address.clone(),
                decimals: target_decimals,
                chain_id: req.chain_id,
            },
            from_amount: req.amount.clone(),
            from_amount_display: req.amount_display,
            to_amount: data.res_amount.to_string(),
            to_amount_display: data.res_amount,
            to_amount_min: raw_amount_to_display(&data.min_return_amount, target_decimals),
            price_impact_pct: data.price_impact,
            exchange_rate: if req.amount_display > 0.0 {
                data.res_amount / req.amount_display
            } else {
                0.0
            },
            route_summary,
            dex_sources: vec![data.use_source.clone()],
            route_id: data.id.clone(),
            router_to: data.to.clone(),
            calldata: data.data.clone(),
            value: data.value.clone(),
            gas_limit: parse_optional_u64(data.gas_limit)?,
            estimated_gas: parse_optional_u64(data.estimated_gas)?,
            estimated_gas_usd: None,
            raw_dodo_response: raw_response.get("data").cloned().ok_or_else(|| {
                ChainError::DodoApi {
                    code: 0,
                    message: "DODO API response missing raw data payload".to_string(),
                }
            })?,
            chain_id: req.chain_id,
            slippage: req.slippage,
        })
    }
}

fn parse_optional_u64(value: Option<FlexibleU64>) -> Result<Option<u64>> {
    value.map(FlexibleU64::into_u64).transpose()
}

fn raw_amount_to_display(raw: &str, decimals: u8) -> String {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with('-') {
        return raw.to_string();
    }
    let decimals = decimals as usize;
    if decimals == 0 {
        return raw.to_string();
    }
    let padded = if raw.len() <= decimals {
        format!("{}{}", "0".repeat(decimals + 1 - raw.len()), raw)
    } else {
        raw.to_string()
    };
    let split = padded.len().saturating_sub(decimals);
    let (whole, frac) = padded.split_at(split);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{frac}")
    }
}

fn parse_route_summary(route_info: &Option<DodoRouteInfo>) -> Vec<RouteHop> {
    let Some(route_info) = route_info else {
        return Vec::new();
    };

    let mut hops = Vec::new();
    for sub_route in &route_info.sub_route {
        for mid_path in &sub_route.mid_path {
            if mid_path.pool_details.is_empty() {
                hops.push(RouteHop {
                    pool_address: String::new(),
                    dex_name: String::new(),
                    from_token: mid_path.from_token.clone(),
                    to_token: mid_path.to_token.clone(),
                    percent: sub_route.mid_path_part,
                });
                continue;
            }

            for pool in &mid_path.pool_details {
                hops.push(RouteHop {
                    pool_address: String::new(),
                    dex_name: pool.pool_name.clone(),
                    from_token: mid_path.from_token.clone(),
                    to_token: mid_path.to_token.clone(),
                    percent: sub_route.mid_path_part * pool.pool_part / 100.0,
                });
            }
        }
    }

    hops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_amount_to_display_converts_dodo_min_return_once() {
        assert_eq!(
            raw_amount_to_display("1679119161479651", 18),
            "0.001679119161479651"
        );
        assert_eq!(raw_amount_to_display("2970000000", 6), "2970");
    }
}
