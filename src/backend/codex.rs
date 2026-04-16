use anyhow::{anyhow, Context, Result};
use std::io::Write as _;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

use crate::prompt::SYSTEM_PROMPT;
use crate::sanitize::sanitize_command;

// Cap on the output file size so a rogue codex run can't OOM us.
const MAX_OUTPUT_BYTES: u64 = 1_048_576; // 1 MiB

pub(crate) fn ask_codex(user_prompt: &str) -> Result<String> {
    // Use `tempfile::NamedTempFile` so the output file is created with
    // O_CREAT|O_EXCL and mode 0600 — prevents symlink pre-creation attacks
    // and makes the file unreadable to other local users. The file is auto-
    // deleted when the `NamedTempFile` guard is dropped (including on panic
    // and normal returns), but we drop cleanup is best-effort on SIGKILL.
    let tmp = NamedTempFile::new().context("failed to create secure temp file for codex output")?;

    let full_prompt = format!("{SYSTEM_PROMPT}\n\n---\n\n{user_prompt}");

    // Pass the prompt via stdin ("-" tells codex exec to read PROMPT from stdin).
    // This keeps the prompt text out of /proc/<pid>/cmdline and `ps` output.
    let mut child = Command::new("codex")
        .arg("exec")
        .arg("--skip-git-repo-check")
        .arg("-o")
        .arg(tmp.path())
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn `codex` (is Codex CLI installed?)")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(full_prompt.as_bytes())
            .context("failed to write prompt to codex stdin")?;
    }

    let output = child
        .wait_with_output()
        .context("failed to wait on codex")?;

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();

    // Bounded read from the output file.
    let content = read_bounded(tmp.path(), MAX_OUTPUT_BYTES);

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
    if haystack.contains("not authenticated") || haystack.contains("not logged in") {
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

fn read_bounded(path: &std::path::Path, cap: u64) -> String {
    use std::io::Read as _;
    match std::fs::File::open(path) {
        Ok(f) => {
            let mut reader = f.take(cap);
            let mut buf = String::new();
            let _ = reader.read_to_string(&mut buf);
            buf
        }
        Err(_) => String::new(),
    }
}
