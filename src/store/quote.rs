use chrono::Utc;
use std::path::PathBuf;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::error::Result;
use crate::models::quote::Quote;
use crate::models::token::{CustomTokenRecord, TokenInfo};
use serde::{Deserialize, Serialize};

pub struct QuoteStore {
    quotes_dir: PathBuf,
    history_dir: PathBuf,
    custom_tokens_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CustomTokenStoreFile {
    tokens: Vec<CustomTokenRecord>,
}

impl QuoteStore {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let quotes_dir = config.quotes_dir();
        let history_dir = config.history_dir();
        let custom_tokens_path = config.custom_tokens_path();
        std::fs::create_dir_all(&quotes_dir)?;
        std::fs::create_dir_all(&history_dir)?;

        Ok(Self {
            quotes_dir,
            history_dir,
            custom_tokens_path,
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

    pub fn save_custom_token_info(&self, info: &TokenInfo) -> Result<CustomTokenRecord> {
        let record = CustomTokenRecord::from_token_info(info);
        self.save_custom_token(&record)?;
        Ok(record)
    }

    pub fn save_custom_token(&self, token: &CustomTokenRecord) -> Result<()> {
        let mut file = self.load_custom_tokens_file()?;
        file.tokens.retain(|existing| {
            !(existing.chain_id == token.chain_id
                && existing.address.eq_ignore_ascii_case(&token.address))
        });
        file.tokens.push(token.clone());
        self.save_custom_tokens_file(&file)
    }

    pub fn find_custom_token_by_symbol(
        &self,
        chain_id: u64,
        symbol: &str,
    ) -> Result<Option<crate::models::quote::TokenRef>> {
        let upper = symbol.to_uppercase();
        let file = self.load_custom_tokens_file()?;
        Ok(file
            .tokens
            .iter()
            .rev()
            .find(|token| token.chain_id == chain_id && token.symbol.to_uppercase() == upper)
            .map(|token| crate::models::quote::TokenRef {
                symbol: token.symbol.clone(),
                address: token.address.clone(),
                decimals: token.decimals,
                chain_id: token.chain_id,
            }))
    }

    fn load_custom_tokens_file(&self) -> Result<CustomTokenStoreFile> {
        if !self.custom_tokens_path.exists() {
            return Ok(CustomTokenStoreFile::default());
        }
        let json = std::fs::read_to_string(&self.custom_tokens_path)?;
        Ok(serde_json::from_str(&json)?)
    }

    fn save_custom_tokens_file(&self, file: &CustomTokenStoreFile) -> Result<()> {
        let json = serde_json::to_string_pretty(file)?;
        std::fs::write(&self.custom_tokens_path, json)?;
        Ok(())
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
    use crate::models::token::TokenInfo;

    /// Build a QuoteStore backed by a unique temp directory.
    fn temp_store() -> (QuoteStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("chain_test_{}", Uuid::new_v4()));
        let m = crate::config::test_metadata_config_fields();
        let config = AppConfig {
            rpc_url: "https://test.example.com".to_string(),
            rpc_url_overridden: false,
            chain_id: 1,
            chain_id_overridden: false,
            private_key: None,
            keystore_path: None,
            keystore_password_file: None,
            keystore_password_env: None,
            wallet_address: None,
            dodo_api_url: String::new(),
            dodo_api_key: String::new(),
            dodo_project_id: String::new(),
            coingecko_api_url: m.coingecko_api_url,
            coingecko_api_key: m.coingecko_api_key,
            dexscreener_api_url: m.dexscreener_api_url,
            debank_api_url: m.debank_api_url,
            debank_api_key: m.debank_api_key,
            goldrush_api_url: m.goldrush_api_url,
            goldrush_api_key: m.goldrush_api_key,
            zerion_api_url: m.zerion_api_url,
            zerion_api_key: m.zerion_api_key,
            dune_api_url: m.dune_api_url,
            dune_api_key: m.dune_api_key,
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

    fn make_custom_token_info(
        chain_id: u64,
        address: &str,
        symbol: &str,
        name: &str,
        decimals: u8,
    ) -> TokenInfo {
        TokenInfo {
            address: address.to_string(),
            symbol: symbol.to_string(),
            name: name.to_string(),
            decimals,
            chain_id,
            chain: None,
            website: None,
            social_links: crate::models::token::TokenSocialLinks::default(),
            price: None,
            market_cap: None,
            fdv: None,
            top_liquidity: None,
            volume_24h: None,
            price_change_24h: None,
            risk_level: None,
            sources: crate::models::token::TokenInfoSources::default(),
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

    #[test]
    fn custom_token_roundtrip_and_latest_symbol_wins() {
        let (store, _dir) = temp_store();

        let older = make_custom_token_info(
            1,
            "0x1111111111111111111111111111111111111111",
            "USDC",
            "USD Coin Old",
            6,
        );
        let newer = make_custom_token_info(
            1,
            "0x2222222222222222222222222222222222222222",
            "USDC",
            "USD Coin New",
            6,
        );

        store.save_custom_token_info(&older).unwrap();
        store.save_custom_token_info(&newer).unwrap();

        let resolved = store
            .find_custom_token_by_symbol(1, "usdc")
            .unwrap()
            .unwrap();
        assert_eq!(resolved.address, newer.address);
        assert_eq!(resolved.symbol, "USDC");
        assert_eq!(resolved.decimals, 6);
    }

    #[test]
    fn custom_token_upsert_replaces_same_address() {
        let (store, _dir) = temp_store();
        let first = make_custom_token_info(
            1,
            "0x3333333333333333333333333333333333333333",
            "ABC",
            "Token ABC",
            18,
        );
        let second = make_custom_token_info(
            1,
            "0x3333333333333333333333333333333333333333",
            "ABCD",
            "Token ABCD",
            18,
        );

        store.save_custom_token_info(&first).unwrap();
        store.save_custom_token_info(&second).unwrap();

        assert!(store
            .find_custom_token_by_symbol(1, "ABC")
            .unwrap()
            .is_none());
        let updated = store
            .find_custom_token_by_symbol(1, "ABCD")
            .unwrap()
            .unwrap();
        assert_eq!(updated.address, second.address);
    }
}
