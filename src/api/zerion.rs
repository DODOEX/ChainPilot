use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::api::send_retrying;
use crate::error::{ChainError, Result};

#[derive(Clone)]
pub struct ZerionClient {
    client: Client,
    base_url: String,
    api_key: String,
}

// ── portfolio ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PortfolioResponse {
    data: Option<PortfolioData>,
}

#[derive(Debug, Deserialize)]
struct PortfolioData {
    attributes: Option<PortfolioAttributes>,
}

#[derive(Debug, Deserialize)]
struct PortfolioAttributes {
    #[serde(default)]
    positions_distribution_by_chain: std::collections::HashMap<String, f64>,
    total: Option<PortfolioTotal>,
}

#[derive(Debug, Deserialize)]
struct PortfolioTotal {
    positions: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ZerionPortfolio {
    pub total_usd: Option<f64>,
    pub chains: Vec<ZerionChainBalance>,
}

#[derive(Debug, Clone)]
pub struct ZerionChainBalance {
    pub slug: String,
    pub chain_id: Option<u64>,
    pub usd_value: f64,
}

// ── positions ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PositionsResponse {
    #[serde(default)]
    data: Vec<PositionItem>,
}

#[derive(Debug, Deserialize)]
struct PositionItem {
    attributes: Option<PositionAttributes>,
    relationships: Option<PositionRelationships>,
}

#[derive(Debug, Deserialize)]
struct PositionAttributes {
    name: Option<String>,
    quantity: Option<PositionQuantity>,
    value: Option<f64>,
    price: Option<f64>,
    fungible_info: Option<FungibleInfo>,
    flags: Option<PositionFlags>,
    position_type: Option<String>,
    /// Zerion sometimes returns this as a plain string slug, sometimes as
    /// an object (`{id, name, url, ...}`). Park the raw JSON and extract
    /// downstream so unexpected shapes don't blow up the whole response.
    #[serde(default)]
    protocol: Option<serde_json::Value>,
    application_metadata: Option<ApplicationMetadata>,
}

