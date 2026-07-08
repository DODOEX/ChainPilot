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
    /// boundary overlaps on character length (BTC 26–35, Solana 32–44), so we
    /// resolve it by the DECODED payload length — BTC P2PKH/P2SH decode to 25
    /// bytes, Solana pubkeys to 32 — rather than a fragile length+prefix guess.
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

        // 4 & 5. BTC legacy vs Solana pubkey. Their character-length ranges
        // overlap (BTC 26–35, Solana 32–44), so a short Solana pubkey starting
        // with `1`/`3` — e.g. the System Program `1111…1111` (32 chars) — would
        // be mis-tagged as Bitcoin by a length+prefix heuristic. Disambiguate
        // by the decoded PAYLOAD length instead: BTC P2PKH/P2SH decode to 25
        // bytes (1 version + 20 hash + 4 checksum), Solana pubkeys to 32 bytes.
        // Decoding also rejects strings that merely look base58 but aren't a
        // valid address of either length.
        match base58_decode(trimmed) {
            Some(bytes) if bytes.len() == 25 && matches!(bytes.first(), Some(0x00) | Some(0x05)) => {
                Ok(AddressRef::Bvm(trimmed.to_string()))
            }
            Some(bytes) if bytes.len() == 32 => Ok(AddressRef::Svm(trimmed.to_string())),
            _ => Err(ChainError::InvalidAddress(input.to_string())),
        }
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

/// Decode a base58 string to its raw bytes (Bitcoin/Base58 alphabet, no
/// checksum verification). Returns `None` if any char is outside the alphabet.
/// Used only to disambiguate address families by decoded length, so the exact
/// bytes past the length/version aren't validated further.
fn base58_decode(s: &str) -> Option<Vec<u8>> {
    let mut bytes: Vec<u8> = Vec::with_capacity(s.len());
    for c in s.bytes() {
        let mut carry = BASE58_ALPHABET.iter().position(|&b| b == c)? as u32;
        for byte in bytes.iter_mut() {
            carry += (*byte as u32) * 58;
            *byte = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    // Each leading '1' encodes one leading zero byte.
    for c in s.bytes() {
        if c == b'1' {
            bytes.push(0);
        } else {
            break;
        }
    }
    bytes.reverse();
    Some(bytes)
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
    fn short_svm_pubkey_starting_with_one_is_not_btc() {
        // The System Program: a real 32-char Solana pubkey (32 zero bytes)
        // that starts with `1` and is well inside BTC's 26–35 char range.
        // Decoded-length classification must still route it to SVM (32 bytes),
        // not BTC (25 bytes).
        let system_program = "11111111111111111111111111111111";
        let a = AddressRef::parse(system_program).unwrap();
        assert!(matches!(a, AddressRef::Svm(_)));
    }

    #[test]
    fn rejects_base58_of_wrong_decoded_length() {
        // Looks base58 but decodes to neither a 25-byte BTC payload nor a
        // 32-byte Solana pubkey — must be rejected, not silently accepted.
        assert!(AddressRef::parse("11").is_err());
        assert!(AddressRef::parse("2g").is_err());
    }
}
