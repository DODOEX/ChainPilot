use std::path::PathBuf;

use crate::error::Result;

pub mod chains;
pub use chains::{chain_config, ChainConfig};

pub const DEFAULT_CHAIN_ID: u64 = 1;
/// Fallback RPC used only when no chain config matches the active chain_id.
const FALLBACK_RPC_URL: &str = "https://ethereum-rpc.publicnode.com";
pub const DEFAULT_DODO_API_URL: &str = "https://api.dodoex.io/route-service/v2/widget/getdodoroute";
pub const DEFAULT_KEYSTORE_PASSWORD_ENV: &str = "KEYSTORE_PASSWORD";

/// Compile-time default: set `DODO_API_KEY` at build time to bake a key into the binary.
/// Runtime `DODO_API_KEY` env var takes precedence.
pub const DEFAULT_DODO_API_KEY: &str = match option_env!("DODO_API_KEY") {
    Some(v) => v,
    None => "",
};

/// Compile-time default: set `DODO_PROJECT_ID` at build time to bake a project ID into the binary.
/// Runtime `DODO_PROJECT_ID` env var takes precedence.
pub const DEFAULT_DODO_PROJECT_ID: &str = match option_env!("DODO_PROJECT_ID") {
    Some(v) => v,
    None => "",
};

pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_QUOTE_TTL_SECS: u64 = 1080;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub rpc_url: String,
    pub rpc_url_overridden: bool,
    pub chain_id: u64,
    pub private_key: Option<String>,
    pub keystore_path: Option<String>,
    pub keystore_password_file: Option<String>,
    pub keystore_password_env: Option<String>,
    pub wallet_address: Option<String>,
    pub dodo_api_url: String,
    pub dodo_api_key: String,
    /// Project ID for the DODO tokenlist API (`/config-center/user/tokenlist/v2`).
    /// Set via `DODO_PROJECT_ID`. Without this, tokenlist lookup is skipped.
    pub dodo_project_id: String,
    pub data_dir: PathBuf,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let chain_id: u64 = std::env::var("CHAIN_ID")
            .unwrap_or_else(|_| DEFAULT_CHAIN_ID.to_string())
            .parse()
            .unwrap_or(DEFAULT_CHAIN_ID);

        let rpc_url_overridden = false;
        let rpc_url = {
            chains::chain_config(chain_id)
                .and_then(|c| c.rpc_urls.first().copied())
                .unwrap_or(FALLBACK_RPC_URL)
                .to_string()
        };

        let private_key = std::env::var("PRIVATE_KEY").ok();
        let keystore_path = std::env::var("KEYSTORE_PATH").ok();
        let keystore_password_file = std::env::var("KEYSTORE_PASSWORD_FILE").ok();
        let keystore_password_env = std::env::var("KEYSTORE_PASSWORD_ENV").ok().or_else(|| {
            std::env::var(DEFAULT_KEYSTORE_PASSWORD_ENV)
                .ok()
                .map(|_| DEFAULT_KEYSTORE_PASSWORD_ENV.to_string())
        });
        let wallet_address = std::env::var("WALLET_ADDRESS").ok();

        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("chain");

        let dodo_api_url =
            std::env::var("DODO_API_URL").unwrap_or_else(|_| DEFAULT_DODO_API_URL.to_string());

        let dodo_api_key =
            std::env::var("DODO_API_KEY").unwrap_or_else(|_| DEFAULT_DODO_API_KEY.to_string());

        let dodo_project_id = std::env::var("DODO_PROJECT_ID")
            .unwrap_or_else(|_| DEFAULT_DODO_PROJECT_ID.to_string());

        Ok(Self {
            rpc_url,
            rpc_url_overridden,
            chain_id,
            private_key,
            keystore_path,
            keystore_password_file,
            keystore_password_env,
            wallet_address,
            dodo_api_url,
            dodo_api_key,
            dodo_project_id,
            data_dir,
        })
    }

    /// Returns the static chain configuration for the active chain_id, if supported.
    #[cfg(test)]
    pub fn chain_config(&self) -> Option<&'static ChainConfig> {
        chains::chain_config(self.chain_id)
    }

    /// Resolve the static chain configuration for a specific chain ID.
    pub fn chain_config_for(&self, chain_id: u64) -> Option<&'static ChainConfig> {
        chains::chain_config(chain_id)
    }

    /// Resolve the RPC URL for a target chain.
    /// Precedence: explicit CLI/env override > chain default RPC > configured fallback RPC.
    pub fn rpc_url_for_chain(&self, chain_id: u64) -> String {
        if self.rpc_url_overridden || chain_id == self.chain_id {
            return self.rpc_url.clone();
        }

        chains::chain_config(chain_id)
            .and_then(|c| c.rpc_urls.first().copied())
            .unwrap_or(self.rpc_url.as_str())
            .to_string()
    }

    /// Update the active chain while preserving explicit RPC overrides.
    pub fn set_chain_id(&mut self, chain_id: u64) {
        if !self.rpc_url_overridden {
            self.rpc_url = self.rpc_url_for_chain(chain_id);
        }
        self.chain_id = chain_id;
    }

    pub fn quotes_dir(&self) -> PathBuf {
        self.data_dir.join("quotes")
    }

    pub fn history_dir(&self) -> PathBuf {
        self.data_dir.join("history")
    }

    pub fn tokenlist_cache_path(&self) -> PathBuf {
        self.data_dir.join("tokenlist_cache.json")
    }

    pub fn custom_tokens_path(&self) -> PathBuf {
        self.data_dir.join("custom_tokens.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env-var tests mutate process-global state; this mutex serialises them.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Set env vars for the duration of `f`, then restore originals.
    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved: Vec<(&str, Option<String>)> = vars
            .iter()
            .map(|(k, _)| (*k, std::env::var(k).ok()))
            .collect();
        for (k, v) in vars {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
        f();
        for (k, v) in &saved {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }

    #[test]
    fn default_chain_id_is_1() {
        with_env(&[("CHAIN_ID", None)], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.chain_id, DEFAULT_CHAIN_ID);
        });
    }

    #[test]
    fn chain_id_read_from_env() {
        with_env(&[("CHAIN_ID", Some("56"))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.chain_id, 56);
        });
    }

    #[test]
    fn invalid_chain_id_falls_back_to_default() {
        with_env(&[("CHAIN_ID", Some("not_a_number"))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.chain_id, DEFAULT_CHAIN_ID);
        });
    }

    #[test]
    fn wallet_address_read_from_env() {
        let wallet = "0x1111111111111111111111111111111111111111";
        with_env(&[("WALLET_ADDRESS", Some(wallet))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.wallet_address.as_deref(), Some(wallet));
        });
    }

    #[test]
    fn rpc_url_defaults_to_chain_config() {
        with_env(&[("CHAIN_ID", Some("1"))], || {
            let cfg = AppConfig::load().unwrap();
            assert!(cfg.rpc_url.starts_with("https://"));
            assert!(!cfg.rpc_url_overridden);
        });
    }

    #[test]
    fn rpc_url_falls_back_to_hardcoded_for_unknown_chain() {
        with_env(&[("CHAIN_ID", Some("999999"))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.rpc_url, FALLBACK_RPC_URL);
        });
    }

    #[test]
    fn private_key_read_from_env() {
        let private_key = "0xabc123";
        with_env(&[("PRIVATE_KEY", Some(private_key))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.private_key.as_deref(), Some(private_key));
        });
    }

    #[test]
    fn keystore_fields_read_from_env() {
        with_env(
            &[
                ("KEYSTORE_PATH", Some("/tmp/test.keystore")),
                ("KEYSTORE_PASSWORD_FILE", Some("/tmp/test.pass")),
                ("KEYSTORE_PASSWORD_ENV", Some("CHAINPILOT_PASS")),
            ],
            || {
                let cfg = AppConfig::load().unwrap();
                assert_eq!(cfg.keystore_path.as_deref(), Some("/tmp/test.keystore"));
                assert_eq!(
                    cfg.keystore_password_file.as_deref(),
                    Some("/tmp/test.pass")
                );
                assert_eq!(
                    cfg.keystore_password_env.as_deref(),
                    Some("CHAINPILOT_PASS")
                );
            },
        );
    }

    #[test]
    fn default_keystore_password_env_is_detected_when_value_present() {
        with_env(&[(DEFAULT_KEYSTORE_PASSWORD_ENV, Some("secret"))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(
                cfg.keystore_password_env.as_deref(),
                Some(DEFAULT_KEYSTORE_PASSWORD_ENV)
            );
        });
    }

    #[test]
    fn rpc_url_for_chain_prefers_explicit_override() {
        with_env(&[("CHAIN_ID", Some("1"))], || {
            let mut cfg = AppConfig::load().unwrap();
            cfg.rpc_url = "https://override.example.com".to_string();
            cfg.rpc_url_overridden = true;
            assert_eq!(cfg.rpc_url_for_chain(8453), "https://override.example.com");
        });
    }

    #[test]
    fn rpc_url_for_chain_uses_target_chain_default_without_override() {
        with_env(&[("CHAIN_ID", Some("1"))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.rpc_url_for_chain(8453), "https://mainnet.base.org");
        });
    }

    #[test]
    fn set_chain_id_updates_rpc_url_when_not_overridden() {
        with_env(&[("CHAIN_ID", Some("1"))], || {
            let mut cfg = AppConfig::load().unwrap();
            cfg.set_chain_id(11155111);
            assert_eq!(cfg.chain_id, 11155111);
            assert_eq!(cfg.rpc_url, "https://ethereum-sepolia-rpc.publicnode.com");
        });
    }

    #[test]
    fn set_chain_id_preserves_explicit_rpc_override() {
        with_env(&[("CHAIN_ID", Some("1"))], || {
            let mut cfg = AppConfig::load().unwrap();
            cfg.rpc_url = "https://override.example.com".to_string();
            cfg.rpc_url_overridden = true;
            cfg.set_chain_id(11155111);
            assert_eq!(cfg.chain_id, 11155111);
            assert_eq!(cfg.rpc_url, "https://override.example.com");
        });
    }

    #[test]
    fn timeout_and_ttl_use_defaults() {
        with_env(&[], || {
            assert_eq!(DEFAULT_REQUEST_TIMEOUT_SECS, 30);
            assert_eq!(DEFAULT_QUOTE_TTL_SECS, 1080);
        });
    }

    #[test]
    fn dodo_fields_read_from_env() {
        with_env(
            &[
                ("DODO_API_URL", Some("https://custom-dodo.example.com")),
                ("DODO_API_KEY", Some("my-key")),
                ("DODO_PROJECT_ID", Some("proj-42")),
            ],
            || {
                let cfg = AppConfig::load().unwrap();
                assert_eq!(cfg.dodo_api_url, "https://custom-dodo.example.com");
                assert_eq!(cfg.dodo_api_key, "my-key");
                assert_eq!(cfg.dodo_project_id, "proj-42");
            },
        );
    }

    #[test]
    fn data_dir_defaults_to_local_chain_dir() {
        with_env(&[], || {
            let cfg = AppConfig::load().unwrap();
            assert!(cfg.quotes_dir().ends_with("chain/quotes"));
            assert!(cfg.history_dir().ends_with("chain/history"));
            assert!(cfg
                .tokenlist_cache_path()
                .ends_with("chain/tokenlist_cache.json"));
        });
    }

    #[test]
    fn chain_config_method_returns_static_config() {
        with_env(&[("CHAIN_ID", Some("1"))], || {
            let cfg = AppConfig::load().unwrap();
            let cc = cfg.chain_config().unwrap();
            assert_eq!(cc.chain_id, 1);
        });
    }

    #[test]
    fn chain_config_method_returns_none_for_unknown() {
        with_env(&[("CHAIN_ID", Some("999999"))], || {
            let cfg = AppConfig::load().unwrap();
            assert!(cfg.chain_config().is_none());
        });
    }

    #[test]
    fn chain_config_for_returns_requested_chain() {
        with_env(&[("CHAIN_ID", Some("56"))], || {
            let cfg = AppConfig::load().unwrap();
            let cc = cfg.chain_config_for(8453).unwrap();
            assert_eq!(cc.chain_id, 8453);
        });
    }
}
