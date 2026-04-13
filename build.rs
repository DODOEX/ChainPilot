/// Reads `.env` at build time and forwards selected keys to the compiler via
/// `cargo:rustc-env`, making them available to `option_env!()`.
///
/// Priority at compile time:  shell env > .env file
/// Priority at runtime:       --arg > runtime env var > compiled-in default
fn main() {
    // Re-run this script whenever .env changes.
    println!("cargo:rerun-if-changed=.env");

    // Keys that should be baked into the binary as compile-time defaults.
    const COMPILE_TIME_KEYS: &[&str] = &["DODO_API_KEY", "DODO_PROJECT_ID"];

    // Tell cargo to re-run if any of these change in the shell environment too.
    for key in COMPILE_TIME_KEYS {
        println!("cargo:rerun-if-env-changed={}", key);
    }

    // If the key is already set in the shell environment, cargo:rustc-env is
    // not needed — option_env! will pick it up directly. Only emit it when
    // the shell doesn't have it but .env does.
    let missing_from_shell: Vec<&str> = COMPILE_TIME_KEYS
        .iter()
        .copied()
        .filter(|k| std::env::var(k).is_err())
        .collect();

    if missing_from_shell.is_empty() {
        return;
    }

    let Ok(content) = std::fs::read_to_string(".env") else {
        return;
    };

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !missing_from_shell.contains(&key) {
            continue;
        }
        // Strip optional surrounding quotes from the value.
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        println!("cargo:rustc-env={}={}", key, value);
    }
}
