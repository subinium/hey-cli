mod backend;
mod cli;
mod preset;
mod prompt;
mod risk;
mod sanitize;
mod style;
mod ui;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::env;
use std::process::{Command, Stdio};

use backend::claude::ask_claude;
use backend::codex::ask_codex;
use backend::openrouter::{ask_openrouter, DEFAULT_MODEL};
use backend::{backend_persona, resolve_auto_chain};
use cli::{Backend, Cli};
use preset::apply_presets;
use prompt::build_user_prompt;
use risk::{assess_risk, check_sensitive, Risk};
use sanitize::{looks_like_command, split_command_and_explanation};
use style::*;
use ui::{clear_thinking, confirm, copy_to_clipboard, print_command_block, print_thinking, Decision};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    backend::init_bin_cache();
    if let Err(err) = run().await {
        eprintln!("{RED}hey:{RESET} {err:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Subcommand-style backend selection: `hey claude ...`, `hey codex ...`, `hey openrouter ...`
    let mut prompt_words = cli.prompt.clone();
    let inline_backend = match prompt_words.first().map(|s| s.to_lowercase()) {
        Some(ref w) if w == "claude" => Some(Backend::Claude),
        Some(ref w) if w == "codex" => Some(Backend::Codex),
        Some(ref w) if w == "openrouter" || w == "or" => Some(Backend::Openrouter),
        Some(ref w) if w == "auto" => Some(Backend::Auto),
        _ => None,
    };
    if inline_backend.is_some() {
        prompt_words.remove(0);
    }

    let request = prompt_words.join(" ");
    if request.trim().is_empty() {
        return Err(anyhow!(
            "empty prompt — try `hey find files bigger than 100mb` or `hey claude explain this regex`"
        ));
    }

    // Content filter: block prompts that contain API keys or private key material
    // from being sent to a remote model.
    if !cli.allow_sensitive {
        if let Some(label) = check_sensitive(&request) {
            return Err(anyhow!(
                "prompt contains a sensitive value ({label}).\n\
                 Remove it from the prompt, or pass --allow-sensitive to override."
            ));
        }
    }

    let explicit = if cli.claude {
        Some(Backend::Claude)
    } else if cli.codex {
        Some(Backend::Codex)
    } else if let Some(b) = inline_backend {
        if matches!(b, Backend::Auto) {
            None
        } else {
            Some(b)
        }
    } else if !matches!(cli.backend, Backend::Auto) {
        Some(cli.backend)
    } else {
        None
    };

    // Build the chain: explicit backend = single entry, auto = full available chain.
    let chain: Vec<Backend> = match explicit {
        Some(b) => vec![b],
        None => resolve_auto_chain()?,
    };

    let user_prompt = build_user_prompt(&request, cli.explain);
    let model = cli
        .model
        .clone()
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());

    // Try each backend in the chain. On error in auto-mode, surface a short
    // fallback notice and try the next one. Any explicit backend just errors.
    let mut raw_result: Option<String> = None;
    let mut last_err: Option<anyhow::Error> = None;
    let mut used_backend = chain[0];

    for (i, candidate) in chain.iter().copied().enumerate() {
        print_thinking(candidate, &model);
        let attempt = match candidate {
            Backend::Auto => unreachable!("auto resolved above"),
            Backend::Openrouter => {
                let api_key = env::var("OPENROUTER_API_KEY")
                    .context("OPENROUTER_API_KEY not set. Export it in your shell rc.");
                match api_key {
                    Ok(key) => ask_openrouter(&key, &model, &user_prompt).await,
                    Err(e) => Err(e),
                }
            }
            Backend::Claude => ask_claude(&user_prompt).await,
            Backend::Codex => ask_codex(&user_prompt),
        };
        clear_thinking();

        match attempt {
            Ok(r) => {
                used_backend = candidate;
                raw_result = Some(r);
                break;
            }
            Err(e) => {
                let is_last = i == chain.len() - 1;
                if !is_last {
                    let (_, color, name, _) = backend_persona(candidate);
                    let (_, next_color, next_name, _) = backend_persona(chain[i + 1]);
                    eprintln!(
                        "  {GRAY}╭─{RESET} {DIM}{RED}⚠{RESET}  {color}{name}{RESET} {DIM}failed: {}{RESET}",
                        e
                    );
                    eprintln!(
                        "  {GRAY}│{RESET}  {DIM}falling back to{RESET} {next_color}{next_name}{RESET}…"
                    );
                }
                last_err = Some(e);
            }
        }
    }

    let raw =
        raw_result.ok_or_else(|| last_err.unwrap_or_else(|| anyhow!("all backends failed")))?;
    let backend = used_backend;

    let (mut command, explanation) = split_command_and_explanation(&raw);
    if command.is_empty() {
        return Err(anyhow!("no command returned"));
    }
    let first_line = command.lines().next().unwrap_or("");
    if !looks_like_command(first_line) {
        return Err(anyhow!(
            "backend returned prose instead of a command:\n\n{}\n\nTry a different backend with `hey claude ...` / `hey codex ...` / `hey openrouter ...`.",
            command
        ));
    }
    if !cli.raw {
        command = apply_presets(&command);
    }

    let (risk, risk_note) = assess_risk(&command);
    let blocked = matches!(risk, Risk::Block);
    let closed = cli.dry_run || cli.yes || blocked;
    print_command_block(
        &command,
        explanation.as_deref(),
        backend,
        &model,
        risk,
        risk_note,
        closed,
    );

    if cli.dry_run {
        return Ok(());
    }
    if blocked {
        // Destructive commands: print + copy to clipboard (macOS), never auto-execute.
        copy_to_clipboard(&command);
        println!("  {DIM_GRAY}copied to clipboard · paste & run manually{RESET}");
        println!();
        return Ok(());
    }

    let to_run = if cli.yes {
        command
    } else {
        match confirm(&command)? {
            Decision::Run(c) => c,
            Decision::Abort => {
                println!("  {GRAY}╰─{RESET} {GRAY}aborted{RESET}");
                println!();
                return Ok(());
            }
        }
    };

    let code = run_command(&to_run)?;
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

fn run_command(command: &str) -> Result<i32> {
    // Use the user's shell so aliases/functions resolve, but fall back to bash.
    // We deliberately DO NOT re-apply presets here — `run()` already prettified
    // once, and re-applying would break `--raw` and user edits.
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
    let status = Command::new(&shell)
        .arg("-c")
        .arg(command)
        .env("CLICOLOR", "1")
        .env("CLICOLOR_FORCE", "1")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to spawn {shell}"))?;
    Ok(status.code().unwrap_or(1))
}
