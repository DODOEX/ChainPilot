//! Cross-VM wallet/token address typing.
//!
//! Replaces the EVM-only `alloy::primitives::Address::parse` guard at command
//! entry points so SVM (Solana base58 pubkeys) and BVM (Bitcoin mainnet
//! addresses) can flow through the same handlers, with each handler branching
//! on the discriminant to pick the right upstream data source.

use alloy::primitives::Address;

use crate::error::{ChainError, Result};

/// A wallet/account address tagged with the VM it lives on.
///
/// The string form is preserved exactly as the user typed it (modulo a leading
/// whitespace trim) for variants that wrap non-EVM addresses, because the
/// downstream APIs (Debank, Zerion, mempool.space) accept the address as a
/// raw string — re-encoding would just be an opportunity for bugs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressRef {
    /// 20-byte EVM address.
    Evm(Address),
    /// Solana base58-encoded pubkey (32 bytes → 43–44 chars).
    Svm(String),
    /// Bitcoin mainnet address: Bech32 (`bc1…`), P2PKH (`1…`), or P2SH (`3…`).
    Bvm(String),
}

impl AddressRef {
    /// Parse a user-supplied address string, dispatching on shape.
    ///
    /// Order of checks matters: EVM hex is unambiguous (`0x` prefix), Bitcoin
    /// Bech32 is unambiguous (`bc1`/`tb1` prefix). The legacy Bitcoin / Solana
    /// boundary overlaps on length 32–35 chars when leading byte happens to
    /// encode to `1` or `3`; we resolve that by requiring legacy Bitcoin to
    /// be ≤ 35 chars (real BTC P2PKH/P2SH addresses are 26–35), so a 44-char
    /// Solana pubkey starting with `1` still classifies as SVM.
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ChainError::InvalidAddress(input.to_string()));
        }

        // 1. EVM — 0x + 40 hex chars. alloy's `Address::parse` only accepts
        // the lowercase `0x` prefix; reject the uppercase form here instead
        // of bubbling alloy's less helpful error.
        if let Some(rest) = trimmed.strip_prefix("0x") {
            if rest.len() == 40 && rest.bytes().all(|b| b.is_ascii_hexdigit()) {
                let addr: Address = trimmed
                    .parse()
                    .map_err(|_| ChainError::InvalidAddress(input.to_string()))?;
                return Ok(AddressRef::Evm(addr));
            }
        }

        // 2. Bitcoin Bech32 (mainnet `bc1`, testnet `tb1`)
        if trimmed.starts_with("bc1") || trimmed.starts_with("tb1") {
            // Bech32 alphabet is a strict subset of base58 + 0 and l; just
            // make sure it's printable ASCII so we don't misclassify garbage.
            if trimmed.len() >= 14 && trimmed.bytes().all(|b| b.is_ascii_alphanumeric()) {
                return Ok(AddressRef::Bvm(trimmed.to_string()));
            }
            return Err(ChainError::InvalidAddress(input.to_string()));
        }

        // 3. Base58 alphabet — needed for both BTC legacy and Solana pubkeys.
        if !is_base58(trimmed) {
            return Err(ChainError::InvalidAddress(input.to_string()));
        }

        // 4. BTC legacy: starts with `1` (P2PKH) or `3` (P2SH), length 26–35.
        let starts_btc_legacy = trimmed.starts_with('1') || trimmed.starts_with('3');
        if starts_btc_legacy && trimmed.len() <= 35 {
            return Ok(AddressRef::Bvm(trimmed.to_string()));
        }

        // 5. Solana base58 pubkey: 32 bytes, 43–44 chars when encoded.
        // We accept 32–44 to be lenient (some encoders drop leading zeros).
        if trimmed.len() >= 32 && trimmed.len() <= 44 {
            return Ok(AddressRef::Svm(trimmed.to_string()));
        }

        Err(ChainError::InvalidAddress(input.to_string()))
    }

    /// String form suitable for echoing back to the user and for use as a
    /// query parameter to upstream APIs. Reserved for future PRs that will
    /// surface the parsed form in command output.
    #[allow(dead_code)]
    pub fn as_str(&self) -> String {
        match self {
            AddressRef::Evm(a) => a.to_string(),
            AddressRef::Svm(s) | AddressRef::Bvm(s) => s.clone(),
        }
    }

    /// Short label for diagnostics and error messages. Reserved for future
    /// PRs that will surface the VM tag in command output.
    #[allow(dead_code)]
    pub fn vm(&self) -> &'static str {
        match self {
            AddressRef::Evm(_) => "evm",
            AddressRef::Svm(_) => "svm",
            AddressRef::Bvm(_) => "bvm",
        }
    }
}

/// Bitcoin's base58 alphabet (also a superset of what Solana uses for pubkeys).
/// Excludes the visually ambiguous chars `0`, `O`, `I`, `l`.
const BASE58_ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

fn is_base58(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| BASE58_ALPHABET.contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVM: &str = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"; // vitalik.eth
    // Real on-chain Solana pubkey: Wrapped SOL mint authority — known good base58.
    const SVM: &str = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
    const BTC_BECH32: &str = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq";
    const BTC_LEGACY_P2PKH: &str = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"; // genesis coinbase
    const BTC_LEGACY_P2SH: &str = "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy";

    #[test]
    fn parses_evm() {
        let a = AddressRef::parse(EVM).unwrap();
        assert!(matches!(a, AddressRef::Evm(_)));
        assert_eq!(a.vm(), "evm");
    }

    #[test]
    fn parses_svm_pubkey() {
        let a = AddressRef::parse(SVM).unwrap();
        assert!(matches!(a, AddressRef::Svm(_)));
        assert_eq!(a.as_str(), SVM);
    }

    #[test]
    fn parses_btc_bech32() {
        let a = AddressRef::parse(BTC_BECH32).unwrap();
        assert!(matches!(a, AddressRef::Bvm(_)));
    }

    #[test]
    fn parses_btc_legacy_p2pkh() {
        let a = AddressRef::parse(BTC_LEGACY_P2PKH).unwrap();
        assert!(matches!(a, AddressRef::Bvm(_)));
    }

    #[test]
    fn parses_btc_legacy_p2sh() {
        let a = AddressRef::parse(BTC_LEGACY_P2SH).unwrap();
        assert!(matches!(a, AddressRef::Bvm(_)));
    }

    #[test]
    fn rejects_empty_and_garbage() {
        assert!(AddressRef::parse("").is_err());
        assert!(AddressRef::parse("   ").is_err());
        assert!(AddressRef::parse("not-an-address").is_err()); // contains '-'
        assert!(AddressRef::parse("0xdeadbeef").is_err()); // too short for EVM
    }

    #[test]
    fn trims_whitespace_before_classifying() {
        let a = AddressRef::parse(&format!("  {SVM}  ")).unwrap();
        assert!(matches!(a, AddressRef::Svm(_)));
    }

    #[test]
    fn long_svm_pubkey_starting_with_one_is_not_btc() {
        // Synthetic 44-char base58 starting with `1` — must classify as SVM,
        // not BTC, because BTC legacy addrs are ≤ 35 chars.
        let svm_like = "1".to_string() + &"A".repeat(43);
        let a = AddressRef::parse(&svm_like).unwrap();
        assert!(matches!(a, AddressRef::Svm(_)));
    }
}
