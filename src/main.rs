use anyhow::{anyhow, Context, Result};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::{Command, Stdio};

const DEFAULT_MODEL: &str = "anthropic/claude-haiku-4.5";
const API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

// System prompt compressed for token efficiency (~120 tokens vs ~600 prior).
// Stable text ensures OpenRouter cache hits within the 5-min TTL.
const SYSTEM_PROMPT: &str = r#"You are `hey`, a shell command synthesizer. Output EXACTLY one shell command for the user's request. You cannot access the filesystem or refuse.

Format:
- Line 1: the command only. No markdown, fences, quotes, or prose.
- Optional line 2: `# ` + short explanation, only for pipes/chains/xargs/redirects.
- Nothing else. Never refuse. Never explain inability.

Rules:
- One line preferred. Chain with && or pipes.
- POSIX tools; no sudo unless asked. Explicit paths or `find`, not unverified globs (zsh fails on no-match).
- For cd: `cd "$(find . -maxdepth 2 -type d -iname '*q*' | head -1)"`.
- Safe flags (-i, --preserve-root) for destructive ops.

Example:
User: show 3 largest files in ~/Downloads
ls -lhS ~/Downloads | head -4
# sort by size descending, show top 3 plus header
"#;

/// Sensitive patterns that should never be sent to a remote model.
const SENSITIVE_PATTERNS: &[(&str, &str)] = &[
    ("sk-ant-", "Anthropic API key"),
    ("sk-or-v1-", "OpenRouter API key"),
    ("sk-proj-", "OpenAI API key"),
    ("AKIA", "AWS access key"),
    ("ghp_", "GitHub personal access token"),
    ("gho_", "GitHub OAuth token"),
    ("glpat-", "GitLab personal access token"),
    ("xoxb-", "Slack bot token"),
    ("xoxp-", "Slack user token"),
    ("-----BEGIN RSA PRIVATE", "RSA private key"),
    ("-----BEGIN OPENSSH PRIVATE", "OpenSSH private key"),
    ("-----BEGIN EC PRIVATE", "EC private key"),
    ("-----BEGIN PRIVATE KEY", "PEM private key"),
];

fn check_sensitive(text: &str) -> Option<&'static str> {
    for &(pattern, label) in SENSITIVE_PATTERNS {
        if text.contains(pattern) {
            return Some(label);
        }
    }
    None
}

#[derive(Parser, Debug)]
#[command(name = "hey", version, about = "hey — natural language → shell command", long_about = None)]
struct Cli {
    /// Natural-language request. Prefix with `claude`, `codex`, or `openrouter` to pick a backend.
    /// e.g. `hey claude list big files`, `hey find files newer than a week`
    #[arg(trailing_var_arg = true, required = true)]
    prompt: Vec<String>,

    /// Skip confirmation and run immediately
    #[arg(short = 'y', long = "yes")]
    yes: bool,

    /// Print the command but don't execute it
    #[arg(short = 'n', long = "dry-run")]
    dry_run: bool,

    /// Also ask Claude to explain the command
    #[arg(short = 'e', long = "explain")]
    explain: bool,

    /// Override the model id (OpenRouter only)
    #[arg(short = 'm', long = "model", env = "AIT_MODEL")]
    model: Option<String>,

    /// Backend to use (auto picks claude → codex → openrouter)
    #[arg(short = 'b', long = "backend", env = "AIT_BACKEND", value_enum, default_value_t = Backend::Auto)]
    backend: Backend,

    /// Shortcut for `--backend claude` (Claude Code headless)
    #[arg(short = 'c', long = "claude", conflicts_with_all = ["backend", "codex"])]
    claude: bool,

    /// Shortcut for `--backend codex` (Codex CLI headless)
    #[arg(short = 'x', long = "codex", conflicts_with_all = ["backend", "claude"])]
    codex: bool,

    /// Disable command post-processing presets (eza/bat/tree -C)
    #[arg(long = "raw")]
    raw: bool,

