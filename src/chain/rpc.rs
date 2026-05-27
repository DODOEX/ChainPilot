//! Ethereum RPC calls via alloy Provider.

use std::io::IsTerminal;

use alloy::network::TransactionBuilder;
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use alloy_signer_local::PrivateKeySigner;
use url::Url;

use crate::chain::OnChainClient;
use crate::config::{AppConfig, DEFAULT_KEYSTORE_PASSWORD_ENV};
use crate::error::{ChainError, Result};

#[derive(Debug, serde::Serialize)]
pub struct TxStatus {
    pub confirmed: bool,
    pub success: bool,
    pub block_number: Option<u64>,
    pub gas_used: Option<u64>,
    pub effective_gas_price: Option<f64>,
}

pub async fn get_eth_balance(client: &OnChainClient, address: Address) -> Result<(String, f64)> {
    let balance = client
        .provider
        .get_balance(address)
        .await
        .map_err(|e| ChainError::Rpc(format!("get_balance failed: {:?}", e)))?;

    let balance_str = balance.to_string();
    let balance_eth = parse_wei(&balance_str);
    Ok((balance_str, balance_eth))
}

pub async fn get_gas_price_gwei(client: &OnChainClient) -> Result<f64> {
    let gas_price = client
        .provider
        .get_gas_price()
        .await
        .map_err(|e| ChainError::Rpc(format!("get_gas_price failed: {:?}", e)))?;
    Ok(gas_price as f64 / 1e9)
}

pub async fn get_tx_receipt(client: &OnChainClient, tx_hash: &str) -> Result<Option<TxStatus>> {
    let tx_hash = tx_hash
        .parse()
        .map_err(|_| ChainError::Rpc(format!("invalid tx hash: {}", tx_hash)))?;

    let receipt = client
        .provider
        .get_transaction_receipt(tx_hash)
        .await
        .map_err(|e| ChainError::Rpc(format!("get_transaction_receipt failed: {:?}", e)))?;

    Ok(receipt.map(|r| {
        let success = r.inner.status();
        TxStatus {
            confirmed: true,
            success,
            block_number: r.block_number,
            gas_used: Some(r.gas_used),
            effective_gas_price: Some(r.effective_gas_price as f64 / 1e9),
        }
    }))
}

pub async fn estimate_gas(
    client: &OnChainClient,
    from: Address,
    to: Address,
    data: &str,
    value: &str,
) -> Result<u64> {
    let data_bytes = if data.starts_with("0x") {
        &data[2..]
    } else {
        data
    };
    let value_u256 = parse_value_u256(value)?;

    let input_bytes =
        hex::decode(data_bytes).map_err(|_| ChainError::Rpc("invalid data hex".to_string()))?;

    let tx = TransactionRequest::default()
        .with_from(from)
        .with_to(to)
        .with_input(input_bytes)
        .with_value(value_u256);

    let gas = client
        .provider
        .estimate_gas(tx)
        .await
        .map_err(|e| ChainError::Rpc(format!("estimate_gas failed: {:?}", e)))?;

    Ok(gas)
}

/// Return the number of confirmed transactions sent by `address` (i.e. the confirmed nonce).
pub async fn get_nonce(client: &OnChainClient, address: Address) -> Result<u64> {
    client
        .provider
        .get_transaction_count(address)
        .await
        .map_err(|e| ChainError::Rpc(format!("get_transaction_count failed: {:?}", e)))
}

/// Derive the wallet address from a hex private key without sending anything.
pub fn address_from_private_key(private_key: &str) -> Result<Address> {
    let pk = if private_key.starts_with("0x") {
        &private_key[2..]
    } else {
        private_key
    };
    let signer: PrivateKeySigner = pk
        .parse()
        .map_err(|_| ChainError::InvalidPrivateKey(private_key.to_string()))?;
    Ok(signer.address())
}

