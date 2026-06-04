use serde::{Deserialize, Serialize};

// ── chain info ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    pub chain: String,
    pub chain_id: Option<u64>,
    pub native_token: Option<String>,
    pub native_price: Option<f64>,
    pub tvl: Option<f64>,
    pub active_addresses: Option<u64>,
    pub tx_count_24h: Option<u64>,
    pub fees_24h: Option<f64>,
    pub throughput: Option<f64>,
    pub sources: ChainInfoSources,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChainInfoSources {
    pub chain: Option<String>,
    pub chain_id: Option<String>,
    pub native_token: Option<String>,
    pub native_price: Option<String>,
    pub tvl: Option<String>,
    pub active_addresses: Option<String>,
    pub tx_count_24h: Option<String>,
    pub fees_24h: Option<String>,
    pub throughput: Option<String>,
}

// ── chain flows ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainFlows {
    pub chain: String,
    pub net_flow_usd: Option<f64>,
    pub inflow_usd: Option<f64>,
    pub outflow_usd: Option<f64>,
    pub bridge_flow: Vec<FlowEntry>,
    pub cex_flow: Vec<FlowEntry>,
    pub stablecoin_flow: Vec<FlowEntry>,
    pub sources: ChainFlowsSources,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChainFlowsSources {
    pub net_flow_usd: Option<String>,
    pub inflow_usd: Option<String>,
    pub outflow_usd: Option<String>,
    pub bridge_flow: Option<String>,
    pub cex_flow: Option<String>,
    pub stablecoin_flow: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEntry {
    pub name: String,
    pub flow_usd: f64,
}

// ── chain stablecoins ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainStablecoins {
    pub chain: String,
    pub stablecoin_supply: Option<f64>,
    pub stablecoin_types: Vec<StablecoinType>,
    pub stablecoin_flow_24h: Option<f64>,
    pub sources: ChainStablecoinsSources,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChainStablecoinsSources {
    pub stablecoin_supply: Option<String>,
    pub stablecoin_types: Option<String>,
    pub stablecoin_flow_24h: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StablecoinType {
    pub name: String,
    pub supply: f64,
    pub share_pct: f64,
}

// ── chain protocols ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainProtocols {
    pub chain: String,
    pub protocols: Vec<ChainProtocolEntry>,
    pub sources: ChainProtocolsSources,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChainProtocolsSources {
    pub protocols: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainProtocolEntry {
    pub name: String,
    pub tvl: Option<f64>,
    pub revenue: Option<f64>,
    pub users: Option<f64>,
    pub category: Option<String>,
}

// ── TableRenderable ──────────────────────────────────────────────────────────

impl crate::output::TableRenderable for ChainInfo {
    fn render_table(&self) {
        use comfy_table::Table;

        let mut table = Table::new();
        table.set_header(vec!["Field", "Value", "Source"]);
        table.add_row(vec![
            "Chain",
            &self.chain,
            self.sources.chain.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            "Chain ID",
            &self
                .chain_id
                .map(|v| v.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            self.sources.chain_id.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            "Native Token",
            self.native_token.as_deref().unwrap_or("N/A"),
            self.sources.native_token.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            "Native Price (USD)",
            &format_optional_usd(self.native_price),
            self.sources.native_price.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            "TVL (USD)",
            &format_optional_usd(self.tvl),
            self.sources.tvl.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            "Active Addresses",
            &self
                .active_addresses
                .map(|v| v.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            self.sources.active_addresses.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            "TX Count 24h",
            &self
                .tx_count_24h
                .map(|v| v.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            self.sources.tx_count_24h.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            "Fees 24h (USD)",
            &format_optional_usd(self.fees_24h),
            self.sources.fees_24h.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            "Throughput",
            &self
                .throughput
                .map(|v| format!("{:.2}", v))
                .unwrap_or_else(|| "N/A".to_string()),
            self.sources.throughput.as_deref().unwrap_or(""),
        ]);
        println!("{}", table);
    }
}

impl crate::output::TableRenderable for ChainFlows {
    fn render_table(&self) {
        use comfy_table::Table;

        let mut table = Table::new();
        table.set_header(vec!["Field", "Value", "Source"]);
        table.add_row(vec!["Chain", &self.chain, ""]);
        table.add_row(vec![
            "Net Flow (USD)",
            &format_optional_usd(self.net_flow_usd),
            self.sources.net_flow_usd.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            "Inflow (USD)",
            &format_optional_usd(self.inflow_usd),
            self.sources.inflow_usd.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            "Outflow (USD)",
            &format_optional_usd(self.outflow_usd),
            self.sources.outflow_usd.as_deref().unwrap_or(""),
        ]);
        println!("{}", table);

        if !self.bridge_flow.is_empty() {
            let mut t = Table::new();
            t.set_header(vec!["Bridge", "Flow (USD)"]);
            for entry in &self.bridge_flow {
                t.add_row(vec![&entry.name, &format_usd(entry.flow_usd)]);
            }
            println!("Bridge flows:");
            println!("{}", t);
        }

        if !self.cex_flow.is_empty() {
            let mut t = Table::new();
            t.set_header(vec!["Exchange", "Flow (USD)"]);
            for entry in &self.cex_flow {
                t.add_row(vec![&entry.name, &format_usd(entry.flow_usd)]);
            }
            println!("CEX flows:");
            println!("{}", t);
        }

        if !self.stablecoin_flow.is_empty() {
            let mut t = Table::new();
            t.set_header(vec!["Stablecoin", "Flow (USD)"]);
            for entry in &self.stablecoin_flow {
                t.add_row(vec![&entry.name, &format_usd(entry.flow_usd)]);
            }
            println!("Stablecoin flows:");
            println!("{}", t);
        }
    }
}

impl crate::output::TableRenderable for ChainStablecoins {
    fn render_table(&self) {
        use comfy_table::Table;

        let mut table = Table::new();
        table.set_header(vec!["Field", "Value", "Source"]);
        table.add_row(vec!["Chain", &self.chain, ""]);
        table.add_row(vec![
            "Stablecoin Supply (USD)",
            &format_optional_usd(self.stablecoin_supply),
            self.sources.stablecoin_supply.as_deref().unwrap_or(""),
        ]);
        table.add_row(vec![
            "Stablecoin Flow 24h (USD)",
            &format_optional_usd(self.stablecoin_flow_24h),
            self.sources.stablecoin_flow_24h.as_deref().unwrap_or(""),
        ]);
        println!("{}", table);

        if !self.stablecoin_types.is_empty() {
            let mut t = Table::new();
            t.set_header(vec!["Stablecoin", "Supply (USD)", "Share"]);
            for st in &self.stablecoin_types {
                t.add_row(vec![
                    &st.name,
                    &format_usd(st.supply),
                    &format!("{:.2}%", st.share_pct),
                ]);
            }
            println!("Stablecoin breakdown:");
            println!("{}", t);
        }
    }
}

impl crate::output::TableRenderable for ChainProtocols {
    fn render_table(&self) {
        use comfy_table::Table;

        println!("Chain: {}", self.chain);
        if self.protocols.is_empty() {
            println!("No protocols found.");
            return;
        }

        let mut table = Table::new();
        table.set_header(vec!["Protocol", "TVL (USD)", "Revenue (USD)", "Users", "Category"]);
        for p in &self.protocols {
            table.add_row(vec![
                &p.name,
                &format_optional_usd(p.tvl),
                &format_optional_usd(p.revenue),
                &p.users
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
                p.category.as_deref().unwrap_or("N/A"),
            ]);
        }
        println!("{}", table);
    }
}

fn format_usd(value: f64) -> String {
    if value.abs() >= 1.0 {
        format!("{:.2}", value)
    } else {
        format!("{:.8}", value)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn format_optional_usd(value: Option<f64>) -> String {
    value.map(format_usd).unwrap_or_else(|| "N/A".to_string())
}
