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
    #[arg(long, default_value = "1")]
    pub chain_id: u64,

    /// Token addresses to check balances for (comma-separated)
    #[arg(long)]
    pub tokens: Option<String>,
}
