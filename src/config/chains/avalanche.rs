use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 43114,
    name: "Avalanche",
    rpc_urls: &[
        "https://api.avax.network/ext/bc/C/rpc",
        "https://rpc.ankr.com/avalanche",
        "https://ava-mainnet.public.blastapi.io/ext/bc/C/rpc",
    ],
    contracts: ChainContracts {
        dodo_approve: "0xCFea63e3DE31De53D68780Dd65675F169439e470",
    },
    native_token: NativeToken {
        symbol: "AVAX",
        name: "Avalanche",
        decimals: 18,
        wrapped_symbol: "WAVAX",
        wrapped_address: "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7",
        coingecko_id: "avalanche-2",
    },
};
