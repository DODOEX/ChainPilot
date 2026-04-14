use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub quote_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub from_token: TokenRef,
    pub to_token: TokenRef,
    pub from_amount: String,
    pub from_amount_display: f64,
    pub to_amount: String,
    pub to_amount_display: f64,
    pub to_amount_min: String,
    pub price_impact_pct: f64,
    pub exchange_rate: f64,
    pub route_summary: Vec<RouteHop>,
    pub dex_sources: Vec<String>,
    pub route_id: Option<String>,
    pub router_to: String,
    pub calldata: String,
    pub value: String,
    pub gas_limit: Option<u64>,
    pub estimated_gas: Option<u64>,
    pub estimated_gas_usd: Option<f64>,
    pub raw_dodo_response: serde_json::Value,
    pub chain_id: u64,
    pub slippage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRef {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    pub chain_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHop {
    pub pool_address: String,
    pub dex_name: String,
    pub from_token: String,
    pub to_token: String,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub from: String,
    pub to: String,
    pub amount: String,
    pub amount_display: f64,
    pub chain_id: u64,
    pub slippage: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_token_ref() -> TokenRef {
        TokenRef {
            symbol: "ETH".to_string(),
            address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
            decimals: 18,
            chain_id: 1,
        }
    }

    fn sample_quote() -> Quote {
        let created_at = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let expires_at = Utc.with_ymd_and_hms(2024, 1, 1, 0, 5, 0).unwrap();
        Quote {
            quote_id: Uuid::nil(),
            created_at,
            expires_at,
            from_token: TokenRef {
                symbol: "ETH".to_string(),
                address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                decimals: 18,
                chain_id: 1,
            },
            to_token: TokenRef {
                symbol: "USDC".to_string(),
                address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
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
            route_summary: vec![RouteHop {
                pool_address: "0xPool".to_string(),
                dex_name: "DODO".to_string(),
                from_token: "ETH".to_string(),
                to_token: "USDC".to_string(),
                percent: 100.0,
            }],
            dex_sources: vec!["DODO".to_string()],
            route_id: Some("route-1".to_string()),
            router_to: "0xRouter".to_string(),
            calldata: "0xdeadbeef".to_string(),
            value: "0".to_string(),
            gas_limit: Some(200_000),
            estimated_gas: Some(150_000),
            estimated_gas_usd: Some(5.0),
            raw_dodo_response: serde_json::json!({"key": "val"}),
            chain_id: 1,
            slippage: 1.0,
        }
    }

    #[test]
    fn token_ref_serde_roundtrip() {
        let t = sample_token_ref();
        let json = serde_json::to_string(&t).unwrap();
        let back: TokenRef = serde_json::from_str(&json).unwrap();
        assert_eq!(back.symbol, t.symbol);
        assert_eq!(back.address, t.address);
        assert_eq!(back.decimals, t.decimals);
        assert_eq!(back.chain_id, t.chain_id);
    }

    #[test]
    fn quote_serde_roundtrip() {
        let q = sample_quote();
        let json = serde_json::to_string(&q).unwrap();
        let back: Quote = serde_json::from_str(&json).unwrap();
        assert_eq!(back.quote_id, q.quote_id);
        assert_eq!(back.from_token.symbol, "ETH");
        assert_eq!(back.to_token.symbol, "USDC");
        assert_eq!(back.from_amount_display, 1.0);
        assert_eq!(back.to_amount_display, 3000.0);
        assert_eq!(back.exchange_rate, 3000.0);
        assert_eq!(back.route_summary.len(), 1);
        assert_eq!(back.route_summary[0].dex_name, "DODO");
        assert_eq!(back.chain_id, 1);
        assert_eq!(back.slippage, 1.0);
    }

    #[test]
    fn quote_optional_fields_can_be_none() {
        let mut q = sample_quote();
        q.gas_limit = None;
        q.estimated_gas = None;
        q.estimated_gas_usd = None;
        q.route_id = None;
        let json = serde_json::to_string(&q).unwrap();
        let back: Quote = serde_json::from_str(&json).unwrap();
        assert!(back.gas_limit.is_none());
        assert!(back.estimated_gas.is_none());
        assert!(back.route_id.is_none());
    }

    #[test]
    fn quote_request_serde_roundtrip() {
        let req = QuoteRequest {
            from: "0xFrom".to_string(),
            to: "0xTo".to_string(),
            amount: "0.5".to_string(),
            amount_display: 0.5,
            chain_id: 137,
            slippage: 0.5,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: QuoteRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.from, "0xFrom");
        assert_eq!(back.amount, "0.5");
        assert_eq!(back.amount_display, 0.5);
        assert_eq!(back.chain_id, 137);
        assert_eq!(back.slippage, 0.5);
    }
}
