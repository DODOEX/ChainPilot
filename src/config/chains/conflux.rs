use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 1030,
    name: "Conflux eSpace",
    rpc_urls: &["https://evm.confluxrpc.com"],
    contracts: ChainContracts {
        dodo_approve: "0x5BaF16d57620Cb361F622232F3cb4090e35F3da2",
    },
    native_token: NativeToken {
        symbol: "CFX",
        name: "CFX",
        decimals: 18,
        wrapped_symbol: "WCFX",
        wrapped_address: "0x14b2d3bc65e74dae1030eafd8ac30c533c976a9b",
    },
};
