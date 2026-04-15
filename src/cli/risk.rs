use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct RiskCmd {
    #[command(subcommand)]
    pub action: RiskAction,
}

#[derive(Subcommand)]
pub enum RiskAction {
    /// Token risk analysis
    Token(TokenRiskArgs),
    /// Wallet risk overview
    Wallet(WalletRiskArgs),
    /// Check token approval status
    Approval(ApprovalRiskArgs),
}

#[derive(Args)]
pub struct TokenRiskArgs {
    pub token: String,
}

#[derive(Args)]
pub struct WalletRiskArgs {
    pub address: String,
}

#[derive(Args)]
pub struct ApprovalRiskArgs {
    pub address: String,

    /// Token symbol or contract address
    #[arg(long)]
    pub token: String,

    #[arg(long)]
    pub spender: String,
}
