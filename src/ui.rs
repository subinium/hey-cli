use std::io::{self, BufRead, IsTerminal, Write};
use std::process::{Command, Stdio};

use crate::backend::{backend_label, backend_persona, which_bin};
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
    let (icon, color, _name, voice) = backend_persona(backend);
    let risk_chip = match risk {
        Risk::Safe => String::new(),
        Risk::Warn => format!("  {BG_YELLOW_BLACK} warn {RESET}"),
        Risk::Block => format!("  {BG_RED_WHITE} BLOCKED {RESET}"),
    };
    println!();
    println!("  {GRAY}╭─{RESET} {BOLD_CYAN}hey{RESET} {GRAY}·{RESET} {label}{risk_chip}");
    println!("  {GRAY}│{RESET}");
    if !voice.is_empty() {
        let quip = match risk {
            Risk::Block => "careful — you should run this one yourself",
            Risk::Warn => "this one has a sharp edge",
            Risk::Safe => voice,
        };
        println!("  {GRAY}│{RESET}  {color}{icon}{RESET}  {DIM_ITALIC}{quip}{RESET}");
        println!("  {GRAY}│{RESET}");
    }
    for (i, line) in command.lines().enumerate() {
        if i == 0 {
            println!("  {GRAY}│{RESET}  {DIM}${RESET} {BOLD_WHITE}{line}{RESET}");
        } else {
            println!("  {GRAY}│{RESET}    {BOLD_WHITE}{line}{RESET}");
        }
    }
    if let Some(expl) = explanation {
        println!("  {GRAY}│{RESET}");
        println!("  {GRAY}│{RESET}  {DIM_ITALIC}{expl}{RESET}");
    }
    if let Some(note) = risk_note {
        let rcolor = match risk {
            Risk::Block => BOLD_RED,
            Risk::Warn => BOLD_YELLOW,
            Risk::Safe => DIM,
        };
        println!("  {GRAY}│{RESET}");
        println!("  {GRAY}│{RESET}  {rcolor}▲{RESET}  {DIM_ITALIC}{note}{RESET}");
    }
    println!("  {GRAY}│{RESET}");
    if closed {
        println!("  {GRAY}╰─{RESET}");
        println!();
    }
}

pub(crate) fn confirm(command: &str) -> anyhow::Result<Decision> {
    let stdin = io::stdin();
    loop {
        print!(
            "  {GRAY}╰─{RESET}{MAGENTA}❯{RESET}  {DIM}[{RESET}{BOLD_GREEN}Y{RESET}{DIM}]es{RESET}  {DIM}[{RESET}{BOLD_RED}n{RESET}{DIM}]o{RESET}  {DIM}[{RESET}{BOLD_YELLOW}e{RESET}{DIM}]dit{RESET}  {GRAY}›{RESET} "
        );
        io::stdout().flush().ok();
        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        let ans = line.trim().to_lowercase();
        match ans.as_str() {
            "" | "y" | "yes" => {
                println!();
                return Ok(Decision::Run(command.to_string()));
            }
            "n" | "no" => return Ok(Decision::Abort),
            "e" | "edit" => {
                if let Some(edited) = edit_command(command)? {
                    return Ok(Decision::Run(edited));
                } else {
                    return Ok(Decision::Abort);
                }
            }
            _ => continue,
        }
    }
}

pub(crate) fn edit_command(initial: &str) -> anyhow::Result<Option<String>> {
    println!("  {GRAY}│{RESET}  {DIM}edit — empty keeps original{RESET}");
    print!("  {GRAY}│{RESET}  {DIM}${RESET} {BOLD}{initial}{RESET}\n  {GRAY}╰─{RESET}{YELLOW}✎{RESET}  ");
    io::stdout().flush().ok();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let trimmed = line.trim();
    println!();
    if trimmed.is_empty() {
        Ok(Some(initial.to_string()))
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

pub(crate) fn print_thinking(backend: Backend, model: &str) {
    if !io::stderr().is_terminal() {
        return;
    }
    let label = backend_label(backend, model);
    let (icon, color, name, _) = backend_persona(backend);
    let verb = match backend {
        Backend::Claude => "is thinking",
        Backend::Codex => "is computing",
        Backend::Openrouter => "is cooking",
        Backend::Auto => "is thinking",
    };
    eprint!(
        "\n  {GRAY}╭─{RESET} {BOLD_CYAN}hey{RESET} {GRAY}·{RESET} {label}\n  {GRAY}│{RESET}  {color}{icon}{RESET}  {DIM_ITALIC}{name} {verb}…{RESET}"
    );
    io::stderr().flush().ok();
}

pub(crate) fn clear_thinking() {
    if !io::stderr().is_terminal() {
        return;
    }
    // Move up 2 lines and clear them so the real block redraws cleanly.
    eprint!("\r\x1b[K\x1b[1A\r\x1b[K\x1b[1A\r\x1b[K");
    io::stderr().flush().ok();
}

pub(crate) fn copy_to_clipboard(text: &str) {
    let bin = if cfg!(target_os = "macos") {
        "pbcopy"
    } else if which_bin("wl-copy") {
        "wl-copy"
    } else if which_bin("xclip") {
        "xclip"
    } else {
        return;
    };
    if let Ok(mut child) = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}