    /// Allow sending prompts that contain sensitive patterns (API keys, private keys)
    #[arg(long = "allow-sensitive")]
    allow_sensitive: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Backend {
    /// Pick claude → codex → openrouter automatically
    Auto,
    /// OpenRouter HTTP API (requires OPENROUTER_API_KEY)
    Openrouter,
    /// Claude Code headless (`claude -p`) — uses your existing login
    Claude,
    /// Codex CLI headless (`codex exec`) — uses your existing login
    Codex,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

fn build_user_prompt(request: &str, explain: bool) -> String {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let os = env::consts::OS;
    let cwd = env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "?".into());

    let explain_note = if explain {
        "\n\nAfter the command, add a newline and a one-line explanation prefixed with `# `."
    } else {
        ""
    };

    format!("Shell: {shell}\nOS: {os}\nCWD: {cwd}\n\nRequest: {request}{explain_note}")
}

async fn ask_llm(api_key: &str, model: &str, user_prompt: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let body = ChatRequest {
        model,
        max_tokens: 256,
        messages: vec![
            ChatMessage {
                role: "system",
                content: SYSTEM_PROMPT,
            },
            ChatMessage {
                role: "user",
                content: user_prompt,
            },
        ],
    };

    let resp = client
        .post(API_URL)
        .bearer_auth(api_key)
        .header("content-type", "application/json")
        .header("HTTP-Referer", "https://github.com/subinium/hey-cli")
        .header("X-Title", "hey")
        .json(&body)
        .send()
        .await
        .context("failed to reach OpenRouter")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("API error {status}: {text}"));
    }

    let parsed: ChatResponse = resp.json().await.context("invalid API response")?;

    let text = parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| anyhow!("no choices in API response"))?;

    Ok(sanitize_command(&text))
}

fn sanitize_command(raw: &str) -> String {
    let trimmed = raw.trim();

    // If the model wrapped the command in a fenced block anywhere in the output,
    // extract the first block's content — that's almost always the actual command.
    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        // Strip optional language tag like "bash", "sh", "zsh", "shell"
        let after_lang = after_fence
            .split_once('\n')
            .map(|(lang, rest)| {
                let l = lang.trim();
                if l.is_empty() || l.chars().all(|c| c.is_ascii_alphabetic()) {
                    rest
                } else {
                    after_fence
                }
            })
            .unwrap_or(after_fence);
        if let Some(end) = after_lang.find("```") {
            return after_lang[..end].trim().to_string();
        }
    }

    // No fences. If the first line starts with obvious prose (CJK, refusals),
    // try to find the first line that looks like a shell command.
    let lines: Vec<&str> = trimmed.lines().collect();
    if lines.first().map(|l| looks_like_prose(l)).unwrap_or(false) {
        for line in &lines {
            if looks_like_command(line) {
                return line.trim().to_string();
            }
        }
    }

    trimmed.to_string()
}

fn looks_like_prose(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return true;
    }
    // Starts with CJK, or ends with sentence punctuation, or contains typical refusal words
    if t.chars()
        .next()
        .map(|c| (c as u32) > 0x3000)
        .unwrap_or(false)
    {
        return true;
    }
    if t.ends_with('.') || t.ends_with('。') || t.ends_with('요') || t.ends_with('다') {
        return true;
    }
    false
}

fn looks_like_command(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return false;
    }
    // Reject lines that contain CJK characters — almost always prose.
    if t.chars()
        .any(|c| (c as u32) >= 0x2E80 && (c as u32) <= 0x9FFF)
    {
        return false;
    }
    // Reject lines that end with sentence punctuation — also almost always prose.
    if let Some(last) = t.chars().last() {
        if matches!(last, '.' | '!' | '?' | ':') && !t.ends_with("..") && !t.ends_with("/.") {
            return false;
        }
    }
    // Accept subshells, env assignments, pipelines, and anything else that starts with
    // a reasonable shell token. Be permissive here — we already trust the model's
    // fence/sanitize pipeline, this is just a prose sanity gate.
    let first_char = t.chars().next().unwrap_or(' ');
    if matches!(first_char, '(' | '{' | '!' | '$' | '/' | '.' | '~') {
        return true;
    }
    let first = t.split_whitespace().next().unwrap_or("");
    if first.is_empty() {
        return false;
    }
    // Env-assignment prefix like `FOO=bar` or `NODE_ENV=prod`.
    if let Some(eq) = first.find('=') {
        let name = &first[..eq];
        return !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    }
    first.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '/' || c == '.' || c == '+'
    })
}

