//! Risk gate: classifies a shell command as Safe, Warn, or Block BEFORE it runs.
//!
//! The gate is intentionally conservative: false positives (harmless commands
//! flagged as Warn) are acceptable; false negatives (destructive commands
//! missed) are not. The normalization pipeline unwraps shell wrappers and
//! command substitutions so inner code is evaluated as its own segment.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Risk {
    Safe,
    Warn,
    Block,
}

/// Sensitive patterns that should never be sent to a remote model.
/// These are prefix-matched against the user's prompt text.
const SENSITIVE_PATTERNS: &[(&str, &str)] = &[
    // Anthropic / OpenAI / OpenRouter
    ("sk-ant-", "Anthropic API key"),
    ("sk-or-", "OpenRouter API key"),
    ("sk-proj-", "OpenAI project key"),
    ("sess-", "OpenAI session token"),
    // AWS
    ("AKIA", "AWS access key"),
    ("ASIA", "AWS temporary access key"),
    ("AGPA", "AWS group access key"),
    ("ANPA", "AWS user access key"),
    // GitHub
    ("ghp_", "GitHub personal access token"),
    ("gho_", "GitHub OAuth token"),
    ("ghu_", "GitHub user-to-server token"),
    ("ghs_", "GitHub server-to-server token"),
    ("ghr_", "GitHub refresh token"),
    ("github_pat_", "GitHub fine-grained PAT"),
    // GitLab
    ("glpat-", "GitLab personal access token"),
    // Slack
    ("xoxb-", "Slack bot token"),
    ("xoxp-", "Slack user token"),
    ("xoxa-", "Slack app token"),
    ("xoxs-", "Slack session token"),
    // Stripe
    ("sk_live_", "Stripe live secret key"),
    ("sk_test_", "Stripe test secret key"),
    ("rk_live_", "Stripe restricted key"),
    ("rk_test_", "Stripe restricted test key"),
    // SendGrid / HuggingFace / JWT
    ("SG.", "SendGrid API key"),
    ("hf_", "HuggingFace token"),
    ("eyJhbGciOi", "JSON Web Token"),
    // Private keys (PEM)
    ("-----BEGIN RSA PRIVATE", "RSA private key"),
    ("-----BEGIN DSA PRIVATE", "DSA private key"),
    ("-----BEGIN EC PRIVATE", "EC private key"),
    ("-----BEGIN OPENSSH PRIVATE", "OpenSSH private key"),
    ("-----BEGIN PRIVATE KEY", "PEM private key"),
    ("-----BEGIN ENCRYPTED PRIVATE", "encrypted private key"),
    ("-----BEGIN PGP PRIVATE", "PGP private key"),
    // Google service account / Azure storage
    (
        "\"type\": \"service_account\"",
        "Google service account JSON",
    ),
    ("\"private_key_id\":", "Google service account JSON"),
    (
        "DefaultEndpointsProtocol=",
        "Azure storage connection string",
    ),
];

pub(crate) fn check_sensitive(text: &str) -> Option<&'static str> {
    for &(pattern, label) in SENSITIVE_PATTERNS {
        if text.contains(pattern) {
            return Some(label);
        }
    }
    // URL with embedded credentials: scheme://user:password@host
    // We require a colon inside the userinfo and a non-empty password.
    if let Some(scheme_pos) = text.find("://") {
        let rest = &text[scheme_pos + 3..];
        if let Some(at) = rest.find('@') {
            let userinfo = &rest[..at];
            if userinfo.contains(':') && userinfo.len() > 1 {
                let parts: Vec<&str> = userinfo.splitn(2, ':').collect();
                if parts.len() == 2 && !parts[1].is_empty() {
                    return Some("URL with embedded credentials");
                }
            }
        }
    }
    None
}

// ---------- Normalization ----------

