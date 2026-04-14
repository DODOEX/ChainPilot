mod arbitrum;
mod aurora;
mod avalanche;
mod base;
mod bsc;
mod conflux;
mod ethereum;
mod linea;
mod manta;
mod mantle;
mod okchain;
mod optimism;
mod plume;
mod polygon;
mod scroll;
mod sepolia;
mod taiko;

/// Native (gas) token of a chain. All native tokens share the same sentinel address.
pub const NATIVE_ADDR: &str = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";

pub struct NativeToken {
    pub symbol: &'static str,
    #[allow(dead_code)]
    pub name: &'static str,
    pub decimals: u8,
    pub wrapped_symbol: &'static str,
    pub wrapped_address: &'static str,
}

pub struct ChainContracts {
    /// DODOApprove contract. All ERC-20 approvals for DODO swaps target this address.
    pub dodo_approve: &'static str,
}

pub struct ChainConfig {
    pub chain_id: u64,
    pub name: &'static str,
    /// Ordered RPC endpoints. The first is used as default when `ETH_RPC_URL` is not set.
    pub rpc_urls: &'static [&'static str],
    pub contracts: ChainContracts,
    pub native_token: NativeToken,
}

static CHAINS: &[&ChainConfig] = &[
    &ethereum::CONFIG,
    &bsc::CONFIG,
    &polygon::CONFIG,
    &arbitrum::CONFIG,
    &optimism::CONFIG,
    &avalanche::CONFIG,
    &base::CONFIG,
    &linea::CONFIG,
    &scroll::CONFIG,
    &manta::CONFIG,
    &mantle::CONFIG,
    &aurora::CONFIG,
    &okchain::CONFIG,
    &conflux::CONFIG,
    &taiko::CONFIG,
    &plume::CONFIG,
    &sepolia::CONFIG,
];

/// Look up the static config for a given chain ID. Returns `None` for unsupported chains.
pub fn chain_config(chain_id: u64) -> Option<&'static ChainConfig> {
    CHAINS.iter().copied().find(|c| c.chain_id == chain_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_chains_are_found() {
        let cases = [
            (1, "Ethereum"),
            (56, "BSC"),
            (137, "Polygon"),
            (42161, "Arbitrum"),
            (10, "Optimism"),
            (43114, "Avalanche"),
            (8453, "Base"),
            (59144, "Linea"),
            (534352, "Scroll"),
        ];
        for (chain_id, label) in cases {
            assert!(
                chain_config(chain_id).is_some(),
                "{label} (chain {chain_id}) not found"
            );
        }
    }

    #[test]
    fn unknown_chain_returns_none() {
        assert!(chain_config(0).is_none());
        assert!(chain_config(999999).is_none());
    }

    #[test]
    fn all_chains_have_at_least_one_rpc_url() {
        for c in CHAINS {
            assert!(
                !c.rpc_urls.is_empty(),
                "chain {} ({}) has no RPC URLs",
                c.name,
                c.chain_id
            );
            for url in c.rpc_urls {
                assert!(url.starts_with("https://"), "RPC URL not HTTPS: {url}");
            }
        }
    }

    #[test]
    fn all_chains_have_unique_ids() {
        let mut ids: Vec<u64> = CHAINS.iter().map(|c| c.chain_id).collect();
        let original_len = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), original_len, "duplicate chain IDs detected");
    }

    #[test]
    fn native_token_decimals_are_18() {
        for c in CHAINS {
            assert_eq!(
                c.native_token.decimals, 18,
                "chain {} native token has unexpected decimals",
                c.name
            );
        }
    }

    #[test]
    fn native_addr_sentinel_value() {
        assert_eq!(NATIVE_ADDR, "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE");
    }

    #[test]
    fn chain_config_name_matches_chain_id() {
        let eth = chain_config(1).unwrap();
        assert!(eth.name.contains("Ethereum"));

        let bsc = chain_config(56).unwrap();
        assert!(bsc.name.to_lowercase().contains("bsc") || bsc.name.to_lowercase().contains("bnb"));
    }
}
