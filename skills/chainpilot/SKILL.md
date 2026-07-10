---
name: chainpilot
description: >
  Use the ChainPilot CLI to perform DeFi operations on EVM-compatible chains —
  getting swap quotes via the DODO aggregator, simulating and executing swaps,
  managing ERC-20 approvals, querying token metadata, creating tokens through
  DODO's ERC20V3Factory, minting mintable tokens, checking wallet balances,
  cross-chain portfolio overviews, DeFi positions, PnL analysis, transaction
  history, and wallet labels (via Debank / Zerion / Goldrush / Dune),
  chain-level analytics (TVL, fees, native price, stablecoins, top protocols,
  active addresses / tx count / throughput), and running risk analysis. Always
  use this skill when the user mentions chainpilot, wants to swap tokens, create
  a token, mint a token, check a token's risk score, query wallet balances,
  DeFi positions, portfolio breakdown, PnL, transaction history, wallet labels,
  inspect a chain's TVL / fees / stablecoins / top protocols / activity, approve
  or revoke a spender, or inspect token contract metadata on any EVM chain
  (Ethereum, Arbitrum, Base, BNB Chain, Polygon, etc.).
---

# ChainPilot CLI

ChainPilot is a Rust CLI for on-chain DeFi operations on EVM networks. It uses
the DODO aggregator API for swap routing and `alloy` for RPC interaction.

## Non-EVM (SVM / BVM) support matrix

ChainPilot's read-only commands also work on Solana (SVM) and Bitcoin
mainnet (BVM). Swap *execution* (simulate / execute / status / history),
approvals, and token create / mint / renounce remain EVM-only by design.
The one read-only exception is `swap quote`, which works on Solana via the
Jupiter aggregator (both `--from` and `--to` must be SPL mints). Use
`solana` / `svm` and `bitcoin` / `bvm` as chain aliases for analytics; pass
SPL mints or BTC addresses directly for token/wallet/risk queries — address
shape decides routing.

| Command | EVM | SVM (Solana) | BVM (Bitcoin) |
|---|---|---|---|
| `chain info/flows/stablecoins/protocols` | ✓ | ✓ DefiLlama | ✓ DefiLlama |
| `protocol info/tvl/revenue/chains` | ✓ | ✓ DefiLlama | ✓ DefiLlama |
| `wallet balance` | ✓ | ✓ Debank → Zerion | ✓ mempool.space |
| `wallet overview` | ✓ | ✓ Debank → Zerion | ✓ derived from balance |
| `wallet pnl` | ✓ | ✓ Zerion | ✗ no data source |
| `wallet history` | ✓ | ✓ Zerion → Debank | ✓ mempool.space |
| `wallet labels` | ✓ | ✓ Zerion (Dune is EVM-only) | ✗ not integrated¹ |
| `wallet defi` | ✓ | ✓ Debank → Zerion | ✗ no data source |
| `token info` | ✓ | ✓ Jupiter + CoinGecko + DexScreener | ✗ no metadata source |
| `token price` | ✓ | ✓ CoinGecko + DexScreener | ✗ no metadata source |
| `token liquidity` | ✓ | ✓ DexScreener | ✗ no metadata source |
| `token risk` | ✓ GoPlus | ✓ GoPlus Solana (authority-based) | ✗ no metadata source |
| `token contract / add / create / mint / fee / renounce` | ✓ | ✗ EVM-only | ✗ EVM-only |
| `risk token` | ✓ | ✓ GoPlus Solana | ✗ no metadata source |
| `risk wallet` | ✓ ETH balance + GoPlus address reputation | ✓ GoPlus address reputation | ✗ no data source |
| `risk approval` | ✓ | ✗ EVM-only concept | ✗ EVM-only concept |
| `swap quote` | ✓ DODO | ✓ Jupiter (read-only, SPL mints) | ✗ no route provider |
| `swap simulate / execute / status / history` | ✓ | ✗ EVM-only | ✗ EVM-only |

For Solana wallet queries, **Debank is strongly recommended over Zerion** —
Zerion's Solana indexing is asynchronous and may time out for cold wallets.
On the `--chain-id` global flag: SVM/BVM ignore it (chain context comes
from the address itself); EVM commands continue to require it for the
target chain selection.

¹ Bitcoin entity labels are not integrated. The integrated provider
(mempool.space) returns raw on-chain data only — no attribution. Sources that
do cover BTC labels (Breadcrumbs, MetaSleuth, Arkham, Chainalysis) all require
a new client and a paid/keyed API, and BTC isn't a DODO trading target, so
this is deferred rather than impossible.

## Installation

Before using ChainPilot, check if it is installed:

```bash
chainpilot --version
```

