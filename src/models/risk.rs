use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskSignal {
    pub signal: String,
    pub description: String,
    pub severity: RiskLevel,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskReport {
    pub subject: String,
    pub subject_type: String,
    pub overall_risk: RiskLevel,
    pub signals: Vec<RiskSignal>,
    pub metadata: serde_json::Value,
    pub analyzed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRisk {
    pub address: String,
    pub spender: String,
    pub token_address: String,
    pub token_symbol: String,
    pub current_allowance: String,
    pub is_unlimited: bool,
    pub risk: RiskLevel,
    pub signals: Vec<RiskSignal>,
    pub analyzed_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_level_serializes_as_screaming_snake_case() {
        assert_eq!(serde_json::to_string(&RiskLevel::Low).unwrap(), "\"LOW\"");
        assert_eq!(
            serde_json::to_string(&RiskLevel::Medium).unwrap(),
            "\"MEDIUM\""
        );
        assert_eq!(serde_json::to_string(&RiskLevel::High).unwrap(), "\"HIGH\"");
        assert_eq!(
            serde_json::to_string(&RiskLevel::Critical).unwrap(),
            "\"CRITICAL\""
        );
    }

    #[test]
    fn risk_level_deserializes_from_screaming_snake_case() {
        let low: RiskLevel = serde_json::from_str("\"LOW\"").unwrap();
        assert!(matches!(low, RiskLevel::Low));

        let critical: RiskLevel = serde_json::from_str("\"CRITICAL\"").unwrap();
        assert!(matches!(critical, RiskLevel::Critical));
    }

    #[test]
    fn risk_report_serde_roundtrip() {
        let report = RiskReport {
            subject: "0xToken".to_string(),
            subject_type: "token".to_string(),
            overall_risk: RiskLevel::High,
            signals: vec![RiskSignal {
                signal: "UNVERIFIED_CONTRACT".to_string(),
                description: "Source not verified".to_string(),
                severity: RiskLevel::High,
                value: serde_json::json!(true),
            }],
            metadata: serde_json::json!({"chain_id": 1}),
            analyzed_at: Utc::now(),
        };
        let json = serde_json::to_string(&report).unwrap();
        let back: RiskReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back.subject, "0xToken");
        assert!(matches!(back.overall_risk, RiskLevel::High));
        assert_eq!(back.signals.len(), 1);
        assert_eq!(back.signals[0].signal, "UNVERIFIED_CONTRACT");
    }

    #[test]
    fn approval_risk_unlimited_flag() {
        let ar = ApprovalRisk {
            address: "0xWallet".to_string(),
            spender: "0xSpender".to_string(),
            token_address: "0xToken".to_string(),
            token_symbol: "USDC".to_string(),
            current_allowance:
                "115792089237316195423570985008687907853269984665640564039457584007913129639935"
                    .to_string(),
            is_unlimited: true,
            risk: RiskLevel::Critical,
            signals: vec![],
            analyzed_at: Utc::now(),
        };
        let json = serde_json::to_string(&ar).unwrap();
        let back: ApprovalRisk = serde_json::from_str(&json).unwrap();
        assert!(back.is_unlimited);
        assert!(matches!(back.risk, RiskLevel::Critical));
    }
}
