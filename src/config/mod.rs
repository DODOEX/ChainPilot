use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::Result;

pub mod chains;
pub use chains::{chain_config, ChainConfig};

pub const DEFAULT_CHAIN_ID: u64 = 1;
/// Fallback RPC used only when no chain config matches the active chain_id.
const FALLBACK_RPC_URL: &str = "https://ethereum-rpc.publicnode.com";
pub const DEFAULT_DODO_API_URL: &str = "https://api.dodoex.io/route-service/v2/widget/getdodoroute";
pub const DEFAULT_COINGECKO_API_URL: &str = "https://api.coingecko.com/api/v3";
pub const DEFAULT_DEXSCREENER_API_URL: &str = "https://api.dexscreener.com/latest/dex";
pub const DEFAULT_DEBANK_API_URL: &str = "https://pro-openapi.debank.com/v1";
pub const DEFAULT_GOLDRUSH_API_URL: &str = "https://api.covalenthq.com/v1";
pub const DEFAULT_ZERION_API_URL: &str = "https://api.zerion.io/v1";
pub const DEFAULT_DUNE_API_URL: &str = "https://api.dune.com/api/v1";
pub const DEFAULT_DEFILLAMA_API_URL: &str = "https://api.llama.fi";
/// DefiLlama serves stablecoin data from a separate host, not `api.llama.fi`.
pub const DEFAULT_DEFILLAMA_STABLECOINS_API_URL: &str = "https://stablecoins.llama.fi";
/// growthepie open API: per-chain activity (active addresses, tx count, throughput).
pub const DEFAULT_GROWTHEPIE_API_URL: &str = "https://api.growthepie.xyz";
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
    /// Per-chain RPC overrides configured via `RPC_URL_<chainId>` env vars
    /// (persisted in `config.env`). Lower precedence than the `--rpc-url` flag,
    /// higher than the chain's built-in default RPC.
    pub rpc_overrides: HashMap<u64, String>,
    pub chain_id: u64,
    pub chain_id_overridden: bool,
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
    pub coingecko_api_url: String,
    pub coingecko_api_key: Option<String>,
    pub dexscreener_api_url: String,
    pub debank_api_url: String,
    pub debank_api_key: String,
    pub goldrush_api_url: String,
    pub goldrush_api_key: String,
    pub zerion_api_url: String,
    pub zerion_api_key: String,
    pub dune_api_url: String,
    pub dune_api_key: String,
    pub data_dir: PathBuf,
}

/// Prefix for per-chain RPC override env vars, e.g. `RPC_URL_56`.
pub const RPC_URL_ENV_PREFIX: &str = "RPC_URL_";

/// The chain's built-in default RPC, or the global fallback for unknown chains.
fn chain_default_rpc(chain_id: u64) -> &'static str {
    chains::chain_config(chain_id)
        .and_then(|c| c.rpc_urls.first().copied())
        .unwrap_or(FALLBACK_RPC_URL)
}

