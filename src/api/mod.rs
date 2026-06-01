mod debank;
mod dodo;
mod dune;
mod goldrush;
mod token_metadata;
mod zerion;

pub use debank::{debank_chain_to_id, DebankAssetRecord, DebankClient};
pub use dodo::DodoClient;
pub use dune::DuneClient;
pub use goldrush::{GoldrushAssetRecord, GoldrushChainBalance, GoldrushClient};
pub use token_metadata::TokenMetadataClient;
pub use zerion::{ZerionChainBalance, ZerionClient, ZerionPortfolio, ZerionPositionRecord};

use crate::config::AppConfig;
use crate::error::{ChainError, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, USER_AGENT};
use reqwest::StatusCode;
use std::time::Duration;

/// Send an HTTP request, retrying with backoff on `429 Too Many Requests` or
/// `503 Service Unavailable`. Other failures are surfaced immediately so we
/// don't mask schema or auth errors as transient.
///
/// `RequestBuilder` must be cloneable (no streaming body); all our aggregator
/// callers build GETs with `.query(...)` so they qualify.
///
/// `label` is used only for telemetry — keep it short, e.g. `"zerion.portfolio"`.
pub async fn send_retrying(
    req: reqwest::RequestBuilder,
    label: &str,
) -> Result<reqwest::Response> {
    const MAX_ATTEMPTS: u32 = 3;
    let mut attempt: u32 = 1;
    loop {
        let cloned = req.try_clone().ok_or_else(|| {
            ChainError::Config(format!(
                "{label}: request is not cloneable, cannot apply retry policy"
            ))
        })?;
        let resp = cloned.send().await.map_err(ChainError::Http)?;
        let status = resp.status();
        let retryable =
            status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE;
        if !retryable || attempt >= MAX_ATTEMPTS {
            return Ok(resp);
        }
        let delay = retry_after_duration(&resp)
            .unwrap_or_else(|| Duration::from_millis(500u64 << (attempt - 1)));
        tracing::warn!(
            target: "rate_limit",
            label = label,
            status = %status,
            attempt = attempt,
            max_attempts = MAX_ATTEMPTS,
            delay_ms = delay.as_millis() as u64,
            "rate-limited; backing off before retry"
        );
        drop(resp);
        tokio::time::sleep(delay).await;
        attempt += 1;
    }
}

fn retry_after_duration(resp: &reqwest::Response) -> Option<Duration> {
    retry_after_from_str(resp.headers().get(reqwest::header::RETRY_AFTER)?.to_str().ok()?)
}

/// Parse a `Retry-After` header value (integer seconds form only — the
/// HTTP-date form is rare enough in API responses that we skip it). The
/// returned duration is clamped to 30 s so a misbehaving server can't pin a
/// CLI command in a silent sleep.
fn retry_after_from_str(raw: &str) -> Option<Duration> {
    raw.trim()
        .parse::<u64>()
        .ok()
        .map(|secs| Duration::from_secs(secs.min(30)))
}

pub struct ApiClients {
    pub dodo: DodoClient,
    pub token_metadata: TokenMetadataClient,
    pub debank: DebankClient,
    pub zerion: ZerionClient,
    pub goldrush: GoldrushClient,
    pub dune: DuneClient,
}

impl ApiClients {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
            ),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(
                crate::config::DEFAULT_REQUEST_TIMEOUT_SECS,
            ))
            .default_headers(headers)
            .build()?;

        Ok(Self {
            dodo: DodoClient::new(
                client.clone(),
                &config.dodo_api_url,
                &config.dodo_api_key,
                &config.dodo_project_id,
            ),
            token_metadata: TokenMetadataClient::new(client.clone(), config),
            debank: DebankClient::new(
                client.clone(),
                &config.debank_api_url,
                &config.debank_api_key,
            ),
            zerion: ZerionClient::new(
                client.clone(),
                &config.zerion_api_url,
                &config.zerion_api_key,
            ),
            goldrush: GoldrushClient::new(
                client.clone(),
                &config.goldrush_api_url,
                &config.goldrush_api_key,
            ),
            dune: DuneClient::new(
                client,
                &config.dune_api_url,
                &config.dune_api_key,
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::retry_after_from_str;
    use std::time::Duration;

    #[test]
    fn retry_after_parses_integer_seconds() {
        assert_eq!(retry_after_from_str("5"), Some(Duration::from_secs(5)));
        assert_eq!(retry_after_from_str("  12 "), Some(Duration::from_secs(12)));
    }

    #[test]
    fn retry_after_clamps_oversized_hints_to_30_seconds() {
        assert_eq!(retry_after_from_str("600"), Some(Duration::from_secs(30)));
    }

    #[test]
    fn retry_after_rejects_http_date_form() {
        // We intentionally don't parse the HTTP-date form; the caller falls
        // back to exponential backoff in that case.
        assert!(retry_after_from_str("Wed, 21 Oct 2026 07:28:00 GMT").is_none());
        assert!(retry_after_from_str("").is_none());
    }
}
