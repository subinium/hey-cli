#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Risk {
    Safe,
    Warn,
    Block,
}

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

pub(crate) fn check_sensitive(text: &str) -> Option<&'static str> {
    for &(pattern, label) in SENSITIVE_PATTERNS {
        if text.contains(pattern) {
            return Some(label);
        }
    }
    None
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

pub(crate) fn assess_risk(command: &str) -> (Risk, Option<&'static str>) {
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
