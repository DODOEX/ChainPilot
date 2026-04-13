# AGENT.md

## Non-Obvious Conventions

**All output must go through `print_output`**: Every command handler uses `crate::output::print_output::<T>(result, "command.subcommand", output_mode)`. Direct `println!` is forbidden — it breaks `--json` mode.

**Token resolution order** (`resolve_token` in `src/commands/mod.rs`): native token symbol → `0x` address passed through directly (decimals fetched on-chain) → DODO tokenlist cache (1-hour TTL) → on-chain ERC-20 fallback.

**Quote dual TTL**: The quote returned by DODO carries its own 20-minute deadline, but `chainpilot` also enforces a local expiry via `QUOTE_TTL_SECS` (default 5 minutes). Both `simulate` and `execute` reject expired quotes.

**Config precedence**: CLI flag > runtime env var > `.env` file > compile-time `option_env!()` > hardcoded default.