pub fn resolve_signer(config: &AppConfig) -> Result<PrivateKeySigner> {
    if let Some(private_key) = config.private_key.as_deref() {
        let pk = if private_key.starts_with("0x") {
            &private_key[2..]
        } else {
            private_key
        };
        return pk
            .parse()
            .map_err(|_| ChainError::InvalidPrivateKey(private_key.to_string()));
    }

    let keystore_path = config
        .keystore_path
        .as_deref()
        .ok_or(ChainError::NoWallet)?;
    let password = resolve_keystore_password(config)?;

    PrivateKeySigner::decrypt_keystore(keystore_path, password.as_bytes()).map_err(|e| {
        ChainError::Keystore(format!(
            "failed to decrypt keystore at {}: {}",
            keystore_path, e
        ))
    })
}

/// Sign and broadcast a swap transaction. Returns `(from_address, tx_hash)`.
pub async fn send_tx(
    rpc_url: &str,
    chain_id: u64,
    signer: PrivateKeySigner,
    to: Address,
    data: &str,
    value_hex: &str,
    gas_limit: Option<u64>,
    max_fee_gwei: Option<f64>,
) -> Result<(Address, String)> {
    use alloy::network::EthereumWallet;
    use alloy::providers::ProviderBuilder;
    let from = signer.address();
    let wallet = EthereumWallet::from(signer);

    let url: Url = rpc_url
        .parse()
        .map_err(|e| ChainError::Rpc(format!("invalid rpc url: {}", e)))?;
    // In alloy 1.8, recommended fillers (gas, nonce, chain_id) are included by default.
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(url);

    let value_u256 = parse_value_u256(value_hex)?;

    let data_hex = if data.starts_with("0x") {
        &data[2..]
    } else {
        data
    };
    let data_bytes =
        hex::decode(data_hex).map_err(|_| ChainError::Rpc("invalid calldata hex".to_string()))?;

    let mut tx = TransactionRequest::default()
        .with_from(from)
        .with_to(to)
        .with_input(data_bytes)
        .with_value(value_u256)
        .with_chain_id(chain_id);

    if let Some(gas) = gas_limit {
        tx = tx.with_gas_limit(gas);
    }
    if let Some(fee_gwei) = max_fee_gwei {
        tx = tx.with_max_fee_per_gas((fee_gwei * 1e9) as u128);
    }

    let pending: alloy::providers::PendingTransactionBuilder<alloy::network::Ethereum> = provider
        .send_transaction(tx)
        .await
        .map_err(|e| ChainError::Rpc(format!("send_transaction failed: {:?}", e)))?;

    Ok((from, pending.tx_hash().to_string()))
}

fn parse_wei(raw: &str) -> f64 {
    let wei: u128 = raw.parse().unwrap_or(0);
    wei as f64 / 1e18
}

fn resolve_keystore_password(config: &AppConfig) -> Result<String> {
    if let Some(password_file) = config.keystore_password_file.as_deref() {
        let raw = std::fs::read_to_string(password_file).map_err(|e| {
            ChainError::Keystore(format!(
                "failed to read password file {}: {}",
                password_file, e
            ))
        })?;
        return Ok(trim_password(raw));
    }

    if let Some(password_env) = config.keystore_password_env.as_deref() {
        return std::env::var(password_env).map_err(|_| {
            ChainError::Keystore(format!("environment variable {} is not set", password_env))
        });
    }

    if let Ok(password) = std::env::var(DEFAULT_KEYSTORE_PASSWORD_ENV) {
        return Ok(password);
    }

    if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
        eprint!("Keystore password: ");
        let _ = std::io::Write::flush(&mut std::io::stderr());
        let mut password = String::new();
        std::io::stdin().read_line(&mut password).map_err(|e| {
            ChainError::Keystore(format!("failed to read keystore password: {}", e))
        })?;
        return Ok(trim_password(password));
    }

    Err(ChainError::Keystore(
        "keystore password required; use --password-file, set KEYSTORE_PASSWORD, or run in an interactive terminal".to_string(),
    ))
}

fn trim_password(raw: String) -> String {
    raw.trim_end_matches(['\r', '\n']).to_string()
}

