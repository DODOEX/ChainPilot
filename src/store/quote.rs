use chrono::Utc;
use std::path::PathBuf;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::error::Result;
use crate::models::quote::Quote;

pub struct QuoteStore {
    quotes_dir: PathBuf,
    history_dir: PathBuf,
}

impl QuoteStore {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let quotes_dir = config.quotes_dir();
        let history_dir = config.history_dir();
        std::fs::create_dir_all(&quotes_dir)?;
        std::fs::create_dir_all(&history_dir)?;

        Ok(Self {
            quotes_dir,
            history_dir,
        })
    }

    pub fn save_quote(&self, quote: &Quote) -> Result<()> {
        let path = self.quote_path(&quote.quote_id.to_string());
        let json = serde_json::to_string_pretty(quote)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_quote(&self, quote_id: &str) -> Result<Option<Quote>> {
        self.cleanup_expired()?;

        let path = self.quote_path(quote_id);
        if !path.exists() {
            return Ok(None);
        }
        let json = std::fs::read_to_string(path)?;
        let quote: Quote = serde_json::from_str(&json)?;
        Ok(Some(quote))
    }

    #[cfg(test)]
    pub fn delete_quote(&self, quote_id: &str) -> Result<()> {
        let path = self.quote_path(quote_id);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn cleanup_expired(&self) -> Result<usize> {
        let now = Utc::now();
        let mut removed = 0;

        for entry in std::fs::read_dir(&self.quotes_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Ok(json) = std::fs::read_to_string(&path) {
                if let Ok(quote) = serde_json::from_str::<Quote>(&json) {
                    if now > quote.expires_at {
                        std::fs::remove_file(path).ok();
                        removed += 1;
                    }
                }
            }
        }
        Ok(removed)
    }

    pub fn save_history(&self, record: &crate::models::swap::SwapHistoryRecord) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let path = self.history_path(&id);
        let json = serde_json::to_string_pretty(record)?;
        std::fs::write(path, json)?;
        Ok(id)
    }

    pub fn load_history(&self, limit: u32) -> Result<Vec<crate::models::swap::SwapHistoryRecord>> {
        let mut records = Vec::new();

        let mut entries: Vec<_> = std::fs::read_dir(&self.history_dir)?
            .filter_map(|e| e.ok())
            .collect();

        entries.sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));

        for entry in entries.into_iter().take(limit as usize) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Ok(json) = std::fs::read_to_string(&path) {
                if let Ok(record) =
                    serde_json::from_str::<crate::models::swap::SwapHistoryRecord>(&json)
                {
                    records.push(record);
                }
            }
        }

        Ok(records)
    }

    fn quote_path(&self, quote_id: &str) -> PathBuf {
        self.quotes_dir.join(format!("{}.json", quote_id))
    }

    fn history_path(&self, id: &str) -> PathBuf {
        self.history_dir.join(format!("{}.json", id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::models::quote::{Quote, TokenRef};
    use crate::models::swap::{ExecutionResult, ExecutionStatus, SwapHistoryRecord};

    /// Build a QuoteStore backed by a unique temp directory.
    fn temp_store() -> (QuoteStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("chain_test_{}", Uuid::new_v4()));
        let config = AppConfig {
            rpc_url: "https://test.example.com".to_string(),
            rpc_url_overridden: false,
            chain_id: 1,
            private_key: None,
            wallet_address: None,
            dodo_api_url: String::new(),
            dodo_api_key: String::new(),
            dodo_project_id: String::new(),
            data_dir: dir.clone(),
        };
        let store = QuoteStore::new(&config).expect("create store");
        (store, dir)
    }

    fn make_quote(expires_in_secs: i64) -> Quote {
        let now = Utc::now();
        Quote {
            quote_id: Uuid::new_v4(),
            created_at: now,
            expires_at: now + chrono::Duration::seconds(expires_in_secs),
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
            dex_sources: vec!["DODO".to_string()],
            route_id: Some("r1".to_string()),
            router_to: "0xRouter".to_string(),
            calldata: "0xdata".to_string(),
            value: "0".to_string(),
            gas_limit: Some(200_000),
            estimated_gas: Some(150_000),
            estimated_gas_usd: None,
            raw_dodo_response: serde_json::json!({}),
            chain_id: 1,
            slippage: 1.0,
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let (store, _dir) = temp_store();
        let q = make_quote(300);
        let id = q.quote_id.to_string();

        store.save_quote(&q).unwrap();
        let loaded = store.load_quote(&id).unwrap().expect("quote should exist");

        assert_eq!(loaded.quote_id, q.quote_id);
        assert_eq!(loaded.from_token.symbol, "ETH");
        assert_eq!(loaded.to_token.symbol, "USDC");
        assert_eq!(loaded.from_amount_display, 1.0);
    }

    #[test]
    fn load_nonexistent_quote_returns_none() {
        let (store, _dir) = temp_store();
        let result = store
            .load_quote("00000000-0000-0000-0000-000000000000")
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn delete_removes_quote() {
        let (store, _dir) = temp_store();
        let q = make_quote(300);
        let id = q.quote_id.to_string();

        store.save_quote(&q).unwrap();
        store.delete_quote(&id).unwrap();

        let result = store.load_quote(&id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn delete_nonexistent_is_ok() {
        let (store, _dir) = temp_store();
        // Should not error
        store.delete_quote("no-such-id").unwrap();
    }

    #[test]
    fn cleanup_expired_removes_old_quotes() {
        let (store, _dir) = temp_store();

        let expired = make_quote(-10); // expired 10 seconds ago
        let fresh = make_quote(300); // expires in 5 minutes

        store.save_quote(&expired).unwrap();
        store.save_quote(&fresh).unwrap();

        let removed = store.cleanup_expired().unwrap();
        assert_eq!(removed, 1);

        // Fresh quote should still be loadable
        let loaded = store.load_quote(&fresh.quote_id.to_string()).unwrap();
        assert!(loaded.is_some());

        // Expired quote should be gone
        let path = store.quote_path(&expired.quote_id.to_string());
        assert!(!path.exists());
    }

    #[test]
    fn load_quote_auto_cleans_expired() {
        let (store, _dir) = temp_store();

        let expired = make_quote(-1);
        let id = expired.quote_id.to_string();

        store.save_quote(&expired).unwrap();

        // load_quote calls cleanup_expired internally before checking existence
        let result = store.load_quote(&id).unwrap();
        assert!(
            result.is_none(),
            "expired quote should be cleaned up on load"
        );
    }

    #[test]
    fn save_and_load_history() {
        let (store, _dir) = temp_store();

        let q = make_quote(300);
        let execution = ExecutionResult {
            quote_id: q.quote_id,
            executed_at: Utc::now(),
            dry_run: false,
            tx_hash: Some("0xTx".to_string()),
            status: ExecutionStatus::Confirmed,
            calldata: "0x".to_string(),
            to_contract: "0xRouter".to_string(),
            value_eth: "0".to_string(),
            from_address: Some("0xWallet".to_string()),
            gas_used: Some(120_000),
            effective_gas_price_gwei: Some(20.0),
        };
        let rec = SwapHistoryRecord::from_execution(&q, &execution, "hist-1".to_string());

        store.save_history(&rec).unwrap();

        let history = store.load_history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].from_token, "ETH");
        assert_eq!(history[0].to_token, "USDC");
        assert!(matches!(history[0].status, ExecutionStatus::Confirmed));
    }

    #[test]
    fn load_history_respects_limit() {
        let (store, _dir) = temp_store();

        for _ in 0..5 {
            let q = make_quote(300);
            let execution = ExecutionResult {
                quote_id: q.quote_id,
                executed_at: Utc::now(),
                dry_run: false,
                tx_hash: None,
                status: ExecutionStatus::Confirmed,
                calldata: "0x".to_string(),
                to_contract: "0xRouter".to_string(),
                value_eth: "0".to_string(),
                from_address: None,
                gas_used: None,
                effective_gas_price_gwei: None,
            };
            let rec = SwapHistoryRecord::from_execution(&q, &execution, Uuid::new_v4().to_string());
            store.save_history(&rec).unwrap();
        }

        let limited = store.load_history(3).unwrap();
        assert_eq!(limited.len(), 3);
    }
}
