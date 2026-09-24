use serde::Deserialize;
use serde_json::json;

use super::GenerateError;
use crate::prompt::{strip_fences, system_prompt, user_prompt, SiteRequest};
use crate::settings::Settings;

const NAME: &str = "Anthropic";

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    content: Vec<Block>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
}

pub async fn generate(
    client: &reqwest::Client,
    settings: &Settings,
    request: &SiteRequest,
) -> Result<String, GenerateError> {
    let endpoint = settings.endpoint.trim_end_matches('/');
    let res = client
        .post(format!("{endpoint}/v1/messages"))
        .header("x-api-key", &settings.api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": settings.model,
            "max_tokens": settings.max_tokens,
            "system": system_prompt(&settings.universe),
            "messages": [{ "role": "user", "content": user_prompt(request) }]
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
        .content
        .into_iter()
        .find(|b| b.kind == "text")
        .and_then(|b| b.text)
        .filter(|t| !t.is_empty())
        .ok_or(GenerateError::NoContent(NAME))?;
    Ok(strip_fences(&content))
}
