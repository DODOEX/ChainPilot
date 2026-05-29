use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct WalletCmd {
    #[command(subcommand)]
    pub action: WalletAction,
}

#[derive(Subcommand)]
pub enum WalletAction {
    /// Aggregated wallet balance (total USD, assets, chain allocation)
    Balance(BalanceArgs),
    /// Wallet portfolio overview (chain/token allocation, active protocols, top holdings)
    Overview(OverviewArgs),
    /// Wallet PnL analysis (realized/unrealized gains, ROI, win rate)
    Pnl(PnlArgs),
    /// Wallet transaction history
    History(HistoryArgs),
}

#[derive(Args)]
pub struct BalanceArgs {
    /// Wallet address
    pub address: String,

    /// Hide assets worth less than this many USD (default: 1.0)
    #[arg(long, default_value_t = 1.0)]
    pub min_usd: f64,
}

#[derive(Args)]
pub struct OverviewArgs {
    /// Wallet address
    pub address: String,

    /// Limit on number of top holdings returned (default 5)
    #[arg(long, default_value_t = 5)]
    pub top: usize,
}

#[derive(Args)]
pub struct PnlArgs {
    /// Wallet address
    pub address: String,
}

#[derive(Args)]
pub struct HistoryArgs {
    /// Wallet address
    pub address: String,

    /// Max number of transactions to return (default 20, max 100)
    #[arg(long, default_value_t = 20)]
    pub limit: u32,
}
