use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

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
struct DuneExecuteSqlResponse {
    execution_id: String,
}

#[derive(Debug, Serialize)]
struct DuneExecuteSqlRequest {
    sql: String,
    performance: &'static str,
}

#[derive(Debug, Deserialize)]
struct DuneQueryResponse {
    state: Option<String>,
    error: Option<DuneQueryError>,
    #[serde(default)]
    is_execution_finished: bool,
    result: Option<DuneQueryResult>,
}

#[derive(Debug, Deserialize)]
struct DuneQueryError {
    message: Option<String>,
    #[serde(rename = "type")]
    error_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DuneQueryResult {
    #[serde(default)]
    rows: Vec<DuneLabelRow>,
}

#[derive(Debug, Deserialize)]
struct DuneLabelRow {
    blockchain: Option<String>,
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

    /// Fetch wallet labels from Dune's official Data API by querying
    /// `labels.addresses`.
    pub async fn wallet_labels(&self, address: &str) -> Result<Vec<DuneLabelRecord>> {
        self.require_key()?;

        let normalized = address.to_lowercase();
        let sql = format!(
            "SELECT DISTINCT blockchain, name AS label, label_type, source \
             FROM labels.addresses \
             WHERE address = from_hex(replace('{normalized}', '0x', '')) \
             ORDER BY blockchain, label"
        );

        let execute_url = format!("{}/sql/execute", self.base_url);
        let req = self
            .client
            .post(&execute_url)
            .header("X-Dune-API-Key", &self.api_key)
            .header("accept", "application/json")
            .timeout(Duration::from_secs(15))
            .json(&DuneExecuteSqlRequest {
                sql,
                performance: "medium",
            });

        let execution: DuneExecuteSqlResponse = send_retrying(req, "dune.wallet_labels.execute")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .json()
            .await
            .map_err(ChainError::Http)?;

        let resp = self
            .wait_for_execution_results(&execution.execution_id)
            .await?;

        let rows = resp.result.map(|r| r.rows).unwrap_or_default();

        let records: Vec<DuneLabelRecord> = rows
            .into_iter()
            .filter_map(|row| {
                let label = row.label?;
                if label.is_empty() {
                    return None;
                }
                let mut details = Vec::new();
                if let Some(blockchain) = row.blockchain {
                    if !blockchain.is_empty() {
                        details.push(format!("chain: {}", blockchain));
                    }
                }
                if let Some(label_type) = row.label_type {
                    if !label_type.is_empty() {
                        details.push(format!("type: {}", label_type));
                    }
                }
                if let Some(source) = row.source {
                    if !source.is_empty() {
                        details.push(format!("source: {}", source));
                    }
                }
                let reason = if details.is_empty() {
                    None
                } else {
                    Some(details.join(", "))
                };
                Some(DuneLabelRecord {
                    label,
                    score: None,
                    reason,
                })
            })
            .collect();

        Ok(records)
    }

    async fn wait_for_execution_results(&self, execution_id: &str) -> Result<DuneQueryResponse> {
        const MAX_POLLS: usize = 8;
        const POLL_DELAY_MS: u64 = 1_000;

        let url = format!("{}/execution/{}/results", self.base_url, execution_id);

        for poll in 0..MAX_POLLS {
            let req = self
                .client
                .get(&url)
                .header("X-Dune-API-Key", &self.api_key)
                .header("accept", "application/json")
                .timeout(Duration::from_secs(15))
                .query(&[("limit", "100"), ("allow_partial_results", "true")]);

            let body = send_retrying(req, "dune.wallet_labels.results")
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

            if let Some(err) = &resp.error {
                let err_type = err.error_type.as_deref().unwrap_or("unknown");
                let message = err.message.as_deref().unwrap_or("unknown error");
                return Err(ChainError::Config(format!(
                    "Dune labels query failed ({err_type}): {message}"
                )));
            }

            if resp.is_execution_finished
                || matches!(resp.state.as_deref(), Some("QUERY_STATE_COMPLETED"))
            {
                return Ok(resp);
            }

            if matches!(
                resp.state.as_deref(),
                Some("QUERY_STATE_FAILED") | Some("QUERY_STATE_CANCELLED") | Some("QUERY_STATE_EXPIRED")
            ) {
                return Err(ChainError::Config(format!(
                    "Dune labels query ended in state {}",
                    resp.state.as_deref().unwrap_or("unknown")
                )));
            }

            if poll + 1 < MAX_POLLS {
                tokio::time::sleep(Duration::from_millis(POLL_DELAY_MS)).await;
            }
        }

        Err(ChainError::Config(format!(
            "Dune labels query did not finish in time for execution {}",
            execution_id
        )))
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
