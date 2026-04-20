use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 167000,
    name: "Taiko",
    rpc_urls: &["https://rpc.mainnet.taiko.xyz"],
    contracts: ChainContracts {
        dodo_approve: "0x2629E610dB4AC081c108cCDf8b19ED39D702df43",
        erc20_v3_factory: Some("0x4A81Aa47722E1cF27A8726F247E68a766bE58d8e"),
    },
    native_token: NativeToken {
        symbol: "ETH",
        name: "Ether",
        decimals: 18,
        wrapped_symbol: "WETH",
        wrapped_address: "0xA51894664A773981C6C112C43ce576f315d5b1B6",
    },
};
