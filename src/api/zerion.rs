use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::api::send_retrying;
use crate::error::{ChainError, Result};

#[derive(Clone)]
pub struct ZerionClient {
    client: Client,
    base_url: String,
    api_key: String,
}

// ── portfolio ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PortfolioResponse {
    data: Option<PortfolioData>,
}

#[derive(Debug, Deserialize)]
struct PortfolioData {
    attributes: Option<PortfolioAttributes>,
}

#[derive(Debug, Deserialize)]
struct PortfolioAttributes {
    #[serde(default)]
    positions_distribution_by_chain: std::collections::HashMap<String, f64>,
    total: Option<PortfolioTotal>,
}

#[derive(Debug, Deserialize)]
struct PortfolioTotal {
    positions: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ZerionPortfolio {
    pub total_usd: Option<f64>,
    pub chains: Vec<ZerionChainBalance>,
}

#[derive(Debug, Clone)]
pub struct ZerionChainBalance {
    pub slug: String,
    pub chain_id: Option<u64>,
    pub usd_value: f64,
}

// ── positions ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PositionsResponse {
    #[serde(default)]
    data: Vec<PositionItem>,
}

#[derive(Debug, Deserialize)]
struct PositionItem {
    attributes: Option<PositionAttributes>,
    relationships: Option<PositionRelationships>,
}

#[derive(Debug, Deserialize)]
struct PositionAttributes {
    name: Option<String>,
    quantity: Option<PositionQuantity>,
    value: Option<f64>,
    price: Option<f64>,
    fungible_info: Option<FungibleInfo>,
    flags: Option<PositionFlags>,
    position_type: Option<String>,
    /// Zerion sometimes returns this as a plain string slug, sometimes as
    /// an object (`{id, name, url, ...}`). Park the raw JSON and extract
    /// downstream so unexpected shapes don't blow up the whole response.
    #[serde(default)]
    protocol: Option<serde_json::Value>,
    application_metadata: Option<ApplicationMetadata>,
}

