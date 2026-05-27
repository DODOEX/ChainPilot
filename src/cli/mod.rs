pub mod config;
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

    /// Path to an encrypted JSON keystore for signing
    #[arg(long, env = "KEYSTORE_PATH", global = true)]
    pub keystore_path: Option<String>,

    /// Read the keystore password from a file
    #[arg(long, env = "KEYSTORE_PASSWORD_FILE", global = true)]
    pub password_file: Option<String>,

    /// Read the keystore password from the named environment variable
    #[arg(long, env = "KEYSTORE_PASSWORD_ENV", global = true)]
    pub password_env: Option<String>,

    /// Wallet address to use for quote/simulate context (overrides WALLET_ADDRESS env var)
    #[arg(long, env = "WALLET_ADDRESS", global = true)]
    pub wallet_address: Option<String>,

    /// Ethereum RPC endpoint
    #[arg(long, global = true)]
    pub rpc_url: Option<String>,

    /// Active chain ID
    #[arg(long, env = "CHAIN_ID", global = true)]
    pub chain_id: Option<u64>,

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
    /// Manage API keys and configuration
    Config(config::ConfigCmd),
}
