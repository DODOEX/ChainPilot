use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 8453,
    name: "Base",
    rpc_urls: &["https://mainnet.base.org"],
    contracts: ChainContracts {
        dodo_approve: "0x89872650fA1A391f58B4E144222bB02e44db7e3B",
        erc20_v3_factory: Some("0xCb3dC90E800C961d4a206BeAAFd92A6d2E06495e"),
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
