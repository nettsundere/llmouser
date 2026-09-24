use serde::Deserialize;
use serde_json::json;

use super::GenerateError;
use crate::prompt::{strip_fences, system_prompt, user_prompt, SiteRequest};
use crate::settings::Settings;

const NAME: &str = "OpenAI";

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    content: Option<String>,
}

pub async fn generate(
    client: &reqwest::Client,
    settings: &Settings,
    request: &SiteRequest,
) -> Result<String, GenerateError> {
    let endpoint = settings.endpoint.trim_end_matches('/');
    let res = client
        .post(format!("{endpoint}/chat/completions"))
        .bearer_auth(&settings.api_key)
        .json(&json!({
            "model": settings.model,
            "max_tokens": settings.max_tokens,
            "messages": [
                { "role": "system", "content": system_prompt(&settings.universe) },
                { "role": "user", "content": user_prompt(request) }
            ]
        }))
        .send()
        .await
        .map_err(|e| GenerateError::Network(e.to_string()))?;

    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(GenerateError::Http {
            provider: NAME,
            status: status.as_u16(),
            body,
        });
    }
    let data: Response = res
        .json()
        .await
        .map_err(|e| GenerateError::Other(e.to_string()))?;
    let content = data
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message)
        .and_then(|m| m.content)
        .filter(|c| !c.is_empty())
        .ok_or(GenerateError::NoContent(NAME))?;
    Ok(strip_fences(&content))
}
