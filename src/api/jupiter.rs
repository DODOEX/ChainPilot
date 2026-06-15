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

#[derive(Debug, Deserialize)]
struct JupiterTokenResponse {
    address: String,
    name: String,
    symbol: String,
    decimals: u8,
    #[serde(rename = "logoURI")]
    logo_uri: Option<String>,
}

impl JupiterClient {
    pub fn new(client: Client, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Fetch metadata for a single SPL mint. Returns `None` when the mint is
    /// not in Jupiter's index (HTTP 404) rather than surfacing it as an
    /// error, so callers can fall through to CoinGecko/DexScreener cleanly.
    pub async fn token(&self, mint: &str) -> Result<Option<JupiterToken>> {
        let url = format!("{}/tokens/v1/token/{}", self.base_url, mint);
        let resp = send_retrying(self.client.get(&url), "jupiter.token").await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let body = resp
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let parsed: JupiterTokenResponse = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "Jupiter token response could not be parsed: {e}"
            ))
        })?;

        Ok(Some(JupiterToken {
            address: parsed.address,
            name: parsed.name,
            symbol: parsed.symbol,
            decimals: parsed.decimals,
            logo_uri: parsed.logo_uri,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jupiter_token_response_shape() {
        // Real Jupiter response shape for USDC mint, trimmed to fields we use.
        let body = r#"{
            "address": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
            "name": "USD Coin",
            "symbol": "USDC",
            "decimals": 6,
            "logoURI": "https://example.com/usdc.png"
        }"#;
        let r: JupiterTokenResponse = serde_json::from_str(body).unwrap();
        assert_eq!(r.symbol, "USDC");
        assert_eq!(r.decimals, 6);
        assert!(r.logo_uri.is_some());
    }

    #[test]
    fn jupiter_token_decimals_uses_native_u8_range() {
        // Real SPL mints range 0..=18 decimals; the wire format pins to u8
        // already, but lock it in test-wise so a future schema drift breaks
        // here instead of silently truncating.
        let body =
            r#"{"address":"x","name":"x","symbol":"x","decimals":18,"logoURI":null}"#;
        let r: JupiterTokenResponse = serde_json::from_str(body).unwrap();
        assert_eq!(r.decimals, 18);
    }
}
