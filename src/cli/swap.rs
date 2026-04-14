use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct SwapCmd {
    #[command(subcommand)]
    pub action: SwapAction,
}

#[derive(Subcommand)]
pub enum SwapAction {
    /// Get a swap quote
    Quote(QuoteArgs),
    /// Simulate a swap from a saved quote
    Simulate(SimulateArgs),
    /// Execute a swap (requires wallet)
    Execute(ExecuteArgs),
    /// Check swap transaction status
    Status(StatusArgs),
    /// Show swap history
    History(HistoryArgs),
    /// Approve token spending
    Approve(ApproveArgs),
    /// Revoke token spending approval
    Revoke(RevokeArgs),
}

#[derive(Args)]
pub struct QuoteArgs {
    /// Source token symbol or address (e.g. ETH, USDC, 0x...)
    #[arg(long)]
    pub from: String,

    /// Destination token symbol or address
    #[arg(long)]
    pub to: String,

    /// Amount of source token to swap (in human-readable units)
    #[arg(long)]
    pub amount: String,

    /// Chain ID (default: 1 for Ethereum mainnet)
    #[arg(long)]
    pub chain_id: Option<u64>,

    /// Slippage tolerance passed directly to the DODO API (e.g. 0.2)
    #[arg(long, default_value = "0.2")]
    pub slippage: f64,
}

#[derive(Args)]
pub struct SimulateArgs {
    /// Quote ID returned by `chainpilot swap quote`
    #[arg(long)]
    pub quote_id: String,

    /// Wallet address for balance/allowance checks
    #[arg(long, env = "WALLET_ADDRESS")]
    pub wallet: Option<String>,
}

#[derive(Args)]
pub struct ExecuteArgs {
    /// Quote ID returned by `chainpilot swap quote`
    #[arg(long)]
    pub quote_id: String,

    /// Simulate execution without broadcasting the transaction
    #[arg(long)]
    pub dry_run: bool,

    /// Gas limit override
    #[arg(long)]
    pub gas_limit: Option<u64>,

    /// Max fee per gas in gwei
    #[arg(long)]
    pub max_fee_gwei: Option<f64>,

    /// Wallet address for dry-run simulation (no private key needed; env: WALLET_ADDRESS)
    #[arg(long, env = "WALLET_ADDRESS")]
    pub wallet: Option<String>,

    /// Private key for signing and broadcasting the transaction (required unless --dry-run; env: PRIVATE_KEY)
    #[arg(long, env = "PRIVATE_KEY")]
    pub private_key: Option<String>,

    /// Wait for the transaction to be mined and show the final on-chain status
    #[arg(long)]
    pub wait: bool,

    /// Skip eth_estimateGas pre-flight check and use the quote's gas estimate instead
    #[arg(long)]
    pub skip_estimate: bool,

    /// Add a percentage buffer on top of eth_estimateGas result (e.g. 20 for +20%)
    #[arg(long)]
    pub gas_buffer_pct: Option<u64>,
}

#[derive(Args)]
pub struct StatusArgs {
    /// Transaction hash to check
    #[arg(long)]
    pub tx_hash: String,

    /// Chain ID
    #[arg(long)]
    pub chain_id: Option<u64>,
}

#[derive(Args)]
pub struct HistoryArgs {
    /// Maximum number of records to return
    #[arg(long, default_value = "20")]
    pub limit: u32,

    /// Filter by status: pending, success, failed
    #[arg(long)]
    pub status: Option<String>,
}

#[derive(Args)]
pub struct ApproveArgs {
    /// Quote ID to derive token and spender from (uses from-token and router address)
    #[arg(long)]
    pub quote_id: Option<String>,

    /// Token symbol or address to approve (overrides quote's from-token)
    #[arg(long)]
    pub token: Option<String>,

    /// Spender contract address (overrides quote's router address)
    #[arg(long)]
    pub spender: Option<String>,

    /// Amount to approve in human-readable units (omit for unlimited / U256::MAX)
    #[arg(long)]
    pub amount: Option<String>,

    /// Dry-run mode: show what would be approved without sending
    #[arg(long)]
    pub dry_run: bool,

    /// Chain ID
    #[arg(long)]
    pub chain_id: Option<u64>,

    /// Private key for signing (required unless --dry-run; env: PRIVATE_KEY)
    #[arg(long, env = "PRIVATE_KEY")]
    pub private_key: Option<String>,
}

#[derive(Args)]
pub struct RevokeArgs {
    #[arg(long)]
    pub token: String,

    #[arg(long)]
    pub spender: String,

    #[arg(long)]
    pub dry_run: bool,

    /// Private key for signing (required unless --dry-run; env: PRIVATE_KEY)
    #[arg(long, env = "PRIVATE_KEY")]
    pub private_key: Option<String>,

    /// Chain ID
    #[arg(long)]
    pub chain_id: Option<u64>,
}
