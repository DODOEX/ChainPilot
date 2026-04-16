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
    /// Save a custom token so later symbol lookups can resolve it
    Add(TokenAddArgs),
}

#[derive(Args)]
pub struct TokenIdentArg {
    /// Token symbol or contract address
    pub token: String,
}

#[derive(Args)]
pub struct TokenAddArgs {
    /// Token contract address
    pub address: String,
}