/// Scan the environment for `RPC_URL_<chainId>` entries and build a chain_id → url map.
/// Keys whose suffix is not a valid `u64` (e.g. a bare `RPC_URL`) are skipped.
fn collect_rpc_overrides() -> HashMap<u64, String> {
    std::env::vars()
        .filter_map(|(key, value)| {
            let suffix = key.strip_prefix(RPC_URL_ENV_PREFIX)?;
            let chain_id: u64 = suffix.parse().ok()?;
            let value = value.trim();
            if value.is_empty() {
                return None;
            }
            Some((chain_id, value.to_string()))
        })
        .collect()
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let chain_id_raw = std::env::var("CHAIN_ID").ok();
        let chain_id: u64 = chain_id_raw
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_CHAIN_ID);
        // Treat env var as explicit only when it parses to a real id;
        // a garbage value is the same as "not set" in our precedence model.
        let chain_id_overridden = chain_id_raw
            .as_deref()
            .is_some_and(|s| s.parse::<u64>().is_ok());

        // Collect per-chain RPC overrides from `RPC_URL_<chainId>` env vars.
        // These come from `config.env` (loaded into the environment at startup) or
        // a real env var. `RPC_URL` without a numeric suffix is intentionally ignored:
        // there is no single "all chains" RPC — that role belongs to the `--rpc-url` flag.
        let rpc_overrides = collect_rpc_overrides();

        let rpc_url_overridden = false;
        let rpc_url = rpc_overrides
            .get(&chain_id)
            .cloned()
            .unwrap_or_else(|| chain_default_rpc(chain_id).to_string());

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

        let coingecko_api_url = std::env::var("COINGECKO_API_URL")
            .unwrap_or_else(|_| DEFAULT_COINGECKO_API_URL.to_string());
        let coingecko_api_key = std::env::var("COINGECKO_API_KEY").ok();
        let dexscreener_api_url = std::env::var("DEXSCREENER_API_URL")
            .unwrap_or_else(|_| DEFAULT_DEXSCREENER_API_URL.to_string());
        let debank_api_url =
            std::env::var("DEBANK_API_URL").unwrap_or_else(|_| DEFAULT_DEBANK_API_URL.to_string());
        let debank_api_key = std::env::var("DEBANK_API_KEY").unwrap_or_default();
        let goldrush_api_url = std::env::var("GOLDRUSH_API_URL")
            .unwrap_or_else(|_| DEFAULT_GOLDRUSH_API_URL.to_string());
        let goldrush_api_key = std::env::var("GOLDRUSH_API_KEY").unwrap_or_default();
        let zerion_api_url =
            std::env::var("ZERION_API_URL").unwrap_or_else(|_| DEFAULT_ZERION_API_URL.to_string());
        let zerion_api_key = std::env::var("ZERION_API_KEY").unwrap_or_default();
        let dune_api_url =
            std::env::var("DUNE_API_URL").unwrap_or_else(|_| DEFAULT_DUNE_API_URL.to_string());
        let dune_api_key = std::env::var("DUNE_API_KEY").unwrap_or_default();
        Ok(Self {
            rpc_url,
            rpc_url_overridden,
            chain_id,
            chain_id_overridden,
            rpc_overrides,
            private_key,
            keystore_path,
            keystore_password_file,
            keystore_password_env,
            wallet_address,
            dodo_api_url,
            dodo_api_key,
            dodo_project_id,
            coingecko_api_url,
            coingecko_api_key,
            dexscreener_api_url,
            debank_api_url,
            debank_api_key,
            goldrush_api_url,
            goldrush_api_key,
            zerion_api_url,
            zerion_api_key,
            dune_api_url,
            dune_api_key,
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
    /// Precedence: `--rpc-url` flag > configured `RPC_URL_<chainId>` > chain default RPC
    /// > current `rpc_url` fallback.
    pub fn rpc_url_for_chain(&self, chain_id: u64) -> String {
        // The `--rpc-url` flag wins for every chain.
        if self.rpc_url_overridden {
            return self.rpc_url.clone();
        }
        // The active chain's `rpc_url` was already resolved (override-or-default) in `load`.
        if chain_id == self.chain_id {
            return self.rpc_url.clone();
        }
        // Per-chain configured override, then the chain's built-in default.
        if let Some(url) = self.rpc_overrides.get(&chain_id) {
            return url.clone();
        }
        chains::chain_config(chain_id)
            .and_then(|c| c.rpc_urls.first().copied())
            .unwrap_or(self.rpc_url.as_str())
            .to_string()
    }

    /// Update the active chain while preserving explicit RPC overrides.
    /// Marks the chain id as explicitly set by the caller.
    pub fn set_chain_id(&mut self, chain_id: u64) {
        if !self.rpc_url_overridden {
            self.rpc_url = self.rpc_url_for_chain(chain_id);
        }
        self.chain_id = chain_id;
        self.chain_id_overridden = true;
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

    pub fn config_env_path(&self) -> PathBuf {
        self.data_dir.join("config.env")
    }
}

#[cfg(test)]
pub fn test_metadata_config_fields() -> TestMetadataConfigFields {
    TestMetadataConfigFields {
        coingecko_api_url: DEFAULT_COINGECKO_API_URL.to_string(),
        coingecko_api_key: None,
        dexscreener_api_url: DEFAULT_DEXSCREENER_API_URL.to_string(),
        debank_api_url: DEFAULT_DEBANK_API_URL.to_string(),
        debank_api_key: String::new(),
        goldrush_api_url: DEFAULT_GOLDRUSH_API_URL.to_string(),
        goldrush_api_key: String::new(),
        zerion_api_url: DEFAULT_ZERION_API_URL.to_string(),
        zerion_api_key: String::new(),
        dune_api_url: DEFAULT_DUNE_API_URL.to_string(),
        dune_api_key: String::new(),
    }
}

#[cfg(test)]
pub struct TestMetadataConfigFields {
    pub coingecko_api_url: String,
    pub coingecko_api_key: Option<String>,
    pub dexscreener_api_url: String,
    pub debank_api_url: String,
    pub debank_api_key: String,
    pub goldrush_api_url: String,
    pub goldrush_api_key: String,
    pub zerion_api_url: String,
    pub zerion_api_key: String,
    pub dune_api_url: String,
    pub dune_api_key: String,
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
            assert!(!cfg.chain_id_overridden);
        });
    }

    #[test]
    fn chain_id_read_from_env() {
        with_env(&[("CHAIN_ID", Some("56"))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.chain_id, 56);
            assert!(cfg.chain_id_overridden);
        });
    }

    #[test]
    fn invalid_chain_id_falls_back_to_default() {
        with_env(&[("CHAIN_ID", Some("not_a_number"))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.chain_id, DEFAULT_CHAIN_ID);
            assert!(!cfg.chain_id_overridden);
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
            assert!(cfg.chain_id_overridden);
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
    fn configured_rpc_override_sets_active_chain_rpc() {
        with_env(
            &[
                ("CHAIN_ID", Some("1")),
                ("RPC_URL_1", Some("https://my-eth-node.example")),
            ],
            || {
                let cfg = AppConfig::load().unwrap();
                assert_eq!(cfg.rpc_url, "https://my-eth-node.example");
                assert!(!cfg.rpc_url_overridden);
                assert_eq!(
                    cfg.rpc_overrides.get(&1).map(String::as_str),
                    Some("https://my-eth-node.example")
                );
            },
        );
    }

    #[test]
    fn configured_rpc_override_used_for_non_active_chain() {
        with_env(
            &[
                ("CHAIN_ID", Some("1")),
                ("RPC_URL_56", Some("https://my-bsc-node.example")),
                ("RPC_URL_1", None),
            ],
            || {
                let cfg = AppConfig::load().unwrap();
                // Active chain keeps its built-in default...
                assert!(cfg.rpc_url.starts_with("https://"));
                assert_ne!(cfg.rpc_url, "https://my-bsc-node.example");
                // ...while chain 56 resolves to the configured override.
                assert_eq!(cfg.rpc_url_for_chain(56), "https://my-bsc-node.example");
            },
        );
    }

    #[test]
    fn explicit_rpc_flag_beats_configured_override() {
        with_env(
            &[
                ("CHAIN_ID", Some("1")),
                ("RPC_URL_56", Some("https://my-bsc-node.example")),
            ],
            || {
                let mut cfg = AppConfig::load().unwrap();
                cfg.rpc_url = "https://flag-override.example".to_string();
                cfg.rpc_url_overridden = true;
                // The flag wins for every chain, including one with a configured override.
                assert_eq!(cfg.rpc_url_for_chain(56), "https://flag-override.example");
            },
        );
    }

    #[test]
    fn configured_rpc_override_resolves_unknown_chain() {
        with_env(
            &[
                ("CHAIN_ID", Some("1")),
                ("RPC_URL_999999", Some("https://custom-unknown.example")),
            ],
            || {
                let cfg = AppConfig::load().unwrap();
                assert_eq!(
                    cfg.rpc_url_for_chain(999999),
                    "https://custom-unknown.example"
                );
            },
        );
    }

    #[test]
    fn bare_rpc_url_env_is_ignored_as_override() {
        with_env(
            &[
                ("CHAIN_ID", Some("1")),
                ("RPC_URL_1", None),
                ("RPC_URL", Some("https://should-be-ignored.example")),
            ],
            || {
                let cfg = AppConfig::load().unwrap();
                // No numeric suffix means no per-chain override; falls back to chain default.
                assert!(cfg.rpc_overrides.get(&1).is_none());
                assert!(cfg.rpc_url.starts_with("https://"));
                assert_ne!(cfg.rpc_url, "https://should-be-ignored.example");
            },
        );
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
    fn token_metadata_fields_read_from_env() {
        with_env(
            &[
                ("COINGECKO_API_URL", Some("https://cg.example.com")),
                ("COINGECKO_API_KEY", Some("cg-key")),
                ("DEXSCREENER_API_URL", Some("https://dex.example.com")),
            ],
            || {
                let cfg = AppConfig::load().unwrap();
                assert_eq!(cfg.coingecko_api_url, "https://cg.example.com");
                assert_eq!(cfg.coingecko_api_key.as_deref(), Some("cg-key"));
                assert_eq!(cfg.dexscreener_api_url, "https://dex.example.com");
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
