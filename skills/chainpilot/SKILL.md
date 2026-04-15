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
chainpilot [GLOBAL_FLAGS] <COMMAND> [SUBcommand_FLAGS]
```

| Flag | Env var | Default | Notes |
|---|---|---|---|
| `--json` | — | off | Structured JSON output for scripting |
| `--quiet` | — | off | Suppress all output except errors |
| `--private-key <hex>` | `PRIVATE_KEY` | — | Signer for write transactions |
| `--wallet-address <addr>` | `WALLET_ADDRESS` | — | Read-only wallet context |

**Context propagation**: Once the user specifies a `--wallet-address` or `--chain-id`,
carry those values forward to all subsequent commands in the same conversation unless
the user explicitly asks for a different one.
| `--rpc-url <url>` | `ETH_RPC_URL` | Chain's public RPC | JSON-RPC endpoint |
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
- `--amount`: human-readable amount (e.g. `1.0`, `100`) — **required by CLI** (the CLI has no default; if user omits it, default to `1` yourself)
- `--chain-id`: chain ID (default: 1)
- `--slippage`: slippage tolerance in percent (default: 0.2)

Returns a `quote_id` to pass to subsequent commands.

**Token not found handling**: If `chainpilot swap quote` returns an error indicating
the token symbol was not found, use the Coingecko API to search for the token's
contract address on the target chain:

```bash
# Search token address via Coingecko
curl -s "https://api.coingecko.com/api/v3/search?query=<SYMBOL>" | jq '.coins[] | select(.symbol == "<SYMBOL_LOWER>") | {name, id, platforms}'
```

Then show the found address (filtered by target chain, e.g. `ethereum`, `polygon`,
`arbitrum`, `base`, `bnb`) to the user and ask for confirmation before retrying
with the address.

### `swap simulate`

Read-only pre-flight. Checks balance, allowance, gas estimate, and revert risk
without spending gas.

```bash
chainpilot swap simulate --quote-id <ID> --wallet <ADDR>
```

- `--wallet`: wallet address for balance/allowance checks (not `--wallet-address`)

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

# Dry-run (no tx sent, private-key not needed)
chainpilot swap approve --quote-id <ID> --dry-run
```

### `swap execute`

```bash
chainpilot swap execute --quote-id <ID> --private-key 0x... [OPTIONS]
```

| Flag | Description |
|---|---|
| `--dry-run` | Simulate execution without broadcasting — use `--wallet` instead of `--private-key` |
| `--wallet <ADDR>` | Wallet address for dry-run (not `--wallet-address`) |
| `--wait` | Block until mined, print final on-chain status |
| `--gas-limit <N>` | Hard-override gas limit |
| `--max-fee-gwei <N>` | Override EIP-1559 max fee (in gwei) |
| `--gas-buffer-pct <N>` | Add N% buffer on top of `eth_estimateGas` |
| `--skip-estimate` | Skip `eth_estimateGas`, use quote's gas estimate directly |

### `swap revoke`

```bash
chainpilot swap revoke --token <ADDR> --spender <ADDR> [--dry-run]
chainpilot swap revoke --token USDC --spender 0xRouter --private-key 0x...
```

- `--dry-run`: dry-run mode, private-key not required.

### `swap status`

```bash
chainpilot swap status --tx-hash <HASH> [--chain-id <N>]
```

### `swap history`

```bash
chainpilot swap history [--limit <N>] [--status pending|success|failed]
```

---

## `token` Subcommands

### `token info`

```bash
chainpilot token info <TOKEN> [--chain-id <N>]
```

ERC-20 metadata: name, symbol, decimals, total supply.
`<TOKEN>` can be a symbol (`USDC`) or contract address (`0x...`).

### `token contract`

```bash
chainpilot token contract <TOKEN> [--chain-id <N>]
```

On-chain contract details: proxy, owner, implementation address.

---

## `wallet` Subcommands

### `wallet balance`

```bash
chainpilot wallet balance <ADDRESS> [--chain-id <N>] [--tokens <ADDR1,ADDR2>]
```

Native + ERC-20 balances for an address.

---

## `risk` Subcommands

### `risk token`

```bash
chainpilot risk token <TOKEN> [--chain-id <N>]
```

Token risk: honeypot detection, ownership, liquidity flags.

### `risk wallet`

```bash
chainpilot risk wallet <ADDRESS>
```

Wallet risk: exposure summary, high-risk approvals.

### `risk approval`

```bash
chainpilot risk approval <ADDRESS> --token <TOKEN> --spender <SPENDER> [--chain-id <N>]
```

Single approval state.

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
