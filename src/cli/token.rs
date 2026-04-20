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
    /// Create a token via DODO's ERC20V3Factory
    Create(TokenCreateCmd),
    /// Read the token creation fee from the configured factory
    Fee(TokenFeeArgs),
    /// Mint additional supply on a mintable token
    Mint(TokenMintArgs),
    /// Renounce token ownership by calling abandonOwnership(address(0))
    RenounceOwnership(TokenOwnershipArgs),
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

#[derive(Args)]
pub struct TokenCreateCmd {
    #[command(subcommand)]
    pub action: TokenCreateAction,
}

#[derive(Subcommand)]
pub enum TokenCreateAction {
    /// Create a standard ERC-20 token
    Std(TokenCreateStdArgs),
    /// Create a custom ERC-20 token with trade burn / fee hooks
    Custom(TokenCreateCustomArgs),
    /// Create a mintable ERC-20 token
    Mintable(TokenCreateMintableArgs),
}

#[derive(Args, Clone)]
pub struct TokenCreateStdArgs {
    /// Token name
    #[arg(long)]
    pub name: String,

    /// Token symbol
    #[arg(long)]
    pub symbol: String,

    /// Initial supply in human-readable units
    #[arg(long)]
    pub supply: String,

    /// Token decimals
    #[arg(long, default_value_t = 18)]
    pub decimals: u8,

    /// Show calldata and gas estimate without sending a transaction
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

#[derive(Args, Clone)]
pub struct TokenCreateCustomArgs {
    /// Token name
    #[arg(long)]
    pub name: String,

    /// Token symbol
    #[arg(long)]
    pub symbol: String,

    /// Initial supply in human-readable units
    #[arg(long)]
    pub supply: String,

    /// Token decimals
    #[arg(long, default_value_t = 18)]
    pub decimals: u8,

    /// Trade burn percentage, from 0 to 50 with up to 2 decimals, e.g. 0.1 or 1.25
    #[arg(long, default_value = "0")]
    pub burn_pct: String,

    /// Trade fee percentage, from 0 to 50 with up to 2 decimals, e.g. 0.1 or 1.25
    #[arg(long, default_value = "0")]
    pub fee_pct: String,

    /// Fee recipient / team account; defaults to the active signer or wallet address
    #[arg(long)]
    pub team_account: Option<String>,

    /// Show calldata and gas estimate without sending a transaction
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

#[derive(Args, Clone)]
pub struct TokenCreateMintableArgs {
    /// Token name
    #[arg(long)]
    pub name: String,

    /// Token symbol
    #[arg(long)]
    pub symbol: String,

    /// Initial supply in human-readable units
    #[arg(long)]
    pub supply: String,

    /// Token decimals
    #[arg(long, default_value_t = 18)]
    pub decimals: u8,

    /// Trade burn percentage, from 0 to 50 with up to 2 decimals, e.g. 0.1 or 1.25
    #[arg(long, default_value = "0")]
    pub burn_pct: String,

    /// Trade fee percentage, from 0 to 50 with up to 2 decimals, e.g. 0.1 or 1.25
    #[arg(long, default_value = "0")]
    pub fee_pct: String,

    /// Token owner, mint authority, and fee recipient; defaults to the active signer or wallet address
    #[arg(long)]
    pub owner: Option<String>,

    /// Show calldata and gas estimate without sending a transaction
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct TokenFeeArgs {}

#[derive(Args, Clone)]
pub struct TokenMintArgs {
    /// Token contract address
    #[arg(long)]
    pub token: String,

    /// Recipient address
    #[arg(long)]
    pub to: String,

    /// Amount in human-readable units
    #[arg(long)]
    pub amount: String,

    /// Show calldata and gas estimate without sending a transaction
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

#[derive(Args, Clone)]
pub struct TokenOwnershipArgs {
    /// Token contract address
    #[arg(long)]
    pub token: String,

    /// Show calldata and gas estimate without sending a transaction
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}
