mod quote;

pub use quote::QuoteStore;

use crate::config::AppConfig;
use crate::error::Result;

pub fn open_store(config: &AppConfig) -> Result<QuoteStore> {
    QuoteStore::new(config)
}
