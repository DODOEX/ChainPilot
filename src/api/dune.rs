use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::api::send_retrying;
use crate::error::{ChainError, Result};

#[derive(Clone)]
pub struct DuneClient {
    client: Client,
    base_url: String,
    api_key: String,
}

// ── wallet labels ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DuneQueryResponse {
    result: Option<DuneQueryResult>,
}

#[derive(Debug, Deserialize)]
struct DuneQueryResult {
    #[serde(default)]
    rows: Vec<DuneLabelRow>,
}

#[derive(Debug, Deserialize)]
struct DuneLabelRow {
    #[allow(dead_code)]
    address: Option<String>,
    label: Option<String>,
    label_type: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DuneLabelRecord {
    pub label: String,
    pub score: Option<f64>,
    pub reason: Option<String>,
}

impl DuneClient {
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

    /// Fetch wallet labels from Dune's labels API.
    /// Uses the labels endpoint to get community-maintained wallet tags.
    pub async fn wallet_labels(&self, address: &str) -> Result<Vec<DuneLabelRecord>> {
        self.require_key()?;

        // Use Dune's labels API endpoint
        let url = format!("{}/labels/{}", self.base_url, address.to_lowercase());

        let req = self
            .client
            .get(&url)
            .header("X-Dune-API-Key", &self.api_key)
            .header("accept", "application/json")
            .timeout(Duration::from_secs(15));

        let body = send_retrying(req, "dune.wallet_labels")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let resp: DuneQueryResponse = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "Dune labels response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })?;

        let rows = resp.result.map(|r| r.rows).unwrap_or_default();

        let records: Vec<DuneLabelRecord> = rows
            .into_iter()
            .filter_map(|row| {
                let label = row.label?;
                if label.is_empty() {
                    return None;
                }
                let reason = row
                    .label_type
                    .map(|t| format!("type: {}", t))
                    .or_else(|| row.source.map(|s| format!("source: {}", s)));
                Some(DuneLabelRecord {
                    label,
                    score: None,
                    reason,
                })
            })
            .collect();

        Ok(records)
    }

    fn require_key(&self) -> Result<()> {
        if self.api_key.is_empty() {
            Err(ChainError::Config(
                "DUNE_API_KEY not set. Run: chainpilot config set dune_api_key <key>".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Trim a response body for inclusion in error messages.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_without_key_reports_unconfigured() {
        let http = Client::new();
        let client = DuneClient::new(http, "https://api.dune.com/api/v1", "");
        assert!(!client.is_configured());
        assert!(client.require_key().is_err());
    }

    #[test]
    fn client_with_key_is_configured() {
        let http = Client::new();
        let client = DuneClient::new(http, "https://api.dune.com/api/v1", "test-key");
        assert!(client.is_configured());
        assert!(client.require_key().is_ok());
    }
}
