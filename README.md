# ChainPilot

A command-line tool for on-chainpilot DeFi operations on EVM-compatible networks. Built in Rust using [alloy](https://github.com/alloy-rs/alloy) for RPC interaction and the [DODO](https://dodoex.io) aggregator API for swap routing.

## Features

- Get swap quotes across 17+ EVM chains via DODO's routing engine
- Simulate swaps before execution (balance/allowance checks, gas estimation, revert detection)
- Execute swaps with optional tx waiting and status polling
- Manage ERC-20 approvals (approve / revoke)
- Query token metadata and wallet balances on-chain
- Risk analysis for tokens, wallets, and approval allowances
- Machine-readable JSON output (`--json`) for scripting and agent pipelines

## Installation

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

| Variable              | CLI flag            | Default                        | Description                            |
|-----------------------|---------------------|--------------------------------|----------------------------------------|
| `PRIVATE_KEY`         | `--wallet`          | —                              | Private key for signing transactions   |
| `ETH_RPC_URL`         | `--rpc-url`         | Chain's built-in public RPC    | JSON-RPC endpoint                      |
| `CHAIN_ID`            | —                   | `1` (Ethereum mainnet)         | Active chainpilot                           |
| `DODO_API_KEY`        | `--dodo-api-key`    | Compiled-in default            | DODO routing API key                   |
| `DODO_PROJECT_ID`     | `--dodo-project-id` | Compiled-in default            | DODO project ID for token list lookup  |
| `DODO_API_URL`        | —                   | DODO production endpoint       | Override routing API URL               |
| `CHAIN_DATA_DIR`      | —                   | OS data dir (`chain/`)         | Directory for quotes and history       |
| `REQUEST_TIMEOUT_SECS`| —                   | `30`                           | HTTP request timeout                   |
| `QUOTE_TTL_SECS`      | —                   | `300`                          | How long a saved quote stays valid     |

Enable debug logging:

```bash
RUST_LOG=debug chainpilot ...
```

## Supported Chains

| Chain              | Chain ID |
|--------------------|----------|
| Ethereum Mainnet   | 1        |
| BNB Smart Chain    | 56       |
| Polygon            | 137      |
| Arbitrum One       | 42161    |
| Optimism           | 10       |
| Avalanche C-Chain  | 43114    |
| Base               | 8453     |
| Linea              | 59144    |
| Scroll             | 534352   |
| Manta Pacific      | 169      |
| Mantle             | 5000     |
| Aurora             | 1313161554 |
| OKChain (X Layer)  | 66       |
| Conflux eSpace     | 1030     |
| Taiko              | 167000   |
| Plume              | 98866    |
| Sepolia Testnet    | 11155111 |

For unsupported chainpilot IDs, set `ETH_RPC_URL` manually.

## Usage

### Swap

**Get a quote:**
```bash
chainpilot swap quote --from ETH --to USDC --amount 1.0
chainpilot swap quote --from ETH --to USDC --amount 1.0 --chain-id 42161 --slippage 0.5
```

**Simulate a quote (pre-flight checks without executing):**
```bash
chainpilot swap simulate --quote-id <QUOTE_ID>
chainpilot swap simulate --quote-id <QUOTE_ID> --wallet 0xYourAddress
```

**Execute a swap:**
```bash
# Dry-run (no transaction broadcast)
chainpilot swap execute --quote-id <QUOTE_ID> --dry-run

# Live execution (requires PRIVATE_KEY)
chainpilot swap execute --quote-id <QUOTE_ID> --private-key 0x...

# Wait for the transaction to be mined
chainpilot swap execute --quote-id <QUOTE_ID> --private-key 0x... --wait
```

**Check transaction status:**
```bash
chainpilot swap status --tx-hash 0x...
```

**View swap history:**
```bash
chainpilot swap history
chainpilot swap history --limit 50 --status confirmed
```

**Approve token spending:**
```bash
# Approve from a saved quote (unlimited amount to DODOApprove contract)
chainpilot swap approve --quote-id <QUOTE_ID> --private-key 0x...

# Explicit token, spender, and amount
chainpilot swap approve --token USDC --spender 0x... --amount 1000 --private-key 0x...

# Dry-run
chainpilot swap approve --quote-id <QUOTE_ID> --dry-run
```

**Revoke an approval:**
```bash
chainpilot swap revoke --token 0xTokenAddr --spender 0xSpenderAddr --private-key 0x...
```

### Token

```bash
chainpilot token info USDC
chainpilot token info 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
chainpilot token contract USDC --chain-id 137
```

### Wallet

```bash
chainpilot wallet balance 0xYourAddress
chainpilot wallet balance 0xYourAddress --chain-id 56
chainpilot wallet balance 0xYourAddress --tokens 0xToken1,0xToken2
```

### Risk

```bash
# Token risk analysis
chainpilot risk token USDC
chainpilot risk token 0xSomeAddress --chain-id 1

# Wallet risk overview
chainpilot risk wallet 0xYourAddress

# Check an approval
chainpilot risk approval 0xYourAddress --token USDC --spender 0xSpenderAddr
```

## Output Modes

By default, output is formatted as colored tables. Add `--json` for structured JSON output suitable for piping to `jq` or agent pipelines:

```bash
chainpilot --json swap quote --from ETH --to USDC --amount 1.0 | jq .data.quote_id
```

`--quiet` suppresses all output except errors.

## Typical Swap Workflow

```bash
# 1. Get a quote and save its ID
QUOTE_ID=$(chainpilot --json swap quote --from ETH --to USDC --amount 0.1 | jq -r .data.quote_id)

# 2. Simulate (check balance, allowance, gas, potential reverts)
chainpilot swap simulate --quote-id "$QUOTE_ID" --wallet 0xYourAddress

# 3. Approve spending if needed
chainpilot swap approve --quote-id "$QUOTE_ID" --private-key "$PRIVATE_KEY"

# 4. Execute
chainpilot swap execute --quote-id "$QUOTE_ID" --private-key "$PRIVATE_KEY" --wait
```

## Building and Testing

```bash
cargo build
cargo test
cargo build --release
```
