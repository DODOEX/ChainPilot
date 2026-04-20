# ChainPilot

English | [中文](README_CN.md)

A command-line tool for on-chain DeFi operations on EVM-compatible networks. Built in Rust using [alloy](https://github.com/alloy-rs/alloy) for RPC interaction and the [DODO](https://dodoex.io) aggregator API for swap routing.

## Features

- Get swap quotes across 17+ EVM chains via DODO's routing engine
- Simulate swaps before execution (balance/allowance checks, gas estimation, revert detection)
- Execute swaps with optional tx waiting and status polling
- Manage ERC-20 approvals (approve / revoke)
- Query token metadata and wallet balances on-chain
- Create ERC-20 tokens via DODO ERC20V3Factory, mint on mintable tokens, and renounce ownership
- Risk analysis for tokens, wallets, and approval allowances
- Machine-readable JSON output (`--json`) for scripting and agent pipelines

## Installation

**Linux / macOS (recommended):**
```bash
curl -fsSL https://raw.githubusercontent.com/DODOEX/ChainPilot/main/scripts/install.sh | bash 2>&1
```

This downloads the latest pre-built binary for your platform into `~/.chainpilot/bin` and adds it to your `PATH`.

**Windows (PowerShell):**
```powershell
powershell -ExecutionPolicy Bypass -Command "iwr https://raw.githubusercontent.com/DODOEX/ChainPilot/main/scripts/install.ps1 -UseBasicParsing | iex"
```

This downloads the latest Windows release into `%USERPROFILE%\.chainpilot\bin` and adds it to your user `PATH`.

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

Supported environment variables are intentionally limited. Runtime config is read only from `PRIVATE_KEY`, `KEYSTORE_PATH`, `KEYSTORE_PASSWORD_FILE`, `KEYSTORE_PASSWORD_ENV`, `KEYSTORE_PASSWORD`, `WALLET_ADDRESS`, `CHAIN_ID`, `DODO_API_KEY`, `DODO_PROJECT_ID`, and `DODO_API_URL`. CLI flags still override where available.

| Variable               | CLI flag              | Default                        | Description                                    |
|------------------------|-----------------------|--------------------------------|------------------------------------------------|
| `PRIVATE_KEY`          | `--private-key`       | —                              | Private key for signing transactions           |
| `KEYSTORE_PATH`        | `--keystore-path`     | —                              | Encrypted JSON keystore for signing            |
| `KEYSTORE_PASSWORD_FILE` | `--password-file`   | —                              | Read keystore password from file               |
| `KEYSTORE_PASSWORD_ENV` | `--password-env`     | —                              | Read keystore password from named env var      |
| `KEYSTORE_PASSWORD`    | —                     | —                              | Default env var used for keystore password     |
| `WALLET_ADDRESS`       | `--wallet-address`    | —                              | Wallet address for balance/simulate context    |
| `--rpc-url`            | CLI only              | Chain's built-in public RPC    | Explicit JSON-RPC override                     |
| `CHAIN_ID`             | `--chain-id`          | `1` (Ethereum mainnet)         | Active chain ID                                |
| `DODO_API_KEY`         | —                     | Compiled-in default            | DODO routing API key                           |
| `DODO_PROJECT_ID`      | —                     | Compiled-in default            | DODO project ID for token list lookup          |
| `DODO_API_URL`         | —                     | DODO production endpoint       | Override routing API URL                       |

Global flags (`--json`, `--quiet`, `--private-key`, `--keystore-path`, `--password-file`, `--password-env`, `--wallet-address`, `--rpc-url`, `--chain-id`) apply to every subcommand and must appear before the subcommand name:

```bash
chainpilot --json --chain-id 42161 swap quote --from ETH --to USDC --amount 1.0
```

Enable debug logging:

```bash
RUST_LOG=debug chainpilot ...
```

If `--keystore-path` is set, password resolution order is:

1. `--password-file`
2. `--password-env <NAME>`
3. `KEYSTORE_PASSWORD`
4. Interactive prompt when running in a TTY

## Token Resolution

Tokens can be specified as a symbol (`ETH`, `USDC`) or a `0x` contract address. Resolution order:

1. Native token symbol (e.g. `ETH`, `BNB`)
2. Raw `0x` address — decimals fetched on-chain
3. DODO tokenlist cache (1-hour TTL)
4. Custom token store (`token add` and successful address-based quotes)

Custom token behavior:

- Save a token manually with `chainpilot [--chain-id <N>] token add <0xTOKEN>`
- If a user gets a successful quote using a token address in `--from` or `--to`, the CLI automatically saves that token's metadata locally
- Later symbol lookups fall back to this local store when the DODO tokenlist does not contain the symbol

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

For unsupported chain IDs, pass `--rpc-url` manually.

## Typical Swap Workflow

This is the recommended end-to-end flow:

```bash
# 1. Get a quote and capture its ID
QUOTE_ID=$(chainpilot --json swap quote --from ETH --to USDC --amount 0.1 | jq -r .data.quote_id)

# 2. Simulate — checks balance, allowance, gas, and potential reverts without spending gas
chainpilot swap simulate --quote-id "$QUOTE_ID" --wallet 0xYourAddress

# 3. Approve token spending if needed (skip for native ETH swaps)
chainpilot --keystore-path ~/.chainpilot/main.json swap approve --quote-id "$QUOTE_ID"

# 4. Execute and wait for confirmation
chainpilot --keystore-path ~/.chainpilot/main.json swap execute --quote-id "$QUOTE_ID" --wait
```

Quotes expire after the local default TTL of 18 minutes, and the DODO-issued route carries its own 20-minute deadline. Both `simulate` and `execute` reject expired quotes.

## Usage

### Swap

**Get a quote:**
```bash
# Basic quote on Ethereum mainnet
chainpilot swap quote --from ETH --to USDC --amount 1.0

# Quote on Arbitrum with custom slippage tolerance
chainpilot --chain-id 42161 swap quote --from ETH --to USDC --amount 1.0 --slippage 0.5
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

# Live execution with a raw private key
chainpilot swap execute --quote-id <QUOTE_ID> --private-key 0x...

# Live execution with a keystore; prompts for password if needed
chainpilot --keystore-path ~/.chainpilot/main.json swap execute --quote-id <QUOTE_ID>

# Non-interactive keystore execution
chainpilot --keystore-path ~/.chainpilot/main.json --password-file ~/.chainpilot/main.pass \
  swap execute --quote-id <QUOTE_ID> --wait

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
chainpilot --chain-id 42161 swap status --tx-hash 0x...
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

# Same flow with a keystore signer
chainpilot --keystore-path ~/.chainpilot/main.json swap approve --quote-id <QUOTE_ID>

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

# Keystore signer
chainpilot --keystore-path ~/.chainpilot/main.json swap revoke --token 0xTokenAddr --spender 0xSpenderAddr

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
chainpilot --chain-id 137 token contract USDC

# Save a custom token locally for later symbol resolution
chainpilot token add 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
chainpilot --chain-id 8453 token add 0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913

# Read the token creation fee from the configured ERC20V3Factory
chainpilot --chain-id 11155111 token fee

# Create a standard token
chainpilot --chain-id 11155111 token create std \
  --name "ChainPilot Token" \
  --symbol CPT \
  --supply 1000000

# Create a custom token with trade burn / fee ratios
chainpilot --chain-id 11155111 token create custom \
  --name "ChainPilot Tax Token" \
  --symbol CPTX \
  --supply 1000000 \
  --burn-pct 0.1 \
  --fee-pct 1.25

# Create a mintable token
chainpilot --chain-id 11155111 token create mintable \
  --name "ChainPilot Mintable" \
  --symbol CPM \
  --supply 1000000

# Mint additional supply to a recipient
chainpilot --chain-id 11155111 token mint \
  --token 0xYourMintableToken \
  --to 0xRecipient \
  --amount 100

# Renounce ownership on a token that supports abandonOwnership(address(0))
chainpilot --chain-id 11155111 token renounce-ownership \
  --token 0xYourToken
```

### Wallet

```bash
# Native and ERC-20 balances
chainpilot wallet balance 0xYourAddress
chainpilot --chain-id 56 wallet balance 0xYourAddress

# Check balances for specific tokens only
chainpilot wallet balance 0xYourAddress --tokens 0xToken1,0xToken2
```

### Risk

```bash
# Token risk analysis (honeypot detection, ownership, liquidity)
chainpilot risk token USDC
chainpilot --chain-id 1 risk token 0xSomeAddress

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
