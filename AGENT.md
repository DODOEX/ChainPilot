# AGENT.md

## Skills

The `skills/` directory in the project root contains skills for this project. Each subdirectory is a skill with a `SKILL.md` describing its usage (e.g. `skills/chainpilot/SKILL.md`).

Currently available skills:
- `skills/chainpilot/` — ChainPilot CLI usage and DeFi operation patterns
- `skills/evm-wallet/` — EVM wallet management and contract interaction via Foundry cast

## Non-Obvious Conventions

**All output must go through `print_output`**: Every command handler uses `crate::output::print_output::<T>(result, "command.subcommand", output_mode)`. Direct `println!` is forbidden — it breaks `--json` mode.

**Token resolution order** (`resolve_token` in `src/commands/mod.rs`): native token symbol → `0x` address passed through directly (decimals fetched on-chain) → DODO tokenlist cache (1-hour TTL) → on-chain ERC-20 fallback.

**Quote dual TTL**: The quote returned by DODO carries its own 20-minute deadline, but `chainpilot` also enforces a local expiry via `QUOTE_TTL_SECS` (default 18 minutes). Both `simulate` and `execute` reject expired quotes.

**Config precedence**: CLI flag > runtime env var > `.env` file > compile-time `option_env!()` > hardcoded default.
