use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 5000,
    name: "Mantle",
    rpc_urls: &["https://rpc.mantle.xyz"],
    contracts: ChainContracts {
        dodo_approve: "0xa71415675F68f29259ddD63215E5518d2735bf0a",
        erc20_v3_factory: Some("0xFD2b7994f91c08aAa5e013E899334A2DBb500DF1"),
    },
    native_token: NativeToken {
        symbol: "MNT",
        name: "Mantle",
        decimals: 18,
        wrapped_symbol: "WMNT",
        wrapped_address: "0x78c1b0C915c4FAA5FffA6CAbf0219DA63d7f4cb8",
        coingecko_id: "mantle",
    },
};
