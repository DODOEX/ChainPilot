use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 98866,
    name: "Plume",
    rpc_urls: &["https://phoenix-rpc.plumenetwork.xyz"],
    contracts: ChainContracts {
        dodo_approve: "0x5977F12664b4E634dFbAAD0ad4a6a81057254dA8",
    },
    native_token: NativeToken {
        symbol: "PLUME",
        name: "PLUME",
        decimals: 18,
        wrapped_symbol: "WPLUME",
        wrapped_address: "0xEa237441c92CAe6FC17Caaf9a7acB3f953be4bd1",
    },
};
