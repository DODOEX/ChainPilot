use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 169,
    name: "Manta Pacific",
    rpc_urls: &["https://pacific-rpc.manta.network/http"],
    contracts: ChainContracts {
        dodo_approve: "0x0226fCE8c969604C3A0AD19c37d1FAFac73e13c2",
        erc20_v3_factory: Some("0xc0F9553Df63De5a97Fe64422c8578D0657C360f7"),
    },
    native_token: NativeToken {
        symbol: "ETH",
        name: "Ether",
        decimals: 18,
        wrapped_symbol: "WETH",
        wrapped_address: "0x0Dc808adcE2099A9F62AA87D9670745AbA741746",
        coingecko_id: "ethereum",
    },
};
