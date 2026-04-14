---
name: cast-wallet
description: >
  Manage Ethereum/EVM wallets and interact with smart contracts using the
  Foundry `cast` CLI — covering wallet creation, vanity addresses, keystore
  import, message signing, ETH transfers, contract calls (cast call / cast send),
  gas estimation, and on-chain queries. Use this skill whenever the user asks
  about wallets, private keys, signing, ERC-20 operations, or sending any
  transaction from the command line.
---

# cast wallet — Web3 Wallet & Contract Interaction

`cast` is Foundry's CLI tool covering wallet management, contract interaction, and on-chain queries.

## Installation

```bash
curl -L https://foundry.paradigm.xyz | bash
foundryup
cast --version   # verify
```

---

## Commands at a Glance

| Command | What it does |
|---|---|
| `cast wallet new` | Generate a fresh random keypair |
| `cast wallet address` | Derive the address from a private key |
| `cast wallet vanity` | Mine a keypair whose address matches a pattern |
| `cast wallet import` | Encrypt and save a private key to a keystore file |
| `cast wallet list` | Show all keystores in the default directory |
| `cast wallet sign` | Sign a message with a private key or keystore |
| `cast wallet verify` | Verify a message signature |
| `cast balance` | Query the ETH balance of an address |
| `cast call` | Read-only contract call (no gas, not on-chain) |
| `cast send` | Send a state-changing transaction on-chain |
| `cast estimate` | Estimate the gas cost of a transaction |
| `cast receipt` | Fetch a transaction receipt by hash |

---

## Creating a Wallet

```bash
cast wallet new
```

Example output:
```
Successfully created new keypair.
Address:     0xAbCd...1234
Private key: 0xdeadbeef...
```

> The private key is printed once and never stored. Save it immediately, then use `cast wallet import` to encrypt it into a keystore.

---

## Derive Address from Private Key

```bash
cast wallet address --private-key 0xYOUR_PRIVATE_KEY
```

Useful for confirming which address a key controls before importing it.

---

## Vanity Address Generation

Mine an address that starts or ends with a specific hex pattern:

```bash
# Address starting with "dead"
cast wallet vanity --starts-with dead

# Address ending with "beef"
cast wallet vanity --ends-with beef

# Both prefix and suffix
cast wallet vanity --starts-with 00 --ends-with ff
```

> Longer patterns take exponentially more time: 4 chars ~minutes, 6 chars ~hours.

To generate a vanity **contract** address (the address the keypair would deploy to at a given nonce):

```bash
cast wallet vanity --starts-with dead --nonce 0
```

---

## Keystore Management

Keystores are JSON files that store a private key encrypted with a password.
Default location: `~/.foundry/keystores/`

### Import a private key

```bash
# Interactive (prompts for password)
cast wallet import my-account --private-key 0xYOUR_PRIVATE_KEY

# Non-interactive (for scripts)
cast wallet import my-account \
  --private-key 0xYOUR_PRIVATE_KEY \
  --password-file /path/to/password.txt
```

> Never use `--password <plaintext>` — it appears in shell history.

### List stored accounts

```bash
cast wallet list
```

### Use a keystore account in other commands

Pass `--account <name>` to any `cast` command that needs a signer:

```bash
cast send 0xCONTRACT "mint(address)" 0xRECIPIENT \
  --account my-account \
  --rpc-url $RPC_URL
```

---

## Signing & Verifying Messages

```bash
# Sign with a keystore account (recommended)
cast wallet sign --account my-account "Hello, world"

# Sign with a raw private key
cast wallet sign --private-key 0xKEY "Hello, world"

# Sign arbitrary hex bytes
cast wallet sign --private-key 0xKEY 0xDEADBEEF

# Skip EIP-191 prefix (raw hash signing)
cast wallet sign --no-hash --private-key 0xKEY "raw message"
```

Output is the 65-byte signature in hex (`r`, `s`, `v`).

```bash
# Verify
cast wallet verify \
  --address 0xSIGNER_ADDRESS \
  "Hello, world" \
  0xSIGNATURE_HEX
```

---

## Querying On-Chain State (read-only)

