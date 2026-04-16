use anyhow::{anyhow, Context, Result};
use std::env;
use std::process::{Command, Stdio};

use crate::prompt::SYSTEM_PROMPT;
use crate::sanitize::sanitize_command;

pub(crate) fn ask_codex(user_prompt: &str) -> Result<String> {
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
