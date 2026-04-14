pub mod risk;
pub mod swap;
pub mod token;
pub mod wallet;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "chainpilot",
    version,
    about = "On-chain CLI for DeFi operations",
    long_about = None,
)]
pub struct Cli {
    /// Output as machine-readable JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress all output except errors
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Private key for signing (overrides PRIVATE_KEY env var)
    #[arg(long, env = "PRIVATE_KEY", global = true, hide_env_values = true)]
    pub private_key: Option<String>,

    /// Wallet address to use for quote/simulate context (overrides WALLET_ADDRESS env var)
    #[arg(long, env = "WALLET_ADDRESS", global = true)]
    pub wallet_address: Option<String>,

    /// Ethereum RPC endpoint
    #[arg(long, env = "ETH_RPC_URL", global = true)]
    pub rpc_url: Option<String>,

    /// DODO API key (overrides DODO_API_KEY env var and compile-time default)
    #[arg(long, env = "DODO_API_KEY", global = true, hide_env_values = true)]
    pub dodo_api_key: Option<String>,

    /// DODO project ID for tokenlist lookup (overrides DODO_PROJECT_ID env var and compile-time default)
    #[arg(long, env = "DODO_PROJECT_ID", global = true, hide_env_values = true)]
    pub dodo_project_id: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Token swap operations (quote, simulate, execute, status, history, approve, revoke)
    Swap(swap::SwapCmd),
    /// Token information (info, contract)
    Token(token::TokenCmd),
    /// Wallet data (balance)
    Wallet(wallet::WalletCmd),
    /// Risk analysis (token, wallet, approval)
    Risk(risk::RiskCmd),
}
