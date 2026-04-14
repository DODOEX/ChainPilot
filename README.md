# ChainPilot

English | [中文](README_CN.md)

A command-line tool for on-chain DeFi operations on EVM-compatible networks. Built in Rust using [alloy](https://github.com/alloy-rs/alloy) for RPC interaction and the [DODO](https://dodoex.io) aggregator API for swap routing.

## Features

- Get swap quotes across 17+ EVM chains via DODO's routing engine
- Simulate swaps before execution (balance/allowance checks, gas estimation, revert detection)
- Execute swaps with optional tx waiting and status polling
- Manage ERC-20 approvals (approve / revoke)
- Query token metadata and wallet balances on-chain
- Risk analysis for tokens, wallets, and approval allowances
- Machine-readable JSON output (`--json`) for scripting and agent pipelines

## Installation

**Linux / macOS (recommended):**
```bash
curl -fsSL https://raw.githubusercontent.com/DODOEX/ChainPilot/main/scripts/install.sh | bash 2>&1
```

This downloads the latest pre-built binary for your platform into `~/.chainpilot/bin` and adds it to your `PATH`.

**Build from source:**
```bash
cargo build --release
# Binary at: target/release/chainpilot
```

To bake a DODO API key and project ID into the binary at compile time:

```bash
DODO_API_KEY=your-key DODO_PROJECT_ID=your-id cargo build --release
```

Alternatively, create a `.env` file in the project root before building:

```
DODO_API_KEY=your-key
DODO_PROJECT_ID=your-id
```

## Configuration

All settings can be supplied as environment variables, a `.env` file, or CLI flags. CLI flags take highest precedence.

| Variable               | CLI flag              | Default                        | Description                                    |
|------------------------|-----------------------|--------------------------------|------------------------------------------------|
| `PRIVATE_KEY`          | `--private-key`       | —                              | Private key for signing transactions           |
| `WALLET_ADDRESS`       | `--wallet-address`    | —                              | Wallet address for balance/simulate context    |
| `ETH_RPC_URL`          | `--rpc-url`           | Chain's built-in public RPC    | JSON-RPC endpoint                              |
| `CHAIN_ID`             | `--chain-id`          | `1` (Ethereum mainnet)         | Active chain ID                                |
| `DODO_API_KEY`         | `--dodo-api-key`      | Compiled-in default            | DODO routing API key                           |
| `DODO_PROJECT_ID`      | `--dodo-project-id`   | Compiled-in default            | DODO project ID for token list lookup          |
| `DODO_API_URL`         | —                     | DODO production endpoint       | Override routing API URL                       |
| `CHAIN_DATA_DIR`       | —                     | OS data dir (`chain/`)         | Directory for quotes and history               |
| `REQUEST_TIMEOUT_SECS` | —                     | `30`                           | HTTP request timeout                           |
| `QUOTE_TTL_SECS`       | —                     | `1080`                         | How long a saved quote stays valid (seconds)   |

Global flags (`--json`, `--quiet`, `--private-key`, `--wallet-address`, `--rpc-url`, `--dodo-api-key`, `--dodo-project-id`) apply to every subcommand and must appear before the subcommand name:

```bash
chainpilot --json --chain-id 42161 swap quote --from ETH --to USDC --amount 1.0
```

Enable debug logging:

```bash
RUST_LOG=debug chainpilot ...
```

## Token Resolution

Tokens can be specified as a symbol (`ETH`, `USDC`) or a `0x` contract address. Resolution order:

1. Native token symbol (e.g. `ETH`, `BNB`)
2. Raw `0x` address — decimals fetched on-chain
3. DODO tokenlist cache (1-hour TTL)
4. On-chain ERC-20 fallback

## Supported Chains

| Chain              | Chain ID   |
|--------------------|------------|
| Ethereum Mainnet   | 1          |
| BNB Smart Chain    | 56         |
| Polygon            | 137        |
| Arbitrum One       | 42161      |
| Optimism           | 10         |
| Avalanche C-Chain  | 43114      |
| Base               | 8453       |
| Linea              | 59144      |
| Scroll             | 534352     |
| Manta Pacific      | 169        |
| Mantle             | 5000       |
| Aurora             | 1313161554 |
| OKChain (X Layer)  | 66         |
| Conflux eSpace     | 1030       |
| Taiko              | 167000     |
| Plume              | 98866      |
| Sepolia Testnet    | 11155111   |

For unsupported chain IDs, set `ETH_RPC_URL` manually.

## Typical Swap Workflow

This is the recommended end-to-end flow:

```bash
# 1. Get a quote and capture its ID
QUOTE_ID=$(chainpilot --json swap quote --from ETH --to USDC --amount 0.1 | jq -r .data.quote_id)

# 2. Simulate — checks balance, allowance, gas, and potential reverts without spending gas
chainpilot swap simulate --quote-id "$QUOTE_ID" --wallet 0xYourAddress

# 3. Approve token spending if needed (skip for native ETH swaps)
chainpilot swap approve --quote-id "$QUOTE_ID" --private-key "$PRIVATE_KEY"

# 4. Execute and wait for confirmation
chainpilot swap execute --quote-id "$QUOTE_ID" --private-key "$PRIVATE_KEY" --wait
```

Quotes expire after `QUOTE_TTL_SECS` (default 18 minutes) locally, and the DODO-issued route carries its own 20-minute deadline. Both `simulate` and `execute` reject expired quotes.

## Usage

### Swap

**Get a quote:**
```bash
# Basic quote on Ethereum mainnet
chainpilot swap quote --from ETH --to USDC --amount 1.0

# Quote on Arbitrum with custom slippage tolerance
chainpilot swap quote --from ETH --to USDC --amount 1.0 --chain-id 42161 --slippage 0.5
```

The quote is saved locally and identified by a `quote_id`. Pass this ID to `simulate`, `approve`, and `execute`.

**Simulate a quote (pre-flight checks, no gas cost):**
```bash
# Check balance, allowance, gas estimate, and revert risk
chainpilot swap simulate --quote-id <QUOTE_ID> --wallet 0xYourAddress
```

Simulation is read-only — it costs no gas and does not broadcast any transaction. Use it to verify a quote is safe to execute.

**Execute a swap:**
```bash
# Dry-run: build and simulate the transaction without broadcasting
chainpilot swap execute --quote-id <QUOTE_ID> --dry-run --wallet 0xYourAddress

# Live execution (PRIVATE_KEY required)
chainpilot swap execute --quote-id <QUOTE_ID> --private-key 0x...

# Wait for the transaction to be mined before returning
chainpilot swap execute --quote-id <QUOTE_ID> --private-key 0x... --wait

# Override gas parameters
chainpilot swap execute --quote-id <QUOTE_ID> --private-key 0x... \
  --gas-limit 300000 \
  --max-fee-gwei 25 \
  --gas-buffer-pct 20

# Skip eth_estimateGas pre-flight and use quote's estimate directly
chainpilot swap execute --quote-id <QUOTE_ID> --private-key 0x... --skip-estimate
```

| Execute flag        | Description                                                              |
|---------------------|--------------------------------------------------------------------------|
| `--dry-run`         | Build and simulate the tx without broadcasting; `--wallet` instead of key |
| `--wait`            | Block until the tx is mined and show final on-chain status               |
| `--gas-limit`       | Hard override for gas limit                                              |
| `--max-fee-gwei`    | Override max fee per gas (EIP-1559), in gwei                             |
| `--gas-buffer-pct`  | Add N% buffer on top of `eth_estimateGas` result (e.g. `20` = +20%)     |
| `--skip-estimate`   | Skip `eth_estimateGas` and use the quote's gas estimate directly         |

**Check transaction status:**
```bash
chainpilot swap status --tx-hash 0x...
chainpilot swap status --tx-hash 0x... --chain-id 42161
```

**View swap history:**
```bash
chainpilot swap history
chainpilot swap history --limit 50
chainpilot swap history --limit 50 --status confirmed
# --status values: pending | confirmed | failed
```

**Approve token spending:**
```bash
# Approve from a saved quote (derives token and DODOApprove spender automatically)
chainpilot swap approve --quote-id <QUOTE_ID> --private-key 0x...

# Explicit token, spender, and amount
chainpilot swap approve --token USDC --spender 0x... --amount 1000 --private-key 0x...

# Omit --amount for unlimited approval (U256::MAX)
chainpilot swap approve --token USDC --spender 0x... --private-key 0x...

# Dry-run to preview without sending
chainpilot swap approve --quote-id <QUOTE_ID> --dry-run
```

**Revoke an approval:**
```bash
chainpilot swap revoke --token 0xTokenAddr --spender 0xSpenderAddr --private-key 0x...

# Dry-run
chainpilot swap revoke --token 0xTokenAddr --spender 0xSpenderAddr --dry-run
```

### Token

```bash
# Metadata: name, symbol, decimals, total supply
chainpilot token info USDC
chainpilot token info 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48

# On-chain contract details: proxy, owner, implementation
chainpilot token contract USDC
chainpilot token contract USDC --chain-id 137
```

### Wallet

```bash
# Native and ERC-20 balances
chainpilot wallet balance 0xYourAddress
chainpilot wallet balance 0xYourAddress --chain-id 56

# Check balances for specific tokens only
chainpilot wallet balance 0xYourAddress --tokens 0xToken1,0xToken2
```

### Risk

```bash
# Token risk analysis (honeypot detection, ownership, liquidity)
chainpilot risk token USDC
chainpilot risk token 0xSomeAddress --chain-id 1

# Wallet risk overview (exposure, high-risk approvals)
chainpilot risk wallet 0xYourAddress

# Check the current state of a specific approval
chainpilot risk approval 0xYourAddress --token USDC --spender 0xSpenderAddr
```

## Output Modes

By default, output is formatted as colored tables.

**JSON output** — add `--json` for structured output suitable for `jq` or agent pipelines:

```bash
# Capture a quote ID directly
QUOTE_ID=$(chainpilot --json swap quote --from ETH --to USDC --amount 1.0 | jq -r .data.quote_id)

# Inspect the full quote payload
chainpilot --json swap quote --from ETH --to USDC --amount 1.0 | jq .

# Check if a simulate passed
chainpilot --json swap simulate --quote-id "$QUOTE_ID" --wallet 0xAddr | jq .data.ok
```

The JSON envelope is always `{ "ok": true, "data": ... }` on success or `{ "ok": false, "error": "..." }` on failure.

**Quiet mode** — `--quiet` suppresses all output except errors. Useful in scripts where only the exit code matters.

## Building and Testing

```bash
cargo build
cargo test
cargo build --release
```