If not found, refer to the installation instructions in the
[ChainPilot README](https://github.com/DODOEX/ChainPilot) and ask the user
to install it manually before proceeding. Do **not** run remote install
scripts on the user's behalf.

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
| private-key flag | `PRIVATE_KEY` | — | Supported by the CLI for signer input; prefer keystore or env-configured signer context in generated commands |
| `--keystore-path <path>` | `KEYSTORE_PATH` | — | Encrypted JSON keystore for write transactions |
| `--password-file <path>` | `KEYSTORE_PASSWORD_FILE` | — | Read keystore password from file |
| `--password-env <NAME>` | `KEYSTORE_PASSWORD_ENV` | — | Read keystore password from the named env var |
| `--wallet-address <addr>` | `WALLET_ADDRESS` | — | Read-only wallet context and dry-run sender fallback |
| `--rpc-url <url>` | — | Chain's public RPC | Explicit JSON-RPC override |
| `--chain-id <id>` | `CHAIN_ID` | `1` | Global chain context |

**Context propagation**: Once the user specifies a `--wallet-address` or `--chain-id`,
carry those values forward to all subsequent commands in the same conversation unless
the user explicitly asks for a different one.

## Credential Safety

- Never ask the user to paste a raw private key, API key, mnemonic, or keystore password into chat.
- Never include a real secret value in a generated command, example, log snippet, or echoed output.
- When a signer is needed, prefer `--keystore-path` plus interactive password prompt, `--password-env <NAME>`, or an already-set environment variable such as `PRIVATE_KEY`.
- Avoid generating commands that pass secrets via CLI flags when an env var or keystore-based alternative exists. In particular, do not suggest `--private-key <secret>` in normal responses.
- If a command must reference a secret-backed env var, mention only the variable name, for example `PRIVATE_KEY` or `DODO_API_KEY`, and do not expand it in the response.
- If the user already shared a secret in chat, do not repeat it; instruct them to rotate it if exposure risk is material.

Runtime env vars are intentionally limited to `PRIVATE_KEY`, `KEYSTORE_PATH`,
`KEYSTORE_PASSWORD_FILE`, `KEYSTORE_PASSWORD_ENV`, `KEYSTORE_PASSWORD`,
`WALLET_ADDRESS`, `CHAIN_ID`, `DODO_API_KEY`, `DODO_PROJECT_ID`,
`DODO_API_URL`, `COINGECKO_API_URL`, `COINGECKO_API_KEY`,
`DEXSCREENER_API_URL`, `DEBANK_API_KEY`, `DEBANK_API_URL`,
`ZERION_API_KEY`, `ZERION_API_URL`, `GOLDRUSH_API_KEY`,
`GOLDRUSH_API_URL`, `DUNE_API_KEY`, and `DUNE_API_URL`.

Runtime config precedence: CLI flag > existing environment variable / `.env` file
> persistent `config.env` file > compile-time default.

Use `chainpilot config set` for supported API keys instead of asking the user to
paste secrets into shell commands. `config.env` is stored in ChainPilot's local
data directory and sensitive values are masked by `config get` / `config list`.

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

## JSON Output Field Reference

When reading `--json` output, some fields are **already human-readable** and others are **raw integers
in the token's smallest unit**. Never present a raw-integer field directly to the user — convert it
first or use the matching `*_display` field instead.

### `swap quote` — `Quote` object

| Field | Display-ready? | Notes |
|---|---|---|
| `from_amount_display` | ✓ | Always prefer this for display |
| `from_amount` | ✓ | Same value as `from_amount_display`, stringified |
| `to_amount_display` | ✓ | Always prefer this for display |
| `to_amount` | ✓ | Same value as `to_amount_display`, stringified |
| `to_amount_min` | **✗ raw** | Integer in `to_token.decimals` units — divide by `10^to_token.decimals` to display |
| `value` | **✗ raw** | Native token value in wei — divide by `1e18` to show as ETH |
| `exchange_rate` | ✓ | Human-readable ratio |
| `price_impact_pct` | ✓ | Already a percentage |

### `swap simulate` — `SimulationResult` object

| Field | Display-ready? | Notes |
|---|---|---|
| `expected_out` | ✓ | Mirrors `to_amount` — human-readable |
| `min_out` | **✗ raw** | Same raw integer as `to_amount_min`; divide by `10^to_token.decimals` |
| `wallet_balance` | **✗ raw** | Integer in from-token's smallest unit |
| `current_allowance` | **✗ raw** | Integer in from-token's smallest unit |
| `suggested_approve_amount` | **✗ raw** | Integer in from-token's smallest unit |
| `total_gas_cost_eth` | ✓ | Already in ETH |
| `gas_price_gwei` | ✓ | Already in gwei |

**Conversion formula** (when `decimals` is known):
```
human_amount = raw_integer / 10^decimals
```
The token's decimals are available at `data.to_token.decimals` (quote) or from the quote used for
the simulation. For native ETH, use `decimals = 18`.

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

### External signer dry-run workflow

Use `--json --dry-run` when an external wallet service needs an unsigned
transaction payload instead of ChainPilot signing locally. Dry-run output is not
execution: it does not sign, broadcast, or return a transaction hash.

```bash
# Swap execution payload from a saved quote
chainpilot --json swap execute --quote-id <ID> --dry-run --wallet <ADDR>

# ERC-20 approval payload from a saved quote
chainpilot --json swap approve --quote-id <ID> --dry-run --wallet-address <ADDR>

# Explicit ERC-20 approval payload
chainpilot --json swap approve \
  --token USDC \
  --spender 0xSpenderAddr \
  --amount 100 \
  --dry-run \
  --wallet-address <ADDR>

# ERC-20 revoke payload
chainpilot --json swap revoke \
  --token 0xTokenAddr \
  --spender 0xSpenderAddr \
  --dry-run \
  --wallet-address <ADDR>
```

The JSON `data` object includes the legacy preview fields plus:

| Field | Notes |
|---|---|
| `source` | Always `chainpilot` |
| `operation` | `swap_execute`, `approve`, or `revoke` |
| `chain_id` / `caip2` | EVM chain identity |
| `from` | Wallet address supplied for dry-run |
| `transaction.to` | Router for swaps; token contract for approve/revoke |
| `transaction.value` | Native value as hex, e.g. `0x0` |
| `transaction.data` | Calldata; approve/revoke use ERC-20 `approve(address,uint256)` |
| `transaction.chain_id` | Must match `chain_id` |
| `quote` | Present for quote-derived swap execution |
| `risk` | Token, amount, spender/router, and gas metadata when available |

All external-signer dry-run commands require a sender wallet. For
`swap execute --dry-run`, use `--wallet`; if it is omitted, the command falls
back to the global `--wallet-address` / `WALLET_ADDRESS` sender context. For
approve/revoke dry-runs, use the global `--wallet-address` / `WALLET_ADDRESS`
context. If no sender can be resolved, stop on the CLI error instead of
treating the payload as usable.

External signer integrations must still perform their own authorization,
confirmation, budget, and risk checks before submitting the transaction.

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

**Solana (SVM) quotes**: if both `--from` and `--to` are SPL mints, the quote
is routed to the Jupiter aggregator instead of DODO (read-only pricing — no
DODO liquidity is used). The result uses the same `Quote` shape with
`chain_id: 0`; EVM-only fields (`router_to`, `calldata`, gas) are empty and
`dex_sources` lists the Jupiter route venues. This is quote-only: the returned
`quote_id` is **not** persisted, so `swap simulate/execute` don't accept it —
those stay EVM-only. Symbols aren't resolved on SVM; pass mint addresses.
Mixing an SPL mint with an EVM token, or using a BTC address, is rejected.

If the user passes a token address in `--from` or `--to` and the quote succeeds,
the CLI automatically persists that token's metadata locally. Future symbol
lookups can fall back to this local store when the DODO tokenlist does not have
the symbol.

**Token not found handling**: If `chainpilot swap quote` returns an error indicating
the token symbol was not found, first try the built-in token search surfaced by
the metadata commands:

```bash
chainpilot [--chain-id <N>] token info <SYMBOL>
```

If candidates are returned, show the candidate address, chain, source, and
liquidity to the user and require explicit confirmation before retrying the swap
with an address. Never pass an externally sourced address directly into a swap
command without the user first approving it.

If the CLI returns no candidates, optionally use the CoinGecko API to search for
the token's contract address on the target chain:

```bash
# Search token address via Coingecko
curl -s "https://api.coingecko.com/api/v3/search?query=<SYMBOL>" | jq '.coins[] | select(.symbol == "<SYMBOL_LOWER>") | {name, id, platforms}'
```

Then show the found address (filtered by target chain, e.g. `ethereum`, `polygon`,
`arbitrum`, `base`, `bnb`) to the user and **require explicit confirmation**
before retrying with the address.

### `swap simulate`

Read-only pre-flight. Checks balance, allowance, gas estimate, and revert risk
without spending gas.

```bash
chainpilot swap simulate --quote-id <ID> --wallet <ADDR>
```

- `--wallet`: wallet address for balance/allowance checks (not `--wallet-address`)

### `swap approve`

> **Confirmation required**: Before running `swap approve`, display the token,
> spender address, and allowance amount to the user and wait for explicit
> approval. This is an on-chain transaction that cannot be undone automatically.

Approve the DODO router to spend the from-token on your behalf.

```bash
# From a saved quote (derives token + spender automatically)
chainpilot --keystore-path /path/to/keystore.json swap approve --quote-id <ID>

# Same flow when a signer is already configured via environment
chainpilot swap approve --quote-id <ID>

# Explicit token, spender, and amount
chainpilot --keystore-path /path/to/keystore.json swap approve --token USDC --spender 0x... --amount 1000

# Unlimited approval (omit --amount), using the configured signer context
chainpilot swap approve --token USDC --spender 0x...

# Dry-run external-signer payload from a saved quote
chainpilot --json swap approve --quote-id <ID> --dry-run --wallet-address <ADDR>

# Dry-run external-signer payload with explicit token/spender/amount
chainpilot --json swap approve --token USDC --spender 0x... --amount 1000 --dry-run --wallet-address <ADDR>
```

### `swap execute`

> **Confirmation required**: Before running `swap execute`, show the user the
> full swap summary (from-token, to-token, amount, estimated output, slippage,
> gas estimate) and wait for explicit approval. This broadcasts an irreversible
> on-chain transaction.

```bash
# Uses the configured signer context from the environment
chainpilot swap execute --quote-id <ID> [OPTIONS]

# Keystore signer; prompts for password if needed
chainpilot --keystore-path /path/to/keystore.json swap execute --quote-id <ID> [OPTIONS]

# Non-interactive keystore execution
chainpilot --keystore-path /path/to/keystore.json --password-file /path/to/keystore.pass \
  swap execute --quote-id <ID> [OPTIONS]

# Dry-run external-signer payload from a saved quote
chainpilot --json swap execute --quote-id <ID> --dry-run --wallet <ADDR>
```

| Flag | Description |
|---|---|
| `--dry-run` | Build an unsigned swap transaction payload without broadcasting — use `--wallet` or global wallet context instead of a signer |
| `--wallet <ADDR>` | Preferred wallet address for execute dry-run payloads; falls back to global `--wallet-address` / `WALLET_ADDRESS` when omitted |
| `--wait` | Block until mined, print final on-chain status |
| `--gas-limit <N>` | Hard-override gas limit |
| `--max-fee-gwei <N>` | Override EIP-1559 max fee (in gwei) |
| `--gas-buffer-pct <N>` | Add N% buffer on top of `eth_estimateGas` |
| `--skip-estimate` | Skip `eth_estimateGas`, use quote's gas estimate directly |

### `swap revoke`

```bash
chainpilot swap revoke --token <ADDR> --spender <ADDR> [--dry-run]
chainpilot swap revoke --token 0xTokenAddr --spender 0xRouter
chainpilot --keystore-path /path/to/keystore.json swap revoke --token 0xTokenAddr --spender 0xRouter
chainpilot --json swap revoke --token 0xTokenAddr --spender 0xSpenderAddr --dry-run --wallet-address <ADDR>
```

- `--dry-run`: build an unsigned ERC-20 revoke payload without sending; use
  `--wallet-address` to populate `data.from`.

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

### Non-EVM token support

`token info`, `token price`, and `token liquidity` accept Solana SPL mints
(base58 strings) in place of an EVM address. Detection is by address shape —
SPL mints (32–44 base58 chars) and Bitcoin addresses (`bc1…`, `1…`, `3…`)
are routed to dedicated paths. Symbol-only queries (e.g. `token info USDC`)
continue to resolve against the active EVM `--chain-id` — to query Solana
USDC, pass its mint: `chainpilot token info EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`.

| Command | Solana | Bitcoin |
|---|---|---|
| `token info` | Jupiter (identity) + CoinGecko + DexScreener | not supported |
| `token price` | CoinGecko `coins/solana/contract` + DexScreener | not supported |
| `token liquidity` | DexScreener Solana pools | not supported |
| `token risk` | GoPlus Solana token_security (authority-based) | not supported |
| `token contract` / create / mint / fee | EVM-only | not supported |

JSON output for SPL queries reports `chain_id: 0` (sentinel for non-EVM)
and `chain: "Solana"`. `token contract`, `token create`, `token mint`, and
`token fee` remain EVM-only and reject non-EVM addresses.

`token risk` on Solana uses authority-based signals — mint authority,
freeze/close authority, transfer fee, transfer hook, non-transferable —
in place of the EVM honeypot/blacklist fields. `risk_level` collapses to
`high` when transfer is fee-charged, hook-restricted, or
non-transferable; `medium` when any authority is active; `low` only when
GoPlus flags the mint as a `trusted_token` and no authority is active.

### `token info`

```bash
chainpilot [--chain-id <N>] token info <TOKEN>
```

Token metadata from on-chain reads plus external enrichment. Output can include
name, symbol, decimals, chain, website/social links, price, market cap, FDV,
liquidity, volume, 24h price change, risk level, and per-field sources.
`<TOKEN>` can be a symbol (`USDC`), native token symbol (`ETH`), or contract
address (`0x...`). Unknown symbols may return candidate matches from
CoinGecko/DexScreener instead of hard failing.

### `token contract`

```bash
chainpilot [--chain-id <N>] token contract <TOKEN>
```

On-chain contract details: proxy, owner, implementation address.

### `token price`

```bash
chainpilot [--chain-id <N>] token price <TOKEN>
```

Real-time price, short-term and mid-term changes, and 24h high/low. CoinGecko is the primary data
source; DexScreener is a fallback for `price`, `price_change_1h`, and `price_change_24h` when
CoinGecko has no value (e.g. long-tail tokens not listed there).

| Field | Primary | Fallback | Notes |
|---|---|---|---|
| `price` | CoinGecko | DexScreener | USD spot price |
| `price_change_1h` | CoinGecko | DexScreener | % change over 1 hour |
| `price_change_24h` | CoinGecko | DexScreener | % change over 24 hours |
| `price_change_7d` | CoinGecko | — | % change over 7 days |
| `high_24h` | CoinGecko | — | 24h high (USD) |
| `low_24h` | CoinGecko | — | 24h low (USD) |

The JSON output includes a `sources` map indicating which API supplied each field, so callers can
distinguish CoinGecko-backed values from DexScreener-backed ones.

### `token liquidity`

```bash
chainpilot [--chain-id <N>] token liquidity <TOKEN>
```

Liquidity overview from DexScreener: top liquidity across all pairs, pair count,
and top pair details (DEX, pair address, 24h volume).

| Field | Source | Notes |
|---|---|---|
| `top_liquidity` | DexScreener | Highest single-pair liquidity (USD) |
| `pair_count` | DexScreener | Number of matching base-token pairs |
| `top_pair.dex` | DexScreener | DEX ID of the top pair (e.g. `uniswap`) |
| `top_pair.pair_address` | DexScreener | Contract address of the top pair |
| `top_pair.liquidity` | DexScreener | Top pair's liquidity (USD) |
| `top_pair.volume_24h` | DexScreener | Top pair's 24h trading volume (USD) |

### `token risk`

```bash
chainpilot [--chain-id <N>] token risk <TOKEN>
```

Token risk analysis from GoPlus Security: honeypot detection, blacklist status,
transfer restrictions, minting, owner privileges, and buy/sell tax. Free API,
no credentials required.

| Field | Source | Notes |
|---|---|---|
| `risk_level` | GoPlus (derived) | `low`, `medium`, or `high` |
| `risk_score` | GoPlus (derived) | 0–100 composite score |
| `honeypot` | GoPlus | Whether the token is a honeypot |
| `blacklist` | GoPlus | Whether the token has a blacklist |
| `transfer_restricted` | GoPlus | Whether transfers are pausable |
| `mintable` | GoPlus | Whether new tokens can be minted |
| `owner_privileged` | GoPlus | Whether owner can change balances |
| `tax_buy` | GoPlus | Buy tax percentage |
| `tax_sell` | GoPlus | Sell tax percentage |

Native tokens (ETH, BNB, etc.) return hardcoded low-risk defaults.

Use `token risk` for the current token-specific GoPlus implementation. The
older top-level `risk token` command remains available but may expose a simpler
legacy risk shape.

### `token add`

```bash
chainpilot [--chain-id <N>] token add <TOKEN_ADDRESS>
```

Fetch the token metadata on-chain and save it to the local custom token store.
Use this when a token symbol is missing from the DODO tokenlist but the user
already knows the contract address.

### `token fee`

```bash
chainpilot [--chain-id <N>] token fee
```

Read the ERC20V3Factory create fee for the active chain.

### `token create std`

```bash
chainpilot [--chain-id <N>] token create std \
  --name <NAME> --symbol <SYMBOL> --supply <AMOUNT> [--decimals <N>] [--dry-run]
```

Create a standard ERC-20 through DODO's ERC20V3Factory.

### `token create custom`

```bash
chainpilot [--chain-id <N>] token create custom \
  --name <NAME> --symbol <SYMBOL> --supply <AMOUNT> \
  [--decimals <N>] [--burn-pct <0-50>] [--fee-pct <0-50>] \
  [--team-account <ADDR>] [--dry-run]
```

Create a custom ERC-20 with trade burn / fee ratios. Percent inputs support up
to 2 decimals, for example `0.1` or `1.25`.

### `token create mintable`

```bash
chainpilot [--chain-id <N>] token create mintable \
  --name <NAME> --symbol <SYMBOL> --supply <AMOUNT> \
  [--decimals <N>] [--burn-pct <0-50>] [--fee-pct <0-50>] \
  [--owner <ADDR>] [--dry-run]
```

Create a mintable ERC-20. The owner defaults to the active signer or configured
wallet if not provided.

### `token mint`

```bash
chainpilot [--chain-id <N>] token mint \
  --token <TOKEN_ADDRESS> --to <RECIPIENT> --amount <AMOUNT> [--dry-run]
```

Mint additional supply on a mintable token.

### `token renounce-ownership`

```bash
chainpilot [--chain-id <N>] token renounce-ownership \
  --token <TOKEN_ADDRESS> [--dry-run]
```

Calls `abandonOwnership(address(0))`. This is irreversible on the token.

---

## `wallet` Subcommands

`wallet` commands query a wallet aggregator for cross-chain balance and
portfolio data. ChainPilot tries data sources in this fixed order; the first
one that is configured and succeeds wins:

1. **Debank** (`debank_api_key`) — primary; provides assets, chain breakdown,
   and DeFi protocol positions.
2. **Zerion** (`zerion_api_key`) — second-tier; same coverage as Debank
   including DeFi positions, used when Debank fails or is not configured.
3. **Goldrush / Covalent** (`goldrush_api_key`) — third-tier; provides assets
   with USD prices but **no protocol positions**, so `active_protocols` will
   be empty when this source is used. Goldrush first calls
   `/address/{addr}/activity/` to discover which chains the wallet has
   touched, then issues `balances_v2` only on those chains in parallel.
4. **On-chain RPC** (no key required) — last resort; returns only the native
   token amount for the active chain. `total_balance_usd`,
   `chain_allocation`, etc. stay `null` / empty because there is no USD
   pricing source.

Configure with `chainpilot config set <key> <value>` (see `config list`).
Without any aggregator key, `wallet balance` falls through to on-chain
single-chain native balance and `wallet overview` errors out.

The `sources` field in the JSON response records which provider supplied
each value (`"debank"`, `"zerion"`, `"goldrush"`, `"onchain"`,
`"mempool.space"`, or `null`).

### Non-EVM wallet support

`wallet` commands also accept Solana base58 pubkeys and Bitcoin mainnet
addresses (Bech32 `bc1…`, legacy `1…` / `3…`):

| Command | Solana (SVM) | Bitcoin (BVM) |
|---|---|---|
| `balance` | Debank → Zerion | mempool.space + CoinGecko price |
| `overview` | Debank → Zerion | derived from balance (single asset) |
| `pnl` | Zerion | not supported |
| `history` | Zerion → Debank | mempool.space |
| `labels` | Zerion only (Dune is EVM-only) | not integrated (Breadcrumbs/MetaSleuth/Arkham exist, paid/keyed) |
| `defi` | Debank → Zerion | not supported |

For Solana, **Debank is strongly recommended** — Zerion's Solana support is
indexed asynchronously and may time out for cold/inactive addresses.
Goldrush and the on-chain RPC fallback are EVM-only and skipped for SVM/BVM.
`--chain-id` does not apply to SVM or BVM lookups.

### `--chain-id` semantics

When `--chain-id <N>` is set explicitly (CLI flag or `CHAIN_ID` env var),
**every** field is scoped to that single chain — `assets`,
`chain_allocation`, `total_balance_usd`, `token_allocation`,
`top_holdings`, and `active_protocols`. Without `--chain-id`, the response
aggregates across every chain the wallet uses.

### `wallet balance`

```bash
chainpilot wallet balance <ADDRESS> [--min-usd <USD>]
chainpilot --chain-id 8453 wallet balance <ADDRESS>
```

No API key strictly required — falls through to on-chain native balance if no
aggregator is configured (but USD values will be `null`). Best results with
`debank_api_key` (preferred), `zerion_api_key`, or `goldrush_api_key`.

| Field | Type | Notes |
|---|---|---|
| `wallet` | string | Echo of the input address |
| `total_balance_usd` | number \| null | Sum across the queried chain(s); `null` on the on-chain fallback |
| `assets[]` | array | Per-token holdings (chain, symbol, amount, `price_usd`, `value_usd`) |
| `chain_allocation[]` | array | Per-chain USD totals with percentages |
| `sources` | object | Which provider supplied each field |

`--min-usd <USD>` hides assets worth less than the threshold (default `1.0`).

### `wallet overview`

```bash
chainpilot wallet overview <ADDRESS> [--top <N>]
chainpilot --chain-id 1 wallet overview <ADDRESS>
```

Requires `debank_api_key` (preferred), `zerion_api_key`, or `goldrush_api_key`
(at least one). Errors if none are configured.

| Field | Type | Notes |
|---|---|---|
| `wallet` | string | Echo of the input address |
| `total_balance_usd` | number \| null | Sum across queried chain(s) |
| `chain_allocation[]` | array | Per-chain USD totals with percentages |
| `token_allocation[]` | array | Cross-chain rollup by token symbol with shares |
| `top_holdings[]` | array | Top-N tokens by USD value (default 5; tune with `--top`) |
| `active_protocols[]` | array | DeFi positions grouped by protocol; empty when Goldrush is the only source |
| `sources` | object | Per-field provider |

`active_protocols` from Debank/Zerion includes the protocol name, primary
chain, net USD value, and site URL when available.

### `wallet pnl`

```bash
chainpilot wallet pnl <ADDRESS>
```

Wallet PnL (profit and loss) analysis. Requires `zerion_api_key`.

| Field | Type | Notes |
|---|---|---|
| `wallet` | string | Echo of the input address |
| `realized_pnl` | number \| null | Realized gains/losses (USD) |
| `unrealized_pnl` | number \| null | Unrealized gains/losses (USD) |
| `total_pnl` | number \| null | Sum of realized + unrealized |
| `roi` | number \| null | Return on investment (%) |
| `win_rate` | number \| null | Percentage of profitable positions |
| `total_invested` | number \| null | Total capital deployed (USD) |
| `total_fee` | number \| null | Total fees paid (USD) |
| `source` | string | Always `"zerion"` |

### `wallet history`

```bash
chainpilot wallet history <ADDRESS> [--limit <N>]
```

Transaction history. Requires `zerion_api_key` or `debank_api_key` (at least one;
Zerion is tried first, Debank is the fallback).

| Flag | Default | Notes |
|---|---|---|
| `--limit` | 20 | Max transactions to return (1–100) |

| Field | Type | Notes |
|---|---|---|
| `wallet` | string | Echo of the input address |
| `transactions[]` | array | List of transactions |
| `transactions[].tx_hash` | string | Transaction hash |
| `transactions[].time` | string | Timestamp (RFC 3339) |
| `transactions[].action` | string | Type: send, receive, swap, approve, deposit, withdraw, etc. |
| `transactions[].token_in` | string \| null | Inbound token symbol |
| `transactions[].token_out` | string \| null | Outbound token symbol |
| `transactions[].value_usd` | number \| null | Transaction value (USD) |
| `transactions[].amount` | number \| null | Token amount |
| `transactions[].success` | bool \| null | Whether the tx succeeded |
| `source` | string | `"zerion"` or `"debank"` |

### `wallet labels`

```bash
chainpilot wallet labels <ADDRESS>
```

Wallet behavioral labels and tags. Requires `debank_api_key`, `dune_api_key`, or
`zerion_api_key` (at least one; Debank is tried first, then Dune, then Zerion).

| Field | Type | Notes |
|---|---|---|
| `wallet` | string | Echo of the input address |
| `labels` | string[] | Flat list of label names |
| `label_scores[]` | array | Labels with scores and reasons |
| `label_scores[].label` | string | Label name |
| `label_scores[].score` | number \| null | Confidence score (0–1) |
| `label_scores[].reason` | string \| null | Why this label was assigned |
| `source` | string | `"debank"`, `"dune"`, or `"zerion"` |

Labels can include value-tier tags (whale/dolphin/fish/shrimp), protocol-specific
tags (aave-user, uniswap-trader), behavior tags (defi-user, yield-farmer,
liquidity-provider, staker, lender, borrower), and risk profile tags (degen,
conservative). Available labels depend on the data source.

### `wallet defi`

```bash
chainpilot wallet defi <ADDRESS> [--min-usd <USD>]
chainpilot --chain-id 1 wallet defi <ADDRESS>
```

DeFi positions across protocols — deposits, LPs, staking, borrows, etc.
Requires `debank_api_key` or `zerion_api_key` (at least one; Debank is the
primary source with per-portfolio-item extraction, Zerion is the fallback).

| Flag | Default | Notes |
|---|---|---|
| `--min-usd` | 1.0 | Hide positions worth less than this (USD) |

| Field | Type | Notes |
|---|---|---|
| `wallet` | string | Echo of the input address |
| `total_value_usd` | number \| null | Sum of all DeFi position values |
| `positions[]` | array | Individual DeFi positions |
| `positions[].protocol` | string | Protocol name (e.g. `aave-v3`, `lido`) |
| `positions[].position_name` | string | Position label (e.g. "Aave V3 USDC Deposit") |
| `positions[].chain` | string | Chain slug (e.g. `eth`, `base`) |
| `positions[].value_usd` | number \| null | Position value in USD |
| `positions[].tokens[]` | array | Tokens held in this position |
| `positions[].tokens[].symbol` | string | Token symbol |
| `positions[].tokens[].amount` | number \| null | Token amount |
| `positions[].position_type` | string | Type: deposit, borrow, stake, liquidity, yield, vault, position |
| `positions[].site_url` | string \| null | Protocol website |
| `source` | string | `"debank"` or `"zerion"` |

`--chain-id` scopes results to a single chain.

---

## `risk` Subcommands

### Non-EVM risk support

| Command | Solana | Bitcoin |
|---|---|---|
| `risk token` | GoPlus Solana token_security | not supported |
| `risk wallet` | GoPlus address-reputation flags (sanctions, phishing, drainer, mixer, …) | not supported |
| `risk approval` | not supported (SPL uses delegate accounts) | not supported (no approval primitive) |

`risk approval` is rejected when **either** owner or spender is a non-EVM
address; the message names which side triggered the rejection.

### `risk token`

```bash
chainpilot [--chain-id <N>] risk token <TOKEN>
```

Token risk: honeypot detection, ownership, liquidity flags.

### `risk wallet`

```bash
chainpilot risk wallet <ADDRESS>
```

Wallet risk: exposure summary, high-risk approvals. On EVM it combines a
native-balance heuristic with GoPlus's malicious-address reputation, taking
the more severe of the two as the overall level. On Solana (base58 address)
it relies on the same chain-agnostic GoPlus library. Either way it emits one
signal per flagged category (sanctions, phishing, stealing/drainer, mixer,
etc.); when GoPlus has no record, EVM falls back to the balance level and SVM
returns `LOW` with an explanatory note. Note GoPlus's reputation coverage is
best-effort — a `LOW`/clean result means "not flagged", not "proven safe"
(e.g. some OFAC-sanctioned contracts are not marked `sanctioned`).

### `risk approval`

```bash
chainpilot [--chain-id <N>] risk approval <ADDRESS> --token <TOKEN> --spender <SPENDER>
```

Single approval state.

---

## `protocol` Subcommands

Protocol-level analytics from DefiLlama. The `<PROTOCOL>` argument is a DefiLlama
slug or protocol name (e.g. `lido`, `aave`, `uniswap`). No API key required.

### `protocol info`

```bash
chainpilot protocol info <PROTOCOL>
```

Overview: name, category, primary chain, website, description, current TVL, 24h
revenue, and 24h fees.

### `protocol tvl`

```bash
chainpilot protocol tvl <PROTOCOL> [--limit <N>] [--offset <N>]
```

Current TVL, 24h/7d/30d TVL change, and a TVL history series. `--limit` defaults
to 7 (max 1000) newest points; `--offset` skips that many newest points first.

### `protocol revenue`

```bash
chainpilot protocol revenue <PROTOCOL>
```

Revenue (24h/7d/30d) and fees (24h/7d).

### `protocol chains`

```bash
chainpilot protocol chains <PROTOCOL>
```

The protocol's TVL distribution across the chains it's deployed on.

---

## `chain` Subcommands

Chain-level analytics. The `<CHAIN>` argument accepts a name, chain ID, or alias
(e.g. `ethereum` / `1` / `eth`, `bsc` / `bnb`, `base`, `arbitrum`). Data comes
from free public sources (DefiLlama, CoinGecko, growthepie) — no API key required.
Every field is rendered with a `Source` column (or `sources` object in `--json`)
naming its data origin; fields with no available source show `N/A`.

**Non-EVM coverage**: The `chain` subcommands also work for non-EVM chains via
DefiLlama. Use `solana` / `sol` / `svm` for Solana and `bitcoin` / `btc` / `bvm`
for Bitcoin mainnet. Only the `chain` subtree supports these — every other
subcommand (`swap`, `token`, `wallet`, `risk`, `protocol info`) assumes an EVM
address space and will reject Solana / Bitcoin addresses. growthepie activity
metrics (active addresses, tx count, throughput) are EVM-only and surface as
`N/A` on Solana / Bitcoin.

### `chain info`

```bash
chainpilot chain info <CHAIN>
```

Overview: chain ID, native token and USD price, TVL, 24h fees, and 24h activity
(active addresses, tx count, throughput). Activity fields come from growthepie and
are only available for the Ethereum-ecosystem chains it tracks (L1 + L2s such as
Base, Arbitrum, Optimism, Polygon, Linea, Scroll). Independent L1s like BNB Chain
and Avalanche show `N/A` for active addresses / tx count / throughput.

### `chain flows`

```bash
chainpilot chain flows <CHAIN>
```

Stablecoin-based fund flow: net flow, inflow, outflow, and a per-stablecoin 24h
breakdown (mint vs burn). Scope is stablecoins only — not total cross-chain flow;
bridge and CEX flows have no free data source and are not reported.

### `chain stablecoins`

```bash
chainpilot chain stablecoins <CHAIN>
```

Stablecoin supply on the chain, 24h supply change, and a per-coin breakdown with
share percentages.

### `chain protocols`

```bash
chainpilot chain protocols <CHAIN> [--limit <N>]
```

Top on-chain DeFi protocols on the chain by chain-specific TVL, with 24h revenue
and category. `--limit` defaults to 20 (max 100). TVL is the protocol's TVL on
this chain specifically (no fallback to its global TVL).

The list is filtered to genuine on-chain protocols:

- Non-protocol categories are excluded — `CEX`, `Bridge`, `Canonical Bridge`,
  `Cross Chain Bridge`, and `Chain`. These report a per-chain TVL in DefiLlama
  (e.g. Binance CEX has an Ethereum TVL) and would otherwise dominate the list.
- Protocols with no per-chain TVL for this chain are dropped, so every row shows
  a real TVL figure.

---

## Token Resolution

Tokens can be a symbol, native token symbol, or a `0x` address. Resolution order:

1. Native token symbol (`ETH`, `BNB`, etc.)
2. Raw `0x` address — decimals fetched on-chain
3. DODO tokenlist cache (1-hour TTL)
4. Local custom token store (`token add` and successful address-based quotes)

For `token info`, `token price`, `token liquidity`, and `token risk`, unresolved
symbols can return external-source candidates. Treat those as suggestions only;
confirm with the user before using any candidate address in a swap or approval.

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

**Non-EVM analytics**: `chain` subcommands (and `protocol info/tvl/revenue/chains`)
additionally accept `solana` / `svm` and `bitcoin` / `bvm` — read-only via
DefiLlama. No `--chain-id` value applies for these; the chain name is the
argument to the `chain` subcommand itself.

---

## `config` Subcommands

Manage API keys and configuration values. Settings are persisted in a config file
(`~/.local/share/chain/config.env` on Linux). Existing environment variables and
`.env` values take precedence over this persisted file at runtime.

### `config set`

```bash
chainpilot config set <KEY> <VALUE>
```

Save an API key or configuration value. The value is written to the persistent config file
and immediately available to the current process. On Unix, the config file is
written with owner-only permissions (`0600`).

### `config get`

```bash
chainpilot config get <KEY>
```

Show the current value of a configuration key. Sensitive values are partially masked.

### `config list`

```bash
chainpilot config list
```

Show all configurable keys with their current values (sensitive values masked).

### `config unset`

```bash
chainpilot config unset <KEY>
```

Remove a configuration key from the config file.

### Configurable Keys

| Key | Env Var | Sensitive | Description |
|---|---|---|---|
| `dodo_api_key` | `DODO_API_KEY` | Yes | DODO API key for swap routing |
| `dodo_project_id` | `DODO_PROJECT_ID` | No | DODO project ID for tokenlist API |
| `coingecko_api_key` | `COINGECKO_API_KEY` | Yes | CoinGecko API key for price data |
| `debank_api_key` | `DEBANK_API_KEY` | Yes | Debank Pro OpenAPI key — primary source for `wallet balance` / `wallet overview` |
| `zerion_api_key` | `ZERION_API_KEY` | Yes | Zerion API key — second-tier wallet aggregator |
| `goldrush_api_key` | `GOLDRUSH_API_KEY` | Yes | Goldrush / Covalent API key — third-tier wallet aggregator |
| `dune_api_key` | `DUNE_API_KEY` | Yes | Dune Analytics API key — wallet labels fallback |

Only these keys are supported by `chainpilot config` today. Other runtime
settings, such as `COINGECKO_API_URL`, `DEXSCREENER_API_URL`,
`DEBANK_API_URL`, `ZERION_API_URL`, `GOLDRUSH_API_URL`, and `DUNE_API_URL`,
can still be provided via environment variables or `.env`.

**Runtime config precedence**: CLI flag > existing environment variable / `.env`
file > `config.env` file > compile-time default.

---

## Scripting Patterns

```bash
# Check if simulate passed
chainpilot --json swap simulate --quote-id "$QUOTE_ID" \
  --wallet 0xAddr | jq '.data.ok'

# Full quote payload
chainpilot --json swap quote --from ETH --to USDC --amount 1.0 | jq .

# Quiet execution with a keystore signer — only care about the exit code
chainpilot --quiet --keystore-path /path/to/keystore.json swap execute --quote-id "$QUOTE_ID" --wait
echo $?  # 0 = success
```

Enable debug logging:

```bash
RUST_LOG=debug chainpilot swap quote --from ETH --to USDC --amount 1
```
