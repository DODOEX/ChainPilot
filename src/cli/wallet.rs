use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct WalletCmd {
    #[command(subcommand)]
    pub action: WalletAction,
}

#[derive(Subcommand)]
pub enum WalletAction {
    /// Aggregated wallet balance (total USD, assets, chain allocation). No API key required; falls back to on-chain native balance. Best with DEBANK_API_KEY, ZERION_API_KEY, or GOLDRUSH_API_KEY
    Balance(BalanceArgs),
    /// Wallet portfolio overview (chain/token allocation, active protocols, top holdings). Requires DEBANK_API_KEY, ZERION_API_KEY, or GOLDRUSH_API_KEY
    Overview(OverviewArgs),
    /// Wallet PnL analysis (realized/unrealized gains, ROI, win rate). Requires ZERION_API_KEY (no fallback)
    Pnl(PnlArgs),
    /// Wallet transaction history. Requires ZERION_API_KEY or DEBANK_API_KEY
    History(HistoryArgs),
    /// Wallet labels and behavioral tags. Requires DEBANK_API_KEY, DUNE_API_KEY, or ZERION_API_KEY
    Labels(LabelsArgs),
    /// DeFi positions across protocols (deposits, LPs, staking, borrows). Requires DEBANK_API_KEY or ZERION_API_KEY
    Defi(DefiArgs),
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

#[derive(Args)]
pub struct LabelsArgs {
    /// Wallet address
    pub address: String,
}

#[derive(Args)]
pub struct DefiArgs {
    /// Wallet address
    pub address: String,

    /// Hide positions worth less than this many USD (default: 1.0)
    #[arg(long, default_value_t = 1.0)]
    pub min_usd: f64,
}
