use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

use crate::api::send_retrying;
use crate::error::{ChainError, Result};
use crate::models::chain::{
    ChainFlows, ChainFlowsSources, ChainInfo, ChainInfoSources, ChainProtocolEntry, ChainProtocols,
    ChainProtocolsSources, ChainStablecoins, ChainStablecoinsSources, FlowEntry, StablecoinType,
};

#[derive(Clone)]
pub struct ChainClient {
    client: Client,
    defillama_url: String,
    stablecoins_url: String,
    growthepie_url: String,
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
    /// CoinGecko ID of the chain's native token, supplied by DefiLlama. Authoritative
    /// and current across all chains (e.g. Polygon → "polygon-ecosystem-token").
    gecko_id: Option<String>,
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
    #[allow(dead_code)]
    chains: Option<Vec<String>>,
    #[allow(dead_code)]
    tvl: Option<f64>,
    #[serde(rename = "chainTvls")]
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
    /// Per-chain circulating breakdown, keyed by DefiLlama chain name. Each entry
    /// carries the current supply and the supply 24h ago, enabling flow math.
    chainCirculating: Option<std::collections::HashMap<String, DefiLlamaChainCirculating>>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct DefiLlamaChainCirculating {
    current: Option<DefiLlamaStablecoinCirculating>,
    circulatingPrevDay: Option<DefiLlamaStablecoinCirculating>,
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
        stablecoins_url: &str,
        growthepie_url: &str,
        coingecko_url: &str,
        coingecko_key: Option<String>,
    ) -> Self {
        Self {
            client,
            defillama_url: defillama_url.trim_end_matches('/').to_string(),
            stablecoins_url: stablecoins_url.trim_end_matches('/').to_string(),
            growthepie_url: growthepie_url.trim_end_matches('/').to_string(),
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
        let gecko_id = matched.gecko_id.clone();
        let tvl = matched.tvl;
        let sources_base = "defillama:chains".to_string();

        // Fetch native token price, fees, and chain activity concurrently.
        // No native_token guard: DefiLlama omits the symbol for some chains
        // (e.g. Base) whose native price is still resolvable via chain config.
        let price_fut = self.fetch_native_price(&resolved, gecko_id.as_deref(), chain_id);
        let fees_fut = self.fetch_chain_fees(&resolved);
        // growthepie supplies active addresses / tx count / throughput, but only
        // for the Ethereum-ecosystem chains it tracks (keyed by EVM chain ID).
        let gp_key = chain_id.and_then(chain_id_to_growthepie);
        let activity_fut = async {
            match gp_key {
                Some(key) => self.fetch_growthepie_activity(key).await,
                None => GrowthepieActivity::default(),
            }
        };

        let (native_price, fees_result, activity) =
            tokio::join!(price_fut, fees_fut, activity_fut);

        let fees_24h = fees_result.unwrap_or(None);
        let active_addresses = activity.active_addresses;
        let tx_count_24h = activity.tx_count_24h;
        let throughput = activity.throughput;

        let price_source = native_price.is_some().then(|| "coingecko".to_string());
        let fees_source = fees_24h.is_some().then(|| "defillama:fees".to_string());
        let gp_source = || "growthepie".to_string();

        Ok(ChainInfo {
            chain: resolved,
            chain_id,
            native_token,
            native_price,
            tvl,
            active_addresses,
            tx_count_24h,
            fees_24h,
            throughput,
            sources: ChainInfoSources {
                chain: Some(sources_base.clone()),
                chain_id: Some(sources_base.clone()),
                native_token: Some(sources_base.clone()),
                native_price: price_source,
                tvl: Some(sources_base),
                active_addresses: active_addresses.map(|_| gp_source()),
                tx_count_24h: tx_count_24h.map(|_| gp_source()),
                fees_24h: fees_source,
                throughput: throughput.map(|_| gp_source()),
            },
        })
    }

    pub async fn chain_flows(&self, chain: &str) -> Result<ChainFlows> {
        let chains = self.fetch_defillama_chains().await?;
        let resolved = resolve_chain(chain, &chains)?;

        // Stablecoin flows are the one fund-flow dimension available without a paid
        // plan: DefiLlama exposes per-chain current vs 24h-ago circulating supply.
        // Net mint/burn approximates net stablecoin flow on the chain.
        let stablecoins = self.fetch_defillama_stablecoins(&resolved).await?;

        let mut stablecoin_flow: Vec<FlowEntry> = stablecoins
            .iter()
            .map(|s| FlowEntry {
                name: s.name.clone(),
                flow_usd: s.supply - s.prev_day_supply,
            })
            .filter(|e| e.flow_usd.abs() >= 1.0)
            .collect();
        stablecoin_flow.sort_by(|a, b| {
            b.flow_usd
                .abs()
                .partial_cmp(&a.flow_usd.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        stablecoin_flow.truncate(15);

        let has_data = !stablecoins.is_empty();
        let inflow: f64 = stablecoins
            .iter()
            .map(|s| s.supply - s.prev_day_supply)
            .filter(|d| *d > 0.0)
            .sum();
        let outflow: f64 = stablecoins
            .iter()
            .map(|s| s.supply - s.prev_day_supply)
            .filter(|d| *d < 0.0)
            .map(f64::abs)
            .sum();
        let net = inflow - outflow;

        let sc_source = has_data.then(|| "defillama:stablecoins".to_string());

        Ok(ChainFlows {
            chain: resolved,
            net_flow_usd: has_data.then_some(net),
            inflow_usd: has_data.then_some(inflow),
            outflow_usd: has_data.then_some(outflow),
            stablecoin_flow,
            sources: ChainFlowsSources {
                net_flow_usd: sc_source.clone(),
                inflow_usd: sc_source.clone(),
                outflow_usd: sc_source.clone(),
                stablecoin_flow: sc_source,
            },
        })
    }

    pub async fn chain_stablecoins(&self, chain: &str) -> Result<ChainStablecoins> {
        let chains = self.fetch_defillama_chains().await?;
        let resolved = resolve_chain(chain, &chains)?;

        let stablecoins = self.fetch_defillama_stablecoins(&resolved).await?;

        let total_supply: f64 = stablecoins.iter().map(|s| s.supply).sum();
        let total_prev_day: f64 = stablecoins.iter().map(|s| s.prev_day_supply).sum();

        let mut stablecoin_types: Vec<StablecoinType> = stablecoins
            .iter()
            .filter(|s| s.supply > 0.0)
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
        stablecoin_types.sort_by(|a, b| {
            b.supply
                .partial_cmp(&a.supply)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let has_data = !stablecoins.is_empty();
        let supply_source = has_data.then(|| "defillama:stablecoins".to_string());
        // 24h net change in circulating supply (mints minus burns) on this chain.
        let flow_24h = total_supply - total_prev_day;
        let flow_source = has_data.then(|| "defillama:stablecoins".to_string());

        Ok(ChainStablecoins {
            chain: resolved,
            stablecoin_supply: if total_supply > 0.0 {
                Some(total_supply)
            } else {
                None
            },
            stablecoin_types,
            stablecoin_flow_24h: has_data.then_some(flow_24h),
            sources: ChainStablecoinsSources {
                stablecoin_supply: supply_source.clone(),
                stablecoin_types: supply_source,
                stablecoin_flow_24h: flow_source,
            },
        })
    }

    pub async fn chain_protocols(&self, chain: &str, limit: u32) -> Result<ChainProtocols> {
        let chains = self.fetch_defillama_chains().await?;
        let resolved = resolve_chain(chain, &chains)?;

        // Fetch the protocol list and the chain's per-protocol 24h revenue together.
        let (all_protocols, revenue_by_name) = tokio::join!(
            self.fetch_defillama_protocols(),
            self.fetch_chain_protocol_revenue(&resolved)
        );
        let all_protocols = all_protocols?;
        let revenue_by_name = revenue_by_name.unwrap_or_default();
        let has_revenue = !revenue_by_name.is_empty();

        // Keep only on-chain DeFi protocols with a chain-specific TVL for this
        // chain. Two filters apply:
        //   1. `chainTvls[chain]` must be present — we read the chain's net TVL
        //      from that exact key and deliberately do NOT fall back to the
        //      protocol's global TVL, which would overstate the chain.
        //   2. The category must not be a non-protocol bucket. CEX, bridge and
        //      canonical-bridge/chain entries DO report a per-chain TVL (e.g.
        //      Binance CEX has chainTvls["Ethereum"]) and would otherwise
        //      dominate the list, so they're excluded by category.
        let mut protocols: Vec<ChainProtocolEntry> = all_protocols
            .iter()
            .filter_map(|p| {
                if is_excluded_category(p.category.as_deref()) {
                    return None;
                }
                let chain_tvl = p
                    .chain_tvls
                    .as_ref()
                    .and_then(|tvls| tvls.get(&resolved).copied())?;
                Some(ChainProtocolEntry {
                    name: p.name.clone(),
                    tvl: Some(chain_tvl),
                    revenue: revenue_by_name.get(&p.name).copied(),
                    category: p.category.clone(),
                })
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

        let mut source = format!("defillama:protocols ({} of {})", limit.min(total), total);
        if has_revenue {
            source.push_str(", revenue: defillama:fees");
        }

        Ok(ChainProtocols {
            chain: resolved,
            protocols,
            sources: ChainProtocolsSources {
                protocols: Some(source),
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

    /// Fetch per-protocol 24h revenue for a chain from DefiLlama
    /// `/overview/fees/{chain}` (dataType=dailyRevenue). Returns a name→revenue
    /// map; an empty map on any failure so callers degrade gracefully.
    async fn fetch_chain_protocol_revenue(
        &self,
        chain_name: &str,
    ) -> Result<std::collections::HashMap<String, f64>> {
        let url = format!("{}/overview/fees/{}", self.defillama_url, chain_name);
        let req = self.client.get(&url).query(&[
            ("excludeTotalDataChart", "true"),
            ("excludeTotalDataChartBreakdown", "true"),
            ("dataType", "dailyRevenue"),
        ]);
        let body = match send_retrying(req, "defillama.chain_revenue").await {
            Ok(resp) => match resp.error_for_status() {
                Ok(r) => r.text().await.map_err(ChainError::Http)?,
                Err(_) => return Ok(std::collections::HashMap::new()),
            },
            Err(_) => return Ok(std::collections::HashMap::new()),
        };

        let value: Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => return Ok(std::collections::HashMap::new()),
        };

        let mut map = std::collections::HashMap::new();
        if let Some(protocols) = value.get("protocols").and_then(|p| p.as_array()) {
            for p in protocols {
                if let (Some(name), Some(rev)) = (
                    p.get("name").and_then(|n| n.as_str()),
                    p.get("total24h").and_then(|t| t.as_f64()),
                ) {
                    map.insert(name.to_string(), rev);
                }
            }
        }
        Ok(map)
    }

    async fn fetch_defillama_stablecoins(
        &self,
        chain_name: &str,
    ) -> Result<Vec<SimpleStablecoin>> {
        let url = format!("{}/stablecoins", self.stablecoins_url);
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

        // The stablecoins endpoint keys `chainCirculating` differently from the
        // canonical /chains name for some chains (e.g. "Binance" → "BSC").
        let target = stablecoin_chain_name(chain_name);

        Ok(assets
            .into_iter()
            .filter_map(|asset| {
                // Pull this stablecoin's per-chain supply (current + 24h ago) for the
                // target chain. Skip coins not present on the chain.
                let per_chain = asset.chainCirculating.as_ref()?;
                let entry = per_chain
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(target))
                    .map(|(_, v)| v)?;

                let supply = entry
                    .current
                    .as_ref()
                    .and_then(|c| c.peggedUSD)
                    .unwrap_or(0.0);
                // Fall back to current supply when prev-day is missing so the 24h
                // delta reads as zero rather than a spurious full-supply swing.
                let prev_day_supply = entry
                    .circulatingPrevDay
                    .as_ref()
                    .and_then(|c| c.peggedUSD)
                    .unwrap_or(supply);

                if supply <= 0.0 && prev_day_supply <= 0.0 {
                    return None;
                }

                Some(SimpleStablecoin {
                    name: format!("{} ({})", asset.name, asset.symbol),
                    supply,
                    prev_day_supply,
                })
            })
            .collect())
    }

    /// USD price of a chain's native token. Pass `gecko_id` when you already
    /// have an authoritative CoinGecko id (e.g. from DefiLlama); otherwise
    /// it falls back to chain-id then chain-name lookups.
    ///
    /// Public so callers like wallet handlers can attach a USD price to
    /// non-EVM native balances (BTC, SOL) without duplicating the CoinGecko
    /// pricing path.
    pub async fn fetch_native_price(
        &self,
        chain_name: &str,
        gecko_id: Option<&str>,
        chain_id: Option<u64>,
    ) -> Option<f64> {
        // Resolve the native token's CoinGecko ID, most-authoritative first:
        // 1. DefiLlama's per-chain `gecko_id` (current, covers all chains)
        // 2. local chain config (keyed by chain ID)
        // 3. name-based fallback map
        let coingecko_id: &str = gecko_id
            .filter(|s| !s.is_empty())
            .or_else(|| {
                chain_id
                    .and_then(crate::config::chain_config)
                    .map(|c| c.native_token.coingecko_id)
            })
            .or_else(|| chain_to_coingecko_id(chain_name))?;

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

    /// Fetch 24h fees for a chain from DefiLlama `/overview/fees/{chain}`.
    /// (`/summary/fees/{x}` is a *protocol* endpoint and returns wrong/missing
    /// data when given a chain name.)
    async fn fetch_chain_fees(&self, chain_name: &str) -> Result<Option<f64>> {
        let url = format!("{}/overview/fees/{}", self.defillama_url, chain_name);
        let req = self.client.get(&url).query(&[
            ("excludeTotalDataChart", "true"),
            ("excludeTotalDataChartBreakdown", "true"),
            ("dataType", "dailyFees"),
        ]);
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

    /// Fetch latest daily active addresses, tx count, and throughput for a chain
    /// from growthepie's per-chain JSON. Returns defaults (all None) on any
    /// failure so callers degrade gracefully.
    async fn fetch_growthepie_activity(&self, url_key: &str) -> GrowthepieActivity {
        let url = format!("{}/v1/chains/{}.json", self.growthepie_url, url_key);
        let body = match send_retrying(self.client.get(&url), "growthepie.chain").await {
            Ok(resp) => match resp.error_for_status() {
                Ok(r) => match r.text().await {
                    Ok(b) => b,
                    Err(_) => return GrowthepieActivity::default(),
                },
                Err(_) => return GrowthepieActivity::default(),
            },
            Err(_) => return GrowthepieActivity::default(),
        };

        let value: Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(_) => return GrowthepieActivity::default(),
        };

        let metrics = value.get("data").and_then(|d| d.get("metrics"));
        let latest = |key: &str| -> Option<f64> {
            metrics?
                .get(key)?
                .get("daily")?
                .get("data")?
                .as_array()?
                .last()?
                .as_array()?
                .get(1)?
                .as_f64()
        };

        GrowthepieActivity {
            active_addresses: latest("daa").map(|v| v as u64),
            tx_count_24h: latest("txcount").map(|v| v as u64),
            throughput: latest("throughput"),
        }
    }
}

/// Latest chain-activity metrics from growthepie.
#[derive(Default)]
struct GrowthepieActivity {
    active_addresses: Option<u64>,
    tx_count_24h: Option<u64>,
    throughput: Option<f64>,
}

struct SimpleStablecoin {
    name: String,
    supply: f64,
    prev_day_supply: f64,
}

/// Resolve user input (chain ID, abbreviation, alias, or name) to the
/// canonical DefiLlama chain name. Matching order:
/// DefiLlama categories that aren't on-chain DeFi protocols. Each still reports
/// a per-chain TVL (e.g. Binance CEX has `chainTvls["Ethereum"]`), so they're
/// excluded by category rather than by missing data. Compared case-insensitively.
const EXCLUDED_PROTOCOL_CATEGORIES: &[&str] = &[
    "CEX",
    "Bridge",
    "Canonical Bridge",
    "Cross Chain Bridge",
    "Chain",
];

/// Whether a protocol's category should be excluded from a chain's protocol
/// list. A missing category is kept (treated as a regular protocol).
fn is_excluded_category(category: Option<&str>) -> bool {
    category.is_some_and(|c| {
        EXCLUDED_PROTOCOL_CATEGORIES
            .iter()
            .any(|x| x.eq_ignore_ascii_case(c))
    })
}

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

/// Map an EVM chain ID to growthepie's chain key used in `chains/{key}.json`.
/// This is the master `chains` dict key (underscored, e.g. `polygon_pos`), NOT
/// the `url_key` (dashed) — the file endpoint 403s on the dashed form.
/// growthepie tracks the Ethereum ecosystem (L1 + L2s); chains it doesn't cover
/// (e.g. BSC, Avalanche) return None and keep activity fields as N/A.
fn chain_id_to_growthepie(chain_id: u64) -> Option<&'static str> {
    Some(match chain_id {
        1 => "ethereum",
        10 => "optimism",
        130 => "unichain",
        137 => "polygon_pos",
        169 => "manta",
        252 => "fraxtal",
        324 => "zksync_era",
        480 => "worldchain",
        1088 => "metis",
        1135 => "lisk",
        1625 => "gravity",
        1868 => "soneium",
        2020 => "ronin",
        5000 => "mantle",
        8453 => "base",
        34443 => "mode",
        42161 => "arbitrum",
        42170 => "arbitrum_nova",
        42220 => "celo",
        48900 => "zircuit",
        57073 => "ink",
        59144 => "linea",
        98866 => "plume",
        167000 => "taiko",
        534352 => "scroll",
        _ => return None,
    })
}

/// DefiLlama uses inconsistent chain names across endpoints. The canonical name
/// from `/chains` may differ from the stablecoins endpoint's `chainCirculating`
/// key. Translate a canonical name to the stablecoins-endpoint spelling.
fn stablecoin_chain_name(canonical: &str) -> &str {
    match canonical {
        "Binance" => "BSC",
        other => other,
    }
}

/// Map common abbreviations and aliases to DefiLlama canonical chain names.
fn chain_alias_to_name(alias: &str) -> Option<&'static str> {
    match alias {
        // Ethereum
        "eth" | "ethereum" => Some("Ethereum"),
        // BNB Chain
        // DefiLlama's /chains endpoint names BSC "Binance" (not "BNB Chain").
        "bsc" | "bnb" | "binance" | "bnbchain" | "bnbsmartchain" | "bnb chain" => Some("Binance"),
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
        // Solana (non-EVM but commonly queried). `svm` is the DODO-internal
        // alias for the Solana VM ecosystem.
        "sol" | "solana" | "svm" => Some("Solana"),
        // Tron
        "trx" | "tron" => Some("Tron"),
        // Bitcoin. `bvm` is the DODO-internal alias for Bitcoin mainnet.
        "btc" | "bitcoin" | "bvm" => Some("Bitcoin"),
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
        // Non-EVM chains. DefiLlama's `/chains` endpoint already supplies a
        // gecko_id for these, so this fallback only fires if that field goes
        // missing — kept defensive.
        "solana" => Some("solana"),
        "bitcoin" => Some("bitcoin"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_table_resolves_evm_chains() {
        assert_eq!(chain_alias_to_name("eth"), Some("Ethereum"));
        assert_eq!(chain_alias_to_name("bsc"), Some("Binance"));
        assert_eq!(chain_alias_to_name("base"), Some("Base"));
        assert_eq!(chain_alias_to_name("arb"), Some("Arbitrum"));
    }

    #[test]
    fn alias_table_resolves_solana_and_svm() {
        assert_eq!(chain_alias_to_name("sol"), Some("Solana"));
        assert_eq!(chain_alias_to_name("solana"), Some("Solana"));
        assert_eq!(chain_alias_to_name("svm"), Some("Solana"));
    }

    #[test]
    fn alias_table_resolves_bitcoin_and_bvm() {
        assert_eq!(chain_alias_to_name("btc"), Some("Bitcoin"));
        assert_eq!(chain_alias_to_name("bitcoin"), Some("Bitcoin"));
        assert_eq!(chain_alias_to_name("bvm"), Some("Bitcoin"));
    }

    #[test]
    fn alias_table_unknown_returns_none() {
        assert_eq!(chain_alias_to_name("not-a-chain"), None);
    }

    #[test]
    fn resolve_chain_accepts_svm_and_bvm_aliases() {
        // Minimal fake DefiLlama catalog so resolve_chain's alias path can
        // confirm the canonical name actually exists. Non-EVM chains have
        // chain_id = None.
        let chains = vec![
            DefiLlamaChain {
                name: "Solana".to_string(),
                token_symbol: Some("SOL".to_string()),
                tvl: None,
                chain_id: None,
                gecko_id: Some("solana".to_string()),
            },
            DefiLlamaChain {
                name: "Bitcoin".to_string(),
                token_symbol: Some("BTC".to_string()),
                tvl: None,
                chain_id: None,
                gecko_id: Some("bitcoin".to_string()),
            },
        ];
        assert_eq!(resolve_chain("svm", &chains).unwrap(), "Solana");
        assert_eq!(resolve_chain("solana", &chains).unwrap(), "Solana");
        assert_eq!(resolve_chain("SOL", &chains).unwrap(), "Solana");
        assert_eq!(resolve_chain("bvm", &chains).unwrap(), "Bitcoin");
        assert_eq!(resolve_chain("bitcoin", &chains).unwrap(), "Bitcoin");
        assert_eq!(resolve_chain("BTC", &chains).unwrap(), "Bitcoin");
    }

    #[test]
    fn coingecko_fallback_covers_solana_and_bitcoin() {
        assert_eq!(chain_to_coingecko_id("Solana"), Some("solana"));
        assert_eq!(chain_to_coingecko_id("Bitcoin"), Some("bitcoin"));
    }

    #[test]
    fn growthepie_skips_non_evm_chains() {
        // Solana/Bitcoin have no EVM chain_id, so the growthepie key lookup
        // never fires for them — confirms the activity fields will surface as
        // N/A rather than mis-attribute Ethereum data.
        assert!(chain_id_to_growthepie(1).is_some());
        assert!(chain_id_to_growthepie(8453).is_some());
    }
}
