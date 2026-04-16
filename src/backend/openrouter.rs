use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::prompt::SYSTEM_PROMPT;
use crate::sanitize::sanitize_command;

pub(crate) const DEFAULT_MODEL: &str = "anthropic/claude-haiku-4.5";
const API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

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