/// Best-effort string extraction from Zerion's `protocol` field, which may be
/// a string, an object with `name`/`id`, or null.
fn extract_protocol_name(value: Option<&serde_json::Value>) -> Option<String> {
    let v = value?;
    if let Some(s) = v.as_str() {
        if !s.is_empty() {
            return Some(s.to_string());
        }
        return None;
    }
    if let Some(obj) = v.as_object() {
        for key in ["name", "id", "title"] {
            if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct PositionQuantity {
    float: Option<f64>,
    numeric: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FungibleInfo {
    name: Option<String>,
    symbol: Option<String>,
    #[serde(default)]
    implementations: Vec<FungibleImplementation>,
}

#[derive(Debug, Deserialize)]
struct FungibleImplementation {
    chain_id: Option<String>,
    address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PositionFlags {
    displayable: Option<bool>,
    is_trash: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ApplicationMetadata {
    url: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PositionRelationships {
    chain: Option<RelationshipRef>,
}

#[derive(Debug, Deserialize)]
struct RelationshipRef {
    data: Option<RelationshipData>,
}

#[derive(Debug, Deserialize)]
struct RelationshipData {
    id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ZerionPositionRecord {
    pub chain_slug: String,
    pub chain_id: Option<u64>,
    pub symbol: String,
    pub name: String,
    /// Zerion's `attributes.name` verbatim — for DeFi positions this is the
    /// position label (e.g. "Aave V3 USDC Deposit"), useful as a fallback
    /// bucket label when `protocol` is missing.
    pub display_name: Option<String>,
    pub address: String,
    pub amount: f64,
    pub price_usd: Option<f64>,
    pub value_usd: Option<f64>,
    pub position_type: String,
    pub protocol: Option<String>,
    pub protocol_url: Option<String>,
}

// ── pnl ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PnlResponse {
    data: Option<PnlData>,
}

#[derive(Debug, Deserialize)]
struct PnlData {
    attributes: Option<PnlAttributes>,
}

#[derive(Debug, Deserialize)]
struct PnlAttributes {
    realized_gain: Option<f64>,
    unrealized_gain: Option<f64>,
    relative_total_gain_percentage: Option<f64>,
    total_invested: Option<f64>,
    total_fee: Option<f64>,
    #[serde(default)]
    breakdown: Vec<PnlBreakdownItem>,
}

#[derive(Debug, Deserialize)]
struct PnlBreakdownItem {
    realized_gain: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ZerionPnlRecord {
    pub realized_pnl: Option<f64>,
    pub unrealized_pnl: Option<f64>,
    pub total_pnl: Option<f64>,
    pub roi: Option<f64>,
    pub win_rate: Option<f64>,
    pub total_invested: Option<f64>,
    pub total_fee: Option<f64>,
}

// ── transactions ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TransactionsResponse {
    #[serde(default)]
    data: Vec<TransactionItem>,
}

#[derive(Debug, Deserialize)]
struct TransactionItem {
    attributes: Option<TransactionAttributes>,
}

#[derive(Debug, Deserialize)]
struct TransactionAttributes {
    operation_type: Option<String>,
    hash: Option<String>,
    mined_at: Option<String>,
    status: Option<String>,
    fee: Option<TransactionFee>,
    #[serde(default)]
    transfers: Vec<TransactionTransfer>,
}

#[derive(Debug, Deserialize)]
struct TransactionFee {
    value: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct TransactionTransfer {
    direction: Option<String>,
    quantity: Option<TransferQuantity>,
    fungible_info: Option<FungibleInfo>,
    value: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct TransferQuantity {
    float: Option<f64>,
    numeric: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ZerionTransactionRecord {
    pub tx_hash: String,
    pub time: String,
    pub action: String,
    pub status: Option<String>,
    pub fee_usd: Option<f64>,
    pub token_in: Option<String>,
    pub token_out: Option<String>,
    pub value_usd: Option<f64>,
    pub amount: Option<f64>,
    pub success: Option<bool>,
}

// ── wallet labels ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ZerionLabelRecord {
    pub label: String,
    pub score: Option<f64>,
    pub reason: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────

impl ZerionClient {
    pub fn new(client: Client, base_url: &str, api_key: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    pub async fn portfolio(&self, address: &str) -> Result<ZerionPortfolio> {
        self.require_key()?;
        let url = format!("{}/wallets/{}/portfolio/", self.base_url, address);

        let req = self
            .client
            .get(&url)
            .basic_auth(&self.api_key, Some(""))
            .header("accept", "application/json")
            .timeout(Duration::from_secs(15))
            .query(&[("currency", "usd")]);
        let body = send_retrying(req, "zerion.portfolio")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let resp: PortfolioResponse = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "Zerion portfolio response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })?;

        let attrs = resp
            .data
            .and_then(|d| d.attributes)
            .ok_or_else(|| ChainError::Config("Zerion portfolio response was empty".to_string()))?;

        let total_usd = attrs.total.and_then(|t| t.positions);
        let chains: Vec<ZerionChainBalance> = attrs
            .positions_distribution_by_chain
            .into_iter()
            .filter(|(_, v)| *v > 0.0)
            .map(|(slug, usd_value)| ZerionChainBalance {
                chain_id: zerion_chain_to_id(&slug),
                slug,
                usd_value,
            })
            .collect();

        // Surface the raw slugs Zerion returned so a user hitting an unknown
        // chain can spot the mismatch in debug logs without having to log the
        // full ~100 KB portfolio body.
        for c in &chains {
            if c.chain_id.is_none() {
                tracing::warn!(
                    target: "zerion",
                    slug = %c.slug,
                    usd_value = c.usd_value,
                    "zerion chain slug not mapped to EVM id — extend zerion_chain_to_id"
                );
            } else {
                tracing::debug!(
                    target: "zerion",
                    slug = %c.slug,
                    chain_id = ?c.chain_id,
                    usd_value = c.usd_value,
                    "zerion chain mapped",
                );
            }
        }

        Ok(ZerionPortfolio { total_usd, chains })
    }

    /// Fetch wallet positions. When `only_simple` is true, Zerion returns only
    /// plain token balances (no DeFi positions) — that maps to our `assets[]`
    /// notion. When false, DeFi positions are included, which `overview` uses
    /// to populate `active_protocols`.
    pub async fn positions(
        &self,
        address: &str,
        only_simple: bool,
        chain_filter: Option<u64>,
    ) -> Result<Vec<ZerionPositionRecord>> {
        self.require_key()?;
        let url = format!("{}/wallets/{}/positions/", self.base_url, address);

        let mut query: Vec<(&str, String)> = vec![
            ("currency", "usd".to_string()),
            ("sort", "value".to_string()),
            ("page[size]", "100".to_string()),
        ];
        query.push((
            "filter[positions]",
            if only_simple { "only_simple" } else { "no_filter" }.to_string(),
        ));
        if let Some(chain_id) = chain_filter {
            // Translate the EVM chain id back to Zerion's slug so the API
            // does the filtering for us — fewer payload bytes to parse.
            let slug = id_to_zerion_chain(chain_id).ok_or_else(|| {
                ChainError::Config(format!(
                    "Zerion does not recognize chain id {chain_id}"
                ))
            })?;
            query.push(("filter[chain_ids]", slug.to_string()));
        }

        let req = self
            .client
            .get(&url)
            .basic_auth(&self.api_key, Some(""))
            .header("accept", "application/json")
            .timeout(Duration::from_secs(20))
            .query(&query);
        let body = send_retrying(req, "zerion.positions")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let resp: PositionsResponse = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "Zerion positions response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })?;

        Ok(resp.data.into_iter().filter_map(map_position).collect())
    }

    pub async fn pnl(&self, address: &str) -> Result<ZerionPnlRecord> {
        self.require_key()?;
        let url = format!("{}/wallets/{}/pnl/", self.base_url, address);

        let req = self
            .client
            .get(&url)
            .basic_auth(&self.api_key, Some(""))
            .header("accept", "application/json")
            .timeout(Duration::from_secs(15))
            .query(&[("currency", "usd")]);
        let body = send_retrying(req, "zerion.pnl")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let resp: PnlResponse = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "Zerion pnl response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })?;

        let attrs = resp
            .data
            .and_then(|d| d.attributes)
            .ok_or_else(|| ChainError::Config("Zerion pnl response was empty".to_string()))?;

        let realized = attrs.realized_gain;
        let unrealized = attrs.unrealized_gain;
        let total_pnl = match (realized, unrealized) {
            (Some(r), Some(u)) => Some(r + u),
            (Some(r), None) => Some(r),
            (None, Some(u)) => Some(u),
            (None, None) => None,
        };

        let win_rate = if attrs.breakdown.is_empty() {
            None
        } else {
            let winners = attrs
                .breakdown
                .iter()
                .filter(|b| b.realized_gain.unwrap_or(0.0) > 0.0)
                .count();
            Some(winners as f64 / attrs.breakdown.len() as f64 * 100.0)
        };

        Ok(ZerionPnlRecord {
            realized_pnl: realized,
            unrealized_pnl: unrealized,
            total_pnl,
            roi: attrs.relative_total_gain_percentage,
            win_rate,
            total_invested: attrs.total_invested,
            total_fee: attrs.total_fee,
        })
    }

    /// Derive behavioral labels from wallet positions and portfolio data.
    /// This analyzes position types, protocols, and portfolio characteristics
    /// to generate wallet behavioral tags.
    pub async fn wallet_labels(&self, address: &str) -> Result<Vec<ZerionLabelRecord>> {
        let (portfolio, positions) = tokio::join!(
            self.portfolio(address),
            self.positions(address, false, None),
        );
        let portfolio = portfolio?;
        let positions = positions?;

        let mut labels: Vec<ZerionLabelRecord> = Vec::new();
        let total_usd = portfolio.total_usd.unwrap_or(0.0);

        // ── Value tier labels ──────────────────────────────────────────────
        if total_usd >= 1_000_000.0 {
            labels.push(ZerionLabelRecord {
                label: "whale".to_string(),
                score: Some(1.0),
                reason: Some(format!("Portfolio ${:.0} > $1M", total_usd)),
            });
        } else if total_usd >= 100_000.0 {
            labels.push(ZerionLabelRecord {
                label: "dolphin".to_string(),
                score: Some(0.9),
                reason: Some(format!("Portfolio ${:.0} in $100K-$1M range", total_usd)),
            });
        } else if total_usd >= 10_000.0 {
            labels.push(ZerionLabelRecord {
                label: "fish".to_string(),
                score: Some(0.8),
                reason: Some(format!("Portfolio ${:.0} in $10K-$100K range", total_usd)),
            });
        } else if total_usd > 0.0 {
            labels.push(ZerionLabelRecord {
                label: "shrimp".to_string(),
                score: Some(0.7),
                reason: Some(format!("Portfolio ${:.0} < $10K", total_usd)),
            });
        }

        // ── Categorize positions ───────────────────────────────────────────
        let mut defi_positions: Vec<&ZerionPositionRecord> = Vec::new();
        let mut wallet_positions: Vec<&ZerionPositionRecord> = Vec::new();
        let mut protocols: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut chains: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut protocol_usd: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        let mut position_types: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        for p in &positions {
            chains.insert(p.chain_slug.clone());
            *position_types.entry(p.position_type.clone()).or_insert(0) += 1;

            if p.position_type == "wallet" {
                wallet_positions.push(p);
            } else {
                defi_positions.push(p);
                if let Some(ref proto) = p.protocol {
                    protocols.insert(proto.clone());
                    *protocol_usd.entry(proto.clone()).or_insert(0.0) += p.value_usd.unwrap_or(0.0);
                }
            }
        }

        // ── Protocol-specific labels ───────────────────────────────────────
        let known_protocols: &[(&str, &str, &str)] = &[
            ("aave-v3", "aave-user", "Aave V3"),
            ("aave-v2", "aave-user", "Aave V2"),
            ("uniswap-v3", "uniswap-trader", "Uniswap V3"),
            ("uniswap-v2", "uniswap-trader", "Uniswap V2"),
            ("lido", "lido-staker", "Lido"),
            ("rocket-pool", "rocket-pool-staker", "Rocket Pool"),
            ("compound-v3", "compound-user", "Compound V3"),
            ("compound-v2", "compound-user", "Compound V2"),
            ("curve", "curve-user", "Curve"),
            ("maker", "maker-user", "MakerDAO"),
            ("gmx", "gmx-trader", "GMX"),
            ("dydx", "perp-trader", "dYdX"),
            ("eigenlayer", "eigenlayer-restaker", "EigenLayer"),
            ("pendle", "pendle-user", "Pendle"),
            ("morpho", "morpho-user", "Morpho"),
            ("spark", "spark-user", "Spark"),
        ];

        for (proto_key, label, display_name) in known_protocols {
            if protocols.contains(*proto_key) {
                let usd = protocol_usd.get(*proto_key).copied().unwrap_or(0.0);
                let score = if usd >= 100_000.0 { 0.95 } else if usd >= 10_000.0 { 0.85 } else { 0.7 };
                labels.push(ZerionLabelRecord {
                    label: label.to_string(),
                    score: Some(score),
                    reason: Some(format!("{} position ${:.0}", display_name, usd)),
                });
            }
        }

        // ── Behavior labels ────────────────────────────────────────────────
        // DeFi user
        if defi_positions.len() >= 3 {
            labels.push(ZerionLabelRecord {
                label: "defi-user".to_string(),
                score: Some(0.9),
                reason: Some(format!("{} DeFi positions across {} protocols", defi_positions.len(), protocols.len())),
            });
        } else if !defi_positions.is_empty() {
            labels.push(ZerionLabelRecord {
                label: "defi-user".to_string(),
                score: Some(0.6),
                reason: Some(format!("{} DeFi position(s)", defi_positions.len())),
            });
        }

        // Yield farmer: multiple deposit/earn positions
        let deposit_count = position_types.get("deposit").copied().unwrap_or(0)
            + position_types.get("earn").copied().unwrap_or(0);
        if deposit_count >= 3 {
            labels.push(ZerionLabelRecord {
                label: "yield-farmer".to_string(),
                score: Some(0.85),
                reason: Some(format!("{} deposit/earn positions", deposit_count)),
            });
        }

        // Liquidity provider: has LP positions
        let lp_count = position_types.get("liquidity").copied().unwrap_or(0)
            + position_types.get("lp").copied().unwrap_or(0);
        if lp_count >= 1 {
            labels.push(ZerionLabelRecord {
                label: "liquidity-provider".to_string(),
                score: Some(0.8),
                reason: Some(format!("{} liquidity position(s)", lp_count)),
            });
        }

        // Staker: has stake positions
        let stake_count = position_types.get("stake").copied().unwrap_or(0)
            + position_types.get("staking").copied().unwrap_or(0);
        if stake_count >= 1 {
            labels.push(ZerionLabelRecord {
                label: "staker".to_string(),
                score: Some(0.8),
                reason: Some(format!("{} staking position(s)", stake_count)),
            });
        }

        // Lender: has lend positions
        let lend_count = position_types.get("lend").copied().unwrap_or(0)
            + position_types.get("supply").copied().unwrap_or(0);
        if lend_count >= 1 {
            labels.push(ZerionLabelRecord {
                label: "lender".to_string(),
                score: Some(0.75),
                reason: Some(format!("{} lending position(s)", lend_count)),
            });
        }

        // Borrower: has borrow positions
        let borrow_count = position_types.get("borrow").copied().unwrap_or(0)
            + position_types.get("debt").copied().unwrap_or(0);
        if borrow_count >= 1 {
            labels.push(ZerionLabelRecord {
                label: "borrower".to_string(),
                score: Some(0.75),
                reason: Some(format!("{} borrow position(s)", borrow_count)),
            });
        }

        // Multi-chain user
        if chains.len() >= 3 {
            labels.push(ZerionLabelRecord {
                label: "multi-chain".to_string(),
                score: Some(0.8),
                reason: Some(format!("Active on {} chains", chains.len())),
            });
        }

        // Diverse portfolio
        if wallet_positions.len() >= 10 {
            labels.push(ZerionLabelRecord {
                label: "diverse-portfolio".to_string(),
                score: Some(0.7),
                reason: Some(format!("Holds {} different tokens", wallet_positions.len())),
            });
        }

        // ── Risk profile labels ────────────────────────────────────────────
        let degen_protocols = ["gmx", "dydx", "kwenta", "gains", "vela"];
        let is_degen = degen_protocols.iter().any(|p| protocols.contains(*p));
        if is_degen {
            labels.push(ZerionLabelRecord {
                label: "degen".to_string(),
                score: Some(0.85),
                reason: Some("Uses high-risk protocols (perps/leverage)".to_string()),
            });
        }

        let blue_chip = ["aave-v3", "aave-v2", "lido", "compound-v3", "maker"];
        let blue_chip_count = blue_chip.iter().filter(|p| protocols.contains(**p)).count();
        if blue_chip_count >= 2 && !is_degen && defi_positions.len() <= 5 {
            labels.push(ZerionLabelRecord {
                label: "conservative".to_string(),
                score: Some(0.75),
                reason: Some(format!("Uses {} blue-chip protocols only", blue_chip_count)),
            });
        }

        Ok(labels)
    }

    pub async fn transactions(
        &self,
        address: &str,
        limit: u32,
    ) -> Result<Vec<ZerionTransactionRecord>> {
        self.require_key()?;
        let url = format!("{}/wallets/{}/transactions/", self.base_url, address);

        let req = self
            .client
            .get(&url)
            .basic_auth(&self.api_key, Some(""))
            .header("accept", "application/json")
            .timeout(Duration::from_secs(20))
            .query(&[
                ("currency", "usd".to_string()),
                ("page[size]", limit.to_string()),
            ]);
        let body = send_retrying(req, "zerion.transactions")
            .await?
            .error_for_status()
            .map_err(ChainError::Http)?
            .text()
            .await
            .map_err(ChainError::Http)?;

        let resp: TransactionsResponse = serde_json::from_str(&body).map_err(|e| {
            ChainError::Config(format!(
                "Zerion transactions response could not be parsed: {e}. Body snippet: {}",
                snippet(&body)
            ))
        })?;

        Ok(resp
            .data
            .into_iter()
            .filter_map(|item| {
                let attrs = item.attributes?;
                let tx_hash = attrs.hash?;
                let time = attrs.mined_at.unwrap_or_default();
                let action = attrs
                    .operation_type
                    .unwrap_or_else(|| "unknown".to_string());
                let success = attrs.status.as_deref().map(|s| s == "confirmed");
                let fee_usd = attrs.fee.and_then(|f| f.value);

                // Extract token_in / token_out / value_usd / amount from transfers
                let mut token_in: Option<String> = None;
                let mut token_out: Option<String> = None;
                let mut amount: Option<f64> = None;
                let mut total_value_in: f64 = 0.0;
                let mut total_value_out: f64 = 0.0;

                for t in &attrs.transfers {
                    let symbol = t
                        .fungible_info
                        .as_ref()
                        .and_then(|f| f.symbol.clone())
                        .unwrap_or_else(|| "unknown".to_string());
                    let val = t.value.unwrap_or(0.0);
                    let qty = t.quantity.as_ref().and_then(|q| {
                        q.float.or_else(|| {
                            q.numeric.as_deref().and_then(|s| s.parse::<f64>().ok())
                        })
                    });
                    match t.direction.as_deref() {
                        Some("in") => {
                            if token_in.is_none() {
                                token_in = Some(symbol);
                                amount = qty;
                            }
                            total_value_in += val;
                        }
                        Some("out") => {
                            if token_out.is_none() {
                                token_out = Some(symbol);
                                // Take amount from out direction when no in transfer
                                if amount.is_none() {
                                    amount = qty;
                                }
                            }
                            total_value_out += val;
                        }
                        _ => {}
                    }
                }

                let value_usd = if total_value_in > 0.0 {
                    Some(total_value_in)
                } else if total_value_out > 0.0 {
                    Some(total_value_out)
                } else {
                    None
                };

                Some(ZerionTransactionRecord {
                    tx_hash,
                    time,
                    action,
                    status: attrs.status,
                    fee_usd,
                    token_in,
                    token_out,
                    value_usd,
                    amount,
                    success,
                })
            })
            .collect())
    }

    fn require_key(&self) -> Result<()> {
        if self.api_key.is_empty() {
            Err(ChainError::Config(
                "ZERION_API_KEY not set. Run: chainpilot config set zerion_api_key <key>"
                    .to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

fn map_position(item: PositionItem) -> Option<ZerionPositionRecord> {
    let attrs = item.attributes?;
    let flags = attrs.flags.unwrap_or(PositionFlags {
        displayable: Some(true),
        is_trash: Some(false),
    });
    if !flags.displayable.unwrap_or(true) || flags.is_trash.unwrap_or(false) {
        return None;
    }

    let quantity = attrs.quantity?;
    let amount = quantity
        .float
        .or_else(|| quantity.numeric.as_deref().and_then(|s| s.parse::<f64>().ok()))?;
    if amount <= 0.0 {
        return None;
    }

    let fungible = attrs.fungible_info.unwrap_or(FungibleInfo {
        name: None,
        symbol: None,
        implementations: Vec::new(),
    });

    let chain_slug = item
        .relationships
        .and_then(|r| r.chain)
        .and_then(|c| c.data)
        .and_then(|d| d.id)
        .unwrap_or_default();
    let chain_id = if chain_slug.is_empty() {
        None
    } else {
        zerion_chain_to_id(&chain_slug)
    };

    // Pick the implementation address matching the active chain when possible.
    let address = fungible
        .implementations
        .iter()
        .find(|i| i.chain_id.as_deref() == Some(chain_slug.as_str()))
        .and_then(|i| i.address.clone())
        .unwrap_or_default();

    let symbol = fungible
        .symbol
        .clone()
        .or_else(|| attrs.name.clone())
        .unwrap_or_default();
    let name = fungible.name.unwrap_or_else(|| symbol.clone());

    let display_name = attrs.name.clone();
    let protocol = extract_protocol_name(attrs.protocol.as_ref()).or_else(|| {
        attrs
            .application_metadata
            .as_ref()
            .and_then(|m| m.name.clone())
    });
    let position_type = attrs.position_type.unwrap_or_else(|| "wallet".to_string());

    tracing::debug!(
        target: "zerion",
        chain_slug = %chain_slug,
        symbol = %symbol,
        position_type = %position_type,
        protocol = ?protocol,
        display_name = ?display_name,
        value_usd = ?attrs.value,
        "zerion position parsed",
    );

    Some(ZerionPositionRecord {
        chain_slug,
        chain_id,
        symbol,
        name,
        display_name,
        address,
        amount,
        price_usd: attrs.price,
        value_usd: attrs.value,
        position_type,
        protocol,
        protocol_url: attrs.application_metadata.and_then(|m| m.url),
    })
}

/// Trim a response body for inclusion in error messages — Zerion responses
/// can be 100+ KB, but the first 400 chars are usually enough to tell whether
/// it's an auth error, a rate-limit notice, or a schema surprise.
fn snippet(body: &str) -> String {
    const MAX: usize = 400;
    if body.len() <= MAX {
        body.to_string()
    } else {
        let mut end = MAX;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &body[..end])
    }
}

/// Zerion chain slug → EVM chain id. Slugs are stable strings used in Zerion's
/// `relationships.chain` and `filter[chain_ids]` query param. Aliases cover
/// historical variants Zerion has used across endpoints (e.g. `arbitrum` vs
/// `arbitrum-one`, `matic` vs `polygon`, `bsc` vs `binance-smart-chain`).
///
/// Keep [`id_to_zerion_chain`] aligned with the canonical (first-listed) slug
/// for each chain.
pub fn zerion_chain_to_id(slug: &str) -> Option<u64> {
    match slug {
        "ethereum" | "eth" => Some(1),
        "binance-smart-chain" | "bsc" | "bnb" => Some(56),
        "polygon" | "polygon-pos" | "matic" => Some(137),
        "arbitrum" | "arbitrum-one" => Some(42161),
        "arbitrum-nova" => Some(42170),
        "optimism" | "optimistic-ethereum" => Some(10),
        "avalanche" | "avalanche-c-chain" => Some(43114),
        "base" => Some(8453),
        "linea" => Some(59144),
        "scroll" => Some(534352),
        "mantle" => Some(5000),
        "aurora" => Some(1313161554),
        "manta-pacific" | "manta" => Some(169),
        "taiko" => Some(167000),
        "fantom" | "fantom-opera" => Some(250),
        "xdai" | "gnosis" => Some(100),
        "celo" => Some(42220),
        "zksync-era" | "zksync" => Some(324),
        "polygon-zkevm" => Some(1101),
        _ => None,
    }
}

/// Reverse mapping for pushing `--chain-id` filters into Zerion's query string.
/// Returns the canonical slug Zerion accepts; callers pass this value into
/// `filter[chain_ids]=...`.
///
/// Some chains supported by this CLI are deliberately absent because Zerion
/// does not index them (as of writing): OKChain/X Layer (66), Conflux eSpace
/// (1030), Plume (98866). Hitting those chain ids forces `wallet balance` /
/// `wallet overview` to fall through to Goldrush or on-chain RPC.
pub fn id_to_zerion_chain(id: u64) -> Option<&'static str> {
    match id {
        1 => Some("ethereum"),
        56 => Some("binance-smart-chain"),
        137 => Some("polygon"),
        42161 => Some("arbitrum"),
        10 => Some("optimism"),
        43114 => Some("avalanche"),
        8453 => Some("base"),
        59144 => Some("linea"),
        534352 => Some("scroll"),
        5000 => Some("mantle"),
        1313161554 => Some("aurora"),
        169 => Some("manta-pacific"),
        167000 => Some("taiko"),
        250 => Some("fantom"),
        100 => Some("xdai"),
        42220 => Some("celo"),
        324 => Some("zksync-era"),
        1101 => Some("polygon-zkevm"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_slug_round_trips_for_supported_chains() {
        for id in [
            1u64, 56, 137, 42161, 10, 43114, 8453, 59144, 534352, 5000, 169, 167000,
        ] {
            let slug = id_to_zerion_chain(id).unwrap_or_else(|| panic!("missing slug for {id}"));
            assert_eq!(
                zerion_chain_to_id(slug),
                Some(id),
                "round-trip failed for {id}/{slug}",
            );
        }
    }

    /// Every mainnet chain registered in `config::chains::CHAINS` must either
    /// have a Zerion slug mapping or be explicitly listed here as unsupported.
    /// If this test fails after adding a new chain, either add the mapping or
    /// add the chain id to the `unsupported` set.
    #[test]
    fn all_config_chains_have_zerion_mapping_or_are_documented_unsupported() {
        let unsupported: std::collections::HashSet<u64> = [66u64, 1030, 98866, 11155111]
            .into_iter()
            .collect();
        for id in crate::config::chains::all_chain_ids() {
            if id_to_zerion_chain(id).is_some() {
                continue;
            }
            assert!(
                unsupported.contains(&id),
                "chain id {id} has no Zerion slug and is not listed as unsupported — \
                 either add a mapping in id_to_zerion_chain or add it to the \
                 unsupported set in this test"
            );
        }
    }

    #[test]
    fn xdai_and_gnosis_aliases_both_map_to_100() {
        assert_eq!(zerion_chain_to_id("xdai"), Some(100));
        assert_eq!(zerion_chain_to_id("gnosis"), Some(100));
    }

    #[test]
    fn unknown_chain_returns_none() {
        assert_eq!(zerion_chain_to_id("unknown"), None);
        assert_eq!(id_to_zerion_chain(999_999), None);
    }

    #[test]
    fn client_without_key_reports_unconfigured() {
        let http = Client::new();
        let client = ZerionClient::new(http, "https://api.zerion.io/v1", "");
        assert!(!client.is_configured());
        assert!(client.require_key().is_err());
    }

    #[test]
    fn client_with_key_is_configured() {
        let http = Client::new();
        let client = ZerionClient::new(http, "https://api.zerion.io/v1", "test-key");
        assert!(client.is_configured());
        assert!(client.require_key().is_ok());
    }
}
