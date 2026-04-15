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
| `--keystore-path <path>` | `KEYSTORE_PATH` | — | Encrypted JSON keystore for write transactions |
| `--password-file <path>` | `KEYSTORE_PASSWORD_FILE` | — | Read keystore password from file |
| `--password-env <NAME>` | `KEYSTORE_PASSWORD_ENV` | — | Read keystore password from the named env var |
| `--wallet-address <addr>` | `WALLET_ADDRESS` | — | Read-only wallet context |
| `--rpc-url <url>` | — | Chain's public RPC | Explicit JSON-RPC override |
| `--chain-id <id>` | `CHAIN_ID` | `1` | Global chain context |

**Context propagation**: Once the user specifies a `--wallet-address` or `--chain-id`,
carry those values forward to all subsequent commands in the same conversation unless
the user explicitly asks for a different one.

Runtime env vars are intentionally limited to `PRIVATE_KEY`, `KEYSTORE_PATH`,
`KEYSTORE_PASSWORD_FILE`, `KEYSTORE_PASSWORD_ENV`, `KEYSTORE_PASSWORD`,
`WALLET_ADDRESS`, `CHAIN_ID`, `DODO_API_KEY`, `DODO_PROJECT_ID`, and
`DODO_API_URL`.

Config precedence: CLI flag > env var > `.env` file > compile-time default.

If `--keystore-path` is set, password resolution order is:

1. `--password-file`
2. `--password-env <NAME>`
3. `KEYSTORE_PASSWORD`
4. Interactive prompt when running in a TTY

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
QUOTE_ID=$(chainpilot --json --chain-id 1 swap quote \
  --from ETH --to USDC --amount 0.1 | jq -r .data.quote_id)

# 2. Simulate — read-only pre-flight (balance, allowance, gas, revert risk)
chainpilot swap simulate --quote-id "$QUOTE_ID" --wallet 0xYourAddress

# 3. Approve token spending if needed (skip for native ETH swaps)
chainpilot --keystore-path /path/to/keystore.json swap approve --quote-id "$QUOTE_ID"

# 4. Execute and wait for on-chain confirmation
chainpilot --keystore-path /path/to/keystore.json swap execute --quote-id "$QUOTE_ID" --wait
```

Quotes have a **dual TTL**: the DODO-issued route expires in 20 minutes; the
local default TTL is 18 minutes and expires first. Both `simulate` and
`execute` reject stale quotes.

---

## `swap` Subcommands

### `swap quote`

```bash
chainpilot [--chain-id <N>] swap quote --from <TOKEN> --to <TOKEN> --amount <AMOUNT> \
  [--slippage <PCT>]
```

- `--from` / `--to`: symbol (`ETH`, `USDC`) or `0x` address
- `--amount`: human-readable amount (e.g. `1.0`, `100`) — **required by CLI** (the CLI has no default; if user omits it, default to `1` yourself)
- `--chain-id`: global chain ID (default: 1); place it before the subcommand
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
chainpilot --private-key 0x... swap approve --quote-id <ID>

# Same flow with a keystore signer
chainpilot --keystore-path /path/to/keystore.json swap approve --quote-id <ID>

# Explicit token, spender, and amount
chainpilot --private-key 0x... swap approve --token USDC --spender 0x... --amount 1000

# Unlimited approval (omit --amount)
chainpilot --private-key 0x... swap approve --token USDC --spender 0x...

# Dry-run (no tx sent, private-key not needed)
chainpilot swap approve --quote-id <ID> --dry-run
```

### `swap execute`

```bash
chainpilot --private-key 0x... swap execute --quote-id <ID> [OPTIONS]

# Keystore signer; prompts for password if needed
chainpilot --keystore-path /path/to/keystore.json swap execute --quote-id <ID> [OPTIONS]

# Non-interactive keystore execution
chainpilot --keystore-path /path/to/keystore.json --password-file /path/to/keystore.pass \
  swap execute --quote-id <ID> [OPTIONS]
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
chainpilot --private-key 0x... swap revoke --token USDC --spender 0xRouter
chainpilot --keystore-path /path/to/keystore.json swap revoke --token USDC --spender 0xRouter
```

- `--dry-run`: dry-run mode, private-key not required.

### `swap status`

```bash
chainpilot [--chain-id <N>] swap status --tx-hash <HASH>
```

### `swap history`

```bash
chainpilot swap history [--limit <N>] [--status pending|success|failed]
```

---

## `token` Subcommands

### `token info`

```bash
chainpilot [--chain-id <N>] token info <TOKEN>
```

ERC-20 metadata: name, symbol, decimals, total supply.
`<TOKEN>` can be a symbol (`USDC`) or contract address (`0x...`).

### `token contract`

```bash
chainpilot [--chain-id <N>] token contract <TOKEN>
```

On-chain contract details: proxy, owner, implementation address.

---

## `wallet` Subcommands

### `wallet balance`

```bash
chainpilot [--chain-id <N>] wallet balance <ADDRESS> [--tokens <ADDR1,ADDR2>]
```

Native + ERC-20 balances for an address.

---

## `risk` Subcommands

### `risk token`

```bash
chainpilot [--chain-id <N>] risk token <TOKEN>
```

Token risk: honeypot detection, ownership, liquidity flags.

### `risk wallet`

```bash
chainpilot risk wallet <ADDRESS>
```

Wallet risk: exposure summary, high-risk approvals.

### `risk approval`

```bash
chainpilot [--chain-id <N>] risk approval <ADDRESS> --token <TOKEN> --spender <SPENDER>
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

For unsupported chain IDs, pass `--rpc-url` manually.

---

## Scripting Patterns

```bash
# Check if simulate passed
chainpilot --json swap simulate --quote-id "$QUOTE_ID" \
  --wallet 0xAddr | jq '.data.ok'

# Full quote payload
chainpilot --json swap quote --from ETH --to USDC --amount 1.0 | jq .

# Quiet execution — only care about the exit code
chainpilot --quiet --private-key "$PRIVATE_KEY" swap execute --quote-id "$QUOTE_ID" --wait
chainpilot --quiet --keystore-path /path/to/keystore.json swap execute --quote-id "$QUOTE_ID" --wait
echo $?  # 0 = success
```

Enable debug logging:

```bash
RUST_LOG=debug chainpilot swap quote --from ETH --to USDC --amount 1
```
