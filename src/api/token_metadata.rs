use std::time::Duration;

use crate::config::AppConfig;
use crate::models::token::{
    TokenInfo, TokenPrice, TokenPriceSources, TokenSearchCandidate, TokenSearchResult,
    TokenSocialLinks,
};
use reqwest::Client;
use serde::Deserialize;

#[derive(Clone)]
pub struct TokenMetadataClient {
    client: Client,
    coingecko_base_url: String,
    coingecko_api_key: Option<String>,
    dexscreener_base_url: String,
}

#[derive(Debug, Default)]
struct TokenMetadataPatch {
    name: Option<(String, String)>,
    symbol: Option<(String, String)>,
    address: Option<(String, String)>,
    website: Option<(String, String)>,
    social_links: Option<(TokenSocialLinks, String)>,
    price: Option<(f64, String)>,
    market_cap: Option<(f64, String)>,
    fdv: Option<(f64, String)>,
    top_liquidity: Option<(f64, String)>,
    volume_24h: Option<(f64, String)>,
    price_change_24h: Option<(f64, String)>,
    risk_level: Option<(String, String)>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoToken {
    symbol: Option<String>,
    name: Option<String>,
    links: Option<CoinGeckoLinks>,
    market_data: Option<CoinGeckoMarketData>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoSearchResponse {
    coins: Option<Vec<CoinGeckoSearchCoin>>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoSearchCoin {
    symbol: Option<String>,
    name: Option<String>,
    platforms: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoLinks {
    homepage: Option<Vec<String>>,
    blockchain_site: Option<Vec<String>>,
    official_forum_url: Option<Vec<String>>,
    chat_url: Option<Vec<String>>,
    twitter_screen_name: Option<String>,
    telegram_channel_identifier: Option<String>,
    repos_url: Option<CoinGeckoRepos>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoRepos {
    github: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoMarketData {
    current_price: Option<UsdValue>,
    market_cap: Option<UsdValue>,
    fully_diluted_valuation: Option<UsdValue>,
    total_volume: Option<UsdValue>,
    high_24h: Option<UsdValue>,
    low_24h: Option<UsdValue>,
    price_change_percentage_1h_in_currency: Option<UsdValue>,
    price_change_percentage_24h: Option<f64>,
    price_change_percentage_7d_in_currency: Option<UsdValue>,
}

#[derive(Debug, Deserialize)]
struct UsdValue {
    usd: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct DexScreenerResponse {
    pairs: Option<Vec<DexScreenerPair>>,
}

#[derive(Debug, Deserialize)]
struct DexScreenerPair {
    #[serde(rename = "chainId")]
    chain_id: Option<String>,
    #[serde(rename = "baseToken")]
    base_token: Option<DexScreenerToken>,
    liquidity: Option<DexScreenerLiquidity>,
    #[serde(rename = "priceUsd")]
    price_usd: Option<String>,
    #[serde(rename = "priceChange")]
    price_change: Option<DexScreenerPriceChange>,
    #[serde(rename = "dexId")]
    dex_id: Option<String>,
    #[serde(rename = "pairAddress")]
    pair_address: Option<String>,
    volume: Option<DexScreenerVolume>,
}

#[derive(Debug, Deserialize)]
struct DexScreenerPriceChange {
    h1: Option<f64>,
    h24: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct DexScreenerVolume {
    h24: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct DexScreenerToken {
    address: Option<String>,
    name: Option<String>,
    symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DexScreenerLiquidity {
    usd: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct GoPlusResponse {
    code: u64,
    result: Option<std::collections::HashMap<String, GoPlusTokenSecurity>>,
}

/// Subset of GoPlus's Solana `token_security` payload. Solana SPL tokens
/// expose authority-based risk (mint authority, freeze authority, etc.)
/// rather than the EVM honeypot/blacklist fields, so the schema is
/// disjoint from [`GoPlusTokenSecurity`].
#[derive(Debug, Deserialize)]
struct GoPlusSvmResponse {
    code: u64,
    result: Option<std::collections::HashMap<String, GoPlusSvmTokenSecurity>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct GoPlusSvmTokenSecurity {
    mintable: Option<GoPlusAuthority>,
    freezable: Option<GoPlusAuthority>,
    closable: Option<GoPlusAuthority>,
    transfer_fee: Option<GoPlusTransferFee>,
    transfer_hook: Option<Vec<serde_json::Value>>,
    non_transferable: Option<String>,
    trusted_token: Option<u8>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct GoPlusAuthority {
    authority: Option<Vec<serde_json::Value>>,
}

impl GoPlusAuthority {
    fn is_active(&self) -> bool {
        self.authority.as_ref().is_some_and(|a| !a.is_empty())
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct GoPlusTransferFee {
    #[serde(default)]
    transfer_fee: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct GoPlusTokenSecurity {
    #[serde(rename = "is_honeypot")]
    is_honeypot: Option<String>,
    #[serde(rename = "is_blacklisted")]
    is_blacklisted: Option<String>,
    #[serde(rename = "transfer_pausable")]
    transfer_pausable: Option<String>,
    #[serde(rename = "is_mintable")]
    is_mintable: Option<String>,
    #[serde(rename = "owner_change_balance")]
    owner_change_balance: Option<String>,
    #[serde(rename = "buy_tax")]
    buy_tax: Option<String>,
    #[serde(rename = "sell_tax")]
    sell_tax: Option<String>,
    #[serde(rename = "trust_list")]
    trust_list: Option<String>,
}

/// Address-reputation flags from GoPlus's malicious-address library. The
/// library is keyed on the raw address string and is largely chain-agnostic,
/// so it covers non-EVM addresses (e.g. Solana base58) that GoPlus has seen
/// flagged. Each field is `true` only when GoPlus returns `"1"`.
#[derive(Clone, Debug, Default)]
pub struct AddressSecurity {
    pub sanctioned: bool,
    pub phishing_activities: bool,
    pub stealing_attack: bool,
    pub blackmail_activities: bool,
    pub cybercrime: bool,
    pub money_laundering: bool,
    pub financial_crime: bool,
    pub darkweb_transactions: bool,
    pub fake_kyc: bool,
    pub malicious_mining_activities: bool,
    pub honeypot_related_address: bool,
    pub blacklist_doubt: bool,
    pub mixer: bool,
}

impl AddressSecurity {
    /// True when GoPlus returned no risk flags for the address — i.e. it has a
    /// record but nothing adverse. Lets callers distinguish "clean" from the
    /// `None` case (no record / request failed).
    pub fn is_clean(&self) -> bool {
        !(self.sanctioned
            || self.phishing_activities
            || self.stealing_attack
            || self.blackmail_activities
            || self.cybercrime
            || self.money_laundering
            || self.financial_crime
            || self.darkweb_transactions
            || self.fake_kyc
            || self.malicious_mining_activities
            || self.honeypot_related_address
            || self.blacklist_doubt
            || self.mixer)
    }
}

/// GoPlus `address_security` returns a flat `result` object (unlike
/// `token_security`, which keys results by contract address).
#[derive(Debug, Deserialize)]
struct GoPlusAddressSecurityResponse {
    code: u64,
    // Kept as a raw `Value` so we can defensively verify the shape before
    // trusting it: deserializing a keyed map (e.g. a future schema change to
    // `{ "<addr>": {...} }`) straight into the flat struct would silently
    // yield all-`None` (an unflagged / "clean" result), a false negative.
    result: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct GoPlusAddressSecurityRaw {
    #[serde(default)]
    sanctioned: Option<String>,
    #[serde(default)]
    phishing_activities: Option<String>,
    #[serde(default)]
    stealing_attack: Option<String>,
    #[serde(default)]
    blackmail_activities: Option<String>,
    #[serde(default)]
    cybercrime: Option<String>,
    #[serde(default)]
    money_laundering: Option<String>,
    #[serde(default)]
    financial_crime: Option<String>,
    #[serde(default)]
    darkweb_transactions: Option<String>,
    #[serde(default)]
    fake_kyc: Option<String>,
    #[serde(default)]
    malicious_mining_activities: Option<String>,
    #[serde(default)]
    honeypot_related_address: Option<String>,
    #[serde(default)]
    blacklist_doubt: Option<String>,
    #[serde(default)]
    mixer: Option<String>,
}

/// GoPlus encodes booleans as the strings `"1"` (true) / `"0"` (false).
fn goplus_flag(value: &Option<String>) -> bool {
    value.as_deref() == Some("1")
}

/// Parse a GoPlus `address_security` response body into [`AddressSecurity`].
/// Pure (no I/O) so the shape guard is unit-testable. Returns `None` when the
/// request wasn't a success (`code != 1`), there's no `result`, or the result
/// isn't the expected flat reputation object — the last case guards against a
/// schema drift silently reading as a false "clean".
fn parse_address_security(body_text: &str) -> Option<AddressSecurity> {
    let data: GoPlusAddressSecurityResponse = serde_json::from_str(body_text).ok()?;
    if data.code != 1 {
        return None;
    }
    let result = data.result?;
    // The flat reputation object must actually carry a known flag key. A keyed
    // map (`{ "<addr>": {...} }`) or renamed schema fails this check and is
    // treated as "no record" (`None`) rather than an unflagged address.
    let obj = result.as_object()?;
    if !obj.contains_key("sanctioned") && !obj.contains_key("phishing_activities") {
        return None;
    }
    let raw: GoPlusAddressSecurityRaw = serde_json::from_value(result).ok()?;
    Some(AddressSecurity {
        sanctioned: goplus_flag(&raw.sanctioned),
        phishing_activities: goplus_flag(&raw.phishing_activities),
        stealing_attack: goplus_flag(&raw.stealing_attack),
        blackmail_activities: goplus_flag(&raw.blackmail_activities),
        cybercrime: goplus_flag(&raw.cybercrime),
        money_laundering: goplus_flag(&raw.money_laundering),
        financial_crime: goplus_flag(&raw.financial_crime),
        darkweb_transactions: goplus_flag(&raw.darkweb_transactions),
        fake_kyc: goplus_flag(&raw.fake_kyc),
        malicious_mining_activities: goplus_flag(&raw.malicious_mining_activities),
        honeypot_related_address: goplus_flag(&raw.honeypot_related_address),
        blacklist_doubt: goplus_flag(&raw.blacklist_doubt),
        mixer: goplus_flag(&raw.mixer),
    })
}

impl TokenMetadataClient {
    pub fn new(client: Client, config: &AppConfig) -> Self {
        Self {
            client,
            coingecko_base_url: config.coingecko_api_url.trim_end_matches('/').to_string(),
            coingecko_api_key: config.coingecko_api_key.clone(),
            dexscreener_base_url: config.dexscreener_api_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn enrich(&self, mut info: TokenInfo) -> TokenInfo {
        let is_native = info
            .address
            .eq_ignore_ascii_case(crate::config::chains::NATIVE_ADDR);
        let cc = crate::config::chain_config(info.chain_id);

        let coingecko = if is_native {
            match cc.map(|c| c.native_token.coingecko_id) {
                Some(id) => self.fetch_coingecko_by_id(id).await.ok(),
                None => None,
            }
        } else {
            match coingecko_platform_id(info.chain_id) {
                Some(platform) => self.fetch_coingecko(platform, &info.address).await.ok(),
                None => None,
            }
        };

        let dexscreener_addr = if is_native {
            cc.map(|c| c.native_token.wrapped_address)
                .unwrap_or(info.address.as_str())
                .to_string()
        } else {
            info.address.clone()
        };
        let dexscreener = self.fetch_dexscreener(&dexscreener_addr).await.ok();

        let mut patch = TokenMetadataPatch::default();
        apply_coingecko(&mut patch, coingecko);
        apply_dexscreener(&mut patch, dexscreener, &dexscreener_addr);

        // Native tokens are inherently low-risk
        if is_native {
            patch.risk_level = Some(("low".to_string(), "chain-config".to_string()));
        }

        // Use GoPlus for risk_level
        if patch.risk_level.is_none() && !is_native {
            if let Some(goplus) = self.fetch_goplus_risk(info.chain_id, &info.address).await {
                let a = assess_goplus_risk(&goplus);
                patch.risk_level = Some((a.risk_level, "goplus".to_string()));
            }
        }

        apply_patch(&mut info, patch);
        info
    }

    pub async fn fetch_price(&self, chain_id: u64, address: &str, symbol: &str) -> TokenPrice {
        let is_native = address.eq_ignore_ascii_case(crate::config::chains::NATIVE_ADDR);
        let cc = crate::config::chain_config(chain_id);

        let coingecko = if is_native {
            match cc.map(|c| c.native_token.coingecko_id) {
                Some(id) => self.fetch_coingecko_by_id(id).await.ok(),
                None => None,
            }
        } else {
            match coingecko_platform_id(chain_id) {
                Some(platform) => self.fetch_coingecko(platform, address).await.ok(),
                None => None,
            }
        };

        // DexScreener: native tokens trade as their wrapped version on DEXes
        let dexscreener_addr = if is_native {
            cc.map(|c| c.native_token.wrapped_address)
                .unwrap_or(address)
        } else {
            address
        };
        let dexscreener = self.fetch_dexscreener(dexscreener_addr).await.ok();

        let mut price = TokenPrice {
            address: address.to_string(),
            symbol: symbol.to_string(),
            chain_id,
            price: None,
            price_change_1h: None,
            price_change_24h: None,
            price_change_7d: None,
            high_24h: None,
            low_24h: None,
            sources: TokenPriceSources::default(),
        };

        apply_coingecko_price(&mut price, coingecko);
        apply_dexscreener_price(&mut price, dexscreener, dexscreener_addr);
        price
    }

    pub async fn fetch_liquidity(
        &self,
        chain_id: u64,
        address: &str,
        symbol: &str,
    ) -> crate::models::token::TokenLiquidity {
        use crate::models::token::{TokenLiquidity, TokenLiquidityTopPair};

        let is_native = address.eq_ignore_ascii_case(crate::config::chains::NATIVE_ADDR);
        let dexscreener_addr = if is_native {
            crate::config::chain_config(chain_id)
                .map(|c| c.native_token.wrapped_address)
                .unwrap_or(address)
        } else {
            address
        };

        let pairs = match self.fetch_dexscreener(dexscreener_addr).await {
            Ok(resp) => resp.pairs.unwrap_or_default(),
            Err(_) => {
                return TokenLiquidity {
                    address: address.to_string(),
                    symbol: symbol.to_string(),
                    chain_id,
                    top_liquidity: None,
                    pair_count: 0,
                    top_pair: None,
                    sources: crate::models::token::TokenLiquiditySources::default(),
                };
            }
        };

        let matching: Vec<&DexScreenerPair> = pairs
            .iter()
            .filter(|p| {
                p.base_token
                    .as_ref()
                    .and_then(|t| t.address.as_deref())
                    .is_some_and(|a| a.eq_ignore_ascii_case(dexscreener_addr))
            })
            .collect();

        let top_liquidity = matching
            .iter()
            .filter_map(|p| p.liquidity.as_ref().and_then(|l| l.usd))
            .fold(0.0f64, f64::max);
        let top_liquidity = if top_liquidity > 0.0 {
            Some(top_liquidity)
        } else {
            None
        };

        let top_pair = matching
            .iter()
            .max_by(|a, b| {
                let la = a.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
                let lb = b.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
                la.partial_cmp(&lb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|p| {
                let pair_address = p.pair_address.clone()?;
                let dex = p.dex_id.clone().unwrap_or_else(|| "unknown".to_string());
                let liquidity = p.liquidity.as_ref().and_then(|l| l.usd);
                let volume_24h = p.volume.as_ref().and_then(|v| v.h24);
                Some(TokenLiquidityTopPair {
                    pair_address,
                    dex,
                    liquidity,
                    volume_24h,
                })
            });

        use crate::models::token::TokenLiquiditySources;

        let src = if matching.is_empty() {
            TokenLiquiditySources::default()
        } else {
            TokenLiquiditySources {
                top_liquidity: Some("dexscreener".to_string()),
                pair_count: Some("dexscreener".to_string()),
                top_pair: Some("dexscreener".to_string()),
            }
        };

        TokenLiquidity {
            address: address.to_string(),
            symbol: symbol.to_string(),
            chain_id,
            top_liquidity,
            pair_count: matching.len(),
            top_pair,
            sources: src,
        }
    }

    pub async fn fetch_risk(
        &self,
        chain_id: u64,
        address: &str,
        symbol: &str,
    ) -> crate::models::token::TokenRisk {
        use crate::models::token::{TokenRisk, TokenRiskSources};

        let is_native = address.eq_ignore_ascii_case(crate::config::chains::NATIVE_ADDR);

        // Native tokens are inherently low-risk
        if is_native {
            return TokenRisk {
                address: address.to_string(),
                symbol: symbol.to_string(),
                chain_id,
                risk_level: Some("low".to_string()),
                risk_score: Some(0.0),
                honeypot: Some(false),
                blacklist: Some(false),
                transfer_restricted: Some(false),
                mintable: Some(false),
                owner_privileged: Some(false),
                tax_buy: Some(0.0),
                tax_sell: Some(0.0),
                sources: TokenRiskSources {
                    risk_level: Some("chain-config".to_string()),
                    risk_score: Some("chain-config".to_string()),
                    honeypot: Some("chain-config".to_string()),
                    blacklist: Some("chain-config".to_string()),
                    transfer_restricted: Some("chain-config".to_string()),
                    mintable: Some("chain-config".to_string()),
                    owner_privileged: Some("chain-config".to_string()),
                    tax_buy: Some("chain-config".to_string()),
                    tax_sell: Some("chain-config".to_string()),
                },
            };
        }

        let goplus = self.fetch_goplus_risk(chain_id, address).await;

        let mut risk = TokenRisk {
            address: address.to_string(),
            symbol: symbol.to_string(),
            chain_id,
            risk_level: None,
            risk_score: None,
            honeypot: None,
            blacklist: None,
            transfer_restricted: None,
            mintable: None,
            owner_privileged: None,
            tax_buy: None,
            tax_sell: None,
            sources: TokenRiskSources::default(),
        };

        if let Some(data) = goplus {
            let src = "goplus";
            let a = assess_goplus_risk(&data);
            macro_rules! set {
                ($model:ident, $val:expr) => {
                    if risk.$model.is_none() {
                        risk.$model = Some($val);
                        risk.sources.$model = Some(src.to_string());
                    }
                };
            }
            set!(honeypot, a.honeypot);
            set!(blacklist, a.blacklist);
            set!(transfer_restricted, a.transfer_restricted);
            set!(mintable, a.mintable);
            set!(owner_privileged, a.owner_privileged);
            if let Some(v) = a.tax_buy {
                set!(tax_buy, v);
            }
            if let Some(v) = a.tax_sell {
                set!(tax_sell, v);
            }
            set!(risk_score, a.risk_score);
            set!(risk_level, a.risk_level);
        }

        risk
    }

    /// Solana-specific token risk via GoPlus. Returns a [`TokenRisk`] with
    /// `chain_id = 0` (sentinel for non-EVM). When GoPlus has no entry for
    /// the mint, returned fields stay `None` (rendered as `N/A`) so the
    /// caller never has to invent fake "low risk" defaults.
    pub async fn fetch_risk_svm(
        &self,
        mint: &str,
        symbol: &str,
    ) -> crate::models::token::TokenRisk {
        use crate::models::token::{TokenRisk, TokenRiskSources};

        let mut risk = TokenRisk {
            address: mint.to_string(),
            symbol: symbol.to_string(),
            chain_id: 0,
            risk_level: None,
            risk_score: None,
            honeypot: None,
            blacklist: None,
            transfer_restricted: None,
            mintable: None,
            owner_privileged: None,
            tax_buy: None,
            tax_sell: None,
            sources: TokenRiskSources::default(),
        };

        let Some(data) = self.fetch_goplus_risk_svm(mint).await else {
            return risk;
        };

        let src = "goplus".to_string();

        let mintable = data.mintable.as_ref().map(GoPlusAuthority::is_active);
        if let Some(v) = mintable {
            risk.mintable = Some(v);
            risk.sources.mintable = Some(src.clone());
        }

        // On Solana the "owner privilege" surface combines freeze and close
        // authorities — either lets the issuer disrupt a holder.
        let freezable = data.freezable.as_ref().is_some_and(GoPlusAuthority::is_active);
        let closable = data.closable.as_ref().is_some_and(GoPlusAuthority::is_active);
        let owner_privileged = freezable || closable;
        risk.owner_privileged = Some(owner_privileged);
        risk.sources.owner_privileged = Some(src.clone());

        // Transfer-restricted: explicit non_transferable flag, an attached
        // transfer hook program, or a non-zero transfer fee.
        let non_transferable = data.non_transferable.as_deref() == Some("1");
        let has_hook = data
            .transfer_hook
            .as_ref()
            .is_some_and(|h| !h.is_empty());
        let transfer_fee_pct = parse_percent(
            data.transfer_fee
                .as_ref()
                .and_then(|f| f.transfer_fee.as_deref()),
        );
        let has_fee = transfer_fee_pct.is_some_and(|p| p > 0.0);
        risk.transfer_restricted = Some(non_transferable || has_hook || has_fee);
        risk.sources.transfer_restricted = Some(src.clone());

        // Solana transfer fees apply symmetrically (no separate buy/sell), so
        // mirror them into both fields when present.
        if let Some(pct) = transfer_fee_pct {
            risk.tax_buy = Some(pct);
            risk.tax_sell = Some(pct);
            risk.sources.tax_buy = Some(src.clone());
            risk.sources.tax_sell = Some(src.clone());
        }

        // GoPlus's `trusted_token` (1 = on a curated list) is the closest
        // SVM signal to "low risk overall". Without it, severity stays None
        // so the user sees `N/A` rather than a guessed level.
        let trusted = data.trusted_token == Some(1);
        let level = if trusted && !mintable.unwrap_or(false) && !owner_privileged {
            Some("low".to_string())
        } else if has_fee || has_hook || non_transferable {
            Some("high".to_string())
        } else if mintable.unwrap_or(false) || owner_privileged {
            Some("medium".to_string())
        } else {
            None
        };
        if let Some(l) = level {
            risk.risk_level = Some(l);
            risk.sources.risk_level = Some(src);
        }

        risk
    }

    /// Fetch GoPlus address-reputation flags for a wallet address. `chain_id`
    /// scopes EVM lookups (and lets GoPlus resolve the `contract_address`
    /// field); pass `None` for non-EVM addresses such as Solana, where the
    /// malicious-address library is keyed on the raw address string. Returns
    /// `None` when GoPlus has no record for the address or the request fails.
    pub async fn fetch_address_security(
        &self,
        address: &str,
        chain_id: Option<u64>,
    ) -> Option<AddressSecurity> {
        let url = format!("https://api.gopluslabs.io/api/v1/address_security/{address}");
        let mut req = self.client.get(&url).timeout(Duration::from_secs(10));
        if let Some(id) = chain_id {
            req = req.query(&[("chain_id", id.to_string())]);
        }
        let resp = req.send().await.ok()?;
        let body_text = resp.text().await.ok()?;
        parse_address_security(&body_text)
    }

    async fn fetch_goplus_risk_svm(&self, mint: &str) -> Option<GoPlusSvmTokenSecurity> {
        let url = "https://api.gopluslabs.io/api/v1/solana/token_security";
        let resp = self
            .client
            .get(url)
            .query(&[("contract_addresses", mint)])
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .ok()?;
        let body_text = resp.text().await.ok()?;
        let data: GoPlusSvmResponse = serde_json::from_str(&body_text).ok()?;
        if data.code != 1 {
            return None;
        }
        let result = data.result?;
        // Solana mints are case-sensitive base58, but the result key is the
        // mint itself — match exact first, fall through to any returned row.
        result.get(mint).cloned().or_else(|| result.into_values().next())
    }

    async fn fetch_goplus_risk(&self, chain_id: u64, address: &str) -> Option<GoPlusTokenSecurity> {
        let goplus_chain = goplus_chain_id(chain_id)?;
        let url = format!(
            "https://api.gopluslabs.io/api/v1/token_security/{}",
            goplus_chain
        );
        let resp = self
            .client
            .get(&url)
            .query(&[("contract_addresses", address)])
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .ok()?;
        let status = resp.status();
        let body_text = resp.text().await.ok()?;
        tracing::debug!(
            "GoPlus risk API status={status}, body={}",
            if body_text.len() > 500 {
                &body_text[..500]
            } else {
                &body_text
            }
        );
        let data: GoPlusResponse = serde_json::from_str(&body_text).ok()?;
        if data.code != 1 {
            return None;
        }
        let result = data.result?;
        let addr_lower = address.to_lowercase();
        result
            .get(&addr_lower)
            .cloned()
            .or_else(|| result.into_values().next())
    }

    pub async fn search_symbol(&self, query: &str, chain_id: u64) -> TokenSearchResult {
        let mut candidates = Vec::new();

        candidates.extend(self.search_coingecko(query, chain_id).await);
        candidates.extend(self.search_dexscreener(query).await);

        TokenSearchResult {
            query: query.to_string(),
            chain_id,
            candidates,
        }
    }

    async fn search_coingecko(&self, query: &str, chain_id: u64) -> Vec<TokenSearchCandidate> {
        let Some(platform) = coingecko_platform_id(chain_id) else {
            return Vec::new();
        };
        let url = format!("{}/search", self.coingecko_base_url);
        let mut req = self
            .client
            .get(url)
            .timeout(Duration::from_secs(8))
            .query(&[("query", query)]);
        if let Some(key) = &self.coingecko_api_key {
            req = req.header("x-cg-demo-api-key", key);
        }

        let Ok(response) = req
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
        else {
            return Vec::new();
        };
        let Ok(search) = response.json::<CoinGeckoSearchResponse>().await else {
            return Vec::new();
        };
        let query_upper = query.to_uppercase();
        search
            .coins
            .unwrap_or_default()
            .into_iter()
            .filter_map(|coin| {
                let symbol = non_empty(coin.symbol).map(|symbol| symbol.to_uppercase())?;
                if symbol != query_upper {
                    return None;
                }
                let address = coin
                    .platforms
                    .as_ref()
                    .and_then(|platforms| platforms.get(platform))
                    .and_then(|address| non_empty(Some(address.clone())))?;
                Some(TokenSearchCandidate {
                    source: "coingecko".to_string(),
                    symbol,
                    name: non_empty(coin.name),
                    address: Some(address),
                    chain: Some(platform.to_string()),
                    top_liquidity: None,
                })
            })
            .take(3)
            .collect()
    }

    async fn search_dexscreener(&self, query: &str) -> Vec<TokenSearchCandidate> {
        let url = format!("{}/search", self.dexscreener_base_url);
        let Ok(response) = self
            .client
            .get(url)
            .timeout(Duration::from_secs(8))
            .query(&[("q", query)])
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
        else {
            return Vec::new();
        };
        let Ok(search) = response.json::<DexScreenerResponse>().await else {
            return Vec::new();
        };
        let query_upper = query.to_uppercase();
        let mut pairs = search.pairs.unwrap_or_default();
        pairs.sort_by(|a, b| {
            liquidity_usd(b)
                .partial_cmp(&liquidity_usd(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        pairs
            .into_iter()
            .filter_map(|pair| {
                let token = pair.base_token?;
                let symbol = non_empty(token.symbol).map(|symbol| symbol.to_uppercase())?;
                if symbol != query_upper {
                    return None;
                }
                let address = non_empty(token.address).filter(|a| {
                    // Only include EVM addresses (0x prefix, 42 chars)
                    a.starts_with("0x") && a.len() == 42
                })?;
                let chain_id = pair.chain_id.as_deref().and_then(|c| c.parse::<u64>().ok());
                Some(TokenSearchCandidate {
                    source: "dexscreener".to_string(),
                    symbol,
                    name: non_empty(token.name),
                    address: Some(address),
                    chain: chain_id.map(|id| id.to_string()),
                    top_liquidity: pair.liquidity.and_then(|liquidity| liquidity.usd),
                })
            })
            .take(3)
            .collect()
    }

    /// Solana-specific enrichment for a `TokenInfo` already populated with
    /// `address` (SPL mint) and basic identity. Pulls CoinGecko data via
    /// `coins/solana/contract/{mint}` and DexScreener pool data; the result
    /// merges in price, market cap, top liquidity, etc. `is_native` logic and
    /// `goplus` risk lookups don't apply on Solana and are skipped.
    pub async fn enrich_svm(&self, mut info: TokenInfo) -> TokenInfo {
        let coingecko = self.fetch_coingecko("solana", &info.address).await.ok();
        let dexscreener = self.fetch_dexscreener(&info.address).await.ok();

        let mut patch = TokenMetadataPatch::default();
        apply_coingecko(&mut patch, coingecko);
        apply_dexscreener(&mut patch, dexscreener, &info.address);
        apply_patch(&mut info, patch);
        info
    }

    /// Solana-specific price lookup. Tags the returned `TokenPrice` with
    /// `chain_id = 0` (sentinel for non-EVM) so JSON consumers can detect
    /// the lack of an EVM chain context.
    pub async fn fetch_price_svm(&self, mint: &str, symbol: &str) -> crate::models::token::TokenPrice {
        use crate::models::token::{TokenPrice, TokenPriceSources};

        let coingecko = self.fetch_coingecko("solana", mint).await.ok();
        let dexscreener = self.fetch_dexscreener(mint).await.ok();

        let mut price = TokenPrice {
            address: mint.to_string(),
            symbol: symbol.to_string(),
            chain_id: 0,
            price: None,
            price_change_1h: None,
            price_change_24h: None,
            price_change_7d: None,
            high_24h: None,
            low_24h: None,
            sources: TokenPriceSources::default(),
        };
        apply_coingecko_price(&mut price, coingecko);
        apply_dexscreener_price(&mut price, dexscreener, mint);
        price
    }

    /// Solana-specific liquidity lookup. DexScreener's `/tokens/{address}`
    /// endpoint already returns Solana pools when given an SPL mint, so this
    /// just bypasses the EVM `is_native`/wrapped-address logic and labels
    /// `chain_id = 0`.
    pub async fn fetch_liquidity_svm(
        &self,
        mint: &str,
        symbol: &str,
    ) -> crate::models::token::TokenLiquidity {
        use crate::models::token::{TokenLiquidity, TokenLiquiditySources, TokenLiquidityTopPair};

        let pairs = match self.fetch_dexscreener(mint).await {
            Ok(resp) => resp.pairs.unwrap_or_default(),
            Err(_) => {
                return TokenLiquidity {
                    address: mint.to_string(),
                    symbol: symbol.to_string(),
                    chain_id: 0,
                    top_liquidity: None,
                    pair_count: 0,
                    top_pair: None,
                    sources: TokenLiquiditySources::default(),
                };
            }
        };

        let matching: Vec<&DexScreenerPair> = pairs
            .iter()
            .filter(|p| {
                p.base_token
                    .as_ref()
                    .and_then(|t| t.address.as_deref())
                    .is_some_and(|a| a.eq_ignore_ascii_case(mint))
            })
            .collect();

        let top_liquidity = matching
            .iter()
            .filter_map(|p| p.liquidity.as_ref().and_then(|l| l.usd))
            .fold(0.0f64, f64::max);
        let top_liquidity = if top_liquidity > 0.0 {
            Some(top_liquidity)
        } else {
            None
        };

        let top_pair = matching
            .iter()
            .max_by(|a, b| {
                let la = a.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
                let lb = b.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0);
                la.partial_cmp(&lb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|p| {
                let pair_address = p.pair_address.clone()?;
                let dex = p.dex_id.clone().unwrap_or_else(|| "unknown".to_string());
                let liquidity = p.liquidity.as_ref().and_then(|l| l.usd);
                let volume_24h = p.volume.as_ref().and_then(|v| v.h24);
                Some(TokenLiquidityTopPair {
                    pair_address,
                    dex,
                    liquidity,
                    volume_24h,
                })
            });

        let src = if matching.is_empty() {
            TokenLiquiditySources::default()
        } else {
            TokenLiquiditySources {
                top_liquidity: Some("dexscreener".to_string()),
                pair_count: Some("dexscreener".to_string()),
                top_pair: Some("dexscreener".to_string()),
            }
        };

        TokenLiquidity {
            address: mint.to_string(),
            symbol: symbol.to_string(),
            chain_id: 0,
            top_liquidity,
            pair_count: matching.len(),
            top_pair,
            sources: src,
        }
    }

    async fn fetch_coingecko(
        &self,
        platform: &str,
        address: &str,
    ) -> Result<CoinGeckoToken, reqwest::Error> {
        let url = format!(
            "{}/coins/{}/contract/{}",
            self.coingecko_base_url, platform, address
        );
        let mut req = self
            .client
            .get(url)
            .timeout(Duration::from_secs(8))
            .query(&[
                ("localization", "false"),
                ("tickers", "false"),
                ("community_data", "false"),
                ("developer_data", "false"),
                ("sparkline", "false"),
            ]);
        if let Some(key) = &self.coingecko_api_key {
            req = req.header("x-cg-demo-api-key", key);
        }
        req.send().await?.error_for_status()?.json().await
    }

    async fn fetch_coingecko_by_id(&self, coin_id: &str) -> Result<CoinGeckoToken, reqwest::Error> {
        let url = format!("{}/coins/{}", self.coingecko_base_url, coin_id);
        let mut req = self
            .client
            .get(url)
            .timeout(Duration::from_secs(8))
            .query(&[
                ("localization", "false"),
                ("tickers", "false"),
                ("community_data", "false"),
                ("developer_data", "false"),
                ("sparkline", "false"),
            ]);
        if let Some(key) = &self.coingecko_api_key {
            req = req.header("x-cg-demo-api-key", key);
        }
        req.send().await?.error_for_status()?.json().await
    }

    async fn fetch_dexscreener(
        &self,
        address: &str,
    ) -> Result<DexScreenerResponse, reqwest::Error> {
        let url = format!("{}/tokens/{}", self.dexscreener_base_url, address);
        self.client
            .get(url)
            .timeout(Duration::from_secs(8))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
    }
}

fn apply_coingecko(patch: &mut TokenMetadataPatch, token: Option<CoinGeckoToken>) {
    let Some(token) = token else {
        return;
    };
    if let Some(name) = non_empty(token.name) {
        patch.name = Some((name, "coingecko".to_string()));
    }
    if let Some(symbol) = non_empty(token.symbol).map(|s| s.to_uppercase()) {
        patch.symbol = Some((symbol, "coingecko".to_string()));
    }

    if let Some(links) = token.links {
        if let Some(url) = first_non_empty(links.homepage) {
            patch.website = Some((url, "coingecko".to_string()));
        }
        let mut social = TokenSocialLinks::default();
        social.x = non_empty(links.twitter_screen_name).map(|name| {
            if name.starts_with("http") {
                name
            } else {
                format!("https://x.com/{}", name.trim_start_matches('@'))
            }
        });
        social.telegram = non_empty(links.telegram_channel_identifier).map(|name| {
            if name.starts_with("http") {
                name
            } else {
                format!("https://t.me/{}", name.trim_start_matches('@'))
            }
        });
        social.discord = first_non_empty(links.chat_url);
        social.github = links
            .repos_url
            .and_then(|repos| first_non_empty(repos.github));
        social.docs = first_non_empty(links.blockchain_site)
            .or_else(|| first_non_empty(links.official_forum_url));
        if has_social_links(&social) {
            patch.social_links = Some((social, "coingecko".to_string()));
        }
    }

    if let Some(market) = token.market_data {
        if let Some(value) = market.current_price.and_then(|v| v.usd) {
            patch.price = Some((value, "coingecko".to_string()));
        }
        if let Some(value) = market.market_cap.and_then(|v| v.usd) {
            patch.market_cap = Some((value, "coingecko".to_string()));
        }
        if let Some(value) = market.fully_diluted_valuation.and_then(|v| v.usd) {
            patch.fdv = Some((value, "coingecko".to_string()));
        }
        if let Some(value) = market.total_volume.and_then(|v| v.usd) {
            patch.volume_24h = Some((value, "coingecko".to_string()));
        }
        if let Some(value) = market.price_change_percentage_24h {
            patch.price_change_24h = Some((value, "coingecko".to_string()));
        }
    }
}

fn apply_dexscreener(
    patch: &mut TokenMetadataPatch,
    response: Option<DexScreenerResponse>,
    address: &str,
) {
    let Some(response) = response else {
        return;
    };
    let Some(pair) = response
        .pairs
        .unwrap_or_default()
        .into_iter()
        .filter(|pair| {
            pair.base_token
                .as_ref()
                .and_then(|token| token.address.as_deref())
                .is_some_and(|base| base.eq_ignore_ascii_case(address))
        })
        .max_by(|a, b| {
            liquidity_usd(a)
                .partial_cmp(&liquidity_usd(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    else {
        return;
    };

    if patch.address.is_none() {
        if let Some(token) = &pair.base_token {
            if let Some(address) = non_empty(token.address.clone()) {
                patch.address = Some((address, "dexscreener".to_string()));
            }
        }
    }
    if patch.name.is_none() {
        if let Some(name) = pair
            .base_token
            .as_ref()
            .and_then(|t| non_empty(t.name.clone()))
        {
            patch.name = Some((name, "dexscreener".to_string()));
        }
    }
    if patch.symbol.is_none() {
        if let Some(symbol) = pair
            .base_token
            .as_ref()
            .and_then(|t| non_empty(t.symbol.clone()))
        {
            patch.symbol = Some((symbol, "dexscreener".to_string()));
        }
    }
    if let Some(value) = pair.liquidity.and_then(|v| v.usd) {
        patch.top_liquidity = Some((value, "dexscreener".to_string()));
    }
}

fn apply_coingecko_price(price: &mut TokenPrice, token: Option<CoinGeckoToken>) {
    let Some(token) = token else {
        return;
    };
    let Some(market) = token.market_data else {
        return;
    };

    if let Some(value) = market.current_price.and_then(|v| v.usd) {
        price.price = Some(value);
        price.sources.price = Some("coingecko".to_string());
    }
    if let Some(value) = market
        .price_change_percentage_1h_in_currency
        .and_then(|v| v.usd)
    {
        price.price_change_1h = Some(value);
        price.sources.price_change_1h = Some("coingecko".to_string());
    }
    if let Some(value) = market.price_change_percentage_24h {
        price.price_change_24h = Some(value);
        price.sources.price_change_24h = Some("coingecko".to_string());
    }
    if let Some(value) = market
        .price_change_percentage_7d_in_currency
        .and_then(|v| v.usd)
    {
        price.price_change_7d = Some(value);
        price.sources.price_change_7d = Some("coingecko".to_string());
    }
    if let Some(value) = market.high_24h.and_then(|v| v.usd) {
        price.high_24h = Some(value);
        price.sources.high_24h = Some("coingecko".to_string());
    }
    if let Some(value) = market.low_24h.and_then(|v| v.usd) {
        price.low_24h = Some(value);
        price.sources.low_24h = Some("coingecko".to_string());
    }
}

fn apply_dexscreener_price(
    price: &mut TokenPrice,
    response: Option<DexScreenerResponse>,
    address: &str,
) {
    let Some(response) = response else {
        return;
    };
    let Some(pair) = response
        .pairs
        .unwrap_or_default()
        .into_iter()
        .filter(|pair| {
            pair.base_token
                .as_ref()
                .and_then(|token| token.address.as_deref())
                .is_some_and(|base| base.eq_ignore_ascii_case(address))
        })
        .max_by(|a, b| {
            liquidity_usd(a)
                .partial_cmp(&liquidity_usd(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    else {
        return;
    };

    if price.price.is_none() {
        if let Some(value) = pair
            .price_usd
            .as_deref()
            .and_then(|s| s.parse::<f64>().ok())
        {
            price.price = Some(value);
            price.sources.price = Some("dexscreener".to_string());
        }
    }
    if let Some(change) = pair.price_change {
        if price.price_change_1h.is_none() {
            if let Some(value) = change.h1 {
                price.price_change_1h = Some(value);
                price.sources.price_change_1h = Some("dexscreener".to_string());
            }
        }
        if price.price_change_24h.is_none() {
            if let Some(value) = change.h24 {
                price.price_change_24h = Some(value);
                price.sources.price_change_24h = Some("dexscreener".to_string());
            }
        }
    }
}

fn apply_patch(info: &mut TokenInfo, patch: TokenMetadataPatch) {
    let mut sources = info.sources.clone();

    if let Some((value, source)) = patch.name {
        info.name = value;
        sources.identity.get_or_insert(source);
    }
    if let Some((value, source)) = patch.symbol {
        info.symbol = value;
        sources.identity.get_or_insert(source);
    }
    if let Some((value, source)) = patch.address {
        let is_native = info
            .address
            .eq_ignore_ascii_case(crate::config::chains::NATIVE_ADDR);
        if !is_native && info.address != value {
            info.address = value;
            sources.identity.get_or_insert(source);
        }
    }
    if let Some((value, source)) = patch.website {
        info.website = Some(value);
        sources.website = Some(source);
    }
    if let Some((value, source)) = patch.social_links {
        info.social_links = value;
        sources.social_links = Some(source);
    }
    if let Some((value, source)) = patch.price {
        info.price = Some(value);
        sources.price = Some(source);
    }
    if let Some((value, source)) = patch.market_cap {
        info.market_cap = Some(value);
        sources.market_cap = Some(source);
    }
    if let Some((value, source)) = patch.fdv {
        info.fdv = Some(value);
        sources.fdv = Some(source);
    }
    if let Some((value, source)) = patch.top_liquidity {
        info.top_liquidity = Some(value);
        sources.top_liquidity = Some(source);
    }
    if let Some((value, source)) = patch.volume_24h {
        info.volume_24h = Some(value);
        sources.volume_24h = Some(source);
    }
    if let Some((value, source)) = patch.price_change_24h {
        info.price_change_24h = Some(value);
        sources.price_change_24h = Some(source);
    }
    if let Some((value, source)) = patch.risk_level {
        info.risk_level = Some(value);
        sources.risk_level = Some(source);
    }

    info.sources = sources;
}

/// Parse a GoPlus percentage string. GoPlus returns fractions ("0.05" =
/// 5%) for EVM and percentages ("5" = 5%) for Solana transfer fees — this
/// only handles the percentage-string form used by Solana's `transfer_fee`,
/// returning the value in percent.
fn parse_percent(raw: Option<&str>) -> Option<f64> {
    raw.and_then(|s| s.trim().parse::<f64>().ok())
}

fn coingecko_platform_id(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        1 => Some("ethereum"),
        56 => Some("binance-smart-chain"),
        137 => Some("polygon-pos"),
        42161 => Some("arbitrum-one"),
        10 => Some("optimistic-ethereum"),
        43114 => Some("avalanche"),
        8453 => Some("base"),
        59144 => Some("linea"),
        534352 => Some("scroll"),
        5000 => Some("mantle"),
        1313161554 => Some("aurora"),
        _ => None,
    }
}

fn goplus_chain_id(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        1 => Some("1"),
        56 => Some("56"),
        137 => Some("137"),
        42161 => Some("42161"),
        10 => Some("10"),
        43114 => Some("43114"),
        8453 => Some("8453"),
        59144 => Some("59144"),
        534352 => Some("534352"),
        169 => Some("169"),
        5000 => Some("5000"),
        1313161554 => Some("1313161554"),
        66 => Some("66"),
        1030 => Some("1030"),
        167000 => Some("167000"),
        98866 => Some("98866"),
        11155111 => Some("11155111"),
        _ => None,
    }
}

struct GoPlusRiskAssessment {
    honeypot: bool,
    blacklist: bool,
    transfer_restricted: bool,
    mintable: bool,
    owner_privileged: bool,
    tax_buy: Option<f64>,
    tax_sell: Option<f64>,
    risk_score: f64,
    risk_level: String,
}

fn assess_goplus_risk(data: &GoPlusTokenSecurity) -> GoPlusRiskAssessment {
    let honeypot = data.is_honeypot.as_deref() == Some("1");
    let blacklist = data.is_blacklisted.as_deref() == Some("1");
    let transfer_restricted = data.transfer_pausable.as_deref() == Some("1");
    let mintable = data.is_mintable.as_deref() == Some("1");
    let owner_privileged = data.owner_change_balance.as_deref() == Some("1");
    let trusted = data.trust_list.as_deref() == Some("1");
    let buy_tax = data.buy_tax.as_deref().and_then(|s| {
        if s.is_empty() {
            Some(0.0)
        } else {
            s.parse::<f64>().ok()
        }
    });
    let sell_tax = data.sell_tax.as_deref().and_then(|s| {
        if s.is_empty() {
            Some(0.0)
        } else {
            s.parse::<f64>().ok()
        }
    });

    // Trusted tokens (e.g. USDT, USDC) have their centralized-control
    // penalties heavily discounted because those features are expected
    // for regulated/centralized assets, not scam signals.
    let centralization_weight: f64 = if trusted { 0.1 } else { 1.0 };

    let mut score: f64 = 0.0;
    // Honeypot is always critical regardless of trust status
    if honeypot {
        score += 100.0;
    }
    // Centralized-control signals: discounted for trusted tokens
    if blacklist {
        score += 30.0 * centralization_weight;
    }
    if transfer_restricted {
        score += 20.0 * centralization_weight;
    }
    if mintable {
        score += 15.0 * centralization_weight;
    }
    if owner_privileged {
        score += 15.0 * centralization_weight;
    }
    // Tax signals apply at full weight regardless of trust
    if let Some(t) = buy_tax {
        score += t.min(50.0);
    }
    if let Some(t) = sell_tax {
        score += t.min(50.0);
    }
    score = score.min(100.0);

    let level = if score >= 70.0 {
        "high"
    } else if score >= 30.0 {
        "medium"
    } else {
        "low"
    };

    GoPlusRiskAssessment {
        honeypot,
        blacklist,
        transfer_restricted,
        mintable,
        owner_privileged,
        tax_buy: buy_tax,
        tax_sell: sell_tax,
        risk_score: score,
        risk_level: level.to_string(),
    }
}

fn first_non_empty(values: Option<Vec<String>>) -> Option<String> {
    values?
        .into_iter()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn has_social_links(links: &TokenSocialLinks) -> bool {
    links.x.is_some()
        || links.telegram.is_some()
        || links.discord.is_some()
        || links.github.is_some()
        || links.docs.is_some()
}

fn liquidity_usd(pair: &DexScreenerPair) -> f64 {
    pair.liquidity.as_ref().and_then(|l| l.usd).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_address_security_reads_flat_flags() {
        let body = r#"{"code":1,"message":"ok","result":{
            "sanctioned":"1","phishing_activities":"0","stealing_attack":"1",
            "cybercrime":"0","mixer":"0"}}"#;
        let sec = parse_address_security(body).expect("flat result should parse");
        assert!(sec.sanctioned);
        assert!(sec.stealing_attack);
        assert!(!sec.phishing_activities);
        assert!(!sec.is_clean());
    }

    #[test]
    fn parse_address_security_clean_when_all_zero() {
        let body = r#"{"code":1,"result":{"sanctioned":"0","phishing_activities":"0"}}"#;
        let sec = parse_address_security(body).expect("should parse");
        assert!(sec.is_clean());
    }

    #[test]
    fn parse_address_security_rejects_non_success_code() {
        let body = r#"{"code":0,"message":"error","result":{"sanctioned":"1"}}"#;
        assert!(parse_address_security(body).is_none());
    }

    #[test]
    fn parse_address_security_rejects_keyed_map_shape() {
        // Schema-drift guard: a result keyed by address must NOT read as a
        // clean/unflagged address (which would be a false negative).
        let body = r#"{"code":1,"result":{"0xabc":{"sanctioned":"1"}}}"#;
        assert!(parse_address_security(body).is_none());
    }

    #[test]
    fn coingecko_platform_id_maps_supported_chains() {
        assert_eq!(coingecko_platform_id(1), Some("ethereum"));
        assert_eq!(coingecko_platform_id(8453), Some("base"));
        assert_eq!(coingecko_platform_id(999999), None);
    }

    #[test]
    fn apply_dexscreener_uses_highest_liquidity_matching_base_pair() {
        let mut patch = TokenMetadataPatch::default();
        apply_dexscreener(
            &mut patch,
            Some(DexScreenerResponse {
                pairs: Some(vec![
                    DexScreenerPair {
                        base_token: Some(DexScreenerToken {
                            address: Some("0xabc".to_string()),
                            name: Some("Low".to_string()),
                            symbol: Some("LOW".to_string()),
                        }),
                        liquidity: Some(DexScreenerLiquidity { usd: Some(10.0) }),
                        price_usd: None,
                        price_change: None,
                        dex_id: None,
                        pair_address: None,
                        volume: None,
                        chain_id: None,
                    },
                    DexScreenerPair {
                        base_token: Some(DexScreenerToken {
                            address: Some("0xAbC".to_string()),
                            name: Some("High".to_string()),
                            symbol: Some("HIGH".to_string()),
                        }),
                        liquidity: Some(DexScreenerLiquidity { usd: Some(20.0) }),
                        price_usd: None,
                        price_change: None,
                        dex_id: None,
                        pair_address: None,
                        volume: None,
                        chain_id: None,
                    },
                ]),
            }),
            "0xabc",
        );

        assert_eq!(patch.name.unwrap().0, "High");
        assert_eq!(patch.top_liquidity.unwrap().0, 20.0);
        assert!(patch.price.is_none());
        assert!(patch.volume_24h.is_none());
        assert!(patch.price_change_24h.is_none());
    }

    #[test]
    fn apply_dexscreener_accepts_highest_liquidity_pair_across_chains() {
        let mut patch = TokenMetadataPatch::default();
        apply_dexscreener(
            &mut patch,
            Some(DexScreenerResponse {
                pairs: Some(vec![DexScreenerPair {
                    base_token: Some(DexScreenerToken {
                        address: Some("0xabc".to_string()),
                        name: Some("Wrong Chain".to_string()),
                        symbol: Some("WRONG".to_string()),
                    }),
                    liquidity: Some(DexScreenerLiquidity { usd: Some(20.0) }),
                    price_usd: None,
                    price_change: None,
                    dex_id: None,
                    pair_address: None,
                    volume: None,
                    chain_id: None,
                }]),
            }),
            "0xabc",
        );

        assert_eq!(patch.name.unwrap().0, "Wrong Chain");
        assert_eq!(patch.top_liquidity.unwrap().0, 20.0);
        assert!(patch.price.is_none());
    }

    fn empty_token_price() -> TokenPrice {
        TokenPrice {
            address: "0xabc".to_string(),
            symbol: "TEST".to_string(),
            chain_id: 1,
            price: None,
            price_change_1h: None,
            price_change_24h: None,
            price_change_7d: None,
            high_24h: None,
            low_24h: None,
            sources: TokenPriceSources::default(),
        }
    }

    #[test]
    fn apply_coingecko_price_populates_all_fields_from_market_data() {
        let mut price = empty_token_price();
        apply_coingecko_price(
            &mut price,
            Some(CoinGeckoToken {
                symbol: None,
                name: None,
                links: None,
                market_data: Some(CoinGeckoMarketData {
                    current_price: Some(UsdValue { usd: Some(1.5) }),
                    market_cap: None,
                    fully_diluted_valuation: None,
                    total_volume: None,
                    high_24h: Some(UsdValue { usd: Some(2.0) }),
                    low_24h: Some(UsdValue { usd: Some(1.0) }),
                    price_change_percentage_1h_in_currency: Some(UsdValue { usd: Some(0.5) }),
                    price_change_percentage_24h: Some(3.0),
                    price_change_percentage_7d_in_currency: Some(UsdValue { usd: Some(10.0) }),
                }),
            }),
        );

        assert_eq!(price.price, Some(1.5));
        assert_eq!(price.price_change_1h, Some(0.5));
        assert_eq!(price.price_change_24h, Some(3.0));
        assert_eq!(price.price_change_7d, Some(10.0));
        assert_eq!(price.high_24h, Some(2.0));
        assert_eq!(price.low_24h, Some(1.0));
        assert_eq!(price.sources.price.as_deref(), Some("coingecko"));
        assert_eq!(price.sources.price_change_7d.as_deref(), Some("coingecko"));
        assert_eq!(price.sources.high_24h.as_deref(), Some("coingecko"));
    }

    #[test]
    fn apply_dexscreener_price_only_fills_unset_fields() {
        let mut price = empty_token_price();
        price.price = Some(1.5);
        price.sources.price = Some("coingecko".to_string());
        price.price_change_24h = Some(3.0);
        price.sources.price_change_24h = Some("coingecko".to_string());

        apply_dexscreener_price(
            &mut price,
            Some(DexScreenerResponse {
                pairs: Some(vec![DexScreenerPair {
                    base_token: Some(DexScreenerToken {
                        address: Some("0xabc".to_string()),
                        name: None,
                        symbol: None,
                    }),
                    liquidity: Some(DexScreenerLiquidity { usd: Some(100.0) }),
                    price_usd: Some("9.9".to_string()),
                    price_change: Some(DexScreenerPriceChange {
                        h1: Some(0.7),
                        h24: Some(99.0),
                    }),
                    dex_id: None,
                    pair_address: None,
                    volume: None,
                    chain_id: None,
                }]),
            }),
            "0xabc",
        );

        assert_eq!(price.price, Some(1.5));
        assert_eq!(price.sources.price.as_deref(), Some("coingecko"));
        assert_eq!(price.price_change_24h, Some(3.0));
        assert_eq!(price.sources.price_change_24h.as_deref(), Some("coingecko"));
        assert_eq!(price.price_change_1h, Some(0.7));
        assert_eq!(
            price.sources.price_change_1h.as_deref(),
            Some("dexscreener")
        );
    }

    #[test]
    fn apply_dexscreener_price_fills_when_coingecko_absent() {
        let mut price = empty_token_price();
        apply_dexscreener_price(
            &mut price,
            Some(DexScreenerResponse {
                pairs: Some(vec![DexScreenerPair {
                    base_token: Some(DexScreenerToken {
                        address: Some("0xabc".to_string()),
                        name: None,
                        symbol: None,
                    }),
                    liquidity: Some(DexScreenerLiquidity { usd: Some(100.0) }),
                    price_usd: Some("2.5".to_string()),
                    price_change: Some(DexScreenerPriceChange {
                        h1: Some(-1.0),
                        h24: Some(5.0),
                    }),
                    dex_id: None,
                    pair_address: None,
                    volume: None,
                    chain_id: None,
                }]),
            }),
            "0xabc",
        );

        assert_eq!(price.price, Some(2.5));
        assert_eq!(price.sources.price.as_deref(), Some("dexscreener"));
        assert_eq!(price.price_change_1h, Some(-1.0));
        assert_eq!(price.price_change_24h, Some(5.0));
        assert!(price.price_change_7d.is_none());
        assert!(price.high_24h.is_none());
        assert!(price.low_24h.is_none());
    }
}
