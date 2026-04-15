---
name: chainpilot
description: >
  Use the ChainPilot CLI to perform DeFi operations on EVM-compatible chains —
  getting swap quotes via the DODO aggregator, simulating and executing swaps,
  managing ERC-20 approvals, querying token metadata, checking wallet balances,
  and running risk analysis. Always use this skill when the user mentions
  chainpilot, wants to swap tokens, check a token's risk score, query wallet
  balances, approve or revoke a spender, or inspect token contract metadata on
  any EVM chain (Ethereum, Arbitrum, Base, BNB Chain, Polygon, etc.).
---

# ChainPilot CLI

ChainPilot is a Rust CLI for on-chain DeFi operations on EVM networks. It uses
the DODO aggregator API for swap routing and `alloy` for RPC interaction.

## Installation

Before using ChainPilot, check if it is installed:

```bash
if ! command -v chainpilot &> /dev/null; then
  echo "ChainPilot not found, installing..."
  # Linux / macOS
  curl -fsSL https://raw.githubusercontent.com/DODOEX/ChainPilot/main/scripts/install.sh | bash 2>&1
  # Windows (PowerShell)
  # powershell -ExecutionPolicy Bypass -Command "iwr https://raw.githubusercontent.com/DODOEX/ChainPilot/main/scripts/install.ps1 -UseBasicParsing | iex"
fi
```

After installation, verify:
```bash
chainpilot --version
```

## Global Flags

These must appear **before** the subcommand:

```bash
chainpilot [GLOBAL_FLAGS] <subcommand> [SUBCOMMAND_FLAGS]
```

| Flag | Env var | Default | Notes |
|---|---|---|---|
| `--json` | — | off | Structured JSON output for scripting |
| `--quiet` | — | off | Suppress all output except errors |
| `--chain-id <N>` | `CHAIN_ID` | `1` (mainnet) | Active chain |
| `--rpc-url <url>` | `ETH_RPC_URL` | Chain's public RPC | JSON-RPC endpoint |
| `--private-key <hex>` | `PRIVATE_KEY` | — | Signer for write transactions |
| `--wallet-address <addr>` | `WALLET_ADDRESS` | — | Read-only wallet context |
| `--dodo-api-key <key>` | `DODO_API_KEY` | Compiled-in | DODO routing API key |
| `--dodo-project-id <id>` | `DODO_PROJECT_ID` | Compiled-in | Token list project ID |

Config precedence: CLI flag > env var > `.env` file > compile-time default.

## JSON Envelope

Every command returns the same shape:

```json
{ "ok": true,  "data": { ... } }   // success
{ "ok": false, "error": "reason" }  // failure
```

Use `--json` and pipe to `jq` for scripting.

---

## Typical Swap Workflow

This is the recommended end-to-end flow:

```bash
# 1. Get a quote (saves locally, returns quote_id)
QUOTE_ID=$(chainpilot --json swap quote \
  --from ETH --to USDC --amount 0.1 | jq -r .data.quote_id)

# 2. Simulate — read-only pre-flight (balance, allowance, gas, revert risk)
chainpilot swap simulate --quote-id "$QUOTE_ID" --wallet 0xYourAddress

# 3. Approve token spending if needed (skip for native ETH swaps)
chainpilot swap approve --quote-id "$QUOTE_ID" --private-key "$PRIVATE_KEY"

# 4. Execute and wait for on-chain confirmation
chainpilot swap execute --quote-id "$QUOTE_ID" \
  --private-key "$PRIVATE_KEY" --wait
```

Quotes have a **dual TTL**: the DODO-issued route expires in 20 minutes; the
local `QUOTE_TTL_SECS` (default 18 min) expires first. Both `simulate` and
`execute` reject stale quotes.

---

## `swap` Subcommands

### `swap quote`

```bash
chainpilot swap quote --from <TOKEN> --to <TOKEN> --amount <AMOUNT> \
  [--chain-id <N>] [--slippage <PCT>]
```

- `--from` / `--to`: symbol (`ETH`, `USDC`) or `0x` address
- `--amount`: human-readable (e.g. `1.0`, `100`)
- `--slippage`: slippage tolerance in percent (e.g. `0.5`)

Returns a `quote_id` to pass to subsequent commands.

### `swap simulate`

Read-only pre-flight. Checks balance, allowance, gas estimate, and revert risk
without spending gas.

