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

    #[error("Transaction reverted: {reason}")]
    Reverted { reason: String },

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("No wallet configured. Set PRIVATE_KEY env var or use --private-key flag")]
    NoWallet,

    #[error("Invalid private key: {0}")]
    InvalidPrivateKey(String),

    #[error("Insufficient balance: have {have}, need {need} {token}")]
    InsufficientBalance {
        have: String,
        need: String,
        token: String,
    },

    #[error("Token not approved. Run: chain swap approve --token {token} --spender {spender}")]
    NotApproved { token: String, spender: String },

    #[error("Quote not found: {0}. Quotes expire after 5 minutes.")]
    QuoteNotFound(String),

    #[error("Quote expired: {0}")]
    QuoteExpired(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Token not found: {0}")]
    TokenNotFound(String),

    #[error("Unsupported chain ID: {0}. Run `chain token chains` to list supported chains.")]
    UnsupportedChain(u64),

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
    fn reverted_error_message() {
        let e = ChainError::Reverted {
            reason: "insufficient output amount".to_string(),
        };
        assert_eq!(
            e.to_string(),
            "Transaction reverted: insufficient output amount"
        );
    }

    #[test]
    fn no_wallet_error_message() {
        let e = ChainError::NoWallet;
        assert!(e.to_string().contains("PRIVATE_KEY"));
        assert!(e.to_string().contains("--private-key"));
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
        assert!(msg.contains("chain swap approve"));
    }

    #[test]
    fn quote_not_found_mentions_expiry() {
        let e = ChainError::QuoteNotFound("abc-123".to_string());
        let msg = e.to_string();
        assert!(msg.contains("abc-123"));
        assert!(msg.contains("5 minutes"));
    }

    #[test]
    fn token_not_found_includes_symbol() {
        let e = ChainError::TokenNotFound("PEPE".to_string());
        assert_eq!(e.to_string(), "Token not found: PEPE");
    }
}
