use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 1313161554,
    name: "Aurora",
    rpc_urls: &["https://mainnet.aurora.dev"],
    contracts: ChainContracts {
        dodo_approve: "0x335aC99bb3E51BDbF22025f092Ebc1Cf2c5cC619",
    },
    native_token: NativeToken {
        symbol: "ETH",
        name: "Ethereum",
        decimals: 18,
        wrapped_symbol: "WETH",
        wrapped_address: "0xC9BdeEd33CD01541e1eeD10f90519d2C06Fe3feB",
        coingecko_id: "ethereum",
    },
};
