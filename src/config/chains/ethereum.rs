use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 1,
    name: "Ethereum Mainnet",
    rpc_urls: &[
        "https://ethereum-rpc.publicnode.com",
        "https://eth-mainnet.public.blastapi.io",
    ],
    contracts: ChainContracts {
        dodo_approve: "0xCB859eA579b28e02B87A1FDE08d087ab9dbE5149",
        erc20_v3_factory: Some("0x6a3B1CC74019e252a857ABBe9ee1B2f03EE1009f"),
    },
    native_token: NativeToken {
        symbol: "ETH",
        name: "Ether",
        decimals: 18,
        wrapped_symbol: "WETH",
        wrapped_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        coingecko_id: "ethereum",
    },
};
