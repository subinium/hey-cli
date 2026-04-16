use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::prompt::SYSTEM_PROMPT;
use crate::sanitize::sanitize_command;

pub(crate) const DEFAULT_MODEL: &str = "anthropic/claude-haiku-4.5";
const API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
// Cap on the response body so a compromised or rogue endpoint can't exhaust
// memory. 1 MiB is ~4000× the size of a typical 256-token completion.
const MAX_RESPONSE_BYTES: u64 = 1_048_576;

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

pub(crate) async fn ask_openrouter(
    api_key: &str,
    model: &str,
    user_prompt: &str,
) -> Result<String> {
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

    let status = resp.status();
    // Refuse oversized bodies up front via Content-Length (best-effort).
    if let Some(len) = resp.content_length() {
        if len > MAX_RESPONSE_BYTES {
            return Err(anyhow!(
                "OpenRouter response too large ({len} bytes, cap {MAX_RESPONSE_BYTES})"
            ));
        }
    }

    let bytes = resp.bytes().await.context("read response body")?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(anyhow!(
            "OpenRouter response exceeded {MAX_RESPONSE_BYTES} bytes"
        ));
    }

    if !status.is_success() {
        let text = String::from_utf8_lossy(&bytes);
        return Err(anyhow!(
            "API error {status}: {}",
            crate::sanitize::strip_ansi(&text)
        ));
    }

    let parsed: ChatResponse = serde_json::from_slice(&bytes).context("invalid API response")?;

    let text = parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| anyhow!("no choices in API response"))?;

    Ok(sanitize_command(&text))
}
