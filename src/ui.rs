use std::io::{self, BufRead, IsTerminal, Write};
use std::process::{Command, Stdio};

use crate::backend::{backend_art, backend_label, backend_persona, which_bin};
use crate::cli::Backend;
use crate::risk::Risk;
use crate::style::*;

pub(crate) enum Decision {
    Run(String),
    Abort,
}

pub(crate) fn print_command_block(
    command: &str,
    explanation: Option<&str>,
    backend: Backend,
    model: &str,
    risk: Risk,
    risk_note: Option<&str>,
    closed: bool,
) {
    let label = backend_label(backend, model);
    let (_, color, _, voice) = backend_persona(backend);
    let risk_chip = match risk {
        Risk::Safe => String::new(),
        Risk::Warn => format!("  {BG_YELLOW_BLACK} warn {RESET}"),
        Risk::Block => format!("  {BG_RED_WHITE} BLOCKED {RESET}"),
    };

    println!("\n  {label}{risk_chip}");
    println!();

    if !voice.is_empty() {
        let quip = match risk {
            Risk::Block => "careful — you should run this one yourself",
            Risk::Warn => "this one has a sharp edge",
            Risk::Safe => voice,
        };
        let art = backend_art(backend);
        if art.is_empty() {
            println!("  {DIM_ITALIC}{quip}{RESET}");
        } else {
            println!("  {color}{}{RESET}  {DIM_ITALIC}{quip}{RESET}", art[0]);
            for line in &art[1..] {
                println!("  {color}{line}{RESET}");
            }
        }
        println!();
    }

    for (i, line) in command.lines().enumerate() {
        if i == 0 {
            println!("  {DIM}${RESET} {BOLD_WHITE}{line}{RESET}");
        } else {
            println!("    {BOLD_WHITE}{line}{RESET}");
        }
    }

    if let Some(expl) = explanation {
        println!();
        println!("  {DIM_ITALIC}{expl}{RESET}");
    }
    if let Some(note) = risk_note {
        let rcolor = match risk {
            Risk::Block => BOLD_RED,
            Risk::Warn => BOLD_YELLOW,
            Risk::Safe => DIM,
        };
        println!();
        println!("  {rcolor}▲{RESET}  {DIM_ITALIC}{note}{RESET}");
    }

    if closed {
        println!();
    }
}

pub(crate) fn confirm(command: &str, risk: Risk) -> anyhow::Result<Decision> {
    let stdin = io::stdin();
    let warn = matches!(risk, Risk::Warn);
    loop {
        println!();
        if warn {
            // Warn: explicit `y` required, Enter defaults to abort.
            print!(
                "  {BOLD_YELLOW}▶{RESET} run? {DIM}y{RESET} / {BOLD_YELLOW}N{RESET} {DIM}(default){RESET} "
            );
        } else {
            print!(
                "  {BOLD_GREEN}▶{RESET} run? {BOLD_GREEN}Y{RESET} {DIM}(default){RESET} / {DIM}N{RESET} "
            );
        }
        io::stdout().flush().ok();

        let mut line = String::new();
        let n = stdin.lock().read_line(&mut line)?;
        // EOF (Ctrl-D or closed stdin): read_line returns Ok(0) without writing to `line`.
        if n == 0 {
            println!();
            println!("  {GRAY}╰─{RESET} {GRAY}aborted (no input){RESET}");
            println!();
            return Ok(Decision::Abort);
        }
        let ans = line.trim().to_lowercase();
        match ans.as_str() {
            "" => {
                if warn {
                    // Blank Enter on warn: treat as No.
                    return Ok(Decision::Abort);
                }
                println!();
                return Ok(Decision::Run(command.to_string()));
            }
            "y" | "yes" => {
                println!();
                return Ok(Decision::Run(command.to_string()));
            }
            "n" | "no" => return Ok(Decision::Abort),
            "e" | "edit" => {
                let copied = copy_to_clipboard(command);
                println!();
                if copied {
                    println!(
                        "  {DIM}copied to clipboard — paste in your shell to edit & run{RESET}"
                    );
                } else {
                    println!(
                        "  {DIM}could not copy to clipboard (install pbcopy/xclip/wl-copy){RESET}"
                    );
                }
                println!();
                return Ok(Decision::Abort);
            }
            _ => continue,
        }
    }
}

/// Number of terminal lines used by print_thinking so clear_thinking can erase them.
static THINKING_LINES: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Hide the terminal cursor.
pub(crate) fn hide_cursor() {
    if io::stderr().is_terminal() {
        eprint!("\x1b[?25l");
        io::stderr().flush().ok();
    }
}

/// Show the terminal cursor.
pub(crate) fn show_cursor() {
    if io::stderr().is_terminal() {
        eprint!("\x1b[?25h");
        io::stderr().flush().ok();
    }
}

pub(crate) fn print_thinking(backend: Backend, model: &str) {
    if !io::stderr().is_terminal() {
        return;
    }
    hide_cursor();
    let label = backend_label(backend, model);
    let (_, color, name, _) = backend_persona(backend);
    let art = backend_art(backend);
    let verb = match backend {
        Backend::Claude => "is thinking",
        Backend::Codex => "is computing",
        Backend::Openrouter => "is cooking",
        Backend::Auto => "is thinking",
    };

    let mut lines = 0u8;
    eprint!("\n  {label}");
    lines += 1;
    eprintln!();
    lines += 1;

    if !art.is_empty() {
        eprint!(
            "  {color}{}{RESET}  {DIM_ITALIC}{name} {verb}…{RESET}",
            art[0]
        );
        lines += 1;
        for line in &art[1..] {
            eprint!("\n  {color}{line}{RESET}");
            lines += 1;
        }
    } else {
        eprint!("  {DIM_ITALIC}{name} {verb}…{RESET}");
        lines += 1;
    }

    THINKING_LINES.store(lines, std::sync::atomic::Ordering::Relaxed);
    io::stderr().flush().ok();
}

pub(crate) fn clear_thinking() {
    if !io::stderr().is_terminal() {
        return;
    }
    let lines = THINKING_LINES.load(std::sync::atomic::Ordering::Relaxed);
    for _ in 0..lines {
        eprint!("\r\x1b[K\x1b[1A");
    }
    // Erase from cursor to end of screen — catches wrapped lines on narrow terminals.
    eprint!("\r\x1b[J");
    show_cursor();
    io::stderr().flush().ok();
}

/// Copies `text` to the system clipboard. Returns true on success, false if no
/// backend was available or the copy failed.
pub(crate) fn copy_to_clipboard(text: &str) -> bool {
    let bin = if cfg!(target_os = "macos") {
        "pbcopy"
    } else if which_bin("wl-copy") {
        "wl-copy"
    } else if which_bin("xclip") {
        "xclip"
    } else {
        return false;
    };
    let mut child = match Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(text.as_bytes()).is_err() {
            // Still wait so we don't leak the child process.
            let _ = child.wait();
            return false;
        }
    }
    match child.wait() {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}