```bash
chainpilot swap simulate --quote-id <ID> --wallet <ADDR>
```

### `swap approve`

Approve the DODO router to spend the from-token on your behalf.

```bash
# From a saved quote (derives token + spender automatically)
chainpilot swap approve --quote-id <ID> --private-key 0x...

# Explicit token, spender, and amount
chainpilot swap approve --token USDC --spender 0x... --amount 1000 \
  --private-key 0x...

# Unlimited approval (omit --amount)
chainpilot swap approve --token USDC --spender 0x... --private-key 0x...

# Dry-run (no tx sent)
chainpilot swap approve --quote-id <ID> --dry-run
```

### `swap execute`

```bash
chainpilot swap execute --quote-id <ID> --private-key 0x... [OPTIONS]
```

| Flag | Description |
|---|---|
| `--dry-run` | Build + simulate the tx, no broadcast; use `--wallet` instead of key |
| `--wait` | Block until mined, print final on-chain status |
| `--gas-limit <N>` | Hard-override gas limit |
| `--max-fee-gwei <N>` | Override EIP-1559 max fee (in gwei) |
| `--gas-buffer-pct <N>` | Add N% buffer on top of `eth_estimateGas` |
| `--skip-estimate` | Skip `eth_estimateGas`, use quote's estimate directly |

### `swap revoke`

```bash
chainpilot swap revoke --token <ADDR> --spender <ADDR> --private-key 0x...
chainpilot swap revoke --token <ADDR> --spender <ADDR> --dry-run
```

### `swap status`

```bash
chainpilot swap status --tx-hash 0x... [--chain-id <N>]
```

### `swap history`

```bash
chainpilot swap history [--limit <N>] [--status pending|confirmed|failed]
```

---

## `token` Subcommands

```bash
# ERC-20 metadata: name, symbol, decimals, total supply
chainpilot token info USDC
chainpilot token info 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48

# On-chain contract details: proxy, owner, implementation address
chainpilot token contract USDC
chainpilot token contract USDC --chain-id 137
```

---

## `wallet` Subcommands

```bash
# Native + ERC-20 balances for an address
chainpilot wallet balance 0xYourAddress
chainpilot wallet balance 0xYourAddress --chain-id 56

# Filter to specific tokens only
chainpilot wallet balance 0xYourAddress --tokens 0xToken1,0xToken2
```

---

## `risk` Subcommands

```bash
# Token risk: honeypot detection, ownership, liquidity flags
chainpilot risk token USDC
chainpilot risk token 0xSomeAddress --chain-id 1

# Wallet risk: exposure summary, high-risk approvals
chainpilot risk wallet 0xYourAddress

# Single approval state
chainpilot risk approval 0xYourAddress --token USDC --spender 0xSpenderAddr
```

---

## Token Resolution

Tokens can be a symbol or a `0x` address. Resolution order:

1. Native token symbol (`ETH`, `BNB`, etc.)
2. Raw `0x` address — decimals fetched on-chain
3. DODO tokenlist cache (1-hour TTL)
4. On-chain ERC-20 fallback

---

## Supported Chains

| Chain | Chain ID |
|---|---|
| Ethereum Mainnet | 1 |
| BNB Smart Chain | 56 |
| Polygon | 137 |
| Arbitrum One | 42161 |
| Optimism | 10 |
| Avalanche C-Chain | 43114 |
| Base | 8453 |
| Linea | 59144 |
| Scroll | 534352 |
| Manta Pacific | 169 |
| Mantle | 5000 |
| Aurora | 1313161554 |
| OKChain (X Layer) | 66 |
| Conflux eSpace | 1030 |
| Taiko | 167000 |
| Plume | 98866 |
| Sepolia Testnet | 11155111 |

For unsupported chain IDs, set `ETH_RPC_URL` manually.

---

## Scripting Patterns

```bash
# Check if simulate passed
chainpilot --json swap simulate --quote-id "$QUOTE_ID" \
  --wallet 0xAddr | jq '.data.ok'

# Full quote payload
chainpilot --json swap quote --from ETH --to USDC --amount 1.0 | jq .

# Quiet execution — only care about the exit code
chainpilot --quiet swap execute --quote-id "$QUOTE_ID" \
  --private-key "$PRIVATE_KEY" --wait
echo $?  # 0 = success
```

Enable debug logging:

```bash
RUST_LOG=debug chainpilot swap quote --from ETH --to USDC --amount 1
```
