use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

use crate::api::send_retrying;
use crate::error::{ChainError, Result};
use crate::models::chain::{
    ChainFlows, ChainFlowsSources, ChainInfo, ChainInfoSources, ChainProtocolEntry, ChainProtocols,
    ChainProtocolsSources, ChainStablecoins, ChainStablecoinsSources, StablecoinType,
};

#[derive(Clone)]
pub struct ChainClient {
    client: Client,
    defillama_url: String,
    coingecko_url: String,
    coingecko_key: Option<String>,
}

// ── DefiLlama response types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DefiLlamaChain {
    name: String,
    #[serde(rename = "tokenSymbol")]
    token_symbol: Option<String>,
    tvl: Option<f64>,
    #[serde(rename = "chainId", deserialize_with = "deserialize_optional_u64_from_string_or_int")]
    chain_id: Option<u64>,
}

/// DefiLlama returns `chainId` as either a JSON number or a quoted string.
/// Accept both so deserialization doesn't fail on chains like "11235".
fn deserialize_optional_u64_from_string_or_int<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct Visitor;

    impl<'de> de::Visitor<'de> for Visitor {
        type Value = Option<u64>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a u64, a string-encoded u64, or null")
        }

        fn visit_none<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
            Ok(Some(v))
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
            Ok(Some(v as u64))
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Self::Value, E> {
            Ok(Some(v as u64))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Self::Value, E> {
            match v.parse::<u64>() {
                Ok(n) => Ok(Some(n)),
                Err(_) => Err(de::Error::invalid_value(
                    de::Unexpected::Str(v),
                    &"a numeric string",
                )),
            }
        }
    }

    deserializer.deserialize_any(Visitor)
}