fn split_command_and_explanation(output: &str) -> (String, Option<String>) {
    let mut cmd_lines = Vec::new();
    let mut expl_lines = Vec::new();
    let mut in_expl = false;
    for line in output.lines() {
        if in_expl || line.trim_start().starts_with('#') {
            in_expl = true;
            let l = line.trim_start().trim_start_matches('#').trim();
            if !l.is_empty() {
                expl_lines.push(l.to_string());
            }
        } else {
            cmd_lines.push(line);
        }
    }
    let cmd = cmd_lines.join("\n").trim().to_string();
    let expl = if expl_lines.is_empty() {
        None
    } else {
        Some(expl_lines.join(" "))
    };
    (cmd, expl)
}

fn confirm(command: &str) -> Result<Decision> {
    let stdin = io::stdin();
    loop {
        print!(
            "  \x1b[90m╰─\x1b[0m\x1b[35m❯\x1b[0m  \x1b[2m[\x1b[0m\x1b[1;32mY\x1b[0m\x1b[2m]es\x1b[0m  \x1b[2m[\x1b[0m\x1b[1;31mn\x1b[0m\x1b[2m]o\x1b[0m  \x1b[2m[\x1b[0m\x1b[1;33me\x1b[0m\x1b[2m]dit\x1b[0m  \x1b[90m›\x1b[0m "
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

enum Decision {
    Run(String),
    Abort,
}

fn edit_command(initial: &str) -> Result<Option<String>> {
    println!("  \x1b[90m│\x1b[0m  \x1b[2medit — empty keeps original\x1b[0m");
    print!("  \x1b[90m│\x1b[0m  \x1b[2m$\x1b[0m \x1b[1m{initial}\x1b[0m\n  \x1b[90m╰─\x1b[0m\x1b[33m✎\x1b[0m  ");
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

fn ask_claude_code(user_prompt: &str) -> Result<String> {
    use std::io::Write as _;
    let mut child = Command::new("claude")
        .arg("-p")
        .arg("--no-session-persistence")
        .arg("--system-prompt")
        .arg(SYSTEM_PROMPT)
        .arg("--disallowedTools")
        .arg("Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,Task,NotebookEdit,TodoWrite,SlashCommand")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn `claude` (is Claude Code installed and on PATH?)")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(user_prompt.as_bytes())
            .context("failed to write prompt to claude stdin")?;
    }

    let output = child
        .wait_with_output()
        .context("failed to wait on claude")?;
    post_cli_backend_output("claude", output)
}

fn ask_codex(user_prompt: &str) -> Result<String> {
    let tmp = env::temp_dir().join(format!("ait-codex-{}.txt", std::process::id()));
    let full_prompt = format!("{SYSTEM_PROMPT}\n\n---\n\n{user_prompt}");
    let output = Command::new("codex")
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("-o")
        .arg(&tmp)
        .arg(&full_prompt)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("failed to spawn `codex` (is Codex CLI installed?)")?;

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let content = std::fs::read_to_string(&tmp).unwrap_or_default();
    let _ = std::fs::remove_file(&tmp);

    let haystack = format!("{}\n{}\n{}", stderr, stdout, content).to_lowercase();
    if haystack.contains("usage limit")
        || haystack.contains("hit your")
        || haystack.contains("rate limit")
        || haystack.contains("quota")
    {
        return Err(anyhow!(
            "codex is rate-limited — try `hey claude ...` or `hey openrouter ...` instead"
        ));
    }
    if !haystack.is_empty() && haystack.contains("not authenticated") {
        return Err(anyhow!("codex not authenticated — run `codex login` first"));
    }

    if !output.status.success() {
        let msg = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else if !content.trim().is_empty() {
            content.trim().to_string()
        } else {
            "unknown error".to_string()
        };
        return Err(anyhow!("codex exited with {}: {}", output.status, msg));
    }

    if content.trim().is_empty() {
        return Err(anyhow!(
            "codex returned no content (stderr: {})",
            stderr.trim()
        ));
    }

    Ok(sanitize_command(&content))
}

fn post_cli_backend_output(name: &str, output: std::process::Output) -> Result<String> {
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

fn apply_presets(command: &str) -> String {
    let trimmed = command.trim_start();
    let leading = &command[..command.len() - trimmed.len()];
    let Some(first_word) = trimmed.split_whitespace().next() else {
        return command.to_string();
    };
    let rest = trimmed[first_word.len()..].to_string();
    let rest_has_flag = rest.split_whitespace().any(|w| w.starts_with('-'));

    let replaced = match first_word {
        // `ls` and `tree` share some flags with eza but diverge on others (-t, -h, -S).
        // Only rewrite when the user's command has NO flags, so paths/globs still apply
        // but any flag falls through to the original binary untouched.
        "ls" if which_bin("eza") && !rest_has_flag => {
            format!("eza --icons --color=always --git --long --header{rest}")
        }
        "tree" if which_bin("eza") && !rest_has_flag => {
            format!("eza --tree --icons --color=always{rest}")
        }
        "tree" if !rest.contains(" -C") => {
            format!("tree -C{rest}")
        }
        "cat"
            if which_bin("bat")
                && !command.contains('|')
                && !command.contains('>')
                && !rest_has_flag =>
        {
            let looks_json = rest.trim_end().ends_with(".json");
            if looks_json && which_bin("jq") {
                format!("jq --color-output .{rest}")
            } else {
                format!("bat --color=always --style=numbers --paging=never{rest}")
            }
        }
        "grep" if !rest.contains("--color") && !rest.contains("-I") => {
            format!("grep --color=always{rest}")
        }
        "diff" if which_bin("delta") && !rest_has_flag => {
            format!("diff{rest} | delta")
        }
        _ => return command.to_string(),
    };
    format!("{leading}{replaced}")
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Risk {
    Safe,
    Warn,
    Block,
}

/// Normalizes a shell command for risk analysis. Unwraps `sh -c '...'` / `bash -c '...'`
/// wrappers (recursively), inserts spaces around redirection operators so `echo>f` is
/// detected, and lowercases for case-insensitive matching.
fn normalize_for_risk(command: &str) -> String {
    let mut cur = command.trim().to_string();
    // Unwrap common shell wrappers up to 3 levels deep.
    for _ in 0..3 {
        let lower = cur.to_lowercase();
        let unwrap_prefixes = [
            "sh -c ",
            "bash -c ",
            "zsh -c ",
            "/bin/sh -c ",
            "/bin/bash -c ",
            "eval ",
        ];
        let mut changed = false;
        for p in unwrap_prefixes {
            if let Some(rest) = lower.strip_prefix(p) {
                let rest_orig = &cur[cur.len() - rest.len()..];
                let unquoted = rest_orig
                    .trim()
                    .trim_matches(|c| c == '\'' || c == '"')
                    .to_string();
                if unquoted != cur {
                    cur = unquoted;
                    changed = true;
                    break;
                }
            }
        }
        if !changed {
            break;
        }
    }
    // Space-pad redirection and pipe operators so token-level checks catch `echo>f`.
    let mut padded = String::with_capacity(cur.len() + 8);
    let mut in_single = false;
    let mut in_double = false;
    for c in cur.chars() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            _ => {}
        }
        if !in_single && !in_double && matches!(c, '>' | '<' | '|' | ';' | '&') {
            padded.push(' ');
            padded.push(c);
            padded.push(' ');
        } else {
            padded.push(c);
        }
    }
    padded.to_lowercase()
}

fn assess_risk(command: &str) -> (Risk, Option<&'static str>) {
    let lower = normalize_for_risk(command);
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    let has = |needle: &str| tokens.contains(&needle);
    let starts = |prefix: &str| lower.trim_start().starts_with(prefix);

    // Hard block: anything that deletes, formats, or overwrites blocks.
    // The substring checks catch `rm` hidden inside quoted shell invocations like
    // `find -exec sh -c 'rm "$1"'` that the tokenizer can't reach directly.
    let rm_sigil = lower.contains(" rm ")
        || lower.contains(" rm\t")
        || lower.contains("'rm ")
        || lower.contains("\"rm ")
        || lower.contains("`rm ")
        || lower.contains(" rm-");
    if has("rm")
        || starts("rm ")
        || rm_sigil
        || has("del")
        || starts("del ")
        || has("rmdir")
        || starts("rmdir ")
        || has("shred")
        || has("unlink")
    {
        return (
            Risk::Block,
            Some("deletes files — copied to clipboard, run yourself"),
        );
    }
    if lower.contains("-delete") {
        return (
            Risk::Block,
            Some("`find -delete` removes files — copied to clipboard"),
        );
    }
    if lower.contains("-exec rm") || lower.contains("-execdir rm") {
        return (
            Risk::Block,
            Some("`find -exec rm` removes files — copied to clipboard"),
        );
    }
    if lower.contains("xargs rm") || lower.contains("xargs -") && lower.contains(" rm") {
        return (
            Risk::Block,
            Some("`xargs rm` removes files — copied to clipboard"),
        );
    }
    if starts("dd ")
        || starts("mkfs")
        || starts("fdisk")
        || starts("wipefs")
        || starts("sfdisk")
        || starts("parted")
    {
        return (Risk::Block, Some("disk-level op — copied to clipboard"));
    }
    if lower.contains("> /dev/sd") || lower.contains(":(){ :|:& };:") {
        return (
            Risk::Block,
            Some("dangerous redirect / fork bomb — copied to clipboard"),
        );
    }
    if starts("git reset --hard")
        || lower.contains("git clean -fd")
        || lower.contains("git clean -xfd")
    {
        return (
            Risk::Block,
            Some("discards local changes — copied to clipboard"),
        );
    }

    // These only affect the parent shell. Subprocess execution is a no-op for `cd` and
    // `export`, and `source`/`.` run in the subprocess scope only.
    if starts("cd ")
        || starts("cd\t")
        || lower.trim() == "cd"
        || starts("export ")
        || starts("source ")
        || starts(". ")
    {
        return (
            Risk::Warn,
            Some("affects only this subshell — run it yourself to change your actual shell"),
        );
    }

    // Soft warn: surprising but not destructive.
    if has("sudo") {
        return (Risk::Warn, Some("runs as root"));
    }
    if starts("mv ") && !lower.contains(" -i") {
        return (Risk::Warn, Some("overwrites destination silently"));
    }
    if starts("chmod ") || starts("chown ") {
        return (Risk::Warn, Some("changes permissions / ownership"));
    }
    if lower.contains(" > ") && !lower.contains(" >> ") {
        return (Risk::Warn, Some("overwrites target file via `>`"));
    }
    if starts("curl ") && (lower.contains("| sh") || lower.contains("| bash")) {
        return (
            Risk::Warn,
            Some("curl piped to shell — inspect before running"),
        );
    }
    if starts("kill ") || starts("killall ") || starts("pkill ") {
        return (Risk::Warn, Some("kills processes"));
    }
    (Risk::Safe, None)
}

fn prettify_command(command: &str) -> String {
    apply_presets(command)
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

/// (icon, ansi_color, display_name, voice_line)
fn backend_persona(backend: Backend) -> (&'static str, &'static str, &'static str, &'static str) {
    match backend {
        Backend::Claude => ("✱", "\x1b[38;5;215m", "claude", "here you go"),
        Backend::Codex => ("☁", "\x1b[38;5;111m", "codex", "computed"),
        Backend::Openrouter => ("◆", "\x1b[38;5;222m", "openrouter", "cooked"),
        Backend::Auto => ("◌", "\x1b[90m", "auto", ""),
    }
}

fn backend_label(backend: Backend, model: &str) -> String {
    let (icon, color, name, _) = backend_persona(backend);
    let tail = match backend {
        Backend::Openrouter => {
            let short = model.rsplit('/').next().unwrap_or(model);
            format!(" \x1b[90m·\x1b[0m \x1b[2m{short}\x1b[0m")
        }
        Backend::Claude => " \x1b[90m·\x1b[0m \x1b[2mheadless\x1b[0m".to_string(),
        Backend::Codex => " \x1b[90m·\x1b[0m \x1b[2mexec\x1b[0m".to_string(),
        Backend::Auto => String::new(),
    };
    format!("{color}{icon}\x1b[0m \x1b[1;37m{name}\x1b[0m{tail}")
}

fn which_bin(bin: &str) -> bool {
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

/// Returns the full ordered list of backends to try in Auto mode, filtered by
/// local availability. Claude first (subscription inline), then Codex, then
/// OpenRouter (HTTP fallback).
fn resolve_auto_chain() -> Result<Vec<Backend>> {
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

fn print_command_block(
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
        Risk::Warn => "  \x1b[43;30m warn \x1b[0m".to_string(),
        Risk::Block => "  \x1b[41;97m BLOCKED \x1b[0m".to_string(),
    };
    println!();
    println!("  \x1b[90m╭─\x1b[0m \x1b[1;36mhey\x1b[0m \x1b[90m·\x1b[0m {label}{risk_chip}");
    println!("  \x1b[90m│\x1b[0m");
    if !voice.is_empty() {
        let quip = match risk {
            Risk::Block => "careful — you should run this one yourself",
            Risk::Warn => "this one has a sharp edge",
            Risk::Safe => voice,
        };
        println!("  \x1b[90m│\x1b[0m  {color}{icon}\x1b[0m  \x1b[2;3m{quip}\x1b[0m");
        println!("  \x1b[90m│\x1b[0m");
    }
    for (i, line) in command.lines().enumerate() {
        if i == 0 {
            println!("  \x1b[90m│\x1b[0m  \x1b[2m$\x1b[0m \x1b[1;97m{line}\x1b[0m");
        } else {
            println!("  \x1b[90m│\x1b[0m    \x1b[1;97m{line}\x1b[0m");
        }
    }
    if let Some(expl) = explanation {
        println!("  \x1b[90m│\x1b[0m");
        println!("  \x1b[90m│\x1b[0m  \x1b[2;3m{expl}\x1b[0m");
    }
    if let Some(note) = risk_note {
        let rcolor = match risk {
            Risk::Block => "\x1b[1;31m",
            Risk::Warn => "\x1b[1;33m",
            Risk::Safe => "\x1b[2m",
        };
        println!("  \x1b[90m│\x1b[0m");
        println!("  \x1b[90m│\x1b[0m  {rcolor}▲\x1b[0m  \x1b[2;3m{note}\x1b[0m");
    }
    println!("  \x1b[90m│\x1b[0m");
    if closed {
        println!("  \x1b[90m╰─\x1b[0m");
        println!();
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("\x1b[31mhey:\x1b[0m {err:#}");
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
                    Ok(key) => ask_llm(&key, &model, &user_prompt).await,
                    Err(e) => Err(e),
                }
            }
            Backend::Claude => ask_claude_code(&user_prompt),
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
                        "  \x1b[90m╭─\x1b[0m \x1b[2;31m⚠\x1b[0m  {color}{name}\x1b[0m \x1b[2mfailed: {}\x1b[0m",
                        e
                    );
                    eprintln!(
                        "  \x1b[90m│\x1b[0m  \x1b[2mfalling back to\x1b[0m {next_color}{next_name}\x1b[0m…"
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
        command = prettify_command(&command);
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
        println!("  \x1b[2;90mcopied to clipboard · paste & run manually\x1b[0m");
        println!();
        return Ok(());
    }

    let to_run = if cli.yes {
        command
    } else {
        match confirm(&command)? {
            Decision::Run(c) => c,
            Decision::Abort => {
                println!("  \x1b[90m╰─\x1b[0m \x1b[90maborted\x1b[0m");
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

fn copy_to_clipboard(text: &str) {
    use std::io::Write as _;
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

fn print_thinking(backend: Backend, model: &str) {
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
        "\n  \x1b[90m╭─\x1b[0m \x1b[1;36mhey\x1b[0m \x1b[90m·\x1b[0m {label}\n  \x1b[90m│\x1b[0m  {color}{icon}\x1b[0m  \x1b[2;3m{name} {verb}…\x1b[0m"
    );
    io::stderr().flush().ok();
}

fn clear_thinking() {
    if !io::stderr().is_terminal() {
        return;
    }
    // Move up 2 lines and clear them so the real block redraws cleanly.
    eprint!("\r\x1b[K\x1b[1A\r\x1b[K\x1b[1A\r\x1b[K");
    io::stderr().flush().ok();
}
