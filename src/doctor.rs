//! `hey doctor` — environment diagnostics. Prints a colored report of detected
//! backends, pretty-preset tools, shell / TTY state, and config env-vars.
//!
//! No network calls. Pure file-system and env-var reads.

use std::env;
use std::io::IsTerminal;
use std::path::PathBuf;

use crate::backend::{backend_persona, resolve_auto_chain};
use crate::cli::Backend;
use crate::style::*;

/// Runs the doctor report and returns Ok(()). Exits with 0 on success — the
/// report itself never fails, missing tools are just reported as missing.
pub(crate) fn run() -> anyhow::Result<()> {
    println!();
    println!("  {BOLD_WHITE_BG}hey doctor{RESET}");
    println!();

    // Backends
    println!("  {BOLD_WHITE}backends:{RESET}");
    print_backend_line(Backend::Claude, find_on_path("claude"), claude_auth_hint());
    print_backend_line(Backend::Codex, find_on_path("codex"), codex_auth_hint());
    print_openrouter_line();
    println!();

    // Tools for pretty presets
    println!("  {BOLD_WHITE}tools for pretty presets:{RESET}");
    print_tool_line("eza", "brew install eza");
    print_tool_line("bat", "brew install bat");
    print_tool_line("jq", "brew install jq");
    print_tool_line("delta", "brew install git-delta");
    print_tool_line("fd", "brew install fd");
    println!();

    // Shell / TTY
    println!("  {BOLD_WHITE}shell:{RESET}");
    let shell = env::var("SHELL").unwrap_or_else(|_| "(unset)".into());
    let term = env::var("TERM").unwrap_or_else(|_| "(unset)".into());
    let stdin_tty = std::io::stdin().is_terminal();
    let stdout_tty = std::io::stdout().is_terminal();
    let stderr_tty = std::io::stderr().is_terminal();
    println!("    SHELL={shell}");
    println!(
        "    TERM={term} {DIM}(is_stdin_tty: {}, is_stdout_tty: {}, is_stderr_tty: {}){RESET}",
        yes_no(stdin_tty),
        yes_no(stdout_tty),
        yes_no(stderr_tty),
    );
    println!();

    // Config
    println!("  {BOLD_WHITE}config:{RESET}");
    print_env_line("AIT_BACKEND", "auto");
    print_env_line("AIT_MODEL", crate::backend::openrouter::DEFAULT_MODEL);
    match env::var("ANTHROPIC_API_KEY") {
        Ok(v) if !v.is_empty() => {
            println!(
                "    ANTHROPIC_API_KEY={DIM}set ({}){RESET} {DIM_GRAY}— claude uses direct API{RESET}",
                mask_key(&v)
            );
        }
        _ => {
            println!(
                "    ANTHROPIC_API_KEY={DIM}(unset){RESET} {DIM_GRAY}— claude uses subprocess path{RESET}"
            );
        }
    }
    println!();

    // Active chain
    match resolve_auto_chain() {
        Ok(chain) => {
            let parts: Vec<String> = chain
                .iter()
                .map(|&b| {
                    let (_, color, name, _) = backend_persona(b);
                    format!("{color}{name}{RESET}")
                })
                .collect();
            println!(
                "  {BOLD_WHITE}active chain{RESET} {DIM}(auto){RESET}: {}",
                parts.join(&format!(" {GRAY}\u{2192}{RESET} "))
            );
        }
        Err(e) => {
            println!("  {BOLD_RED}active chain:{RESET} {DIM_ITALIC}{e}{RESET}");
        }
    }
    println!();
    Ok(())
}

fn find_on_path(bin: &str) -> Option<PathBuf> {
    let path = env::var("PATH").ok()?;
    for dir in path.split(':') {
        let p = std::path::Path::new(dir).join(bin);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn print_backend_line(backend: Backend, path: Option<PathBuf>, hint: String) {
    let (icon, color, name, _) = backend_persona(backend);
    match path {
        Some(p) => {
            println!(
                "    {color}{icon}{RESET} {BOLD_WHITE}{name:<10}{RESET} {DIM}{}{RESET}{hint}",
                p.display()
            );
        }
        None => {
            println!(
                "    {DIM}{icon}{RESET} {DIM}{name:<10}{RESET} {DIM_ITALIC}(not found){RESET}{hint}"
            );
        }
    }
}

fn print_openrouter_line() {
    let (icon, color, name, _) = backend_persona(Backend::Openrouter);
    match env::var("OPENROUTER_API_KEY") {
        Ok(v) if !v.is_empty() => {
            println!(
                "    {color}{icon}{RESET} {BOLD_WHITE}{name:<10}{RESET} OPENROUTER_API_KEY {DIM}set ({}){RESET}",
                mask_key(&v)
            );
        }
        _ => {
            println!(
                "    {DIM}{icon}{RESET} {DIM}{name:<10}{RESET} {DIM_ITALIC}OPENROUTER_API_KEY not set{RESET}"
            );
        }
    }
}

fn claude_auth_hint() -> String {
    if env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())
        .is_some()
    {
        format!(" {DIM_GRAY}(ANTHROPIC_API_KEY set — direct API){RESET}")
    } else if find_on_path("claude").is_some() {
        format!(" {DIM_GRAY}(subprocess — run `claude login` if calls fail){RESET}")
    } else {
        String::new()
    }
}

fn codex_auth_hint() -> String {
    if find_on_path("codex").is_some() {
        format!(" {DIM_GRAY}(run `codex login` if calls fail){RESET}")
    } else {
        String::new()
    }
}

fn print_tool_line(bin: &str, install_hint: &str) {
    match find_on_path(bin) {
        Some(p) => {
            println!(
                "    {BOLD_GREEN}\u{2713}{RESET} {BOLD_WHITE}{bin:<6}{RESET} {DIM}{}{RESET}",
                p.display()
            );
        }
        None => {
            println!(
                "    {DIM}\u{2715}{RESET} {DIM}{bin:<6}{RESET} {DIM_ITALIC}(not found \u{2014} install with `{install_hint}`){RESET}"
            );
        }
    }
}

fn print_env_line(name: &str, default: &str) {
    match env::var(name) {
        Ok(v) if !v.is_empty() => {
            println!("    {name}={BOLD_WHITE}{v}{RESET}");
        }
        _ => {
            println!("    {name}={DIM}(unset, {default}){RESET}");
        }
    }
}

fn mask_key(key: &str) -> String {
    // Show only the publicly-documented prefix (up to the "v1-" part) so no
    // entropy of the actual key body leaks into the diagnostic output.
    let public_prefixes = ["sk-or-v1-", "sk-or-", "sk-ant-", "sk-proj-", "sk-"];
    for p in public_prefixes {
        if key.starts_with(p) {
            return format!("{p}****");
        }
    }
    "****".to_string()
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}
