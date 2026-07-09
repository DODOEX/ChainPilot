use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub quote_id: Uuid,
    pub simulated_at: DateTime<Utc>,
    pub is_valid: bool,
    pub warnings: Vec<String>,
    pub expected_out: String,
    pub min_out: String,
    pub current_price_impact_pct: f64,
    pub wallet_balance: Option<String>,
    pub has_sufficient_balance: Option<bool>,
    pub current_allowance: Option<String>,
    pub needs_approval: Option<bool>,
    pub suggested_approve_amount: Option<String>,
    pub estimated_gas: Option<u64>,
    pub gas_price_gwei: f64,
    pub total_gas_cost_eth: f64,
    pub total_gas_cost_usd: Option<f64>,
    pub calldata: String,
    pub to_contract: String,
    pub value_eth: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub quote_id: Uuid,
    pub executed_at: DateTime<Utc>,
    pub dry_run: bool,
    pub tx_hash: Option<String>,
    pub status: ExecutionStatus,
    pub calldata: String,
    pub to_contract: String,
    pub value_eth: String,
    pub from_address: Option<String>,
    pub gas_used: Option<u64>,
    pub effective_gas_price_gwei: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    DryRun,
    Submitted,
    Confirmed,
    /// Mined but EVM execution reverted (receipt.status = 0).
    Failed,
    /// Transaction was replaced or dropped: nonce advanced with no receipt for this hash.
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapHistoryRecord {
    pub id: String,
    pub quote_id: Uuid,
    pub tx_hash: Option<String>,
    pub dry_run: bool,
    pub from_token: String,
    pub to_token: String,
    pub from_amount_display: f64,
    pub to_amount_display: f64,
    pub status: ExecutionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResult {
    pub token: String,
    pub spender: String,
    /// Raw approved amount as decimal string, or "unlimited" when approving U256::MAX.
    pub raw_amount: String,
    pub dry_run: bool,
    pub tx_hash: Option<String>,
    pub from_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreparedOperation {
    SwapExecute,
    Approve,
    Revoke,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedTransaction {
    pub to: String,
    pub value: String,
    pub data: String,
    pub chain_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_priority_fee_per_gas: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedQuote {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub quote_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage_bps: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedTokenAmount {
    pub chain_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    pub symbol: String,
    pub decimals: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_usd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_amount_raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_amount_display: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedRisk {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_in: Option<PreparedTokenAmount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_out: Option<PreparedTokenAmount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub router: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_gas_usd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedChainpilotTransaction {
    pub source: String,
    pub operation: PreparedOperation,
    pub chain_id: u64,
    pub caip2: String,
    #[serde(rename = "from")]
    pub from_address: String,
    pub transaction: PreparedTransaction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<PreparedQuote>,
    pub risk: PreparedRisk,
}

impl SwapHistoryRecord {
    pub fn from_execution(
        quote: &crate::models::quote::Quote,
        result: &ExecutionResult,
        id: String,
    ) -> Self {
        Self {
            id,
            quote_id: result.quote_id,
            tx_hash: result.tx_hash.clone(),
            dry_run: result.dry_run,
            from_token: quote.from_token.symbol.clone(),
            to_token: quote.to_token.symbol.clone(),
            from_amount_display: quote.from_amount_display,
            to_amount_display: quote.to_amount_display,
            status: result.status.clone(),
            created_at: result.executed_at,
            updated_at: result.executed_at,
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::quote::{Quote, TokenRef};
    use chrono::TimeZone;

    #[test]
    fn execution_status_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&ExecutionStatus::DryRun).unwrap(),
            "\"dry_run\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutionStatus::Submitted).unwrap(),
            "\"submitted\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutionStatus::Confirmed).unwrap(),
            "\"confirmed\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutionStatus::Failed).unwrap(),
            "\"failed\""
        );
        assert_eq!(
            serde_json::to_string(&ExecutionStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }

    #[test]
    fn execution_status_deserializes_from_snake_case() {
        let s: ExecutionStatus = serde_json::from_str("\"dry_run\"").unwrap();
        assert!(matches!(s, ExecutionStatus::DryRun));

        let s: ExecutionStatus = serde_json::from_str("\"confirmed\"").unwrap();
        assert!(matches!(s, ExecutionStatus::Confirmed));
    }

    fn sample_quote() -> Quote {
        let now = Utc.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
        Quote {
            quote_id: Uuid::nil(),
            created_at: now,
            expires_at: now + chrono::Duration::minutes(5),
            from_token: TokenRef {
                symbol: "ETH".to_string(),
                address: "0xEeee".to_string(),
                decimals: 18,
                chain_id: 1,
            },
            to_token: TokenRef {
                symbol: "USDC".to_string(),
                address: "0xA0b8".to_string(),
                decimals: 6,
                chain_id: 1,
            },
            from_amount: "1.0".to_string(),
            from_amount_display: 1.0,
            to_amount: "3000.0".to_string(),
            to_amount_display: 3000.0,
            to_amount_min: "2970.0".to_string(),
            price_impact_pct: -0.01,
            exchange_rate: 3000.0,
            route_summary: vec![],
            dex_sources: vec![],
            route_id: None,
            router_to: "0xRouter".to_string(),
            calldata: "0x".to_string(),
            value: "0".to_string(),
            gas_limit: None,
            estimated_gas: None,
            estimated_gas_usd: None,
            raw_dodo_response: serde_json::json!({}),
            chain_id: 1,
            slippage: 1.0,
        }
    }

    fn sample_execution_result(dry_run: bool) -> ExecutionResult {
        let now = Utc::now();
        ExecutionResult {
            quote_id: Uuid::nil(),
            executed_at: now,
            dry_run,
            tx_hash: if dry_run {
                None
            } else {
                Some("0xTxHash".to_string())
            },
            status: if dry_run {
                ExecutionStatus::DryRun
            } else {
                ExecutionStatus::Confirmed
            },
            calldata: "0xdata".to_string(),
            to_contract: "0xRouter".to_string(),
            value_eth: "0".to_string(),
            from_address: Some("0xWallet".to_string()),
            gas_used: Some(120_000),
            effective_gas_price_gwei: Some(20.0),
        }
    }

    #[test]
    fn from_execution_maps_tokens_and_amounts() {
        let q = sample_quote();
        let r = sample_execution_result(false);
        let rec = SwapHistoryRecord::from_execution(&q, &r, "rec-1".to_string());

        assert_eq!(rec.id, "rec-1");
        assert_eq!(rec.from_token, "ETH");
        assert_eq!(rec.to_token, "USDC");
        assert_eq!(rec.from_amount_display, 1.0);
        assert_eq!(rec.to_amount_display, 3000.0);
        assert!(!rec.dry_run);
        assert_eq!(rec.tx_hash.as_deref(), Some("0xTxHash"));
        assert!(matches!(rec.status, ExecutionStatus::Confirmed));
        assert!(rec.error.is_none());
    }

    #[test]
    fn from_execution_dry_run_has_no_tx_hash() {
        let q = sample_quote();
        let r = sample_execution_result(true);
        let rec = SwapHistoryRecord::from_execution(&q, &r, "dry-1".to_string());

        assert!(rec.dry_run);
        assert!(rec.tx_hash.is_none());
        assert!(matches!(rec.status, ExecutionStatus::DryRun));
    }

    #[test]
    fn swap_history_record_serde_roundtrip() {
        let q = sample_quote();
        let r = sample_execution_result(false);
        let rec = SwapHistoryRecord::from_execution(&q, &r, "round-trip".to_string());

        let json = serde_json::to_string(&rec).unwrap();
        let back: SwapHistoryRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "round-trip");
        assert_eq!(back.from_token, "ETH");
        assert!(matches!(back.status, ExecutionStatus::Confirmed));
    }
}
