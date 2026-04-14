use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::ChainError;

#[derive(Debug, Serialize)]
pub struct ChainOutput<T: Serialize> {
    pub ok: bool,
    pub command: String,
    pub timestamp: DateTime<Utc>,
    pub data: Option<T>,
    pub error: Option<ErrorDetail>,
    pub warnings: Vec<String>,
    pub meta: OutputMeta,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutputMeta {
    pub chain_id: u64,
    pub dry_run: bool,
    pub version: String,
}

impl<T: Serialize> ChainOutput<T> {
    pub fn success(command: &str, data: T, meta: OutputMeta) -> Self {
        Self {
            ok: true,
            command: command.to_string(),
            timestamp: Utc::now(),
            data: Some(data),
            error: None,
            warnings: vec![],
            meta,
        }
    }
}

impl ChainOutput<()> {
    pub fn error(command: &str, err: &ChainError, meta: OutputMeta) -> Self {
        let (code, suggestion) = error_code_and_suggestion(err);
        Self {
            ok: false,
            command: command.to_string(),
            timestamp: Utc::now(),
            data: None,
            error: Some(ErrorDetail {
                code,
                message: err.to_string(),
                suggestion,
            }),
            warnings: vec![],
            meta,
        }
    }
}

fn error_code_and_suggestion(err: &ChainError) -> (String, Option<String>) {
    match err {
        ChainError::QuoteNotFound(_) => (
            "quote_not_found".to_string(),
            Some(
                "Run 'chainpilot swap quote --from ETH --to USDC --amount 1' to get a fresh quote."
                    .to_string(),
            ),
        ),
        ChainError::QuoteExpired(_) => (
            "quote_expired".to_string(),
            Some("Quotes expire after 5 minutes. Run 'chainpilot swap quote' again.".to_string()),
        ),
        ChainError::NoWallet => (
            "no_wallet".to_string(),
            Some("Set PRIVATE_KEY env var or use --private-key flag.".to_string()),
        ),
        ChainError::InvalidAmount(_) => (
            "invalid_amount".to_string(),
            Some("Use a plain decimal amount like '1' or '0.25'.".to_string()),
        ),
        ChainError::NotApproved { token, spender } => (
            "not_approved".to_string(),
            Some(format!(
                "Run: chainpilot swap approve --token {} --spender {}",
                token, spender
            )),
        ),
        ChainError::InsufficientBalance { .. } => ("insufficient_balance".to_string(), None),
        _ => ("unknown_error".to_string(), None),
    }
}
