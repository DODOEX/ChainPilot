use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 534352,
    name: "Scroll",
    rpc_urls: &["https://rpc.scroll.io"],
    contracts: ChainContracts {
        dodo_approve: "0x20E77aD760eC9E922Fd2dA8847ABFbB2471B92CD",
    },
    native_token: NativeToken {
        symbol: "ETH",
        name: "Ether",
        decimals: 18,
        wrapped_symbol: "WETH",
        wrapped_address: "0x5300000000000000000000000000000000000004",
        coingecko_id: "ethereum",
    },
};
