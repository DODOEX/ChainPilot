use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

use crate::api::send_retrying;
use crate::error::{ChainError, Result};
use crate::models::protocol::{
    ProtocolChainMetricSources, ProtocolChainMetrics, ProtocolChains, ProtocolChainsSources,
    ProtocolInfo, ProtocolInfoSources, ProtocolRevenue, ProtocolRevenueSources, ProtocolTvl,
    ProtocolTvlPoint, ProtocolTvlSources,
};

#[derive(Clone)]
pub struct DefillamaClient {
    client: Client,
    base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolCatalogItem {
    name: String,
    url: Option<String>,
    description: Option<String>,
    chain: Option<String>,
    category: Option<String>,
    chains: Option<Vec<String>>,
    slug: String,
    tvl: Option<f64>,
    chain_tvls: Option<HashMap<String, f64>>,
    #[serde(rename = "change_1d")]
    change_1d: Option<f64>,
    #[serde(rename = "change_7d")]
    change_7d: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolDetail {
    name: String,
    url: Option<String>,
    description: Option<String>,
    chain: Option<String>,
    category: Option<String>,
    chains: Option<Vec<String>>,
    linked_protocols: Option<Vec<String>>,
    child_protocols: Option<Vec<ProtocolChild>>,
    other_protocols: Option<Vec<String>>,
    tvl: Option<Vec<ProtocolDetailTvlPoint>>,
    current_chain_tvls: Option<HashMap<String, f64>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolChild {
    name: String,
    display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProtocolDetailTvlPoint {
    date: i64,
    #[serde(rename = "totalLiquidityUSD")]
    total_liquidity_usd: Option<f64>,
}

struct ProtocolBundle {
    catalog: ProtocolCatalogItem,
    detail: ProtocolDetail,
    protocols: Vec<ProtocolCatalogItem>,
}

impl DefillamaClient {
    pub fn new(client: Client, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn info(&self, protocol: &str) -> Result<ProtocolInfo> {
        let bundle = self.protocol_bundle(protocol).await?;
        let revenue = self
            .optional_dimension_summary(&bundle.catalog.slug, "dailyRevenue")
            .await;
        let fees = self
            .optional_dimension_summary(&bundle.catalog.slug, "dailyFees")
            .await;
        let tvl = current_tvl(&bundle.detail).or(bundle.catalog.tvl);
        let (category, category_source) = protocol_category(&bundle);
        let (chain, chain_source) = protocol_chain(&bundle);
        let website = bundle.detail.url.or(bundle.catalog.url);
        let description = bundle.detail.description.or(bundle.catalog.description);
        let revenue_24h = revenue.as_ref().and_then(|v| total_metric(v, "total24h"));
        let fee_24h = fees.as_ref().and_then(|v| total_metric(v, "total24h"));

        Ok(ProtocolInfo {
            name: bundle.detail.name,
            sources: ProtocolInfoSources {
                name: Some("defillama:protocol".to_string()),
                category: category_source,
                chain: chain_source,
                website: source_if_some(&website, "defillama:protocol"),
                description: source_if_some(&description, "defillama:protocol"),
                tvl: source_if_some(&tvl, "defillama:tvl"),
                revenue: source_if_some(&revenue_24h, "defillama:fees:dailyRevenue"),
                fee: source_if_some(&fee_24h, "defillama:fees:dailyFees"),
            },
            category,
            chain,
            website,
            description,
            tvl,
            revenue: revenue_24h,
            fee: fee_24h,
        })
    }

    pub async fn tvl(&self, protocol: &str, limit: u32, offset: u32) -> Result<ProtocolTvl> {
        let bundle = self.protocol_bundle(protocol).await?;
        let current_tvl = current_tvl(&bundle.detail).or(bundle.catalog.tvl);
        let tvl_points = bundle.detail.tvl.as_deref().unwrap_or_default();
        let tvl_change_24h = bundle
            .catalog
            .change_1d
            .or_else(|| tvl_change_days(tvl_points, 1));
        let tvl_change_7d = bundle
            .catalog
            .change_7d
            .or_else(|| tvl_change_days(tvl_points, 7));
        let tvl_change_30d = tvl_change_days(bundle.detail.tvl.as_deref().unwrap_or_default(), 30);
        let tvl_history = bundle
            .detail
            .tvl
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|p| {
                p.total_liquidity_usd
                    .map(|tvl| ProtocolTvlPoint { date: p.date, tvl })
            })
            .collect::<Vec<_>>();
        let tvl_history_total = tvl_history.len();
        let tvl_history_limit = (limit as usize).clamp(1, 1000);
        let tvl_history_offset = offset as usize;
        let tvl_history = paginate_history(tvl_history, tvl_history_limit, tvl_history_offset);
        let tvl_history_source =
            (!tvl_history.is_empty()).then(|| "defillama:tvl_history".to_string());

        Ok(ProtocolTvl {
            protocol: bundle.detail.name,
            current_tvl,
            tvl_change_24h,
            tvl_change_7d,
            tvl_change_30d,
            tvl_history_total,
            tvl_history_limit,
            tvl_history_offset,
            tvl_history,
            sources: ProtocolTvlSources {
                current_tvl: source_if_some(&current_tvl, "defillama:tvl"),
                tvl_change_24h: source_if_some(
                    &tvl_change_24h,
                    if bundle.catalog.change_1d.is_some() {
                        "defillama:protocols:change_1d"
                    } else {
                        "defillama:tvl_history:calculated"
                    },
                ),
                tvl_change_7d: source_if_some(
                    &tvl_change_7d,
                    if bundle.catalog.change_7d.is_some() {
                        "defillama:protocols:change_7d"
                    } else {
                        "defillama:tvl_history:calculated"
                    },
                ),
                tvl_change_30d: source_if_some(&tvl_change_30d, "defillama:tvl_history:calculated"),
                tvl_history: tvl_history_source,
            },
        })
    }

    pub async fn revenue(&self, protocol: &str) -> Result<ProtocolRevenue> {
        let bundle = self.protocol_bundle(protocol).await?;
        let revenue = self
            .dimension_summary(&bundle.catalog.slug, "dailyRevenue")
            .await?;
        let fees = self
            .dimension_summary(&bundle.catalog.slug, "dailyFees")
            .await?;

        let revenue_24h = total_metric(&revenue, "total24h");
        let revenue_7d = total_metric(&revenue, "total7d");
        let revenue_30d = total_metric(&revenue, "total30d");
        let fees_24h = total_metric(&fees, "total24h");
        let fees_7d = total_metric(&fees, "total7d");

        Ok(ProtocolRevenue {
            protocol: bundle.detail.name,
            revenue_24h,
            revenue_7d,
            revenue_30d,
            fees_24h,
            fees_7d,
            sources: ProtocolRevenueSources {
                revenue_24h: source_if_some(&revenue_24h, "defillama:fees:dailyRevenue"),
                revenue_7d: source_if_some(&revenue_7d, "defillama:fees:dailyRevenue"),
                revenue_30d: source_if_some(&revenue_30d, "defillama:fees:dailyRevenue"),
                fees_24h: source_if_some(&fees_24h, "defillama:fees:dailyFees"),
                fees_7d: source_if_some(&fees_7d, "defillama:fees:dailyFees"),
            },
        })
    }

    pub async fn chains(&self, protocol: &str) -> Result<ProtocolChains> {
        let bundle = self.protocol_bundle(protocol).await?;
        let revenue = self
            .optional_dimension_summary(&bundle.catalog.slug, "dailyRevenue")
            .await;
        let chain_tvls = bundle
            .detail
            .current_chain_tvls
            .as_ref()
            .or(bundle.catalog.chain_tvls.as_ref());
        let mut chains_source = chain_tvls
            .is_some()
            .then(|| "defillama:currentChainTvls".to_string());

        let mut chains: Vec<ProtocolChainMetrics> = chain_tvls
            .into_iter()
            .flat_map(|m| m.iter())
            .filter(|(chain, _)| {
                !chain.contains('-') && chain.as_str() != "staking" && chain.as_str() != "pool2"
            })
            .map(|(chain, tvl)| {
                let chain_revenue = revenue
                    .as_ref()
                    .and_then(|v| chain_metric(v, chain, "total24h"));
                ProtocolChainMetrics {
                    chain: chain.clone(),
                    tvl: Some(*tvl),
                    revenue: chain_revenue,
                    sources: ProtocolChainMetricSources {
                        tvl: Some("defillama:currentChainTvls".to_string()),
                        revenue: source_if_some(
                            &chain_revenue,
                            "defillama:fees:dailyRevenue:chainBreakdown",
                        ),
                    },
                }
            })
            .collect();

        if chains.is_empty() {
            let (fallback_chains, fallback_source) =
                match (bundle.detail.chains, bundle.catalog.chains) {
                    (Some(chains), _) => (chains, Some("defillama:protocol:chains".to_string())),
                    (None, Some(chains)) => {
                        (chains, Some("defillama:protocols:chains".to_string()))
                    }
                    (None, None) => (Vec::new(), None),
                };
            chains_source = fallback_source;
            chains = fallback_chains
                .into_iter()
                .map(|chain| {
                    let chain_revenue = revenue
                        .as_ref()
                        .and_then(|v| chain_metric(v, &chain, "total24h"));
                    ProtocolChainMetrics {
                        tvl: None,
                        revenue: chain_revenue,
                        sources: ProtocolChainMetricSources {
                            tvl: None,
                            revenue: source_if_some(
                                &chain_revenue,
                                "defillama:fees:dailyRevenue:chainBreakdown",
                            ),
                        },
                        chain,
                    }
                })
                .collect();
        }

        chains.sort_by(|a, b| {
            b.tvl
                .unwrap_or(0.0)
                .partial_cmp(&a.tvl.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(ProtocolChains {
            protocol: bundle.detail.name,
            chains,
            sources: ProtocolChainsSources {
                chains: chains_source,
            },
        })
    }

    async fn protocol_bundle(&self, protocol: &str) -> Result<ProtocolBundle> {
        let protocols = self.protocols().await?;
        let catalog = resolve_protocol_from(protocol, &protocols);
        let detail = match &catalog {
            Some(catalog) => self.protocol_detail(&catalog.slug).await?,
            None => self.protocol_detail(&slugify(protocol)).await?,
        };
        let catalog = catalog.unwrap_or_else(|| catalog_from_detail(protocol, &detail));
        Ok(ProtocolBundle {
            catalog,
            detail,
            protocols,
        })
    }

    async fn protocols(&self) -> Result<Vec<ProtocolCatalogItem>> {
        let url = format!("{}/protocols", self.base_url);
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

    async fn protocol_detail(&self, slug: &str) -> Result<ProtocolDetail> {
        let url = format!("{}/protocol/{}", self.base_url, slug);
        let req = self.client.get(&url);
        let body = send_retrying(req, "defillama.protocol")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "DefiLlama protocol response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })
    }

    async fn dimension_summary(&self, slug: &str, data_type: &str) -> Result<Value> {
        let url = format!("{}/summary/fees/{}", self.base_url, slug);
        let req = self.client.get(&url).query(&[("dataType", data_type)]);
        let body = send_retrying(req, "defillama.fees")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "DefiLlama fees response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })
    }

    async fn optional_dimension_summary(&self, slug: &str, data_type: &str) -> Option<Value> {
        match self.dimension_summary(slug, data_type).await {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::warn!(
                    slug = slug,
                    data_type = data_type,
                    error = %err,
                    "DefiLlama dimension summary unavailable"
                );
                None
            }
        }
    }
}

fn current_tvl(detail: &ProtocolDetail) -> Option<f64> {
    detail
        .tvl
        .as_deref()
        .and_then(|points| points.iter().rev().find_map(|p| p.total_liquidity_usd))
        .or_else(|| {
            detail
                .current_chain_tvls
                .as_ref()
                .map(|chains| chains.values().sum())
        })
}

fn catalog_from_detail(protocol: &str, detail: &ProtocolDetail) -> ProtocolCatalogItem {
    ProtocolCatalogItem {
        name: detail.name.clone(),
        url: detail.url.clone(),
        description: detail.description.clone(),
        chain: detail.chain.clone(),
        category: detail.category.clone(),
        chains: detail.chains.clone(),
        slug: slugify(protocol),
        tvl: current_tvl(detail),
        chain_tvls: detail.current_chain_tvls.clone(),
        change_1d: None,
        change_7d: None,
    }
}

fn resolve_protocol_from(
    protocol: &str,
    items: &[ProtocolCatalogItem],
) -> Option<ProtocolCatalogItem> {
    let needle = protocol.trim();
    let needle_slug = slugify(needle);

    items
        .iter()
        .find(|item| item.slug.eq_ignore_ascii_case(needle))
        .or_else(|| {
            items
                .iter()
                .find(|item| item.name.eq_ignore_ascii_case(needle))
        })
        .or_else(|| {
            items
                .iter()
                .find(|item| item.slug.eq_ignore_ascii_case(&needle_slug))
        })
        .cloned()
}

fn protocol_category(bundle: &ProtocolBundle) -> (Option<String>, Option<String>) {
    if let Some(category) = bundle
        .detail
        .category
        .clone()
        .or(bundle.catalog.category.clone())
    {
        return (Some(category), Some("defillama:protocol".to_string()));
    }

    let related = related_protocol_names(&bundle.detail);
    let categories: Vec<String> = bundle
        .protocols
        .iter()
        .filter(|item| {
            related.iter().any(|name| {
                item.name.eq_ignore_ascii_case(name)
                    || item.slug.eq_ignore_ascii_case(&slugify(name))
            })
        })
        .filter_map(|item| item.category.clone())
        .collect();

    most_common(categories).map_or((None, None), |category| {
        (
            Some(category),
            Some("defillama:protocols:related_protocols".to_string()),
        )
    })
}

fn protocol_chain(bundle: &ProtocolBundle) -> (Option<String>, Option<String>) {
    if let Some(chain) = bundle.detail.chain.clone().or(bundle.catalog.chain.clone()) {
        return (Some(chain), Some("defillama:protocol".to_string()));
    }

    let chain_tvls = bundle
        .detail
        .current_chain_tvls
        .as_ref()
        .or(bundle.catalog.chain_tvls.as_ref());

    if let Some(chain) = chain_from_tvls(chain_tvls) {
        return (
            Some(chain),
            Some("defillama:currentChainTvls:top_chain".to_string()),
        );
    }

    if let Some(chain) = primary_chain(
        bundle
            .detail
            .chains
            .as_ref()
            .or(bundle.catalog.chains.as_ref()),
    ) {
        return (Some(chain), Some("defillama:protocol".to_string()));
    }

    (None, None)
}

fn related_protocol_names(detail: &ProtocolDetail) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(linked) = &detail.linked_protocols {
        names.extend(linked.iter().cloned());
    }
    if let Some(other) = &detail.other_protocols {
        names.extend(other.iter().cloned());
    }
    if let Some(children) = &detail.child_protocols {
        for child in children {
            names.push(child.name.clone());
            if let Some(display_name) = &child.display_name {
                names.push(display_name.clone());
            }
        }
    }
    names
}

fn chain_from_tvls(chain_tvls: Option<&HashMap<String, f64>>) -> Option<String> {
    chain_tvls?
        .iter()
        .filter(|(chain, tvl)| {
            **tvl > 0.0
                && !chain.contains('-')
                && chain.as_str() != "staking"
                && chain.as_str() != "pool2"
        })
        .max_by(|(left_chain, left_tvl), (right_chain, right_tvl)| {
            left_tvl
                .partial_cmp(right_tvl)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right_chain.cmp(left_chain))
        })
        .map(|(chain, _)| chain.clone())
}

fn most_common(values: Vec<String>) -> Option<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|(left_value, left_count), (right_value, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| right_value.cmp(left_value))
        })
        .map(|(value, _)| value)
}

fn tvl_change_days(points: &[ProtocolDetailTvlPoint], days: usize) -> Option<f64> {
    let current = points.iter().rev().find_map(|p| p.total_liquidity_usd)?;
    let previous = points
        .iter()
        .rev()
        .filter_map(|p| p.total_liquidity_usd)
        .nth(days)?;
    if previous == 0.0 {
        None
    } else {
        Some(((current - previous) / previous) * 100.0)
    }
}

fn paginate_history(
    history: Vec<ProtocolTvlPoint>,
    limit: usize,
    offset: usize,
) -> Vec<ProtocolTvlPoint> {
    let len = history.len();
    if offset >= len {
        return Vec::new();
    }
    let end = len - offset;
    let start = end.saturating_sub(limit);
    history[start..end].to_vec()
}

fn primary_chain(chains: Option<&Vec<String>>) -> Option<String> {
    let chains = chains?;
    match chains.as_slice() {
        [] => None,
        [one] => Some(one.clone()),
        _ => Some("Multi-Chain".to_string()),
    }
}

fn total_metric(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn chain_metric(value: &Value, chain: &str, key: &str) -> Option<f64> {
    value
        .get("chainBreakdown")
        .and_then(|v| v.get(chain))
        .and_then(|v| nested_number(v, key))
}

fn source_if_some<T>(value: &Option<T>, source: &str) -> Option<String> {
    value.as_ref().map(|_| source.to_string())
}

fn nested_number(value: &Value, key: &str) -> Option<f64> {
    match value {
        Value::Object(map) => {
            if let Some(number) = map.get(key).and_then(Value::as_f64) {
                return Some(number);
            }
            map.values().find_map(|v| nested_number(v, key))
        }
        _ => None,
    }
}

fn slugify(input: &str) -> String {
    input
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
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
    fn slugify_protocol_name() {
        assert_eq!(slugify("Uniswap V3"), "uniswap-v3");
        assert_eq!(slugify("  Aave / V2 "), "aave-v2");
    }

    #[test]
    fn nested_number_finds_chain_metrics() {
        let value = serde_json::json!({
            "chainBreakdown": {
                "Ethereum": {
                    "Uniswap V3": { "total24h": 12.5 }
                }
            }
        });

        assert_eq!(chain_metric(&value, "Ethereum", "total24h"), Some(12.5));
    }

    #[test]
    fn tvl_change_days_returns_percent_change() {
        let points = vec![
            ProtocolDetailTvlPoint {
                date: 1,
                total_liquidity_usd: Some(100.0),
            },
            ProtocolDetailTvlPoint {
                date: 2,
                total_liquidity_usd: Some(150.0),
            },
        ];

        assert_eq!(tvl_change_days(&points, 1), Some(50.0));
    }

    #[test]
    fn chain_from_tvls_returns_largest_positive_chain() {
        let tvls = HashMap::from([
            ("Ethereum".to_string(), 100.0),
            ("Base".to_string(), 50.0),
            ("staking".to_string(), 200.0),
            ("Ethereum-staking".to_string(), 200.0),
        ]);

        assert_eq!(chain_from_tvls(Some(&tvls)), Some("Ethereum".to_string()));
    }

    #[test]
    fn chain_from_tvls_returns_single_positive_chain() {
        let tvls = HashMap::from([("Ethereum".to_string(), 100.0), ("Base".to_string(), 0.0)]);

        assert_eq!(chain_from_tvls(Some(&tvls)), Some("Ethereum".to_string()));
    }

    #[test]
    fn most_common_returns_dominant_category() {
        let category = most_common(vec![
            "Dexs".to_string(),
            "Dexs".to_string(),
            "Lending".to_string(),
        ]);

        assert_eq!(category.as_deref(), Some("Dexs"));
    }

    #[test]
    fn paginate_history_returns_latest_page_in_chronological_order() {
        let history = (1..=5)
            .map(|date| ProtocolTvlPoint {
                date,
                tvl: date as f64,
            })
            .collect();

        let page = paginate_history(history, 2, 1);

        assert_eq!(page.iter().map(|p| p.date).collect::<Vec<_>>(), vec![3, 4]);
    }

    #[test]
    fn paginate_history_returns_empty_when_offset_exceeds_history() {
        let history = vec![ProtocolTvlPoint { date: 1, tvl: 1.0 }];

        assert!(paginate_history(history, 10, 10).is_empty());
    }
}
