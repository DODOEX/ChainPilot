//! Jupiter (jup.ag) token metadata client for SPL tokens on Solana.
//!
//! Only the single-mint lookup is wired today (`/tokens/v1/token/{mint}`),
//! enough to answer `token info <mint>` on SVM. The bulk endpoints
//! (`/tokens/v1/all`, `/tokens/v1/tagged/verified`) are intentionally not
//! used: the all-tokens response is ~6 MB and inappropriate for a CLI
//! one-shot lookup.
//!
//! No API key required. The free `lite-api.jup.ag` host is rate-limited but
//! sufficient for interactive use.

use reqwest::Client;
use serde::Deserialize;

use crate::api::send_retrying;
use crate::error::{ChainError, Result};

pub const DEFAULT_JUPITER_API_URL: &str = "https://lite-api.jup.ag";

#[derive(Clone)]
pub struct JupiterClient {
    client: Client,
    base_url: String,
}

/// Subset of the Jupiter token document we surface upward. The endpoint
/// returns more (logos, tags, daily volume), but `name`/`symbol`/`decimals`
/// are what `token info` needs to build a `TokenInfo` for an SPL mint that
/// the user has never been quoted before.
#[derive(Debug, Clone)]
pub struct JupiterToken {
    pub address: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    /// Token icon URL when Jupiter has one indexed. Surfaced for callers
    /// that render token cards; current CLI output doesn't yet display it.
    #[allow(dead_code)]
    pub logo_uri: Option<String>,
}

/// One entry from the Token API v2 `search` response. v2 keys the mint as
/// `id` and the logo as `icon` (v1 used `address` / `logoURI`, and its
/// `/tokens/v1/token/{mint}` route was retired).
#[derive(Debug, Deserialize)]
struct JupiterTokenResponse {
    id: String,
    name: String,
    symbol: String,
    decimals: u8,
    #[serde(default)]
    icon: Option<String>,
}

/// A read-only Jupiter swap quote. Amounts are raw base units (the input
/// mint's smallest unit) as strings, matching Jupiter's wire format; callers
/// convert to display units using the mint decimals. No transaction is built
/// or signed — this is quote-only.
#[derive(Debug, Clone)]
pub struct JupiterQuote {
    pub out_amount: String,
    /// Minimum output after slippage (Jupiter's `otherAmountThreshold`).
    pub other_amount_threshold: String,
    /// Price impact as a decimal fraction (e.g. `0.0018` = 0.18%).
    pub price_impact_pct: f64,
    pub route: Vec<JupiterRouteHop>,
    /// Full response, surfaced verbatim for `--json` / raw inspection.
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct JupiterRouteHop {
    pub amm_key: String,
    pub label: String,
    pub input_mint: String,
    pub output_mint: String,
    pub percent: f64,
}

#[derive(Debug, Deserialize)]
struct JupiterQuoteResponse {
    #[serde(rename = "outAmount")]
    out_amount: String,
    #[serde(rename = "otherAmountThreshold")]
    other_amount_threshold: String,
    // Jupiter serializes this as a decimal string, not a number.
    #[serde(rename = "priceImpactPct", default)]
    price_impact_pct: String,
    #[serde(rename = "routePlan", default)]
    route_plan: Vec<JupiterRoutePlanItem>,
}

#[derive(Debug, Deserialize)]
struct JupiterRoutePlanItem {
    #[serde(rename = "swapInfo")]
    swap_info: JupiterSwapInfo,
    #[serde(default)]
    percent: f64,
}

#[derive(Debug, Deserialize)]
struct JupiterSwapInfo {
    #[serde(rename = "ammKey", default)]
    amm_key: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(rename = "inputMint", default)]
    input_mint: Option<String>,
    #[serde(rename = "outputMint", default)]
    output_mint: Option<String>,
}

