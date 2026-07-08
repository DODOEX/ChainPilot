//! mempool.space client for Bitcoin mainnet wallet data.
//!
//! Free public API (https://mempool.space/docs/api/rest), no key required.
//! Covers the read-only surface we need for BVM:
//! - `address/{addr}` — confirmed + mempool BTC balance (sats)
//! - `address/{addr}/txs` — recent transactions, newest first
//!
//! The endpoints return UTXO-derived figures, not USD; we attach BTC price
//! at the call site if a price source is available.

use reqwest::Client;
use serde::Deserialize;

use crate::api::send_retrying;
use crate::error::{ChainError, Result};

pub const DEFAULT_MEMPOOL_API_URL: &str = "https://mempool.space/api";

#[derive(Clone)]
pub struct MempoolClient {
    client: Client,
    base_url: String,
}

#[derive(Debug, Clone)]
pub struct BitcoinBalance {
    /// Confirmed balance in BTC.
    pub confirmed_btc: f64,
    /// Net mempool (unconfirmed) balance delta in BTC. Can be negative.
    pub mempool_btc: f64,
    /// Total transaction count (confirmed + mempool). Surfaced for future
    /// summaries; current `wallet balance` output doesn't yet display it.
    #[allow(dead_code)]
    pub tx_count: u64,
}

impl BitcoinBalance {
    /// Total spendable + pending balance in BTC.
    pub fn total_btc(&self) -> f64 {
        self.confirmed_btc + self.mempool_btc
    }
}

#[derive(Debug, Clone)]
pub struct BitcoinTx {
    pub txid: String,
    /// Unix seconds of inclusion. None for unconfirmed txs.
    pub block_time: Option<u64>,
    /// Net value delta against the queried address, in BTC (positive = received).
    pub net_btc: f64,
    /// Fee paid by the transaction, in BTC. Always available (mempool too).
    pub fee_btc: f64,
    pub confirmed: bool,
}

// ── mempool.space response types ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AddressStats {
    chain_stats: ChainStats,
    mempool_stats: ChainStats,
}

#[derive(Debug, Deserialize)]
struct ChainStats {
    funded_txo_sum: i64,
    spent_txo_sum: i64,
    tx_count: u64,
}

#[derive(Debug, Deserialize)]
struct TxResponse {
    txid: String,
    fee: u64,
    status: TxStatus,
    vin: Vec<TxInput>,
    vout: Vec<TxOutput>,
}

#[derive(Debug, Deserialize)]
struct TxStatus {
    confirmed: bool,
    block_time: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TxInput {
    prevout: Option<TxOutput>,
}

#[derive(Debug, Deserialize)]
struct TxOutput {
    value: u64,
    scriptpubkey_address: Option<String>,
}

const SAT_PER_BTC: f64 = 100_000_000.0;

fn sats_to_btc(sats: i64) -> f64 {
    sats as f64 / SAT_PER_BTC
}

impl MempoolClient {
    pub fn new(client: Client, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Fetch confirmed + mempool balance for a Bitcoin address.
    pub async fn address_balance(&self, address: &str) -> Result<BitcoinBalance> {
        let url = format!("{}/address/{}", self.base_url, address);
        let body = send_retrying(self.client.get(&url), "mempool.address")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let stats: AddressStats = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "mempool.space address response could not be parsed: {e}"
            ))
        })?;

        let confirmed = stats.chain_stats.funded_txo_sum - stats.chain_stats.spent_txo_sum;
        let mempool_delta =
            stats.mempool_stats.funded_txo_sum - stats.mempool_stats.spent_txo_sum;

        Ok(BitcoinBalance {
            confirmed_btc: sats_to_btc(confirmed),
            mempool_btc: sats_to_btc(mempool_delta),
            tx_count: stats.chain_stats.tx_count + stats.mempool_stats.tx_count,
        })
    }

    /// Fetch the most recent transactions touching this address. The endpoint
    /// returns up to 50 entries; we expose `limit` as a soft cap.
    pub async fn address_txs(&self, address: &str, limit: usize) -> Result<Vec<BitcoinTx>> {
        let url = format!("{}/address/{}/txs", self.base_url, address);
        let body = send_retrying(self.client.get(&url), "mempool.address_txs")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let raw: Vec<TxResponse> = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "mempool.space txs response could not be parsed: {e}"
            ))
        })?;

        let txs: Vec<BitcoinTx> = raw
            .into_iter()
            .take(limit)
            .map(|tx| {
                let received: u64 = tx
                    .vout
                    .iter()
                    .filter(|o| o.scriptpubkey_address.as_deref() == Some(address))
                    .map(|o| o.value)
                    .sum();
                let spent: u64 = tx
                    .vin
                    .iter()
                    .filter_map(|i| i.prevout.as_ref())
                    .filter(|o| o.scriptpubkey_address.as_deref() == Some(address))
                    .map(|o| o.value)
                    .sum();
                let net = received as i64 - spent as i64;
                BitcoinTx {
                    txid: tx.txid,
                    block_time: tx.status.block_time,
                    net_btc: sats_to_btc(net),
                    fee_btc: sats_to_btc(tx.fee as i64),
                    confirmed: tx.status.confirmed,
                }
            })
            .collect();

        Ok(txs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sats_conversion_round_trips_full_btc() {
        assert_eq!(sats_to_btc(100_000_000), 1.0);
        assert_eq!(sats_to_btc(50_000_000), 0.5);
        assert_eq!(sats_to_btc(0), 0.0);
    }

    #[test]
    fn balance_total_combines_confirmed_and_mempool() {
        let b = BitcoinBalance {
            confirmed_btc: 1.5,
            mempool_btc: -0.25,
            tx_count: 12,
        };
        assert_eq!(b.total_btc(), 1.25);
    }

    #[test]
    fn parses_real_mempool_address_response() {
        // Sample matching the genesis coinbase address (1A1z…) shape.
        let body = r#"{
            "chain_stats": {"funded_txo_sum": 8190123456, "spent_txo_sum": 100000000, "tx_count": 1234},
            "mempool_stats": {"funded_txo_sum": 0, "spent_txo_sum": 0, "tx_count": 0}
        }"#;
        let stats: AddressStats = serde_json::from_str(body).unwrap();
        let confirmed = stats.chain_stats.funded_txo_sum - stats.chain_stats.spent_txo_sum;
        assert_eq!(sats_to_btc(confirmed), 80.90123456);
    }
}
