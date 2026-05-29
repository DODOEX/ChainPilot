use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 56,
    name: "BNB Smart Chain",
    rpc_urls: &[
        "https://bsc-dataseed1.binance.org",
        "https://bsc-dataseed2.binance.org",
        "https://bsc-dataseed3.binance.org",
    ],
    contracts: ChainContracts {
        dodo_approve: "0xa128Ba44B2738A558A1fdC06d6303d52D3Cef8c1",
        erc20_v3_factory: Some("0x5eD22a48Fee263207f5cAAE1A1f1a27011e7Dc92"),
    },
    native_token: NativeToken {
        symbol: "BNB",
        name: "BNB",
        decimals: 18,
        wrapped_symbol: "WBNB",
        wrapped_address: "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c",
    },
};
