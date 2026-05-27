use comfy_table::Table;

pub trait TableRenderable {
    fn render_table(&self);
}

impl TableRenderable for String {
    fn render_table(&self) {
        println!("{}", self);
    }
}

impl TableRenderable for crate::chain::TxStatus {
    fn render_table(&self) {
        let mut table = comfy_table::Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Confirmed", &self.confirmed.to_string()]);
        table.add_row(vec!["Success", &self.success.to_string()]);
        if let Some(bn) = self.block_number {
            table.add_row(vec!["Block", &bn.to_string()]);
        }
        if let Some(gas) = self.gas_used {
            table.add_row(vec!["Gas Used", &gas.to_string()]);
        }
        if let Some(price) = self.effective_gas_price {
            table.add_row(vec!["Eff. Gas Price (gwei)", &format!("{}", price)]);
        }
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::quote::Quote {
    fn render_table(&self) {
        let estimated_gas = self
            .estimated_gas
            .map(|v| v.to_string())
            .unwrap_or_else(|| "N/A".to_string());
        let gas_limit = self
            .gas_limit
            .map(|v| v.to_string())
            .unwrap_or_else(|| "N/A".to_string());
        let min_received = format_token_amount(
            &self.to_amount_min,
            self.to_token.decimals,
            &self.to_token.symbol,
        );
        let value_display = format_native_value(&self.value);

        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Quote ID", &self.quote_id.to_string()]);
        table.add_row(vec![
            "From",
            &format!("{} {}", self.from_amount_display, self.from_token.symbol),
        ]);
        table.add_row(vec![
            "To",
            &format!("{} {}", self.to_amount_display, self.to_token.symbol),
        ]);
        table.add_row(vec![
            "Rate",
            &format!(
                "1 {} = {} {}",
                self.from_token.symbol, self.exchange_rate, self.to_token.symbol
            ),
        ]);
        table.add_row(vec!["Price Impact", &format!("{}%", self.price_impact_pct)]);
        table.add_row(vec!["Min Received", &min_received]);
        table.add_row(vec!["Router", &self.router_to]);
        table.add_row(vec!["Value", &value_display]);
        table.add_row(vec!["Est. Gas", &estimated_gas]);
        table.add_row(vec!["Gas Limit", &gas_limit]);
        table.add_row(vec![
            "Expires",
            &self.expires_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        ]);
        if !self.route_summary.is_empty() {
            let hops: Vec<String> = self
                .route_summary
                .iter()
                .map(|h| format!("{} -> {} ({})", h.from_token, h.to_token, h.dex_name))
                .collect();
            table.add_row(vec!["Route", &hops.join("\n")]);
        }
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::swap::SimulationResult {
    fn render_table(&self) {
        let estimated_gas = self
            .estimated_gas
            .map(|v| v.to_string())
            .unwrap_or_else(|| "N/A".to_string());

        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Valid", &self.is_valid.to_string()]);
        table.add_row(vec!["Expected Out", &self.expected_out]);
        table.add_row(vec!["Min Out", &self.min_out]);
        table.add_row(vec!["Gas Estimate", &estimated_gas]);
        table.add_row(vec![
            "Gas Price (gwei)",
            &format!("{}", self.gas_price_gwei),
        ]);
        table.add_row(vec![
            "Gas Cost (ETH)",
            &format!("{}", self.total_gas_cost_eth),
        ]);
        for warning in &self.warnings {
            table.add_row(vec!["Warning", warning]);
        }
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::swap::ExecutionResult {
    fn render_table(&self) {
        let value_display = format_native_value(&self.value_eth);

        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Status", &format!("{:?}", self.status)]);
        table.add_row(vec!["Dry Run", &self.dry_run.to_string()]);
        if let Some(ref tx) = self.tx_hash {
            table.add_row(vec!["TX Hash", tx]);
        }
        table.add_row(vec!["To Contract", &self.to_contract]);
        table.add_row(vec!["Value", &value_display]);
        if let Some(ref from) = self.from_address {
            table.add_row(vec!["From", from]);
        }
        if let Some(gas) = self.gas_used {
            table.add_row(vec!["Gas Used", &format!("{}", gas)]);
        }
        if let Some(price) = self.effective_gas_price_gwei {
            table.add_row(vec!["Eff. Gas Price (gwei)", &format!("{}", price)]);
        }
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::swap::ApprovalResult {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Token", &self.token]);
        table.add_row(vec!["Spender", &self.spender]);
        table.add_row(vec!["Amount", &self.raw_amount]);
        table.add_row(vec!["Dry Run", &self.dry_run.to_string()]);
        if let Some(ref tx) = self.tx_hash {
            table.add_row(vec!["TX Hash", tx]);
        }
        if let Some(ref from) = self.from_address {
            table.add_row(vec!["From", from]);
        }
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::token::TokenOwnershipActionResult {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Dry Run", &self.dry_run.to_string()]);
        table.add_row(vec!["Action", &self.action]);
        table.add_row(vec!["Token", &self.token]);
        if let Some(ref from) = self.from_address {
            table.add_row(vec!["From", from]);
        }
        if let Some(gas) = self.estimated_gas {
            table.add_row(vec!["Estimated Gas", &gas.to_string()]);
        }
        if let Some(ref tx) = self.tx_hash {
            table.add_row(vec!["TX Hash", tx]);
        }
        table.add_row(vec!["Calldata", &self.calldata]);
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::token::TokenContract {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Address", &self.address]);
        table.add_row(vec!["Is Proxy", &self.is_proxy.to_string()]);
        if let Some(ref impl_addr) = self.proxy_implementation {
            table.add_row(vec!["Implementation", impl_addr]);
        }
        if let Some(ref owner) = self.owner {
            table.add_row(vec!["Owner", owner]);
        }
        if let Some(ref deployer) = self.deployer {
            table.add_row(vec!["Deployer", deployer]);
        }
        if let Some(block) = self.deployed_at_block {
            table.add_row(vec!["Deployed At Block", &block.to_string()]);
        }
        if let Some(verified) = self.is_verified {
            table.add_row(vec!["Verified", &verified.to_string()]);
        }
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::token::TokenInfo {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Address", &self.address]);
        table.add_row(vec!["Chain ID", &self.chain_id.to_string()]);
        if let Some(ref chain) = self.chain {
            table.add_row(vec!["Chain", chain]);
        }
        table.add_row(vec!["Symbol", &self.symbol]);
        table.add_row(vec!["Name", &self.name]);
        table.add_row(vec!["Decimals", &self.decimals.to_string()]);
        if let Some(ref website) = self.website {
            table.add_row(vec!["Website", website]);
        }
        let social_links = format_social_links(&self.social_links);
        if !social_links.is_empty() {
            table.add_row(vec!["Social", &social_links]);
        }
        if let Some(price) = self.price {
            table.add_row(vec!["Price (USD)", &format_usd(price)]);
        }
        if let Some(market_cap) = self.market_cap {
            table.add_row(vec!["Market Cap (USD)", &format_usd(market_cap)]);
        }
        if let Some(fdv) = self.fdv {
            table.add_row(vec!["FDV (USD)", &format_usd(fdv)]);
        }
        if let Some(liquidity) = self.top_liquidity {
            table.add_row(vec!["Top Liquidity (USD)", &format_usd(liquidity)]);
        }
        if let Some(volume) = self.volume_24h {
            table.add_row(vec!["Volume 24h (USD)", &format_usd(volume)]);
        }
        if let Some(change) = self.price_change_24h {
            table.add_row(vec!["Price Change 24h", &format!("{}%", change)]);
        }
        if let Some(ref risk_level) = self.risk_level {
            table.add_row(vec!["Risk Level", risk_level]);
        }
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::token::TokenPrice {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value", "Source"]);
        table.add_row(vec!["Address", &self.address, ""]);
        table.add_row(vec!["Symbol", &self.symbol, ""]);
        table.add_row(vec!["Chain ID", &self.chain_id.to_string(), ""]);
        add_price_row(
            &mut table,
            "Price (USD)",
            self.price.map(format_usd),
            self.sources.price.as_deref(),
        );
        add_price_row(
            &mut table,
            "Change 1h",
            self.price_change_1h.map(format_pct),
            self.sources.price_change_1h.as_deref(),
        );
        add_price_row(
            &mut table,
            "Change 24h",
            self.price_change_24h.map(format_pct),
            self.sources.price_change_24h.as_deref(),
        );
        add_price_row(
            &mut table,
            "Change 7d",
            self.price_change_7d.map(format_pct),
            self.sources.price_change_7d.as_deref(),
        );
        add_price_row(
            &mut table,
            "High 24h (USD)",
            self.high_24h.map(format_usd),
            self.sources.high_24h.as_deref(),
        );
        add_price_row(
            &mut table,
            "Low 24h (USD)",
            self.low_24h.map(format_usd),
            self.sources.low_24h.as_deref(),
        );
        println!("{}", table);
    }
}

fn add_price_row(table: &mut Table, label: &str, value: Option<String>, source: Option<&str>) {
    let value = value.unwrap_or_else(|| "N/A".to_string());
    table.add_row(vec![label, &value, source.unwrap_or("")]);
}

impl TableRenderable for crate::models::token::TokenLiquidity {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Address", &self.address]);
        table.add_row(vec!["Symbol", &self.symbol]);
        table.add_row(vec!["Chain ID", &self.chain_id.to_string()]);
        table.add_row(vec![
            "Top Liquidity (USD)",
            &self
                .top_liquidity
                .map(format_usd)
                .unwrap_or_else(|| "N/A".to_string()),
        ]);
        table.add_row(vec!["Pair Count", &self.pair_count.to_string()]);
        if let Some(ref top) = self.top_pair {
            table.add_row(vec!["Top Pair Address", &top.pair_address]);
            table.add_row(vec!["Top Pair DEX", &top.dex]);
            table.add_row(vec![
                "Top Pair Liquidity (USD)",
                &top.liquidity.map(format_usd).unwrap_or_else(|| "N/A".to_string()),
            ]);
            table.add_row(vec![
                "Top Pair Volume 24h (USD)",
                &top.volume_24h.map(format_usd).unwrap_or_else(|| "N/A".to_string()),
            ]);
        }
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::token::TokenRisk {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value", "Source"]);
        table.add_row(vec!["Address", &self.address, ""]);
        table.add_row(vec!["Symbol", &self.symbol, ""]);
        table.add_row(vec!["Chain ID", &self.chain_id.to_string(), ""]);
        add_risk_row(&mut table, "Risk Level", self.risk_level.as_deref(), self.sources.risk_level.as_deref());
        add_risk_row_score(&mut table, "Risk Score", self.risk_score, self.sources.risk_score.as_deref());
        add_risk_row_bool(&mut table, "Honeypot", self.honeypot, self.sources.honeypot.as_deref());
        add_risk_row_bool(&mut table, "Blacklist", self.blacklist, self.sources.blacklist.as_deref());
        add_risk_row_bool(&mut table, "Transfer Restricted", self.transfer_restricted, self.sources.transfer_restricted.as_deref());
        add_risk_row_bool(&mut table, "Mintable", self.mintable, self.sources.mintable.as_deref());
        add_risk_row_bool(&mut table, "Owner Privileged", self.owner_privileged, self.sources.owner_privileged.as_deref());
        add_risk_row_pct(&mut table, "Buy Tax", self.tax_buy, self.sources.tax_buy.as_deref());
        add_risk_row_pct(&mut table, "Sell Tax", self.tax_sell, self.sources.tax_sell.as_deref());
        println!("{}", table);
    }
}

fn add_risk_row(table: &mut Table, label: &str, value: Option<&str>, source: Option<&str>) {
    let value = value.unwrap_or("N/A");
    table.add_row(vec![label, value, source.unwrap_or("")]);
}

fn add_risk_row_bool(table: &mut Table, label: &str, value: Option<bool>, source: Option<&str>) {
    let value = match value {
        Some(true) => "Yes",
        Some(false) => "No",
        None => "N/A",
    };
    table.add_row(vec![label, value, source.unwrap_or("")]);
}

fn add_risk_row_score(table: &mut Table, label: &str, value: Option<f64>, source: Option<&str>) {
    let value = match value {
        Some(v) => format!("{}", v),
        None => "N/A".to_string(),
    };
    table.add_row(vec![label, &value, source.unwrap_or("")]);
}

fn add_risk_row_pct(table: &mut Table, label: &str, value: Option<f64>, source: Option<&str>) {
    let value = match value {
        Some(v) => format_pct(v),
        None => "N/A".to_string(),
    };
    table.add_row(vec![label, &value, source.unwrap_or("")]);
}

fn format_pct(value: f64) -> String {
    format!("{:.2}%", value)
}

impl TableRenderable for crate::models::token::TokenSearchResult {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec![
            "Source",
            "Symbol",
            "Name",
            "Address",
            "Chain",
            "Top Liquidity",
        ]);
        for candidate in &self.candidates {
            table.add_row(vec![
                candidate.source.as_str(),
                candidate.symbol.as_str(),
                candidate.name.as_deref().unwrap_or(""),
                candidate.address.as_deref().unwrap_or(""),
                candidate.chain.as_deref().unwrap_or(""),
                &candidate
                    .top_liquidity
                    .map(format_usd)
                    .unwrap_or_else(String::new),
            ]);
        }
        println!("Token not found in DODO tokenlist. Candidate matches:");
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::token::CustomTokenRecord {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Address", &self.address]);
        table.add_row(vec!["Symbol", &self.symbol]);
        table.add_row(vec!["Name", &self.name]);
        table.add_row(vec!["Decimals", &self.decimals.to_string()]);
        table.add_row(vec!["Chain ID", &self.chain_id.to_string()]);
        table.add_row(vec!["Source", &self.source]);
        table.add_row(vec![
            "Added At",
            &self.added_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        ]);
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::token::TokenCreateFee {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Factory", &self.factory]);
        table.add_row(vec![
            "Create Fee",
            &format!(
                "{} {} ({})",
                self.fee_display, self.fee_symbol, self.fee_raw
            ),
        ]);
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::token::TokenCreateResult {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Dry Run", &self.dry_run.to_string()]);
        table.add_row(vec!["Factory", &self.factory]);
        table.add_row(vec!["Method", &self.method]);
        table.add_row(vec!["Name", &self.token_name]);
        table.add_row(vec!["Symbol", &self.token_symbol]);
        table.add_row(vec!["Decimals", &self.decimals.to_string()]);
        table.add_row(vec![
            "Supply",
            &format!("{} ({})", self.supply_display, self.supply_raw),
        ]);
        table.add_row(vec!["Value", &self.value]);
        if let Some(ref from) = self.from_address {
            table.add_row(vec!["From", from]);
        }
        if let Some(gas) = self.estimated_gas {
            table.add_row(vec!["Estimated Gas", &gas.to_string()]);
        }
        if let Some(ref tx) = self.tx_hash {
            table.add_row(vec!["TX Hash", tx]);
        }
        if let Some(ref token) = self.new_token_address {
            table.add_row(vec!["New Token", token]);
        }
        table.add_row(vec!["Calldata", &self.calldata]);
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::token::TokenMintResult {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Dry Run", &self.dry_run.to_string()]);
        table.add_row(vec!["Token", &self.token]);
        table.add_row(vec!["Recipient", &self.to]);
        table.add_row(vec![
            "Amount",
            &format!("{} ({})", self.amount_display, self.amount_raw),
        ]);
        if let Some(ref from) = self.from_address {
            table.add_row(vec!["From", from]);
        }
        if let Some(gas) = self.estimated_gas {
            table.add_row(vec!["Estimated Gas", &gas.to_string()]);
        }
        if let Some(ref tx) = self.tx_hash {
            table.add_row(vec!["TX Hash", tx]);
        }
        table.add_row(vec!["Calldata", &self.calldata]);
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::wallet::WalletBalance {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Address", &self.address]);
        table.add_row(vec![
            "ETH Balance",
            &format!("{} ({})", self.eth_balance_display, self.eth_balance),
        ]);
        for tb in &self.token_balances {
            table.add_row(vec![
                &format!("{} Balance", tb.symbol),
                &format!("{} ({})", tb.balance_display, tb.balance),
            ]);
        }
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::risk::RiskReport {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Signal", "Severity", "Value"]);
        for sig in &self.signals {
            table.add_row(vec![
                &sig.signal,
                &format!("{:?}", sig.severity),
                &sig.value.to_string(),
            ]);
        }
        table.add_row(vec!["OVERALL", &format!("{:?}", self.overall_risk), ""]);
        println!("{}", table);
    }
}

impl TableRenderable for crate::models::risk::ApprovalRisk {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Field", "Value"]);
        table.add_row(vec!["Token", &self.token_symbol]);
        table.add_row(vec!["Spender", &self.spender]);
        table.add_row(vec!["Current Allowance", &self.current_allowance]);
        table.add_row(vec!["Is Unlimited", &self.is_unlimited.to_string()]);
        table.add_row(vec!["Risk", &format!("{:?}", self.risk)]);
        println!("{}", table);
    }
}

impl TableRenderable for Vec<crate::models::swap::SwapHistoryRecord> {
    fn render_table(&self) {
        let mut table = Table::new();
        table.set_header(vec!["Time", "From", "To", "Amount", "Status", "TX"]);
        for record in self {
            table.add_row(vec![
                &record.created_at.to_rfc3339(),
                &record.from_token,
                &record.to_token,
                &format!(
                    "{} -> {}",
                    record.from_amount_display, record.to_amount_display
                ),
                &format!("{:?}", record.status),
                record.tx_hash.as_deref().unwrap_or("N/A"),
            ]);
        }
        println!("{}", table);
    }
}

fn format_token_amount(raw: &str, decimals: u8, symbol: &str) -> String {
    match raw_to_decimal_string(raw, decimals) {
        Some(display) => format!("{} {} ({})", display, symbol, raw),
        None => format!("{} {}", raw, symbol),
    }
}

fn format_native_value(raw_wei: &str) -> String {
    match raw_to_decimal_string(raw_wei, 18) {
        Some(display) => format!("{} ETH ({} wei)", display, raw_wei),
        None => raw_wei.to_string(),
    }
}

fn format_usd(value: f64) -> String {
    if value.abs() >= 1.0 {
        format!("{:.2}", value)
    } else {
        format!("{:.8}", value).trim_end_matches('0').to_string()
    }
}

fn format_social_links(links: &crate::models::token::TokenSocialLinks) -> String {
    [
        ("X", links.x.as_deref()),
        ("Telegram", links.telegram.as_deref()),
        ("Discord", links.discord.as_deref()),
        ("GitHub", links.github.as_deref()),
        ("Docs", links.docs.as_deref()),
    ]
    .into_iter()
    .filter_map(|(label, value)| value.map(|value| format!("{}: {}", label, value)))
    .collect::<Vec<_>>()
    .join("\n")
}

fn raw_to_decimal_string(raw: &str, decimals: u8) -> Option<String> {
    let digits = raw.strip_prefix('+').unwrap_or(raw);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }

    let decimals = decimals as usize;
    if decimals == 0 {
        return Some(digits.to_string());
    }

    let padded = if digits.len() <= decimals {
        format!("{:0>width$}", digits, width = decimals + 1)
    } else {
        digits.to_string()
    };

    let split = padded.len() - decimals;
    let int_part = &padded[..split];
    let frac_part = padded[split..].trim_end_matches('0');

    if frac_part.is_empty() {
        Some(int_part.to_string())
    } else {
        Some(format!("{}.{}", int_part, frac_part))
    }
}

#[cfg(test)]
mod tests {
    use super::{format_native_value, format_token_amount, raw_to_decimal_string};

    #[test]
    fn raw_to_decimal_string_cases_are_table_driven() {
        for (raw, decimals, expected) in [
            ("1000000", 6, Some("1")),
            ("1234500", 6, Some("1.2345")),
            ("5", 18, Some("0.000000000000000005")),
            ("42", 0, Some("42")),
            ("", 18, None),
            ("-1", 18, None),
        ] {
            assert_eq!(
                raw_to_decimal_string(raw, decimals),
                expected.map(str::to_string)
            );
        }
    }

    #[test]
    fn formatted_amount_helpers_include_units() {
        assert_eq!(
            format_token_amount("1234500", 6, "USDC"),
            "1.2345 USDC (1234500)"
        );
        assert_eq!(
            format_native_value("1000000000000000000"),
            "1 ETH (1000000000000000000 wei)"
        );
    }
}
