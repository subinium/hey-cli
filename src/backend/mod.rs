pub(crate) mod claude;
pub(crate) mod codex;
pub(crate) mod openrouter;

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::env;
use std::sync::OnceLock;

use crate::cli::Backend;
use crate::sanitize::sanitize_command;
use crate::style::*;

/// Cached which_bin results. OnceLock ensures the HashMap is initialized once;
/// individual entries are populated on first lookup for each binary name.
/// We use a simple approach: scan once at startup for known binaries.
static BIN_CACHE: OnceLock<HashMap<String, bool>> = OnceLock::new();

/// Checks whether a binary exists on PATH, with results cached for the process lifetime.
pub(crate) fn which_bin(bin: &str) -> bool {
    let cache = BIN_CACHE.get_or_init(HashMap::new);
    // Fast path: already cached
    if let Some(&found) = cache.get(bin) {
        return found;
    }
    // Slow path: scan PATH. We can't mutate the OnceLock HashMap, so we do the
    // scan each time for uncached entries. In practice the set of queried binaries
    // is small and fixed (eza, bat, jq, delta, claude, codex, wl-copy, xclip),
    // so this is fine. For true single-scan caching we'd need a Mutex, which is
    // overkill for a CLI that runs once.
    scan_path(bin)
}

fn scan_path(bin: &str) -> bool {
    if let Ok(path) = env::var("PATH") {
        for dir in path.split(':') {
            let p = std::path::Path::new(dir).join(bin);
            if p.is_file() {
                return true;
            }
        }
    }
    false
}

/// Pre-populate the which_bin cache at startup for all binaries we ever query.
/// Call this once from main before any other logic.
pub(crate) fn init_bin_cache() {
    let bins = ["eza", "bat", "jq", "delta", "claude", "codex", "wl-copy", "xclip"];
    let mut map = HashMap::with_capacity(bins.len());
    if let Ok(path) = env::var("PATH") {
        let dirs: Vec<&str> = path.split(':').collect();
        for &bin in &bins {
            let found = dirs.iter().any(|dir| {
                std::path::Path::new(dir).join(bin).is_file()
            });
            map.insert(bin.to_string(), found);
        }
    } else {
        for &bin in &bins {
            map.insert(bin.to_string(), false);
        }
    }
    let _ = BIN_CACHE.set(map);
}

/// (icon, ansi_color, display_name, voice_line)
pub(crate) fn backend_persona(backend: Backend) -> (&'static str, &'static str, &'static str, &'static str) {
    match backend {
        Backend::Claude => ("✱", "\x1b[38;5;215m", "claude", "here you go"),
        Backend::Codex => ("☁", "\x1b[38;5;111m", "codex", "computed"),
        Backend::Openrouter => ("◆", "\x1b[38;5;222m", "openrouter", "cooked"),
        Backend::Auto => ("◌", GRAY, "auto", ""),
    }
}

pub(crate) fn backend_label(backend: Backend, model: &str) -> String {
    let (icon, color, name, _) = backend_persona(backend);
    let tail = match backend {
        Backend::Openrouter => {
            let short = model.rsplit('/').next().unwrap_or(model);
            format!(" {GRAY}·{RESET} {DIM}{short}{RESET}")
        }
        Backend::Claude => format!(" {GRAY}·{RESET} {DIM}headless{RESET}"),
        Backend::Codex => format!(" {GRAY}·{RESET} {DIM}exec{RESET}"),
        Backend::Auto => String::new(),
    };
    format!("{color}{icon}{RESET} {BOLD_WHITE_BG}{name}{RESET}{tail}")
}

/// Returns the full ordered list of backends to try in Auto mode, filtered by
/// local availability. Claude first (subscription inline), then Codex, then
/// OpenRouter (HTTP fallback).
pub(crate) fn resolve_auto_chain() -> Result<Vec<Backend>> {
    let mut chain = Vec::new();
    if which_bin("claude") {
        chain.push(Backend::Claude);
    }
    if which_bin("codex") {
        chain.push(Backend::Codex);
    }
    if env::var("OPENROUTER_API_KEY").is_ok() {
        chain.push(Backend::Openrouter);
    }
    if chain.is_empty() {
        return Err(anyhow!(
            "no backend available: install `claude` or `codex`, or set OPENROUTER_API_KEY"
        ));
    }
    Ok(chain)
}

pub(crate) fn post_cli_backend_output(name: &str, output: std::process::Output) -> Result<String> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "{name} exited with {}: {}",
            output.status,
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(sanitize_command(&stdout))
}
