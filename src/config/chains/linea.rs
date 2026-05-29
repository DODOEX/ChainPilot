use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 59144,
    name: "Linea",
    rpc_urls: &["https://rpc.linea.build"],
    contracts: ChainContracts {
        dodo_approve: "0x6de4d882a84A98f4CCD5D33ea6b3C99A07BAbeB1",
        erc20_v3_factory: Some("0x0226fCE8c969604C3A0AD19c37d1FAFac73e13c2"),
    },
    native_token: NativeToken {
        symbol: "ETH",
        name: "Ether",
        decimals: 18,
        wrapped_symbol: "WETH",
        wrapped_address: "0xe5D7C2a44FfDDf6b295A15c148167daaAf5Cf34f",
        coingecko_id: "ethereum",
    },
};