### ETH balance

```bash
cast balance 0xWALLET_ADDRESS --rpc-url $RPC_URL

# Display in ether
cast balance 0xWALLET_ADDRESS --ether --rpc-url $RPC_URL
```

### Read-only contract call (`cast call`)

No gas consumed, does not change state:

```bash
# ERC-20 balance
cast call 0xTOKEN "balanceOf(address)(uint256)" \
  0xWALLET --rpc-url $RPC_URL

# Token name
cast call 0xTOKEN "name()(string)" --rpc-url $RPC_URL

# Contract owner
cast call 0xCONTRACT "owner()(address)" --rpc-url $RPC_URL

# Allowance
cast call 0xTOKEN "allowance(address,address)(uint256)" \
  0xOWNER 0xSPENDER --rpc-url $RPC_URL
```

Return values are automatically decoded per the output types in the signature.

### Transaction receipt

```bash
cast receipt 0xTX_HASH --rpc-url $RPC_URL
```

---

## Sending Transactions (`cast send`)

`cast send` submits a real transaction — costs gas, changes state.

### Send ETH

```bash
cast send 0xRECIPIENT \
  --value 0.1ether \
  --account my-account \
  --rpc-url $RPC_URL
```

### Call a contract method

```bash
# General form
cast send <contract> "<sig>" <args...> \
  --account <keystore-name> \
  --rpc-url <rpc>

# ERC-20 transfer (USDC, 6 decimals — 100 USDC)
cast send 0xUSDC \
  "transfer(address,uint256)" \
  0xRECIPIENT 100000000 \
  --account my-account \
  --rpc-url $RPC_URL

# ERC-20 unlimited approve (uint256 max value)
cast send 0xTOKEN \
  "approve(address,uint256)" \
  0xDEX \
  115792089237316195423570985008687907853269984665640564039457584007913129639935 \
  --account my-account \
  --rpc-url $RPC_URL

# Payable function (attach ETH)
cast send 0xCONTRACT "deposit()" \
  --value 0.1ether \
  --account my-account \
  --rpc-url $RPC_URL

# No-arg function
cast send 0xCONTRACT "harvest()" \
  --account my-account \
  --rpc-url $RPC_URL
```

### Specify gas

```bash
cast send 0xCONTRACT "execute(bytes)" 0xDATA \
  --gas-limit 300000 \
  --gas-price 20gwei \
  --account my-account \
  --rpc-url $RPC_URL
```

> By default `cast send` waits for the transaction to be mined and prints the receipt. Use `--async` to submit without waiting.

### Estimate gas before sending

```bash
cast estimate 0xCONTRACT \
  "transfer(address,uint256)" \
  0xRECIPIENT 1000000 \
  --rpc-url $RPC_URL
```

---

## Tips

### Skip `--rpc-url` with an environment variable

```bash
export ETH_RPC_URL=https://mainnet.infura.io/v3/YOUR_KEY
# All cast commands now pick it up automatically
cast balance 0xADDRESS
cast call 0xTOKEN "name()(string)"
```

### Signer security comparison

| Method | Flag | Security |
|---|---|---|
| Raw private key | `--private-key 0x...` | Lowest — tests only |
| Env var | `ETH_PRIVATE_KEY=0x...` | Low — plaintext in memory |
| Keystore | `--account my-account` | Recommended |
| Keystore + password file | `--account` + `--password-file` | Recommended for scripts |

---

## Security Tips

- **Never commit private keys.** Add `.env` to `.gitignore`, or use keystores.
- **Password files should be `chmod 600`** and stored outside the project directory.
- **Use separate keys per network.** Keep a throwaway key for testnets; never reuse mainnet keys for development.
- **Verify the derived address** with `cast wallet address` before sending funds to a newly created key.

---

## Integration with this project (chainpilot)

chainpilot currently reads the signer from `PRIVATE_KEY` in `.env` (plaintext). To migrate to a keystore:

```bash
# 1. Import the key
cast wallet import chainpilot-key --private-key $PRIVATE_KEY

# 2. Remove PRIVATE_KEY from .env

# 3. Pass --account chainpilot-key to any cast send call
```
