use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 10,
    name: "Optimism",
    rpc_urls: &[
        "https://mainnet.optimism.io",
        "https://optimism-mainnet.public.blastapi.io",
    ],
    contracts: ChainContracts {
        dodo_approve: "0xa492d6eABcdc3E204676f15B950bBdD448080364",
        erc20_v3_factory: Some("0x5a274F1DeADf90FC844C9Ff94f79F9c1fF0E0aCC"),
    },
    native_token: NativeToken {
        symbol: "ETH",
        name: "Ether",
        decimals: 18,
        wrapped_symbol: "WETH",
        wrapped_address: "0x4200000000000000000000000000000000000006",
        coingecko_id: "ethereum",
    },
};
