use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 138,
    name: "DeFi Oracle Meta Mainnet",
    rpc_urls: &["https://rpc.d-bis.org"],
    contracts: ChainContracts {
        dodo_approve: "0xEA5Be91d0A1EdA6a2efc80f7211c30584508D56D",
        erc20_v3_factory: Some("0x8Df0298a9CB839e89eA7d32918076a70467FBACE"),
    },
    native_token: NativeToken {
        symbol: "ETH",
        name: "Ether",
        decimals: 18,
        wrapped_symbol: "WETH",
        wrapped_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        coingecko_id: "ethereum",
    },
};
