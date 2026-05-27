use std::time::Duration;

use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::Deserialize;
use sha2::Sha256;

use crate::config::AppConfig;
use crate::models::token::{
    TokenInfo, TokenPrice, TokenPriceSources, TokenSearchCandidate, TokenSearchResult,
    TokenSocialLinks,
};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct TokenMetadataClient {
    client: Client,
    coingecko_base_url: String,
    coingecko_api_key: Option<String>,
    dexscreener_base_url: String,
    okx_base_url: String,
    okx_api_key: Option<String>,
    okx_api_secret: Option<String>,
    okx_api_passphrase: Option<String>,
    okx_project_id: Option<String>,
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
    primary_liquidity: Option<(f64, String)>,
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
    #[serde(rename = "baseToken")]
    base_token: Option<DexScreenerToken>,
    liquidity: Option<DexScreenerLiquidity>,
    #[serde(rename = "priceUsd")]
    price_usd: Option<String>,
    #[serde(rename = "priceChange")]
    price_change: Option<DexScreenerPriceChange>,
}

#[derive(Debug, Deserialize)]
struct DexScreenerPriceChange {
    h1: Option<f64>,
    h24: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct DexScreenerToken {
    address: Option<String>,
    name: Option<String>,
    symbol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OkxSearchEnvelope {
    data: Option<OkxSearchData>,
}

#[derive(Debug, Deserialize)]
struct OkxSearchData {
    #[serde(rename = "tokenList")]
    token_list: Option<Vec<OkxToken>>,
}

#[derive(Debug, Deserialize)]
struct DexScreenerLiquidity {
    usd: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct OkxEnvelope<T> {
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct OkxToken {
    #[serde(rename = "tokenContractAddress")]
    token_contract_address: Option<String>,
    #[serde(rename = "tokenSymbol")]
    token_symbol: Option<String>,
    #[serde(rename = "tokenName")]
    token_name: Option<String>,
    #[serde(rename = "tagList")]
    tag_list: Option<OkxTagList>,
}

#[derive(Debug, Deserialize)]
struct OkxTagList {
    #[serde(rename = "communityRecognized")]
    community_recognized: Option<bool>,
}

impl TokenMetadataClient {
    pub fn new(client: Client, config: &AppConfig) -> Self {
        Self {
            client,
            coingecko_base_url: config.coingecko_api_url.trim_end_matches('/').to_string(),
            coingecko_api_key: config.coingecko_api_key.clone(),
            dexscreener_base_url: config.dexscreener_api_url.trim_end_matches('/').to_string(),
            okx_base_url: config.okx_dex_api_url.trim_end_matches('/').to_string(),
            okx_api_key: config.okx_api_key.clone(),
            okx_api_secret: config.okx_api_secret.clone(),
            okx_api_passphrase: config.okx_api_passphrase.clone(),
            okx_project_id: config.okx_project_id.clone(),
        }
    }

    pub async fn enrich(&self, mut info: TokenInfo) -> TokenInfo {
        let chain_slug = coingecko_platform_id(info.chain_id);
        let address = info.address.clone();

        let coingecko = match chain_slug {
            Some(platform) => self.fetch_coingecko(platform, &address).await.ok(),
            None => None,
        };
        let dexscreener = self.fetch_dexscreener(&address).await.ok();
        let okx = self.fetch_okx_token(info.chain_id, &address).await;

        let mut patch = TokenMetadataPatch::default();
        apply_coingecko(&mut patch, coingecko);
        apply_dexscreener(&mut patch, dexscreener, &address);
        apply_okx(&mut patch, okx);
        apply_patch(&mut info, patch);
        info
    }

    pub async fn fetch_price(&self, chain_id: u64, address: &str, symbol: &str) -> TokenPrice {
        let chain_slug = coingecko_platform_id(chain_id);

        let coingecko = match chain_slug {
            Some(platform) => self.fetch_coingecko(platform, address).await.ok(),
            None => None,
        };
        let dexscreener = self.fetch_dexscreener(address).await.ok();

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
        apply_dexscreener_price(&mut price, dexscreener, address);
        price
    }

    pub async fn search_symbol(&self, query: &str, chain_id: u64) -> TokenSearchResult {
        let mut candidates = Vec::new();

        candidates.extend(self.search_okx(query, chain_id).await);
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
                    .and_then(|address| non_empty(Some(address.clone())));
                Some(TokenSearchCandidate {
                    source: "coingecko".to_string(),
                    symbol,
                    name: non_empty(coin.name),
                    address,
                    chain: Some(platform.to_string()),
                    primary_liquidity: None,
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
                Some(TokenSearchCandidate {
                    source: "dexscreener".to_string(),
                    symbol,
                    name: non_empty(token.name),
                    address: non_empty(token.address),
                    chain: None,
                    primary_liquidity: pair.liquidity.and_then(|liquidity| liquidity.usd),
                })
            })
            .take(3)
            .collect()
    }

    async fn search_okx(&self, query: &str, chain_id: u64) -> Vec<TokenSearchCandidate> {
        let api_key = match self.okx_api_key.as_ref() {
            Some(value) => value,
            None => return Vec::new(),
        };
        let secret = match self.okx_api_secret.as_ref() {
            Some(value) => value,
            None => return Vec::new(),
        };
        let passphrase = match self.okx_api_passphrase.as_ref() {
            Some(value) => value,
            None => return Vec::new(),
        };
        let request_path = "/api/v6/dex/market/token/search";
        let body = serde_json::json!({
            "chainIndex": chain_id.to_string(),
            "tokenSymbol": query,
        })
        .to_string();
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let Ok(signature) = okx_signature(&timestamp, "POST", request_path, &body, secret) else {
            return Vec::new();
        };
        let url = format!("{}{}", self.okx_base_url, request_path);

        let mut req = self
            .client
            .post(url)
            .timeout(Duration::from_secs(8))
            .header("OK-ACCESS-KEY", api_key)
            .header("OK-ACCESS-SIGN", signature)
            .header("OK-ACCESS-TIMESTAMP", timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase)
            .body(body);
        if let Some(project_id) = &self.okx_project_id {
            req = req.header("OK-ACCESS-PROJECT", project_id);
        }

        let Ok(response) = req
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
        else {
            return Vec::new();
        };
        let Ok(search) = response.json::<OkxSearchEnvelope>().await else {
            return Vec::new();
        };
        let query_upper = query.to_uppercase();
        search
            .data
            .and_then(|data| data.token_list)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|token| {
                let symbol = non_empty(token.token_symbol).map(|symbol| symbol.to_uppercase())?;
                if symbol != query_upper {
                    return None;
                }
                Some(TokenSearchCandidate {
                    source: "okx-onchainos".to_string(),
                    symbol,
                    name: non_empty(token.token_name),
                    address: non_empty(token.token_contract_address),
                    chain: Some(chain_id.to_string()),
                    primary_liquidity: None,
                })
            })
            .take(3)
            .collect()
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

    async fn fetch_okx_token(&self, chain_id: u64, address: &str) -> Option<OkxToken> {
        let api_key = self.okx_api_key.as_ref()?;
        let secret = self.okx_api_secret.as_ref()?;
        let passphrase = self.okx_api_passphrase.as_ref()?;
        let request_path = "/api/v6/dex/market/token/basic-info";
        let body = serde_json::json!([
            {
                "chainIndex": chain_id.to_string(),
                "tokenContractAddress": address,
            }
        ])
        .to_string();
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let signature = okx_signature(&timestamp, "POST", request_path, &body, secret).ok()?;
        let url = format!("{}{}", self.okx_base_url, request_path);

        let mut req = self
            .client
            .post(url)
            .timeout(Duration::from_secs(8))
            .header("OK-ACCESS-KEY", api_key)
            .header("OK-ACCESS-SIGN", signature)
            .header("OK-ACCESS-TIMESTAMP", timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase)
            .body(body);
        if let Some(project_id) = &self.okx_project_id {
            req = req.header("OK-ACCESS-PROJECT", project_id);
        }

        let envelope = req
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json::<OkxEnvelope<Vec<OkxToken>>>()
            .await
            .ok()?;

        envelope.data?.into_iter().find(|token| {
            token
                .token_contract_address
                .as_deref()
                .is_some_and(|token_address| token_address.eq_ignore_ascii_case(address))
        })
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
        patch.primary_liquidity = Some((value, "dexscreener".to_string()));
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

fn apply_okx(patch: &mut TokenMetadataPatch, token: Option<OkxToken>) {
    let Some(token) = token else {
        return;
    };
    if patch.name.is_none() {
        if let Some(name) = non_empty(token.token_name) {
            patch.name = Some((name, "okx-onchainos".to_string()));
        }
    }
    if patch.symbol.is_none() {
        if let Some(symbol) = non_empty(token.token_symbol) {
            patch.symbol = Some((symbol, "okx-onchainos".to_string()));
        }
    }
    if patch.address.is_none() {
        if let Some(address) = non_empty(token.token_contract_address) {
            patch.address = Some((address, "okx-onchainos".to_string()));
        }
    }
    if patch.risk_level.is_none() {
        if let Some(community_recognized) =
            token.tag_list.and_then(|tags| tags.community_recognized)
        {
            let risk = if community_recognized {
                "low"
            } else {
                "unknown"
            };
            patch.risk_level = Some((risk.to_string(), "okx-onchainos".to_string()));
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
        if info.address != value {
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
    if let Some((value, source)) = patch.primary_liquidity {
        info.primary_liquidity = Some(value);
        sources.primary_liquidity = Some(source);
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

fn okx_signature(
    timestamp: &str,
    method: &str,
    request_path: &str,
    body: &str,
    secret: &str,
) -> Result<String, hmac::digest::InvalidLength> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(format!("{}{}{}{}", timestamp, method, request_path, body).as_bytes());
    Ok(base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
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
                    },
                ]),
            }),
            "0xabc",
        );

        assert_eq!(patch.name.unwrap().0, "High");
        assert_eq!(patch.primary_liquidity.unwrap().0, 20.0);
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
                }]),
            }),
            "0xabc",
        );

        assert_eq!(patch.name.unwrap().0, "Wrong Chain");
        assert_eq!(patch.primary_liquidity.unwrap().0, 20.0);
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
