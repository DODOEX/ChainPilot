use std::path::PathBuf;

use crate::error::Result;

pub mod chains;
pub use chains::{chain_config, ChainConfig};

pub const DEFAULT_CHAIN_ID: u64 = 1;
/// Fallback RPC used only when no chain config matches the active chain_id.
const FALLBACK_RPC_URL: &str = "https://ethereum-rpc.publicnode.com";
pub const DEFAULT_DODO_API_URL: &str = "https://api.dodoex.io/route-service/v2/widget/getdodoroute";

/// Compile-time default: set `DODO_API_KEY` at build time to bake a key into the binary.
/// Runtime `DODO_API_KEY` env var or `--dodo-api-key` CLI arg takes precedence.
pub const DEFAULT_DODO_API_KEY: &str = match option_env!("DODO_API_KEY") {
    Some(v) => v,
    None => "",
};

/// Compile-time default: set `DODO_PROJECT_ID` at build time to bake a project ID into the binary.
/// Runtime `DODO_PROJECT_ID` env var or `--dodo-project-id` CLI arg takes precedence.
pub const DEFAULT_DODO_PROJECT_ID: &str = match option_env!("DODO_PROJECT_ID") {
    Some(v) => v,
    None => "",
};

pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_QUOTE_TTL_SECS: u64 = 300;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub wallet_address: Option<String>,
    pub dodo_api_url: String,
    pub dodo_api_key: String,
    /// Project ID for the DODO tokenlist API (`/config-center/user/tokenlist/v2`).
    /// Set via `DODO_PROJECT_ID`. Without this, tokenlist lookup is skipped.
    pub dodo_project_id: String,
    pub request_timeout_secs: u64,
    pub quote_ttl_secs: u64,
    pub data_dir: PathBuf,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let chain_id: u64 = std::env::var("CHAIN_ID")
            .unwrap_or_else(|_| DEFAULT_CHAIN_ID.to_string())
            .parse()
            .unwrap_or(DEFAULT_CHAIN_ID);

        let rpc_url = std::env::var("ETH_RPC_URL").unwrap_or_else(|_| {
            chains::chain_config(chain_id)
                .and_then(|c| c.rpc_urls.first().copied())
                .unwrap_or(FALLBACK_RPC_URL)
                .to_string()
        });

        let wallet_address = std::env::var("WALLET_ADDRESS").ok();

        let data_dir = std::env::var("CHAIN_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_local_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("chain")
            });

        let dodo_api_url =
            std::env::var("DODO_API_URL").unwrap_or_else(|_| DEFAULT_DODO_API_URL.to_string());

        let dodo_api_key =
            std::env::var("DODO_API_KEY").unwrap_or_else(|_| DEFAULT_DODO_API_KEY.to_string());

        let dodo_project_id = std::env::var("DODO_PROJECT_ID")
            .unwrap_or_else(|_| DEFAULT_DODO_PROJECT_ID.to_string());

        let request_timeout_secs = std::env::var("REQUEST_TIMEOUT_SECS")
            .unwrap_or_else(|_| DEFAULT_REQUEST_TIMEOUT_SECS.to_string())
            .parse()
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS);

        let quote_ttl_secs = std::env::var("QUOTE_TTL_SECS")
            .unwrap_or_else(|_| DEFAULT_QUOTE_TTL_SECS.to_string())
            .parse()
            .unwrap_or(DEFAULT_QUOTE_TTL_SECS);

        Ok(Self {
            rpc_url,
            chain_id,
            wallet_address,
            dodo_api_url,
            dodo_api_key,
            dodo_project_id,
            request_timeout_secs,
            quote_ttl_secs,
            data_dir,
        })
    }

    /// Returns the static chain configuration for the active chain_id, if supported.
    pub fn chain_config(&self) -> Option<&'static ChainConfig> {
        chains::chain_config(self.chain_id)
    }

    /// Resolve the effective chain ID for a command.
    /// Precedence: CLI `--chain-id` > env/configured `CHAIN_ID` > default mainnet.
    pub fn effective_chain_id(&self, arg_chain_id: Option<u64>) -> u64 {
        arg_chain_id.unwrap_or(self.chain_id)
    }

    /// Resolve the static chain configuration for a command-scoped chain ID.
    pub fn chain_config_for(&self, arg_chain_id: Option<u64>) -> Option<&'static ChainConfig> {
        chains::chain_config(self.effective_chain_id(arg_chain_id))
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

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.quotes_dir())?;
        std::fs::create_dir_all(self.history_dir())?;
        Ok(())
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
        with_env(
            &[
                ("CHAIN_ID", Some("56")),
                ("ETH_RPC_URL", Some("https://bsc-rpc.example.com")),
            ],
            || {
                let cfg = AppConfig::load().unwrap();
                assert_eq!(cfg.chain_id, 56);
            },
        );
    }

    #[test]
    fn invalid_chain_id_falls_back_to_default() {
        with_env(&[("CHAIN_ID", Some("not_a_number"))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.chain_id, DEFAULT_CHAIN_ID);
        });
    }

    #[test]
    fn rpc_url_read_from_env() {
        let custom = "https://my-node.example.com";
        with_env(&[("ETH_RPC_URL", Some(custom))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.rpc_url, custom);
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
    fn rpc_url_falls_back_to_chain_config() {
        with_env(&[("ETH_RPC_URL", None), ("CHAIN_ID", Some("1"))], || {
            let cfg = AppConfig::load().unwrap();
            assert!(cfg.rpc_url.starts_with("https://"));
        });
    }

    #[test]
    fn rpc_url_falls_back_to_hardcoded_for_unknown_chain() {
        with_env(
            &[("ETH_RPC_URL", None), ("CHAIN_ID", Some("999999"))],
            || {
                let cfg = AppConfig::load().unwrap();
                assert_eq!(cfg.rpc_url, FALLBACK_RPC_URL);
            },
        );
    }

    #[test]
    fn invalid_timeout_falls_back_to_default() {
        with_env(
            &[
                ("REQUEST_TIMEOUT_SECS", Some("bad")),
                ("QUOTE_TTL_SECS", Some("also_bad")),
            ],
            || {
                let cfg = AppConfig::load().unwrap();
                assert_eq!(cfg.request_timeout_secs, DEFAULT_REQUEST_TIMEOUT_SECS);
                assert_eq!(cfg.quote_ttl_secs, DEFAULT_QUOTE_TTL_SECS);
            },
        );
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
    fn data_dir_read_from_env() {
        with_env(&[("CHAIN_DATA_DIR", Some("/tmp/chain_test_cfg"))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.data_dir, PathBuf::from("/tmp/chain_test_cfg"));
            assert_eq!(
                cfg.quotes_dir(),
                PathBuf::from("/tmp/chain_test_cfg/quotes")
            );
            assert_eq!(
                cfg.history_dir(),
                PathBuf::from("/tmp/chain_test_cfg/history")
            );
            assert_eq!(
                cfg.tokenlist_cache_path(),
                PathBuf::from("/tmp/chain_test_cfg/tokenlist_cache.json")
            );
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
    fn effective_chain_id_prefers_cli_arg() {
        with_env(&[("CHAIN_ID", Some("56"))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.effective_chain_id(Some(8453)), 8453);
        });
    }

    #[test]
    fn effective_chain_id_falls_back_to_config() {
        with_env(&[("CHAIN_ID", Some("56"))], || {
            let cfg = AppConfig::load().unwrap();
            assert_eq!(cfg.effective_chain_id(None), 56);
        });
    }
}
