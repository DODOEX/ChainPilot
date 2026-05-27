use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct TokenCmd {
    #[command(subcommand)]
    pub action: TokenAction,
}

#[derive(Subcommand)]
pub enum TokenAction {
    /// Token metadata; unresolved symbols return candidate matches
    Info(TokenIdentArg),
    /// On-chain contract details
    Contract(TokenIdentArg),
    /// Real-time price, % changes, and 24h high/low (CoinGecko primary, DexScreener fallback)
    Price(TokenIdentArg),
    /// Liquidity overview: top liquidity, pair count, top pair details (DexScreener)
    Liquidity(TokenIdentArg),
    /// Token risk analysis: honeypot, blacklist, taxes, owner privileges (GoPlus Security)
    Risk(TokenIdentArg),
    /// Save a custom token so later symbol lookups can resolve it
    Add(TokenAddArgs),
}

#[derive(Args)]
pub struct TokenIdentArg {
    /// Token symbol or contract address; unknown symbols return external-source candidates
    pub token: String,
}

#[derive(Args)]
pub struct TokenAddArgs {
    /// Token contract address
    pub address: String,
}
