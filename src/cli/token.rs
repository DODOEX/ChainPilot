use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct TokenCmd {
    #[command(subcommand)]
    pub action: TokenAction,
}

#[derive(Subcommand)]
pub enum TokenAction {
    /// Token metadata (name, symbol, decimals, supply)
    Info(TokenIdentArg),
    /// On-chain contract details
    Contract(TokenIdentArg),
}

#[derive(Args)]
pub struct TokenIdentArg {
    /// Token symbol or contract address
    pub token: String,
}
