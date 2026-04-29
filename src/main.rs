mod backend;
mod cli;
mod completions;
mod doctor;
mod preset;
mod prompt;
mod risk;
mod sanitize;
mod style;
mod ui;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::env;
use std::io::{self, IsTerminal, Read};
use std::process::{Command, Stdio};

use backend::claude::ask_claude;
use backend::codex::ask_codex;
use backend::openrouter::{ask_openrouter, DEFAULT_MODEL};
use backend::{backend_persona, resolve_auto_chain, which_bin};
use cli::{Backend, Cli};
use preset::apply_presets;
use prompt::build_user_prompt;
use risk::{assess_risk, check_sensitive, Risk};
use sanitize::{looks_like_command, split_command_and_explanation};
use style::*;
use ui::{
    clear_thinking, confirm, copy_to_clipboard, print_command_block, print_thinking, Decision,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Ensure cursor is always restored, even on panic or Ctrl-C.
    ctrlc_show_cursor();
    backend::init_bin_cache();
    if let Err(err) = run().await {
        ui::show_cursor();
        eprintln!("{RED}hey:{RESET} {err:#}");
        std::process::exit(1);
    }
}

fn ctrlc_show_cursor() {
    let _ = ctrlc::set_handler(|| {
        // Best-effort cursor restore before exit.
        eprint!("\x1b[?25h");
        std::process::exit(130);
    });
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Subcommand-style dispatch on the first positional word.
    // Supported: `doctor`, `init <shell>` — these must be exact lowercase and
    // are checked BEFORE backend parsing so `hey doctor` / `hey init zsh` work
    // without any flags.
    if let Some(first) = cli.prompt.first() {
        match first.as_str() {
            "doctor" => return doctor::run(),
            "init" => {
                let shell = cli.prompt.get(1).map(|s| s.as_str());
                return completions::run(shell);
            }
            _ => {}
        }
    }

    // Subcommand-style backend selection: `hey claude ...`, `hey codex ...`, `hey openrouter ...`.
    // Case-sensitive so `hey Claude is fast` doesn't consume `Claude`. Also drop
    // the `or` alias — too ambiguous with the English preposition.
    let mut prompt_words = cli.prompt.clone();
    let inline_backend = match prompt_words.first().map(String::as_str) {
        Some("claude") => Some(Backend::Claude),
        Some("codex") => Some(Backend::Codex),
        Some("openrouter") => Some(Backend::Openrouter),
        Some("auto") => Some(Backend::Auto),
        _ => None,
    };
    if inline_backend.is_some() {
        prompt_words.remove(0);
    }

    // Resolve the prompt text. Precedence:
    //   1. If we have positional words after backend stripping, use them.
    //   2. Else, if stdin is a pipe (not a TTY), read the prompt from stdin.
    //   3. Else, error with a helpful message.
    // When stdin was consumed for the prompt, confirm() would block on nothing
    // readable — require --yes or --dry-run in that case.
    let stdin_was_piped = !io::stdin().is_terminal();
    let have_positional = !prompt_words.is_empty();
    let request = if have_positional {
        prompt_words.join(" ")
    } else if stdin_was_piped {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("failed to read prompt from stdin")?;
        buf.trim().to_string()
    } else {
        String::new()
    };

    if request.trim().is_empty() {
        return Err(anyhow!(
            "empty prompt — try `hey find files bigger than 100mb`, `hey claude explain this regex`, or pipe: `echo 'list docker containers' | hey --yes`"
        ));
    }

    // If the prompt came from stdin, confirm() can't read a y/N answer — require
    // an explicit non-interactive mode.
    if stdin_was_piped && !have_positional && !cli.yes && !cli.dry_run {
        return Err(anyhow!(
            "prompt was read from stdin, but stdin is needed for confirmation — pass --yes to auto-run or --dry-run to just print"
        ));
    }

    // Refuse to run when we have nowhere sensible to print the command. Skipping
    // this silently caused hangs on `hey foo | head`.
    if !io::stdout().is_terminal() && !cli.yes && !cli.dry_run {
        return Err(anyhow!(
            "stdout is not a terminal — pass --yes to auto-run or --dry-run to just print"
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
        None => resolve_auto_chain().map_err(|_| no_backend_error())?,
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
                let api_key = env::var("OPENROUTER_API_KEY").map_err(|_| {
                    anyhow!(
                        "OPENROUTER_API_KEY not set. Get one at https://openrouter.ai/keys, then: export OPENROUTER_API_KEY=sk-or-v1-..."
                    )
                });
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
        let preview = truncate_prose(&command, 200);
        return Err(anyhow!(
            "backend returned prose instead of a command:\n\n{preview}\n\nTry a different backend with `hey claude ...` / `hey codex ...` / `hey openrouter ...`."
        ));
    }
    let preset_label = if cli.raw {
        None
    } else {
        let (rewritten, label) = apply_presets(&command);
        command = rewritten;
        label
    };

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
        preset_label,
        closed,
    );

    if cli.dry_run {
        return Ok(());
    }
    if blocked {
        // Destructive commands: print + copy to clipboard, never auto-execute.
        // Report accurately whether the clipboard copy succeeded.
        if copy_to_clipboard(&command) {
            println!("  {DIM_GRAY}copied to clipboard · paste & run manually{RESET}");
        } else {
            println!(
                "  {DIM_GRAY}could not copy to clipboard (install pbcopy/xclip/wl-copy) — copy manually:{RESET}"
            );
            println!();
            for line in command.lines() {
                println!("  {BOLD_WHITE}{line}{RESET}");
            }
        }
        println!();
        return Ok(());
    }

    let to_run = if cli.yes {
        command
    } else {
        match confirm(&command, risk)? {
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

/// Build a helpful error for the `no backend available` case, tailored to what
/// the user actually has on disk and in env-vars.
fn no_backend_error() -> anyhow::Error {
    let mut lines = vec!["no backend available.".to_string()];
    let has_claude = which_bin("claude");
    let has_codex = which_bin("codex");
    let has_or_key = env::var("OPENROUTER_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())
        .is_some();

    if !has_claude {
        lines.push(
            "  1) install Claude Code (https://docs.anthropic.com/claude/docs/claude-code) and run `claude login`".into(),
        );
    }
    if !has_codex {
        lines.push("  2) install the Codex CLI and run `codex login`".into());
    }
    if !has_or_key {
        lines.push(
            "  3) set OPENROUTER_API_KEY — get a key at https://openrouter.ai/keys, then `export OPENROUTER_API_KEY=sk-or-v1-...`"
                .into(),
        );
    }
    if has_claude || has_codex || has_or_key {
        lines.push("  run `hey doctor` for a full diagnostic.".into());
    }
    anyhow!("{}", lines.join("\n"))
}

/// Truncate prose to `max` chars with an ellipsis suffix. Used to keep the
/// "backend returned prose instead of a command" error from dumping kilobytes.
fn truncate_prose(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}\u{2026}")
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
