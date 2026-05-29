use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 11155111,
    name: "Sepolia Testnet",
    rpc_urls: &["https://ethereum-sepolia-rpc.publicnode.com"],
    contracts: ChainContracts {
        dodo_approve: "0x66c45FF040e86DC613F239123A5E21FFdC3A3fEC",
        erc20_v3_factory: Some("0x3450dBC7094bB20065f430D98087e37708C1ddfE"),
    },
    native_token: NativeToken {
        symbol: "ETH",
        name: "Ether",
        decimals: 18,
        wrapped_symbol: "WETH",
        wrapped_address: "0x7B07164ecFaF0F0D85DFC062Bc205a4674c75Aa0",
    },
};
