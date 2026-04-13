use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 137,
    name: "Polygon",
    rpc_urls: &[
        "https://polygon-rpc.com",
        "https://rpc-mainnet.maticvigil.com",
        "https://rpc.ankr.com/polygon",
    ],
    contracts: ChainContracts {
        dodo_approve: "0x6D310348d5c12009854DFCf72e0DF9027e8cb4f4",
    },
    native_token: NativeToken {
        symbol: "MATIC",
        name: "MATIC",
        decimals: 18,
        wrapped_symbol: "WMATIC",
        wrapped_address: "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270",
    },
};
