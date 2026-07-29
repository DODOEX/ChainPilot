use clap::Args;
use clap::Subcommand;

#[derive(Args)]
pub struct ConfigCmd {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Set an API key or configuration value
    Set(ConfigSetArgs),
    /// Get the current value of a configuration key
    Get(ConfigKeyArg),
    /// List all configuration keys and their (masked) values
    List,
    /// Remove a configuration key
    Unset(ConfigKeyArg),
}

#[derive(Args)]
pub struct ConfigSetArgs {
    /// Configuration key: an API key name, or `rpc_url[.<chainId>]` for a
    /// per-chain RPC endpoint (bare `rpc_url` targets the active `--chain-id`).
    pub key: String,
    /// Value to set. For `rpc_url`, a single URL, or a JSON map of
    /// chainId -> URL to configure several chains at once
    /// (e.g. '{"1":"https://eth","56":"https://bsc"}').
    pub value: String,
}

#[derive(Args)]
pub struct ConfigKeyArg {
    /// Configuration key: an API key name, or `rpc_url[.<chainId>]`. Bare
    /// `rpc_url` targets all chains (or the active `--chain-id` if set).
    pub key: String,
}