#[derive(Debug, Deserialize)]
struct DefiLlamaProtocol {
    name: String,
    #[allow(dead_code)]
    slug: String,
    category: Option<String>,
    chains: Option<Vec<String>>,
    tvl: Option<f64>,
    chain_tvls: Option<std::collections::HashMap<String, f64>>,
    #[allow(dead_code)]
    #[serde(rename = "change_1d")]
    change_1d: Option<f64>,
    #[allow(dead_code)]
    #[serde(rename = "change_7d")]
    change_7d: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct DefiLlamaStablecoin {
    name: String,
    symbol: String,
    #[allow(dead_code)]
    chain: Option<String>,
    chains: Option<Vec<String>>,
    circulating: Option<DefiLlamaStablecoinCirculating>,
    #[allow(dead_code)]
    peggedUSD: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct DefiLlamaStablecoinCirculating {
    peggedUSD: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct DefiLlamaStablecoinsResponse {
    peggedAssets: Option<Vec<DefiLlamaStablecoin>>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoSimplePrice {
    #[serde(flatten)]
    prices: std::collections::HashMap<String, CoinGeckoPriceEntry>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoPriceEntry {
    usd: Option<f64>,
}

impl ChainClient {
    pub fn new(
        client: Client,
        defillama_url: &str,
        coingecko_url: &str,
        coingecko_key: Option<String>,
    ) -> Self {
        Self {
            client,
            defillama_url: defillama_url.trim_end_matches('/').to_string(),
            coingecko_url: coingecko_url.trim_end_matches('/').to_string(),
            coingecko_key,
        }
    }

    pub async fn chain_info(&self, chain: &str) -> Result<ChainInfo> {
        let chains = self.fetch_defillama_chains().await?;
        let resolved = resolve_chain(chain, &chains)?;

        let matched = chains.iter().find(|c| c.name == resolved).unwrap();
        let chain_id = matched.chain_id;
        let native_token = matched.token_symbol.clone();
        let tvl = matched.tvl;
        let sources_base = "defillama:chains".to_string();

        // Fetch native token price, fees, and active users concurrently
        let price_fut = async {
            if let Some(ref token) = native_token {
                self.fetch_native_price(&resolved, token).await
            } else {
                None
            }
        };
        let fees_fut = self.fetch_chain_fees(&resolved);
        let active_fut = self.fetch_chain_active_users(&resolved);

        let (native_price, fees_result, active_result) =
            tokio::join!(price_fut, fees_fut, active_fut);

        let fees_24h = fees_result.unwrap_or(None);
        let active_addresses = active_result.unwrap_or(None);

        let price_source = if native_price.is_some() {
            Some("coingecko".to_string())
        } else {
            None
        };
        let fees_source = if fees_24h.is_some() {
            Some("defillama:fees".to_string())
        } else {
            None
        };
        let active_source = if active_addresses.is_some() {
            Some("defillama:activeUsers".to_string())
        } else {
            None
        };

        Ok(ChainInfo {
            chain: resolved,
            chain_id,
            native_token,
            native_price,
            tvl,
            active_addresses,
            tx_count_24h: None,
            fees_24h,
            throughput: None,
            sources: ChainInfoSources {
                chain: Some(sources_base.clone()),
                chain_id: Some(sources_base.clone()),
                native_token: Some(sources_base.clone()),
                native_price: price_source,
                tvl: Some(sources_base),
                active_addresses: active_source,
                tx_count_24h: None,
                fees_24h: fees_source,
                throughput: None,
            },
        })
    }

    pub async fn chain_flows(&self, chain: &str) -> Result<ChainFlows> {
        let chains = self.fetch_defillama_chains().await?;
        let resolved = resolve_chain(chain, &chains)?;

        // Flow data is limited with public APIs - return structure with available data
        Ok(ChainFlows {
            chain: resolved,
            net_flow_usd: None,
            inflow_usd: None,
            outflow_usd: None,
            bridge_flow: Vec::new(),
            cex_flow: Vec::new(),
            stablecoin_flow: Vec::new(),
            sources: ChainFlowsSources::default(),
        })
    }

    pub async fn chain_stablecoins(&self, chain: &str) -> Result<ChainStablecoins> {
        let chains = self.fetch_defillama_chains().await?;
        let resolved = resolve_chain(chain, &chains)?;

        let stablecoins = self.fetch_defillama_stablecoins(&resolved).await?;

        let total_supply: f64 = stablecoins.iter().map(|s| s.supply).sum();

        let stablecoin_types: Vec<StablecoinType> = stablecoins
            .iter()
            .map(|s| StablecoinType {
                name: s.name.clone(),
                supply: s.supply,
                share_pct: if total_supply > 0.0 {
                    (s.supply / total_supply) * 100.0
                } else {
                    0.0
                },
            })
            .collect();

        let supply_source = if stablecoin_types.is_empty() {
            None
        } else {
            Some("defillama:stablecoins".to_string())
        };

        Ok(ChainStablecoins {
            chain: resolved,
            stablecoin_supply: if total_supply > 0.0 {
                Some(total_supply)
            } else {
                None
            },
            stablecoin_types,
            stablecoin_flow_24h: None,
            sources: ChainStablecoinsSources {
                stablecoin_supply: supply_source.clone(),
                stablecoin_types: supply_source,
                stablecoin_flow_24h: None,
            },
        })
    }

    pub async fn chain_protocols(&self, chain: &str, limit: u32) -> Result<ChainProtocols> {
        let chains = self.fetch_defillama_chains().await?;
        let resolved = resolve_chain(chain, &chains)?;

        let all_protocols = self.fetch_defillama_protocols().await?;

        // Filter protocols by chain and sort by TVL descending
        let mut protocols: Vec<ChainProtocolEntry> = all_protocols
            .iter()
            .filter(|p| {
                p.chains
                    .as_ref()
                    .map(|chains| chains.iter().any(|c| c.eq_ignore_ascii_case(&resolved)))
                    .unwrap_or(false)
            })
            .map(|p| {
                let chain_tvl = p
                    .chain_tvls
                    .as_ref()
                    .and_then(|tvls| tvls.get(&resolved).copied())
                    .or(p.tvl);
                ChainProtocolEntry {
                    name: p.name.clone(),
                    tvl: chain_tvl,
                    revenue: None,
                    users: None,
                    category: p.category.clone(),
                }
            })
            .collect();

        protocols.sort_by(|a, b| {
            b.tvl
                .unwrap_or(0.0)
                .partial_cmp(&a.tvl.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total = protocols.len();
        let limit = limit as usize;
        protocols.truncate(limit);

        Ok(ChainProtocols {
            chain: resolved,
            protocols,
            sources: ChainProtocolsSources {
                protocols: Some(format!(
                    "defillama:protocols ({} of {})",
                    limit.min(total),
                    total
                )),
            },
        })
    }

    // ── internal fetchers ────────────────────────────────────────────────────

    async fn fetch_defillama_chains(&self) -> Result<Vec<DefiLlamaChain>> {
        let url = format!("{}/chains", self.defillama_url);
        let req = self.client.get(&url);
        let body = send_retrying(req, "defillama.chains")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "DefiLlama chains response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })
    }

    async fn fetch_defillama_protocols(&self) -> Result<Vec<DefiLlamaProtocol>> {
        let url = format!("{}/protocols", self.defillama_url);
        let req = self.client.get(&url);
        let body = send_retrying(req, "defillama.protocols")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "DefiLlama protocols response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })
    }

    async fn fetch_defillama_stablecoins(
        &self,
        chain_name: &str,
    ) -> Result<Vec<SimpleStablecoin>> {
        let url = format!("{}/stablecoins", self.defillama_url);
        let req = self.client.get(&url).query(&[("includePrices", "false")]);
        let body = send_retrying(req, "defillama.stablecoins")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let resp: DefiLlamaStablecoinsResponse = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "DefiLlama stablecoins response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })?;

        let assets = resp.peggedAssets.unwrap_or_default();

        Ok(assets
            .into_iter()
            .filter_map(|asset| {
                // Check if this stablecoin exists on the target chain
                let chains = asset.chains.unwrap_or_default();
                if !chains.iter().any(|c| c.eq_ignore_ascii_case(chain_name)) {
                    return None;
                }

                let supply = asset
                    .circulating
                    .as_ref()
                    .and_then(|c| c.peggedUSD)
                    .unwrap_or(0.0);

                if supply <= 0.0 {
                    return None;
                }

                Some(SimpleStablecoin {
                    name: format!("{} ({})", asset.name, asset.symbol),
                    supply,
                })
            })
            .collect())
    }

    async fn fetch_native_price(&self, chain_name: &str, _token_symbol: &str) -> Option<f64> {
        // Map chain name to CoinGecko ID for the native token
        let coingecko_id = chain_to_coingecko_id(chain_name)?;

        let url = format!("{}/simple/price", self.coingecko_url);
        let mut req = self
            .client
            .get(&url)
            .query(&[("ids", coingecko_id), ("vs_currencies", "usd")]);
        if let Some(ref key) = self.coingecko_key {
            req = req.header("x-cg-demo-api-key", key);
        }

        let body = send_retrying(req, "coingecko.simple_price")
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .text()
            .await
            .ok()?;

        let resp: CoinGeckoSimplePrice = serde_json::from_str(&body).ok()?;
        resp.prices.get(coingecko_id)?.usd
    }

    /// Fetch 24h fees for a chain from DefiLlama `/summary/fees/{chain}`.
    async fn fetch_chain_fees(&self, chain_name: &str) -> Result<Option<f64>> {
        let url = format!("{}/summary/fees/{}", self.defillama_url, chain_name);
        let req = self
            .client
            .get(&url)
            .query(&[("dataType", "dailyFees")]);
        let body = match send_retrying(req, "defillama.chain_fees").await {
            Ok(resp) => match resp.error_for_status() {
                Ok(r) => r.text().await.map_err(ChainError::Http)?,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };

        let value: Value = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "DefiLlama chain fees response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })?;

        Ok(value.get("total24h").and_then(|v| v.as_f64()))
    }

    /// Fetch active users for a chain from DefiLlama.
    async fn fetch_chain_active_users(&self, chain_name: &str) -> Result<Option<u64>> {
        let url = format!("{}/overview/activeUsers", self.defillama_url);
        let req = self
            .client
            .get(&url)
            .query(&[("chain", chain_name)]);
        let body = match send_retrying(req, "defillama.chain_active_users").await {
            Ok(resp) => match resp.error_for_status() {
                Ok(r) => r.text().await.map_err(ChainError::Http)?,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };

        let value: Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };

        // Try to extract total active users for the chain
        // Response may have a `total24h` or `users24h` field
        let count = value
            .get("total24h")
            .or_else(|| value.get("users24h"))
            .and_then(|v| v.as_f64())
            .map(|v| v as u64);

        Ok(count)
    }
}

struct SimpleStablecoin {
    name: String,
    supply: f64,
}

/// Resolve user input (chain ID, abbreviation, alias, or name) to the
/// canonical DefiLlama chain name. Matching order:
/// 1. Numeric chain ID (e.g. "1", "8453")
/// 2. Well-known abbreviation/alias (e.g. "eth", "arb", "op")
/// 3. Exact DefiLlama name (case-insensitive)
/// 4. DefiLlama name with spaces removed (e.g. "bnbchain" → "BNB Chain")
fn resolve_chain(input: &str, chains: &[DefiLlamaChain]) -> Result<String> {
    let needle = input.trim().to_lowercase();

    // 1. Numeric chain ID
    if let Ok(chain_id) = needle.parse::<u64>() {
        if let Some(c) = chains.iter().find(|c| c.chain_id == Some(chain_id)) {
            return Ok(c.name.clone());
        }
    }

    // 2. Well-known abbreviation/alias → canonical DefiLlama name
    if let Some(canonical) = chain_alias_to_name(&needle) {
        // Verify it exists in the DefiLlama list
        if chains.iter().any(|c| c.name.eq_ignore_ascii_case(canonical)) {
            return Ok(canonical.to_string());
        }
    }

    // 3. Exact DefiLlama name (case-insensitive)
    if let Some(c) = chains.iter().find(|c| c.name.to_lowercase() == needle) {
        return Ok(c.name.clone());
    }

    // 4. DefiLlama name with spaces removed
    let needle_nospace = needle.replace(' ', "");
    if let Some(c) = chains
        .iter()
        .find(|c| c.name.to_lowercase().replace(' ', "") == needle_nospace)
    {
        return Ok(c.name.clone());
    }

    Err(ChainError::Config(format!(
        "Chain '{}' not found. Use a chain name (ethereum), ID (1), or abbreviation (eth)",
        input
    )))
}

/// Map common abbreviations and aliases to DefiLlama canonical chain names.
fn chain_alias_to_name(alias: &str) -> Option<&'static str> {
    match alias {
        // Ethereum
        "eth" | "ethereum" => Some("Ethereum"),
        // BNB Chain
        "bsc" | "bnb" | "binance" | "bnbchain" | "bnbsmartchain" => Some("BNB Chain"),
        // Polygon
        "matic" | "polygon" | "polygonpos" => Some("Polygon"),
        // Arbitrum
        "arb" | "arbitrum" | "arbitrumone" => Some("Arbitrum"),
        // Optimism
        "op" | "optimism" => Some("Optimism"),
        // Avalanche
        "avax" | "avalanche" => Some("Avalanche"),
        // Base
        "base" => Some("Base"),
        // Linea
        "linea" => Some("Linea"),
        // Scroll
        "scrl" | "scroll" => Some("Scroll"),
        // Mantle
        "mnt" | "mantle" => Some("Mantle"),
        // Aurora
        "aurora" => Some("Aurora"),
        // Blast
        "blast" => Some("Blast"),
        // zkSync
        "zksync" | "era" => Some("zkSync Era"),
        // Starknet
        "starknet" | "strk" => Some("Starknet"),
        // Solana (non-EVM but commonly queried)
        "sol" | "solana" => Some("Solana"),
        // Tron
        "trx" | "tron" => Some("Tron"),
        // Bitcoin
        "btc" | "bitcoin" => Some("Bitcoin"),
        // Cronos
        "cro" | "cronos" => Some("Cronos"),
        // Gnosis / xDai
        "gnosis" | "xdai" => Some("Gnosis"),
        // Fantom
        "ftm" | "fantom" => Some("Fantom"),
        // Celo
        "celo" => Some("Celo"),
        // Moonbeam
        "glmr" | "moonbeam" => Some("Moonbeam"),
        // Kava
        "kava" => Some("Kava"),
        // Manta
        "manta" => Some("Manta Pacific"),
        // Conflux
        "cfx" | "conflux" | "confluxespace" => Some("Conflux"),
        // OKX Chain
        "okt" | "okc" | "okxchain" => Some("OKXChain"),
        // Taiko
        "taiko" => Some("Taiko"),
        // Plume
        "plume" => Some("Plume"),
        // Sonic
        "sonic" | "s" => Some("Sonic"),
        // Sei
        "sei" => Some("Sei"),
        // Mode
        "mode" => Some("Mode"),
        // Bob
        "bob" => Some("BOB"),
        // Abstract
        "abstract" => Some("Abstract"),
        // Berachain
        "bera" | "berachain" => Some("Berachain"),
        // Hyperliquid
        "hyper" | "hyperliquid" => Some("Hyperliquid"),
        _ => None,
    }
}

fn chain_to_coingecko_id(chain_name: &str) -> Option<&'static str> {
    match chain_name.to_lowercase().as_str() {
        "ethereum" => Some("ethereum"),
        "bsc" | "binance" | "bnb chain" | "bnb smart chain" => Some("binancecoin"),
        "polygon" | "polygon pos" | "matic network" => Some("matic-network"),
        "arbitrum" | "arbitrum one" => Some("ethereum"), // ETH on L2
        "optimism" => Some("ethereum"),                   // ETH on L2
        "avalanche" | "avax" => Some("avalanche-2"),
        "base" => Some("ethereum"), // ETH on L2
        "linea" => Some("ethereum"), // ETH on L2
        "scroll" => Some("ethereum"), // ETH on L2
        "mantle" => Some("mantle"),
        "aurora" => Some("ethereum"), // ETH on L2
        _ => None,
    }
}

fn snippet(body: &str) -> String {
    const MAX: usize = 400;
    if body.len() <= MAX {
        body.to_string()
    } else {
        let mut end = MAX;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &body[..end])
    }
}
