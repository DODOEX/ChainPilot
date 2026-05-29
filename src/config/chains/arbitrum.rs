use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 42161,
    name: "Arbitrum One",
    rpc_urls: &[
        "https://arb1.arbitrum.io/rpc",
        "https://rpc.ankr.com/arbitrum",
    ],
    contracts: ChainContracts {
        dodo_approve: "0xA867241cDC8d3b0C07C85cC06F25a0cD3b5474d8",
        erc20_v3_factory: Some("0x5CA5e03228a3C16EF262d29E588Adb6c2445fDbC"),
    },
    native_token: NativeToken {
        symbol: "ETH",
        name: "Ethereum",
        decimals: 18,
        wrapped_symbol: "WETH",
        wrapped_address: "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
        coingecko_id: "ethereum",
    },
};
