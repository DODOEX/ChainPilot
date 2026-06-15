use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct ChainCmd {
    #[command(subcommand)]
    pub action: ChainAction,
}

#[derive(Subcommand)]
pub enum ChainAction {
    /// Chain overview (chain ID, native token, price, TVL, fees, 24h activity)
    Info(ChainArgs),
    /// Chain stablecoin fund flows (net, inflow, outflow, per-coin breakdown)
    Flows(ChainArgs),
    /// Chain stablecoin supply and distribution
    Stablecoins(ChainArgs),
    /// Top protocols on the chain by TVL
    Protocols(ChainProtocolsArgs),
}

#[derive(Args)]
pub struct ChainArgs {
    /// Chain name (e.g. ethereum, base, arbitrum, bsc, solana/svm, bitcoin/bvm)
    pub chain: String,
}

#[derive(Args)]
pub struct ChainProtocolsArgs {
    /// Chain name (e.g. ethereum, base, arbitrum, bsc, solana/svm, bitcoin/bvm)
    pub chain: String,

    /// Maximum number of protocols to return (default 20, max 100)
    #[arg(long, default_value_t = 20)]
    pub limit: u32,
}
