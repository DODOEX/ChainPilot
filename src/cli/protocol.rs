use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct ProtocolCmd {
    #[command(subcommand)]
    pub action: ProtocolAction,
}

#[derive(Subcommand)]
pub enum ProtocolAction {
    /// Protocol overview
    Info(ProtocolArgs),
    /// Protocol TVL and TVL history
    Tvl(ProtocolTvlArgs),
    /// Protocol revenue and fee metrics
    Revenue(ProtocolArgs),
    /// Protocol chain distribution
    Chains(ProtocolArgs),
}

#[derive(Args)]
pub struct ProtocolArgs {
    /// DefiLlama protocol slug or protocol name
    pub protocol: String,
}

#[derive(Args)]
pub struct ProtocolTvlArgs {
    /// DefiLlama protocol slug or protocol name
    pub protocol: String,

    /// Maximum number of TVL history points to return (default 7, max 1000)
    #[arg(long, default_value_t = 7)]
    pub limit: u32,

    /// Number of newest TVL history points to skip before returning results
    #[arg(long, default_value_t = 0)]
    pub offset: u32,
}
