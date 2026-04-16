use anyhow::{anyhow, Context, Result};
use std::env;
use std::io::Write;
use std::process::{Command, Stdio};

use crate::prompt::SYSTEM_PROMPT;
use crate::sanitize::sanitize_command;

// Anthropic native API — used when ANTHROPIC_API_KEY is set for direct, fast calls.
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const ANTHROPIC_MODEL: &str = "claude-haiku-4-5-20251001";

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

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Anthropic API error {status}: {text}"));
    }

    let parsed: serde_json::Value = resp.json().await.context("invalid Anthropic response")?;
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
    super::post_cli_backend_output("claude", output)
}
