use std::env;

// System prompt compressed for token efficiency (~120 tokens vs ~600 prior).
// Stable text ensures OpenRouter cache hits within the 5-min TTL.
pub(crate) const SYSTEM_PROMPT: &str = r#"You are `hey`, a shell command synthesizer. Output EXACTLY one shell command for the user's request. You cannot access the filesystem or refuse.

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

pub(crate) fn build_user_prompt(request: &str, explain: bool) -> String {
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
