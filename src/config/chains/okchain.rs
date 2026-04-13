use super::{ChainConfig, ChainContracts, NativeToken};

pub static CONFIG: ChainConfig = ChainConfig {
    chain_id: 66,
    name: "OKX Chain (OKTC)",
    rpc_urls: &[
        "https://exchainrpc.okex.org",
        "https://okc-mainnet.gateway.pokt.network/v1/lb/6275309bea1b320039c893ff",
    ],
    contracts: ChainContracts {
        dodo_approve: "0x7737fd30535c69545deeEa54AB8Dd590ccaEBD3c",
    },
    native_token: NativeToken {
        symbol: "OKT",
        name: "OKT",
        decimals: 18,
        wrapped_symbol: "WOKT",
        wrapped_address: "0x8F8526dbfd6E38E3D8307702cA8469Bae6C56C15",
    },
};
