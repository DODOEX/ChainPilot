use thiserror::Error;

pub type Result<T> = std::result::Result<T, ChainError>;

#[derive(Debug, Error)]
pub enum ChainError {
    #[error("DODO API error: {message} (code: {code})")]
    DodoApi { code: i64, message: String },

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("No wallet configured. Set PRIVATE_KEY/KEYSTORE_PATH env vars or use --private-key/--keystore-path")]
    NoWallet,

    #[error("Invalid private key: {0}")]
    InvalidPrivateKey(String),

    #[error("Keystore error: {0}")]
    Keystore(String),

    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    #[error("Insufficient balance: have {have}, need {need} {token}")]
    InsufficientBalance {
        have: String,
        need: String,
        token: String,
    },

    #[error(
        "Token not approved. Run: chainpilot swap approve --token {token} --spender {spender}"
    )]
    NotApproved { token: String, spender: String },

    #[error("Quote not found or expired: {0}. Run 'chainpilot swap quote' again.")]
    QuoteNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Token not found: {0}")]
    TokenNotFound(String),

    #[error("Unsupported chain ID: {0}. See README for the supported chain list.")]
    UnsupportedChain(u64),

    #[error("Token creation is not supported on chain ID: {0}")]
    UnsupportedTokenFactoryChain(u64),

    #[error("Missing ERC20V3Factory address for chain ID: {0}")]
    MissingFactoryAddress(u64),

    #[error("Invalid token creation parameters: {0}")]
    InvalidTokenCreateParams(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dodo_api_error_message() {
        let e = ChainError::DodoApi {
            code: 400,
            message: "bad request".to_string(),
        };
        assert_eq!(e.to_string(), "DODO API error: bad request (code: 400)");
    }

    #[test]
    fn rpc_error_message() {
        let e = ChainError::Rpc("connection refused".to_string());
        assert_eq!(e.to_string(), "RPC error: connection refused");
    }

    #[test]
    fn no_wallet_error_message() {
        let e = ChainError::NoWallet;
        assert!(e.to_string().contains("PRIVATE_KEY"));
        assert!(e.to_string().contains("--private-key"));
        assert!(e.to_string().contains("KEYSTORE_PATH"));
        assert!(e.to_string().contains("--keystore-path"));
    }

    #[test]
    fn insufficient_balance_error_message() {
        let e = ChainError::InsufficientBalance {
            have: "0.5".to_string(),
            need: "1.0".to_string(),
            token: "ETH".to_string(),
        };
        assert_eq!(
            e.to_string(),
            "Insufficient balance: have 0.5, need 1.0 ETH"
        );
    }

    #[test]
    fn not_approved_error_message() {
        let e = ChainError::NotApproved {
            token: "0xToken".to_string(),
            spender: "0xSpender".to_string(),
        };
        let msg = e.to_string();
        assert!(msg.contains("0xToken"));
        assert!(msg.contains("0xSpender"));
        assert!(msg.contains("chainpilot swap approve"));
    }

    #[test]
    fn quote_not_found_mentions_expiry_or_missing() {
        let e = ChainError::QuoteNotFound("abc-123".to_string());
        let msg = e.to_string();
        assert!(msg.contains("abc-123"));
        assert!(msg.contains("expired"));
        assert!(msg.contains("swap quote"));
    }

    #[test]
    fn token_not_found_includes_symbol() {
        let e = ChainError::TokenNotFound("PEPE".to_string());
        assert_eq!(e.to_string(), "Token not found: PEPE");
    }

    #[test]
    fn invalid_amount_message() {
        let e = ChainError::InvalidAmount("too many decimal places".to_string());
        assert_eq!(e.to_string(), "Invalid amount: too many decimal places");
    }
}
