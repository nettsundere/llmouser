//! Deterministic offline provider used by the E2E suite (`LLMOUSER_MOCK=1`).
//!
//! Echoes the URL, query params, referer and universe, and includes a search
//! form, navigation links and JS-navigation buttons to exercise every nav path.

use std::time::Duration;

use super::GenerateError;
use crate::page::escape_html;
use crate::prompt::SiteRequest;
use crate::settings::Settings;

/// Behaviours a test can trigger through the requested URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockScenario {
    /// Plain page.
    Normal,
    /// URL contains `throw-error`: the request fails.
    Error,
    /// URL contains `unauthorized`: the request fails like a bad API key.
    Unauthorized,
    /// URL contains `slow`: the page arrives after a delay so tests can cancel.
    Slow,
}

impl MockScenario {
    pub fn for_url(url: &str) -> MockScenario {
        if url.contains("throw-error") {
            MockScenario::Error
        } else if url.contains("unauthorized") {
            MockScenario::Unauthorized
        } else if url.contains("slow") {
            MockScenario::Slow
        } else {
            MockScenario::Normal
        }
    }
}

/// How long a `slow` page takes.
pub const SLOW_DELAY: Duration = Duration::from_secs(3);

pub async fn generate(settings: &Settings, request: &SiteRequest) -> Result<String, GenerateError> {
    match MockScenario::for_url(&request.url) {
        MockScenario::Error => {
            return Err(GenerateError::Other("Mock provider forced error".into()))
        }
        MockScenario::Unauthorized => {
            return Err(GenerateError::Http {
                provider: "Mock",
                status: 401,
                body: "bad key".into(),
            })
        }
        MockScenario::Slow => tokio::time::sleep(SLOW_DELAY).await,
        MockScenario::Normal => {}
    }
    Ok(mock_site(settings, request))
}

/// The page itself (synchronous; also handy for unit tests).
pub fn mock_site(settings: &Settings, request: &SiteRequest) -> String {
    let safe_url = escape_html(&request.url);
    let mut params = String::new();
    let mut favicon = String::new();
    if let Ok(url) = url::Url::parse(&request.url) {
        for (k, v) in url.query_pairs() {
            params.push_str(&format!(
                "<li data-param=\"{}\">{}={}</li>",
                escape_html(&k),
                escape_html(&k),
                escape_html(&v)
            ));
        }
        // "noicon" hosts omit the favicon so tests can exercise the fallback icon.
        if !url.host_str().unwrap_or_default().contains("noicon") {
            favicon = "<link rel=\"icon\" href=\"data:image/svg+xml,\
                %3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20viewBox=%220%200%2016%2016%22%3E\
                %3Crect%20width=%2216%22%20height=%2216%22%20rx=%223%22%20fill=%22%234a90d9%22/%3E\
                %3C/svg%3E\">"
                .to_string();
        }
    }
    let referer = request.referer.clone().unwrap_or_else(|| "none".into());
    format!(
        "<!doctype html>
<html lang=\"en\">
<head><meta charset=\"utf-8\"><title>Mock: {safe_url}</title>{favicon}</head>
<body>
  <h1 id=\"mock-heading\">Mock page for {safe_url}</h1>
  <p id=\"mock-url\">{safe_url}</p>
  <p id=\"mock-provider\">{provider}</p>
  <p id=\"mock-referer\">{referer}</p>
  <p id=\"mock-universe\">{universe}</p>
  <p id=\"mock-history\">{history}</p>
  <ul id=\"mock-params\">{params}</ul>
  <form id=\"mock-search-form\" action=\"/search\">
    <input id=\"mock-search-input\" name=\"q\" type=\"text\" />
    <button id=\"mock-search-submit\" type=\"submit\">Search</button>
  </form>
  <nav>
    <a id=\"mock-link-relative\" href=\"/about\">About</a>
    <a id=\"mock-link-absolute\" href=\"https://other.example/page\">Other site</a>
    <a id=\"mock-link-hash\" href=\"#section\">Jump</a>
    <a id=\"mock-link-blank\" href=\"/popup\" target=\"_blank\">Popup</a>
  </nav>
  <button id=\"mock-js-nav\" onclick=\"location.href='/js-nav'\">JS nav</button>
  <button id=\"mock-js-nav-absolute\" onclick=\"location.href='https://real.example/leak'\">JS nav absolute</button>
  <img id=\"mock-ext-img\" src=\"https://cdn.example/logo.png\" alt=\"\"
    onload=\"this.setAttribute('data-net','loaded')\"
    onerror=\"this.setAttribute('data-net','blocked')\" />
  <div id=\"mock-fetch-result\">pending</div>
  <div id=\"section\" style=\"margin-top:2000px\">Section</div>
  <textarea id=\"mock-textarea\">editable text</textarea>
  <script>
    fetch('https://api.example/data')
      .then(function () {{ document.getElementById('mock-fetch-result').textContent = 'fetch-ok' }})
      .catch(function () {{ document.getElementById('mock-fetch-result').textContent = 'fetch-blocked' }})
  </script>
</body>
</html>",
        provider = escape_html(settings.provider.id()),
        referer = escape_html(&referer),
        universe = escape_html(&settings.universe),
        history = escape_html(&request.history.join(", ")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[test]
    fn scenarios_come_from_the_url() {
        assert_eq!(
            MockScenario::for_url("https://a/throw-error"),
            MockScenario::Error
        );
        assert_eq!(
            MockScenario::for_url("https://slow.example"),
            MockScenario::Slow
        );
        assert_eq!(
            MockScenario::for_url("https://x/unauthorized"),
            MockScenario::Unauthorized
        );
        assert_eq!(MockScenario::for_url("https://a"), MockScenario::Normal);
    }

    #[tokio::test]
    async fn mock_pages_echo_context_and_fail_on_demand() {
        let settings = Settings::defaults_for(Language::En);
        let req = SiteRequest {
            url: "https://noicon.example/p?q=<x>&b=2".into(),
            referer: Some("https://r/".into()),
            history: vec!["https://r/".into()],
        };
        let html = generate(&settings, &req).await.unwrap();
        assert!(html.contains("<li data-param=\"q\">q=&lt;x&gt;</li><li data-param=\"b\">b=2</li>"));
        assert!(html.contains("<p id=\"mock-referer\">https://r/</p>"));
        assert!(!html.contains("rel=\"icon\""));
        assert!(mock_site(
            &settings,
            &SiteRequest {
                url: "https://a".into(),
                referer: None,
                history: vec![]
            }
        )
        .contains("rel=\"icon\""));

        let err = generate(
            &settings,
            &SiteRequest {
                url: "https://a/throw-error".into(),
                referer: None,
                history: vec![],
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.to_string(), "Mock provider forced error");
        let err = generate(
            &settings,
            &SiteRequest {
                url: "https://a/unauthorized".into(),
                referer: None,
                history: vec![],
            },
        )
        .await
        .unwrap_err();
        assert!(err.is_configuration());
    }
}