impl JupiterClient {
    pub fn new(client: Client, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Fetch metadata for a single SPL mint via the Token API v2 `search`
    /// endpoint. Returns `None` when Jupiter has no matching token, so callers
    /// can fall through to CoinGecko/DexScreener cleanly. The response is an
    /// array; we prefer the entry whose `id` exactly equals the mint and fall
    /// back to the first result.
    pub async fn token(&self, mint: &str) -> Result<Option<JupiterToken>> {
        let url = format!("{}/tokens/v2/search", self.base_url);
        let resp = send_retrying(
            self.client.get(&url).query(&[("query", mint)]),
            "jupiter.token",
        )
        .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let body = resp
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let mut parsed: Vec<JupiterTokenResponse> = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!("Jupiter token response could not be parsed: {e}"))
        })?;

        // `/tokens/v2/search` fuzzy-matches, so it can return unrelated tokens
        // for a mint it doesn't index. Require an exact `id == mint` match —
        // returning a fuzzy first hit here would silently resolve a different
        // token's decimals/address into `swap quote` and `token info`. A miss
        // returns `None`, letting callers fall through to CoinGecko/DexScreener
        // or reject an unroutable mint.
        let chosen = parsed
            .iter()
            .position(|t| t.id == mint)
            .map(|i| parsed.swap_remove(i));

        Ok(chosen.map(|t| JupiterToken {
            address: t.id,
            name: t.name,
            symbol: t.symbol,
            decimals: t.decimals,
            logo_uri: t.icon,
        }))
    }

    /// Fetch a read-only swap quote from Jupiter's aggregator. `amount_raw` is
    /// in the input mint's smallest unit; `slippage_bps` is basis points
    /// (100 = 1%). Read-only: Jupiter is queried for pricing only — no
    /// transaction is assembled, signed, or broadcast.
    pub async fn quote(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount_raw: &str,
        slippage_bps: u32,
    ) -> Result<JupiterQuote> {
        let url = format!("{}/swap/v1/quote", self.base_url);
        let slippage = slippage_bps.to_string();
        let resp = send_retrying(
            self.client.get(&url).query(&[
                ("inputMint", input_mint),
                ("outputMint", output_mint),
                ("amount", amount_raw),
                ("slippageBps", slippage.as_str()),
            ]),
            "jupiter.quote",
        )
        .await?;

        let body = resp
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        parse_quote(&body)
    }
}

/// Parse a Jupiter quote response body into [`JupiterQuote`]. Pure (no I/O) so
/// the shape mapping is unit-testable.
fn parse_quote(body: &str) -> Result<JupiterQuote> {
    let raw: serde_json::Value = serde_json::from_str(body).map_err(|e| {
        ChainError::Config(format!("Jupiter quote response could not be parsed: {e}"))
    })?;
    let parsed: JupiterQuoteResponse = serde_json::from_value(raw.clone()).map_err(|e| {
        ChainError::Config(format!("Jupiter quote response shape unexpected: {e}"))
    })?;

    let route = parsed
        .route_plan
        .into_iter()
        .map(|hop| JupiterRouteHop {
            amm_key: hop.swap_info.amm_key.unwrap_or_default(),
            label: hop.swap_info.label.unwrap_or_default(),
            input_mint: hop.swap_info.input_mint.unwrap_or_default(),
            output_mint: hop.swap_info.output_mint.unwrap_or_default(),
            percent: hop.percent,
        })
        .collect();

    Ok(JupiterQuote {
        out_amount: parsed.out_amount,
        other_amount_threshold: parsed.other_amount_threshold,
        price_impact_pct: parsed.price_impact_pct.parse::<f64>().unwrap_or(0.0),
        route,
        raw,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jupiter_token_response_shape() {
        // Token API v2 `search` shape (array of results), trimmed to fields
        // we use: mint is `id`, logo is `icon`.
        let body = r#"[{
            "id": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "name": "USD Coin",
            "symbol": "USDC",
            "decimals": 6,
            "icon": "https://example.com/usdc.png"
        }]"#;
        let r: Vec<JupiterTokenResponse> = serde_json::from_str(body).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].symbol, "USDC");
        assert_eq!(r[0].decimals, 6);
        assert_eq!(r[0].id, "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        assert!(r[0].icon.is_some());
    }

    #[test]
    fn parses_jupiter_quote_shape() {
        // Trimmed real shape: 1 USDC -> SOL, single hop.
        let body = r#"{
            "inputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "outputMint": "So11111111111111111111111111111111111111112",
            "inAmount": "1000000",
            "outAmount": "12794037",
            "otherAmountThreshold": "12730067",
            "swapMode": "ExactIn",
            "slippageBps": 50,
            "priceImpactPct": "0.000183504",
            "routePlan": [
                {"swapInfo": {"ammKey": "POOL1", "label": "Deriverse",
                  "inputMint": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
                  "outputMint": "So11111111111111111111111111111111111111112"},
                 "percent": 100}
            ]
        }"#;
        let q = parse_quote(body).expect("quote should parse");
        assert_eq!(q.out_amount, "12794037");
        assert_eq!(q.other_amount_threshold, "12730067");
        assert!((q.price_impact_pct - 0.000183504).abs() < 1e-9);
        assert_eq!(q.route.len(), 1);
        assert_eq!(q.route[0].label, "Deriverse");
        assert_eq!(q.route[0].percent, 100.0);
    }

    #[test]
    fn jupiter_token_decimals_uses_native_u8_range() {
        // Real SPL mints range 0..=18 decimals; the wire format pins to u8
        // already, but lock it in test-wise so a future schema drift breaks
        // here instead of silently truncating.
        let body = r#"{"id":"x","name":"x","symbol":"x","decimals":18,"icon":null}"#;
        let r: JupiterTokenResponse = serde_json::from_str(body).unwrap();
        assert_eq!(r.decimals, 18);
    }
}
