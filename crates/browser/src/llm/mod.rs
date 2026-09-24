//! LLM providers playing the role of the internet.

mod anthropic;
mod mock;
mod openai;

use crate::prompt::SiteRequest;
use crate::settings::{Provider, Settings};

pub use mock::{mock_site, MockScenario};

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum GenerateError {
    #[error("{0}")]
    Network(String),
    #[error("{provider} request failed ({status}): {body}")]
    Http {
        provider: &'static str,
        status: u16,
        body: String,
    },
    #[error("{0} response contained no content")]
    NoContent(&'static str),
    #[error("{0}")]
    Other(String),
}

impl GenerateError {
    /// True when the user's credentials or configuration are the likely cause
    /// (the error page then offers to open Settings).
    pub fn is_configuration(&self) -> bool {
        matches!(
            self,
            GenerateError::Http {
                status: 401 | 403,
                ..
            }
        )
    }
}

/// Generate a complete HTML document for the request with the configured provider.
pub async fn generate(
    client: &reqwest::Client,
    settings: &Settings,
    request: &SiteRequest,
    mock: bool,
) -> Result<String, GenerateError> {
    if mock {
        return mock::generate(settings, request).await;
    }
    match settings.provider {
        Provider::OpenAi => openai::generate(client, settings, request).await,
        Provider::Anthropic => anthropic::generate(client, settings, request).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// One-shot HTTP server: captures the request, answers with `response`.
    fn serve_once(response: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = stream.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let len: usize = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap())
                        })
                        .unwrap_or(0);
                    if body.len() >= len || n == 0 {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&buf).to_string()
        });
        (addr, handle)
    }

    fn settings(provider: Provider, endpoint: &str) -> Settings {
        let mut s = Settings::defaults_for(Language::En);
        s.provider = provider;
        s.endpoint = endpoint.to_string();
        s.api_key = "sk-test".into();
        s.model = "m1".into();
        s.max_tokens = 123;
        s.universe = "U".into();
        s
    }

    fn request() -> SiteRequest {
        SiteRequest {
            url: "https://example.com/".into(),
            referer: None,
            history: vec![],
        }
    }

    #[tokio::test]
    async fn openai_request_and_response() {
        let (addr, server) = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 71\r\nConnection: close\r\n\r\n\
             {\"choices\":[{\"message\":{\"content\":\"```html\\n<h1>hi</h1>\\n```\"}}]}       ",
        );
        let html = generate(
            &reqwest::Client::new(),
            &settings(Provider::OpenAi, &addr),
            &request(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(html, "<h1>hi</h1>");
        let captured = server.join().unwrap();
        assert!(captured.starts_with("POST /chat/completions HTTP/1.1"));
        assert!(captured.contains("authorization: Bearer sk-test"));
        let body: serde_json::Value =
            serde_json::from_str(captured.split("\r\n\r\n").nth(1).unwrap().trim()).unwrap();
        assert_eq!(body["model"], "m1");
        assert_eq!(body["max_tokens"], 123);
        assert_eq!(body["messages"][0]["role"], "system");
        assert!(body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .ends_with("consistent with it:\nU"));
        assert!(body["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("Host: example.com"));
    }

    #[tokio::test]
    async fn anthropic_request_and_response() {
        let (addr, server) = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 74\r\nConnection: close\r\n\r\n\
             {\"content\":[{\"type\":\"thinking\"},{\"type\":\"text\",\"text\":\"<p>ok</p>\"}]}      ",
        );
        let html = generate(
            &reqwest::Client::new(),
            &settings(Provider::Anthropic, &addr),
            &request(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(html, "<p>ok</p>");
        let captured = server.join().unwrap();
        assert!(captured.starts_with("POST /v1/messages HTTP/1.1"));
        assert!(captured.contains("x-api-key: sk-test"));
        assert!(captured.contains("anthropic-version: 2023-06-01"));
        let body: serde_json::Value =
            serde_json::from_str(captured.split("\r\n\r\n").nth(1).unwrap().trim()).unwrap();
        assert_eq!(body["max_tokens"], 123);
        assert!(body["system"].as_str().unwrap().contains("THE UNIVERSE"));
        assert_eq!(body["messages"][0]["role"], "user");
    }

    #[tokio::test]
    async fn http_errors_surface_status_and_body() {
        let (addr, _server) = serve_once(
            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 11\r\nConnection: close\r\n\r\ninvalid key",
        );
        let err = generate(
            &reqwest::Client::new(),
            &settings(Provider::OpenAi, &addr),
            &request(),
            false,
        )
        .await
        .unwrap_err();
        assert_eq!(
            err,
            GenerateError::Http {
                provider: "OpenAI",
                status: 401,
                body: "invalid key".into()
            }
        );
        assert!(err.is_configuration());
        assert_eq!(err.to_string(), "OpenAI request failed (401): invalid key");
    }

    #[tokio::test]
    async fn empty_content_is_an_error() {
        let (addr, _server) = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 14\r\nConnection: close\r\n\r\n{\"choices\":[]}",
        );
        let err = generate(
            &reqwest::Client::new(),
            &settings(Provider::OpenAi, &addr),
            &request(),
            false,
        )
        .await
        .unwrap_err();
        assert_eq!(err, GenerateError::NoContent("OpenAI"));
    }

    #[tokio::test]
    async fn unreachable_endpoint_is_a_network_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let err = generate(
            &reqwest::Client::new(),
            &settings(Provider::Anthropic, &addr),
            &request(),
            false,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, GenerateError::Network(_)));
        assert!(!err.is_configuration());
    }
}