/// Best-effort string extraction from Zerion's `protocol` field, which may be
/// a string, an object with `name`/`id`, or null.
fn extract_protocol_name(value: Option<&serde_json::Value>) -> Option<String> {
    let v = value?;
    if let Some(s) = v.as_str() {
        if !s.is_empty() {
            return Some(s.to_string());
        }
        return None;
    }
    if let Some(obj) = v.as_object() {
        for key in ["name", "id", "title"] {
            if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct PositionQuantity {
    float: Option<f64>,
    numeric: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FungibleInfo {
    name: Option<String>,
    symbol: Option<String>,
    #[serde(default)]
    implementations: Vec<FungibleImplementation>,
}

#[derive(Debug, Deserialize)]
struct FungibleImplementation {
    chain_id: Option<String>,
    address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PositionFlags {
    displayable: Option<bool>,
    is_trash: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ApplicationMetadata {
    url: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PositionRelationships {
    chain: Option<RelationshipRef>,
}

#[derive(Debug, Deserialize)]
struct RelationshipRef {
    data: Option<RelationshipData>,
}

#[derive(Debug, Deserialize)]
struct RelationshipData {
    id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ZerionPositionRecord {
    pub chain_slug: String,
    pub chain_id: Option<u64>,
    pub symbol: String,
    pub name: String,
    /// Zerion's `attributes.name` verbatim — for DeFi positions this is the
    /// position label (e.g. "Aave V3 USDC Deposit"), useful as a fallback
    /// bucket label when `protocol` is missing.
    pub display_name: Option<String>,
    pub address: String,
    pub amount: f64,
    pub price_usd: Option<f64>,
    pub value_usd: Option<f64>,
    pub position_type: String,
    pub protocol: Option<String>,
    pub protocol_url: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────

impl ZerionClient {
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

    pub async fn portfolio(&self, address: &str) -> Result<ZerionPortfolio> {
        self.require_key()?;
        let url = format!("{}/wallets/{}/portfolio/", self.base_url, address);

        let req = self
            .client
            .get(&url)
            .basic_auth(&self.api_key, Some(""))
            .header("accept", "application/json")
            .timeout(Duration::from_secs(15))
            .query(&[("currency", "usd")]);
        let body = send_retrying(req, "zerion.portfolio")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let resp: PortfolioResponse = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "Zerion portfolio response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })?;

        let attrs = resp
            .data
            .and_then(|d| d.attributes)
            .ok_or_else(|| ChainError::Config("Zerion portfolio response was empty".to_string()))?;

        let total_usd = attrs.total.and_then(|t| t.positions);
        let chains: Vec<ZerionChainBalance> = attrs
            .positions_distribution_by_chain
            .into_iter()
            .filter(|(_, v)| *v > 0.0)
            .map(|(slug, usd_value)| ZerionChainBalance {
                chain_id: zerion_chain_to_id(&slug),
                slug,
                usd_value,
            })
            .collect();

        // Surface the raw slugs Zerion returned so a user hitting an unknown
        // chain can spot the mismatch in debug logs without having to log the
        // full ~100 KB portfolio body.
        for c in &chains {
            if c.chain_id.is_none() {
                tracing::warn!(
                    target: "zerion",
                    slug = %c.slug,
                    usd_value = c.usd_value,
                    "zerion chain slug not mapped to EVM id — extend zerion_chain_to_id"
                );
            } else {
                tracing::debug!(
                    target: "zerion",
                    slug = %c.slug,
                    chain_id = ?c.chain_id,
                    usd_value = c.usd_value,
                    "zerion chain mapped",
                );
            }
        }

        Ok(ZerionPortfolio { total_usd, chains })
    }

    /// Fetch wallet positions. When `only_simple` is true, Zerion returns only
    /// plain token balances (no DeFi positions) — that maps to our `assets[]`
    /// notion. When false, DeFi positions are included, which `overview` uses
    /// to populate `active_protocols`.
    pub async fn positions(
        &self,
        address: &str,
        only_simple: bool,
        chain_filter: Option<u64>,
    ) -> Result<Vec<ZerionPositionRecord>> {
        self.require_key()?;
        let url = format!("{}/wallets/{}/positions/", self.base_url, address);

        let mut query: Vec<(&str, String)> = vec![
            ("currency", "usd".to_string()),
            ("sort", "value".to_string()),
            ("page[size]", "100".to_string()),
        ];
        query.push((
            "filter[positions]",
            if only_simple { "only_simple" } else { "no_filter" }.to_string(),
        ));
        if let Some(chain_id) = chain_filter {
            // Translate the EVM chain id back to Zerion's slug so the API
            // does the filtering for us — fewer payload bytes to parse.
            let slug = id_to_zerion_chain(chain_id).ok_or_else(|| {
                ChainError::Config(format!(
                    "Zerion does not recognize chain id {chain_id}"
                ))
            })?;
            query.push(("filter[chain_ids]", slug.to_string()));
        }

        let req = self
            .client
            .get(&url)
            .basic_auth(&self.api_key, Some(""))
            .header("accept", "application/json")
            .timeout(Duration::from_secs(20))
            .query(&query);
        let body = send_retrying(req, "zerion.positions")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let resp: PositionsResponse = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "Zerion positions response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })?;

        Ok(resp.data.into_iter().filter_map(map_position).collect())
    }

    fn require_key(&self) -> Result<()> {
        if self.api_key.is_empty() {
            Err(ChainError::Config(
                "ZERION_API_KEY not set. Run: chainpilot config set zerion_api_key <key>"
                    .to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

fn map_position(item: PositionItem) -> Option<ZerionPositionRecord> {
    let attrs = item.attributes?;
    let flags = attrs.flags.unwrap_or(PositionFlags {
        displayable: Some(true),
        is_trash: Some(false),
    });
    if !flags.displayable.unwrap_or(true) || flags.is_trash.unwrap_or(false) {
        return None;
    }

    let quantity = attrs.quantity?;
    let amount = quantity
        .float
        .or_else(|| quantity.numeric.as_deref().and_then(|s| s.parse::<f64>().ok()))?;
    if amount <= 0.0 {
        return None;
    }

    let fungible = attrs.fungible_info.unwrap_or(FungibleInfo {
        name: None,
        symbol: None,
        implementations: Vec::new(),
    });

    let chain_slug = item
        .relationships
        .and_then(|r| r.chain)
        .and_then(|c| c.data)
        .and_then(|d| d.id)
        .unwrap_or_default();
    let chain_id = if chain_slug.is_empty() {
        None
    } else {
        zerion_chain_to_id(&chain_slug)
    };

    // Pick the implementation address matching the active chain when possible.
    let address = fungible
        .implementations
        .iter()
        .find(|i| i.chain_id.as_deref() == Some(chain_slug.as_str()))
        .and_then(|i| i.address.clone())
        .unwrap_or_default();

    let symbol = fungible
        .symbol
        .clone()
        .or_else(|| attrs.name.clone())
        .unwrap_or_default();
    let name = fungible.name.unwrap_or_else(|| symbol.clone());

    let display_name = attrs.name.clone();
    let protocol = extract_protocol_name(attrs.protocol.as_ref()).or_else(|| {
        attrs
            .application_metadata
            .as_ref()
            .and_then(|m| m.name.clone())
    });
    let position_type = attrs.position_type.unwrap_or_else(|| "wallet".to_string());

    tracing::debug!(
        target: "zerion",
        chain_slug = %chain_slug,
        symbol = %symbol,
        position_type = %position_type,
        protocol = ?protocol,
        display_name = ?display_name,
        value_usd = ?attrs.value,
        "zerion position parsed",
    );

    Some(ZerionPositionRecord {
        chain_slug,
        chain_id,
        symbol,
        name,
        display_name,
        address,
        amount,
        price_usd: attrs.price,
        value_usd: attrs.value,
        position_type,
        protocol,
        protocol_url: attrs.application_metadata.and_then(|m| m.url),
    })
}

/// Trim a response body for inclusion in error messages — Zerion responses
/// can be 100+ KB, but the first 400 chars are usually enough to tell whether
/// it's an auth error, a rate-limit notice, or a schema surprise.
fn snippet(body: &str) -> String {
    const MAX: usize = 400;
    if body.len() <= MAX {
        body.to_string()
    } else {
        let mut end = MAX;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &body[..end])
    }
}

/// Zerion chain slug → EVM chain id. Slugs are stable strings used in Zerion's
/// `relationships.chain` and `filter[chain_ids]` query param. Aliases cover
/// historical variants Zerion has used across endpoints (e.g. `arbitrum` vs
/// `arbitrum-one`, `matic` vs `polygon`, `bsc` vs `binance-smart-chain`).
///
/// Keep [`id_to_zerion_chain`] aligned with the canonical (first-listed) slug
/// for each chain.
pub fn zerion_chain_to_id(slug: &str) -> Option<u64> {
    match slug {
        "ethereum" | "eth" => Some(1),
        "binance-smart-chain" | "bsc" | "bnb" => Some(56),
        "polygon" | "polygon-pos" | "matic" => Some(137),
        "arbitrum" | "arbitrum-one" => Some(42161),
        "arbitrum-nova" => Some(42170),
        "optimism" | "optimistic-ethereum" => Some(10),
        "avalanche" | "avalanche-c-chain" => Some(43114),
        "base" => Some(8453),
        "linea" => Some(59144),
        "scroll" => Some(534352),
        "mantle" => Some(5000),
        "aurora" => Some(1313161554),
        "manta-pacific" | "manta" => Some(169),
        "taiko" => Some(167000),
        "fantom" | "fantom-opera" => Some(250),
        "xdai" | "gnosis" => Some(100),
        "celo" => Some(42220),
        "zksync-era" | "zksync" => Some(324),
        "polygon-zkevm" => Some(1101),
        _ => None,
    }
}

/// Reverse mapping for pushing `--chain-id` filters into Zerion's query string.
/// Returns the canonical slug Zerion accepts; callers pass this value into
/// `filter[chain_ids]=...`.
///
/// Some chains supported by this CLI are deliberately absent because Zerion
/// does not index them (as of writing): OKChain/X Layer (66), Conflux eSpace
/// (1030), Plume (98866). Hitting those chain ids forces `wallet balance` /
/// `wallet overview` to fall through to Goldrush or on-chain RPC.
pub fn id_to_zerion_chain(id: u64) -> Option<&'static str> {
    match id {
        1 => Some("ethereum"),
        56 => Some("binance-smart-chain"),
        137 => Some("polygon"),
        42161 => Some("arbitrum"),
        10 => Some("optimism"),
        43114 => Some("avalanche"),
        8453 => Some("base"),
        59144 => Some("linea"),
        534352 => Some("scroll"),
        5000 => Some("mantle"),
        1313161554 => Some("aurora"),
        169 => Some("manta-pacific"),
        167000 => Some("taiko"),
        250 => Some("fantom"),
        100 => Some("xdai"),
        42220 => Some("celo"),
        324 => Some("zksync-era"),
        1101 => Some("polygon-zkevm"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_slug_round_trips_for_supported_chains() {
        for id in [
            1u64, 56, 137, 42161, 10, 43114, 8453, 59144, 534352, 5000, 169, 167000,
        ] {
            let slug = id_to_zerion_chain(id).unwrap_or_else(|| panic!("missing slug for {id}"));
            assert_eq!(
                zerion_chain_to_id(slug),
                Some(id),
                "round-trip failed for {id}/{slug}",
            );
        }
    }

    /// Every mainnet chain registered in `config::chains::CHAINS` must either
    /// have a Zerion slug mapping or be explicitly listed here as unsupported.
    /// If this test fails after adding a new chain, either add the mapping or
    /// add the chain id to the `unsupported` set.
    #[test]
    fn all_config_chains_have_zerion_mapping_or_are_documented_unsupported() {
        let unsupported: std::collections::HashSet<u64> = [66u64, 1030, 98866, 11155111]
            .into_iter()
            .collect();
        for id in crate::config::chains::all_chain_ids() {
            if id_to_zerion_chain(id).is_some() {
                continue;
            }
            assert!(
                unsupported.contains(&id),
                "chain id {id} has no Zerion slug and is not listed as unsupported — \
                 either add a mapping in id_to_zerion_chain or add it to the \
                 unsupported set in this test"
            );
        }
    }

    #[test]
    fn xdai_and_gnosis_aliases_both_map_to_100() {
        assert_eq!(zerion_chain_to_id("xdai"), Some(100));
        assert_eq!(zerion_chain_to_id("gnosis"), Some(100));
    }

    #[test]
    fn unknown_chain_returns_none() {
        assert_eq!(zerion_chain_to_id("unknown"), None);
        assert_eq!(id_to_zerion_chain(999_999), None);
    }

    #[test]
    fn client_without_key_reports_unconfigured() {
        let http = Client::new();
        let client = ZerionClient::new(http, "https://api.zerion.io/v1", "");
        assert!(!client.is_configured());
        assert!(client.require_key().is_err());
    }

    #[test]
    fn client_with_key_is_configured() {
        let http = Client::new();
        let client = ZerionClient::new(http, "https://api.zerion.io/v1", "test-key");
        assert!(client.is_configured());
        assert!(client.require_key().is_ok());
    }
}
