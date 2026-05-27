mod dodo;
mod token_metadata;

pub use dodo::DodoClient;
pub use token_metadata::TokenMetadataClient;

use crate::config::AppConfig;
use crate::error::Result;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE, USER_AGENT};

pub struct ApiClients {
    pub dodo: DodoClient,
    pub token_metadata: TokenMetadataClient,
}

impl ApiClients {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
            ),
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(
                crate::config::DEFAULT_REQUEST_TIMEOUT_SECS,
            ))
            .default_headers(headers)
            .build()?;

        Ok(Self {
            dodo: DodoClient::new(
                client.clone(),
                &config.dodo_api_url,
                &config.dodo_api_key,
                &config.dodo_project_id,
            ),
            token_metadata: TokenMetadataClient::new(client, config),
        })
    }
}