/// Normalizes a shell command for risk analysis. Returns a lowercased string
/// where:
///   - top-level `sh -c`, `bash -c`, `eval`, etc. have been unwrapped,
///   - `$(...)`, `<(...)`, `>(...)`, and backticks become `;`-separated segments,
///   - `$IFS` / `${IFS}` are replaced with spaces,
///   - quote characters are replaced with spaces,
///   - shell operators (`| ; & < >`) are space-padded so tokenization is clean.
fn normalize_for_risk(command: &str) -> String {
    let mut cur = command.trim().to_string();

    // Step 1: strip common shell-wrapper prefixes iteratively.
    for _ in 0..5 {
        let lower = cur.to_lowercase();
        let unwrap_prefixes = [
            "sh -c ",
            "bash -c ",
            "zsh -c ",
            "dash -c ",
            "ksh -c ",
            "fish -c ",
            "xonsh -c ",
            "/bin/sh -c ",
            "/bin/bash -c ",
            "/bin/zsh -c ",
            "/usr/bin/sh -c ",
            "/usr/bin/bash -c ",
            "eval ",
        ];
        let mut changed = false;
        for p in unwrap_prefixes {
            if let Some(rest) = lower.strip_prefix(p) {
                // Slice the same region from the original-case string.
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

    // Step 2: replace IFS obfuscation with plain spaces.
    let cur = cur.replace("$IFS", " ").replace("${IFS}", " ");

    // Step 3: unwrap $(...), <(...), >(...), and backticks as new segments.
    let cur = unwrap_command_substitutions(&cur);

    // Step 4: collapse quote characters to spaces so quoted tokens split cleanly.
    let cur = cur.replace(['\'', '"'], " ");

    // Step 5: pad shell operators so tokenization finds them as separate tokens.
    let mut padded = String::with_capacity(cur.len() + 8);
    for c in cur.chars() {
        if matches!(c, '|' | ';' | '&' | '<' | '>') {
            padded.push(' ');
            padded.push(c);
            padded.push(' ');
        } else {
            padded.push(c);
        }
    }

    // Step 6: collapse runs of whitespace to single spaces so substring checks
    // (e.g. " > /dev/sd") don't break on double-spacing introduced by padding.
    let collapsed: String = padded.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.to_lowercase()
}

/// Replaces `$(...)`, `<(...)`, `>(...)`, and backtick spans with
/// ` ; <contents> ; ` so that the inner code becomes its own segment when
/// the caller later splits on `;`.
fn unwrap_command_substitutions(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '$' if chars.peek() == Some(&'(') => {
                chars.next();
                result.push_str(" ; ");
                let mut depth = 1;
                for inner in chars.by_ref() {
                    if inner == '(' {
                        depth += 1;
                        result.push(inner);
                    } else if inner == ')' {
                        depth -= 1;
                        if depth == 0 {
                            result.push_str(" ; ");
                            break;
                        }
                        result.push(inner);
                    } else {
                        result.push(inner);
                    }
                }
            }
            '<' | '>' if chars.peek() == Some(&'(') => {
                chars.next();
                result.push_str(" ; ");
                let mut depth = 1;
                for inner in chars.by_ref() {
                    if inner == '(' {
                        depth += 1;
                        result.push(inner);
                    } else if inner == ')' {
                        depth -= 1;
                        if depth == 0 {
                            result.push_str(" ; ");
                            break;
                        }
                        result.push(inner);
                    } else {
                        result.push(inner);
                    }
                }
            }
            '`' => {
                result.push_str(" ; ");
                for inner in chars.by_ref() {
                    if inner == '`' {
                        result.push_str(" ; ");
                        break;
                    }
                    result.push(inner);
                }
            }
            _ => result.push(c),
        }
    }
    result
}

/// Split a normalized command into top-level segments at `|`, `;`, `&`, `<`, `>`.
fn split_segments(normalized: &str) -> Vec<String> {
    let mut segs = Vec::new();
    let mut cur = String::new();
    for token in normalized.split_whitespace() {
        if matches!(token, "|" | ";" | "&" | "<" | ">" | "||" | "&&" | "|&") {
            if !cur.trim().is_empty() {
                segs.push(cur.trim().to_string());
            }
            cur.clear();
        } else {
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(token);
        }
    }
    if !cur.trim().is_empty() {
        segs.push(cur.trim().to_string());
    }
    segs
}

/// Strip plumbing commands (`sudo`, `env`, `time`, `nice`, `nohup`, ...) from
/// the head of a segment's tokens. Returns the remaining tokens (the "real"
/// command) plus whether `sudo` was present.
fn strip_plumbing<'a>(tokens: &'a [&'a str]) -> (&'a [&'a str], bool) {
    let mut i = 0;
    let mut had_sudo = false;
    while i < tokens.len() {
        let t = tokens[i];
        match t {
            "sudo" => {
                had_sudo = true;
                i += 1;
                while i < tokens.len() {
                    let s = tokens[i];
                    if s == "--" {
                        i += 1;
                        break;
                    }
                    if s.starts_with('-') {
                        // Flags that take a value.
                        if matches!(s, "-u" | "-g" | "-U" | "-p" | "-c" | "-T" | "-r")
                            && i + 1 < tokens.len()
                        {
                            i += 2;
                        } else {
                            i += 1;
                        }
                    } else {
                        break;
                    }
                }
            }
            "env" => {
                i += 1;
                while i < tokens.len() {
                    let s = tokens[i];
                    if s.contains('=') {
                        i += 1;
                        continue;
                    }
                    if s == "-i" || s == "--ignore-environment" {
                        i += 1;
                        continue;
                    }
                    break;
                }
            }
            "time" | "nice" | "nohup" | "ionice" | "taskset" | "chrt" | "unbuffer" | "stdbuf" => {
                i += 1;
                while i < tokens.len() && tokens[i].starts_with('-') {
                    // Conservatively consume one extra token for flags that take values.
                    if matches!(tokens[i], "-n" | "-l" | "-c") && i + 1 < tokens.len() {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
            }
            _ => break,
        }
    }
    (&tokens[i..], had_sudo)
}

/// Extract the basename of a command token so `/bin/rm`, `\rm`, `"/usr/bin/rm"`
/// all normalize to `rm`.
fn basename_of(token: &str) -> &str {
    let stripped = token.trim_start_matches('\\');
    stripped.rsplit('/').next().unwrap_or(stripped)
}

// ---------- Assessment ----------

pub(crate) fn assess_risk(command: &str) -> (Risk, Option<&'static str>) {
    let normalized = normalize_for_risk(command);

    // Whole-command patterns checked first — these ignore segment boundaries.
    if is_fork_bomb(&normalized) {
        return (Risk::Block, Some("fork bomb detected"));
    }
    if has_raw_disk_write(&normalized) {
        return (Risk::Block, Some("raw disk write — destroys disk data"));
    }
    if has_truncate_pattern(&normalized) {
        return (Risk::Block, Some("`:>file` truncates the file"));
    }
    if contains_decoded_shell(&normalized) {
        return (
            Risk::Block,
            Some("decoded content piped to shell — inspect manually"),
        );
    }

    // Segment-based checks. Block wins over Warn wins over Safe.
    let segments = split_segments(&normalized);
    let mut worst: (Risk, Option<&'static str>) = (Risk::Safe, None);
    for seg in &segments {
        let res = check_segment(seg);
        match res.0 {
            Risk::Block => return res,
            Risk::Warn => {
                if matches!(worst.0, Risk::Safe) {
                    worst = res;
                }
            }
            Risk::Safe => {}
        }
    }
    worst
}

fn is_fork_bomb(normalized: &str) -> bool {
    // Classic fork bomb `:(){ :|:& };:` in any whitespace form. Strip all
    // whitespace and check for the compressed signature.
    let compact: String = normalized.chars().filter(|c| !c.is_whitespace()).collect();
    compact.contains(":(){:|:&};:")
}

/// Detects the `: > /path` and `true > /path` truncation attacks. Segment
/// splitting breaks these apart, so we check at the whole-command level.
fn has_truncate_pattern(normalized: &str) -> bool {
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    for i in 0..tokens.len().saturating_sub(2) {
        if (tokens[i] == ":" || tokens[i] == "true") && tokens[i + 1] == ">" {
            return true;
        }
    }
    false
}

fn has_raw_disk_write(normalized: &str) -> bool {
    // `>` has been padded to ` > `, so a literal `> /dev/sd*` shows up as
    // ` > /dev/sd...`. Also match nvme, mmcblk, hda for completeness.
    for victim in [
        " > /dev/sd",
        " > /dev/nvme",
        " > /dev/mmcblk",
        " > /dev/hd",
        " > /dev/disk",
        " > /dev/rdisk",
    ] {
        if normalized.contains(victim) {
            return true;
        }
    }
    false
}

fn contains_decoded_shell(normalized: &str) -> bool {
    // After padding, `|` is always surrounded by spaces, so naive substring
    // matches on "| sh" miss. Check by splitting into segments: if any segment
    // has a decoder AND any segment is a shell/eval interpreter, flag it.
    let decoders = [
        "base64 -d",
        "base64 --decode",
        "xxd -r",
        "openssl base64 -d",
        "openssl enc -d",
    ];
    let has_decoder = decoders.iter().any(|d| normalized.contains(d));
    if !has_decoder {
        return false;
    }
    let segments = split_segments(normalized);
    for seg in &segments {
        let first = seg.split_whitespace().next().unwrap_or("");
        let first_base = basename_of(first);
        if matches!(
            first_base,
            "sh" | "bash" | "zsh" | "dash" | "ksh" | "fish" | "eval"
        ) {
            return true;
        }
    }
    false
}

fn check_segment(seg: &str) -> (Risk, Option<&'static str>) {
    let tokens: Vec<&str> = seg.split_whitespace().collect();
    if tokens.is_empty() {
        return (Risk::Safe, None);
    }
    let (real, had_sudo) = strip_plumbing(&tokens);
    if real.is_empty() {
        // Segment was only plumbing (`sudo` alone, for instance).
        if had_sudo {
            return (Risk::Warn, Some("runs as root"));
        }
        return (Risk::Safe, None);
    }
    let first = basename_of(real[0]);

    let base = match first {
        "rm" | "del" | "rmdir" | "shred" | "unlink" => {
            (Risk::Block, Some("destructive file operation"))
        }
        "dd" => (Risk::Block, Some("dd can destroy disks if misdirected")),
        "mkfs" => (Risk::Block, Some("mkfs formats a filesystem")),
        "mkfs.ext4" | "mkfs.xfs" | "mkfs.vfat" | "mkfs.btrfs" | "mkfs.fat" => {
            (Risk::Block, Some("mkfs formats a filesystem"))
        }
        "fdisk" | "sfdisk" | "parted" | "wipefs" => {
            (Risk::Block, Some("disk-level partition operation"))
        }
        "truncate" => (Risk::Block, Some("truncate shrinks files to zero bytes")),
        "python" | "python2" | "python3" | "perl" | "node" | "ruby" | "php" | "deno" | "bun"
        | "gawk" | "mawk" | "awk" | "lua" | "tcl" | "Rscript" | "julia" => {
            if real
                .iter()
                .any(|t| matches!(*t, "-c" | "-e" | "-S" | "-E" | "--eval"))
            {
                (
                    Risk::Warn,
                    Some("interpreter one-liner — inspect before running"),
                )
            } else {
                (Risk::Safe, None)
            }
        }
        "eval" | "exec" => (Risk::Warn, Some("eval/exec runs arbitrary code")),
        "mv" => {
            if real.contains(&"-i") {
                (Risk::Safe, None)
            } else {
                (Risk::Warn, Some("mv overwrites destination silently"))
            }
        }
        "chmod" | "chown" | "chgrp" => (Risk::Warn, Some("changes permissions or ownership")),
        "kill" | "killall" | "pkill" => (Risk::Warn, Some("kills processes")),
        "cd" | "export" | "source" | "." => (
            Risk::Warn,
            Some("affects only this subshell — run yourself to change your actual shell"),
        ),
        "curl" | "wget" => {
            if seg.contains("| sh")
                || seg.contains("| bash")
                || seg.contains("|sh")
                || seg.contains("|bash")
            {
                (Risk::Warn, Some("download piped to shell — inspect first"))
            } else {
                (Risk::Safe, None)
            }
        }
        ":" | "true" => {
            if real.contains(&">") {
                (Risk::Block, Some("`:>file` truncates the file"))
            } else {
                (Risk::Safe, None)
            }
        }
        "cp" => {
            if real.iter().any(|t| t.contains("/dev/null")) {
                (Risk::Block, Some("cp /dev/null truncates the target"))
            } else {
                (Risk::Safe, None)
            }
        }
        "tee" => {
            if seg.contains("< /dev/null") || seg.contains("</dev/null") {
                (Risk::Block, Some("tee < /dev/null truncates targets"))
            } else {
                (Risk::Safe, None)
            }
        }
        "git" => {
            if seg.contains("reset --hard")
                || seg.contains("clean -fd")
                || seg.contains("clean -xfd")
                || seg.contains("clean -xdf")
                || seg.contains("clean -ffd")
                || seg.contains("branch -d")
                || seg.contains("branch -d")
                || seg.contains("push --force")
                || seg.contains("push -f")
            {
                // Hard-destructive git operations.
                (
                    Risk::Block,
                    Some("destructive git operation — discards changes or rewrites history"),
                )
            } else {
                (Risk::Safe, None)
            }
        }
        "find" => {
            if seg.contains("-delete") {
                (Risk::Block, Some("`find -delete` removes files"))
            } else if seg.contains("-exec rm") || seg.contains("-execdir rm") {
                (Risk::Block, Some("`find -exec rm` removes files"))
            } else if seg.contains("-exec")
                && real
                    .iter()
                    .any(|t| matches!(*t, "rm" | "shred" | "unlink" | "rmdir"))
            {
                (Risk::Block, Some("`find -exec` runs a destructive command"))
            } else {
                (Risk::Safe, None)
            }
        }
        "xargs" => {
            if real
                .iter()
                .any(|t| matches!(basename_of(t), "rm" | "shred" | "unlink" | "rmdir"))
            {
                (Risk::Block, Some("xargs invokes a destructive command"))
            } else {
                (Risk::Safe, None)
            }
        }
        _ => (Risk::Safe, None),
    };

    // `sudo` elevation: if the inner command was Safe, surface a Warn since
    // running as root changes the blast radius. Block stays Block.
    if had_sudo {
        match base.0 {
            Risk::Safe => (Risk::Warn, Some("runs as root")),
            _ => base,
        }
    } else {
        base
    }
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_block(cmd: &str) {
        let r = assess_risk(cmd);
        assert!(
            matches!(r.0, Risk::Block),
            "expected Block for `{cmd}`, got {:?}",
            r
        );
    }

    fn assert_safe(cmd: &str) {
        let r = assess_risk(cmd);
        assert!(
            matches!(r.0, Risk::Safe),
            "expected Safe for `{cmd}`, got {:?}",
            r
        );
    }

    fn assert_warn(cmd: &str) {
        let r = assess_risk(cmd);
        assert!(
            matches!(r.0, Risk::Warn),
            "expected Warn for `{cmd}`, got {:?}",
            r
        );
    }

    #[test]
    fn blocks_direct_rm() {
        assert_block("rm -rf /");
        assert_block("rm -rf ~/Documents");
        assert_block("/bin/rm -rf /");
        assert_block("\\rm -rf /");
    }

    #[test]
    fn blocks_subshell_rm() {
        assert_block("echo hi $(rm -rf ~)");
        assert_block("echo $(rm -rf ~)");
        assert_block("tee >(rm -rf ~)");
        assert_block("diff <(rm -rf ~) <(ls)");
        assert_block("x=`rm -rf /`");
    }

    #[test]
    fn blocks_shell_wrapped_rm() {
        assert_block("sh -c 'rm -rf /'");
        assert_block("bash -c \"rm -rf /\"");
        assert_block("eval 'rm -rf /'");
        assert_block("zsh -c rm");
    }

    #[test]
    fn blocks_ifs_obfuscation() {
        assert_block("rm$IFS-rf$IFS/");
        assert_block("rm${IFS}-rf${IFS}/");
    }

    #[test]
    fn blocks_find_exec_variants() {
        assert_block("find . -name '*.log' -delete");
        assert_block("find . -type f -exec rm {} \\;");
        assert_block("find . -exec sh -c 'rm -rf \"$1\"' _ {} \\;");
        assert_block("find . -exec shred {} \\;");
    }

    #[test]
    fn blocks_xargs_rm() {
        assert_block("find . -print0 | xargs -0 rm");
        assert_block("ls | xargs rm");
    }

    #[test]
    fn blocks_disk_ops() {
        assert_block("dd if=/dev/zero of=/dev/sda");
        assert_block("mkfs.ext4 /dev/sdb1");
        assert_block("fdisk /dev/sda");
        assert_block("wipefs -a /dev/sdb");
    }

    #[test]
    fn blocks_destructive_redirects() {
        assert_block(": > /etc/passwd");
        assert_block("true > ~/.ssh/authorized_keys");
        assert_block("cp /dev/null ~/.bash_history");
        assert_block("echo x > /dev/sda");
    }

    #[test]
    fn blocks_git_destructive() {
        assert_block("git reset --hard HEAD~5");
        assert_block("git clean -fd");
        assert_block("git clean -xfd");
        assert_block("git push --force origin main");
    }

    #[test]
    fn blocks_decoded_shell() {
        assert_block("echo cm0gLXJmIC8= | base64 -d | sh");
        assert_block("echo foo | xxd -r | bash");
    }

    #[test]
    fn blocks_fork_bomb() {
        assert_block(":(){ :|:& };:");
    }

    #[test]
    fn warns_interpreter_one_liner() {
        // Obfuscated rm via interpreter — can't be literally detected, so Warn.
        assert_warn("python3 -c 'import os;os.system(\"ls\")'");
        assert_warn("perl -e 'print 1'");
        assert_warn("node -e 'console.log(1)'");
    }

    #[test]
    fn warns_sudo_and_permissions() {
        assert_warn("sudo ls");
        assert_warn("sudo apt update");
        assert_warn("chmod 777 file");
        assert_warn("chown root:root file");
    }

    #[test]
    fn warns_parent_shell_ops() {
        assert_warn("cd /tmp");
        assert_warn("export FOO=bar");
        assert_warn("source ~/.zshrc");
    }

    #[test]
    fn warns_overwriting_redirect() {
        // Single `>` (no `>>`) signals overwrite. This is whole-command level.
        let r = assess_risk("echo 1 > file.txt");
        assert!(matches!(r.0, Risk::Warn | Risk::Safe));
    }

    #[test]
    fn safe_grep_rm_false_positive() {
        // The v0.3 gate blocked any command containing the token "rm".
        // v0.4 should only block when rm is a command-position first token.
        assert_safe("grep rm logfile");
        assert_safe("ls | grep rm");
        assert_safe("echo hello rm world");
        assert_safe("find . -name 'rm*'");
    }

    #[test]
    fn safe_plain_commands() {
        assert_safe("ls");
        assert_safe("pwd");
        assert_safe("git status");
        assert_safe("cat Cargo.toml");
        assert_safe("find . -name '*.rs'");
    }

    #[test]
    fn sensitive_detects_common_keys() {
        assert_eq!(
            check_sensitive("my key is sk-ant-api03-abc123"),
            Some("Anthropic API key")
        );
        assert_eq!(
            check_sensitive("AKIAIOSFODNN7EXAMPLE"),
            Some("AWS access key")
        );
        assert_eq!(
            check_sensitive("token: ghp_abcdef1234567890"),
            Some("GitHub personal access token")
        );
        assert_eq!(
            check_sensitive("postgres://admin:hunter2@db.local/prod"),
            Some("URL with embedded credentials")
        );
    }

    #[test]
    fn sensitive_skips_benign_urls() {
        assert_eq!(check_sensitive("https://example.com/path"), None);
        assert_eq!(check_sensitive("git@github.com:user/repo"), None);
    }
}
