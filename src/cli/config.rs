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
    /// Configuration key name
    pub key: String,
    /// Value to set
    pub value: String,
}

#[derive(Args)]
pub struct ConfigKeyArg {
    /// Configuration key name
    pub key: String,
}
