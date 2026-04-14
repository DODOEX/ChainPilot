use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct WalletCmd {
    #[command(subcommand)]
    pub action: WalletAction,
}

#[derive(Subcommand)]
pub enum WalletAction {
    /// ETH and token balances
    Balance(BalanceArgs),
}

#[derive(Args)]
pub struct BalanceArgs {
    /// Wallet address
    pub address: String,

    /// Chain ID
    #[arg(long)]
    pub chain_id: Option<u64>,

    /// Token addresses to check balances for (comma-separated)
    #[arg(long)]
    pub tokens: Option<String>,
}
