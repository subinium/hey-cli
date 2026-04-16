use anyhow::{anyhow, Context, Result};
use std::env;
use std::io::Write;
use std::process::{Command, Output, Stdio};

use crate::prompt::SYSTEM_PROMPT;
use crate::sanitize::sanitize_command;

// Anthropic native API — used when ANTHROPIC_API_KEY is set for direct, fast calls.
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const ANTHROPIC_MODEL: &str = "claude-haiku-4-5-20251001";
const MAX_RESPONSE_BYTES: u64 = 1_048_576; // 1 MiB cap on API response

/// Claude backend: prefer direct Anthropic API (~2s) when ANTHROPIC_API_KEY is
/// available, fall back to `claude -p` subprocess (~6s) when it's not.
pub(crate) async fn ask_claude(user_prompt: &str) -> Result<String> {
    if let Ok(key) = env::var("ANTHROPIC_API_KEY") {
        return ask_anthropic_direct(&key, user_prompt).await;
    }
    ask_claude_code(user_prompt)
}

async fn ask_anthropic_direct(api_key: &str, user_prompt: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let body = serde_json::json!({
        "model": ANTHROPIC_MODEL,
        "max_tokens": 256,
        "system": SYSTEM_PROMPT,
        "messages": [{"role": "user", "content": user_prompt}]
    });

    let resp = client
        .post(ANTHROPIC_API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .context("failed to reach Anthropic API")?;

    let status = resp.status();
    if let Some(len) = resp.content_length() {
        if len > MAX_RESPONSE_BYTES {
            return Err(anyhow!(
                "Anthropic response too large ({len} bytes, cap {MAX_RESPONSE_BYTES})"
            ));
        }
    }
    let bytes = resp.bytes().await.context("read Anthropic response")?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(anyhow!(
            "Anthropic response exceeded {MAX_RESPONSE_BYTES} bytes"
        ));
    }

    if !status.is_success() {
        let text = String::from_utf8_lossy(&bytes);
        let safe = crate::sanitize::strip_ansi(&text);
        // 401 / 403 are almost always an invalid or expired API key.
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(anyhow!(
                "Anthropic API rejected the key ({status}). Check ANTHROPIC_API_KEY, or unset it to use `claude login` subprocess mode.\n{}",
                safe.trim()
            ));
        }
        return Err(anyhow!("Anthropic API error {status}: {safe}"));
    }

    let parsed: serde_json::Value =
        serde_json::from_slice(&bytes).context("invalid Anthropic response")?;
    let text = parsed["content"]
        .as_array()
        .and_then(|arr| arr.iter().find(|b| b["type"] == "text"))
        .and_then(|b| b["text"].as_str())
        .ok_or_else(|| anyhow!("no text in Anthropic response"))?
        .to_string();

    Ok(sanitize_command(&text))
}

fn ask_claude_code(user_prompt: &str) -> Result<String> {
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

    if !output.status.success() {
        return Err(classify_claude_error(&output));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(sanitize_command(&stdout))
}

/// Parse `claude -p` stderr/stdout for common failure modes and return a
/// friendlier error. Falls back to the generic exit-status message.
fn classify_claude_error(output: &Output) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}\n{stdout}").to_lowercase();

    if combined.contains("not authenticated")
        || combined.contains("please login")
        || combined.contains("please log in")
        || combined.contains("no api key")
        || combined.contains("anthropic api key")
        || combined.contains("unauthorized")
    {
        return anyhow!(
            "claude is not authenticated — run `claude login`, or set ANTHROPIC_API_KEY to use the direct API"
        );
    }
    if combined.contains("rate limit") || combined.contains("quota") {
        return anyhow!("claude is rate-limited — try `hey codex ...` or `hey openrouter ...`");
    }

    anyhow!("claude exited with {}: {}", output.status, stderr.trim())
}