fn parse_value_u256(value: &str) -> Result<U256> {
    if value.is_empty() || value == "0" || value == "0x0" {
        return Ok(U256::ZERO);
    }

    if let Some(hex) = value.strip_prefix("0x") {
        U256::from_str_radix(hex, 16).map_err(|_| ChainError::Rpc("invalid hex value".to_string()))
    } else {
        U256::from_str_radix(value, 10)
            .map_err(|_| ChainError::Rpc("invalid decimal value".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_signer_local::PrivateKeySigner;
    use rand::thread_rng;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved: Vec<(&str, Option<String>)> = vars
            .iter()
            .map(|(k, _)| (*k, std::env::var(k).ok()))
            .collect();
        for (k, v) in vars {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
        f();
        for (k, v) in &saved {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }

    fn base_config() -> AppConfig {
        let (coingecko_api_url, coingecko_api_key, dexscreener_api_url) =
            crate::config::test_metadata_config_fields();
        AppConfig {
            rpc_url: "https://ethereum-rpc.publicnode.com".to_string(),
            rpc_url_overridden: false,
            chain_id: 1,
            private_key: None,
            keystore_path: None,
            keystore_password_file: None,
            keystore_password_env: None,
            wallet_address: None,
            dodo_api_url: "https://api.dodoex.io".to_string(),
            dodo_api_key: String::new(),
            dodo_project_id: String::new(),
            coingecko_api_url,
            coingecko_api_key,
            dexscreener_api_url,
            data_dir: std::env::temp_dir(),
        }
    }

    #[test]
    fn resolve_signer_prefers_private_key() {
        let mut cfg = base_config();
        cfg.private_key =
            Some("0x59c6995e998f97a5a0044966f0945382dbf7f50a3f2f72f5f7a0b7d7d4f5e5f1".to_string());
        let signer = resolve_signer(&cfg).unwrap();
        assert_eq!(
            signer.address(),
            address_from_private_key(cfg.private_key.as_deref().unwrap()).unwrap()
        );
    }

    #[test]
    fn resolve_signer_reads_keystore_from_password_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut rng = thread_rng();
        let pk = "59c6995e998f97a5a0044966f0945382dbf7f50a3f2f72f5f7a0b7d7d4f5e5f1";
        let expected: PrivateKeySigner = pk.parse().unwrap();
        let (_signer, _uuid) = PrivateKeySigner::encrypt_keystore(
            dir.path(),
            &mut rng,
            hex::decode(pk).unwrap(),
            "hunter2",
            Some("test"),
        )
        .unwrap();
        let password_file = dir.path().join("password.txt");
        std::fs::write(&password_file, "hunter2\n").unwrap();

        let mut cfg = base_config();
        cfg.keystore_path = Some(dir.path().join("test").display().to_string());
        cfg.keystore_password_file = Some(password_file.display().to_string());

        let signer = resolve_signer(&cfg).unwrap();
        assert_eq!(signer.address(), expected.address());
    }

    #[test]
    fn resolve_signer_reads_keystore_password_from_env_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut rng = thread_rng();
        let pk = "59c6995e998f97a5a0044966f0945382dbf7f50a3f2f72f5f7a0b7d7d4f5e5f1";
        let expected: PrivateKeySigner = pk.parse().unwrap();
        let (_signer, _uuid) = PrivateKeySigner::encrypt_keystore(
            dir.path(),
            &mut rng,
            hex::decode(pk).unwrap(),
            "hunter2",
            Some("acct"),
        )
        .unwrap();

        let mut cfg = base_config();
        cfg.keystore_path = Some(dir.path().join("acct").display().to_string());
        cfg.keystore_password_env = Some("CHAINPILOT_KEYSTORE_PASS".to_string());

        with_env(&[("CHAINPILOT_KEYSTORE_PASS", Some("hunter2"))], || {
            let signer = resolve_signer(&cfg).unwrap();
            assert_eq!(signer.address(), expected.address());
        });
    }
}
