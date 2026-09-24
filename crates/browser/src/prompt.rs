//! The "HTTP request" we send to the LLM and how we read its answer.

use serde::{Deserialize, Serialize};

/// A simulated HTTP request: the target plus session context for coherent browsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteRequest {
    pub url: String,
    /// URL of the page the user navigated from (previous visit).
    pub referer: Option<String>,
    /// Recently visited URLs, oldest first.
    pub history: Vec<String>,
}

pub fn system_prompt(universe: &str) -> String {
    format!(
        "You are a web server simulating the entire internet. For each HTTP request you \
         receive, respond with a single, complete, self-contained HTML document that the \
         requested site would plausibly serve. Make pages functional: include working \
         navigation links and forms (search boxes, etc). Honor the query string — a search \
         results page must list plausible results for the query, each result linking to a \
         real-looking URL on the relevant site. Use the Referer and X-Browsing-History \
         headers to keep the session coherent: a page opened from search results should \
         match the result that was clicked. In the <head>, include a favicon for the site \
         as an inline SVG data URI: <link rel=\"icon\" href=\"data:image/svg+xml,...\"> with a \
         small, simple, recognizable logo (URL-encode the SVG; use %22 or %27 for quotes). \
         Respond with ONLY the raw HTML — no explanations, no markdown code fences.\n\n\
         THE UNIVERSE: the internet you simulate exists in the following universe, and \
         every site, brand, person, event and fact must be consistent with it:\n{universe}"
    )
}

/// Render the request as HTTP-style headers so session context travels with it.
pub fn user_prompt(request: &SiteRequest) -> String {
    let mut lines = Vec::new();
    match url::Url::parse(&request.url) {
        Ok(u) if u.host_str().is_some() => {
            let query = u.query().map(|q| format!("?{q}")).unwrap_or_default();
            lines.push(format!("GET {}{} HTTP/1.1", u.path(), query));
            lines.push(format!("Host: {}", u.host_str().unwrap_or_default()));
        }
        _ => lines.push(format!("GET {} HTTP/1.1", request.url)),
    }
    if let Some(referer) = &request.referer {
        lines.push(format!("Referer: {referer}"));
    }
    if !request.history.is_empty() {
        lines.push(format!(
            "X-Browsing-History: {}",
            request.history.join(", ")
        ));
    }
    format!(
        "Generate the webpage for this request:\n\n{}",
        lines.join("\n")
    )
}

/// LLMs often wrap output in ```html fences despite instructions — strip them.
pub fn strip_fences(text: &str) -> String {
    let trimmed = text.trim();

    // Preferred: a fenced code block, possibly preceded by a sentence of prose
    // ("Here is the HTML code for …"). The opening fence must start a line; the
    // closing fence may be anywhere.
    if let Some(after_open) = find_fence_open(trimmed) {
        let close = trimmed[after_open..].rfind("```").map(|r| after_open + r);
        return match close {
            Some(end) => drop_lang(&trimmed[after_open..end]),
            None => drop_lang(&trimmed[after_open..]),
        };
    }

    // Fallback: no fence, but prose before the document itself.
    leading_html(trimmed)
}

/// Byte offset just past the first ``` fence that starts a line.
fn find_fence_open(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if &bytes[i..i + 3] == b"```" && (i == 0 || bytes[i - 1] == b'\n') {
            return Some(i + 3);
        }
        i += 1;
    }
    None
}

/// Drop the language tag on the opening fence line (`html`, or empty).
fn drop_lang(inner: &str) -> String {
    match inner.split_once('\n') {
        Some((tag, body)) if tag.trim().is_empty() || tag.trim().eq_ignore_ascii_case("html") => {
            body.trim().to_string()
        }
        _ => inner.trim().to_string(),
    }
}

/// If the text contains `<!doctype` or `<html`, drop everything before it.
fn leading_html(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let start = ["<!doctype", "<html"]
        .iter()
        .filter_map(|needle| lower.find(needle))
        .min();
    match start {
        Some(pos) => text[pos..].trim().to_string(),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_prompt_reads_like_http() {
        let req = SiteRequest {
            url: "https://www.example.com/search?q=cats".into(),
            referer: Some("https://a.b/".into()),
            history: vec!["https://a.b/".into(), "https://c.d/".into()],
        };
        assert_eq!(
            user_prompt(&req),
            "Generate the webpage for this request:\n\n\
             GET /search?q=cats HTTP/1.1\nHost: www.example.com\nReferer: https://a.b/\n\
             X-Browsing-History: https://a.b/, https://c.d/"
        );
        let bare = SiteRequest {
            url: "nonsense".into(),
            referer: None,
            history: vec![],
        };
        assert!(user_prompt(&bare).ends_with("GET nonsense HTTP/1.1"));
    }

    #[test]
    fn strips_fences() {
        assert_eq!(strip_fences("```html\n<p>x</p>\n```"), "<p>x</p>");
        assert_eq!(strip_fences("```\n<p>x</p>\n```\n"), "<p>x</p>");
        assert_eq!(strip_fences("  <p>x</p> "), "<p>x</p>");
        assert_eq!(strip_fences("```html<p>y</p>```"), "html<p>y</p>");

        // Prose before the fence is discarded, not shown to the user.
        let prose = "Here is the HTML code for a Warhammer 40K-themed image search results page.\n\n```html\n<!doctype html><html><body>hi</body></html>\n```";
        assert_eq!(
            strip_fences(prose),
            "<!doctype html><html><body>hi</body></html>"
        );

        // Prose before a bare document (no fence) is discarded too.
        assert_eq!(
            strip_fences("Sure, here you go:\n<!doctype html><html></html>"),
            "<!doctype html><html></html>"
        );

        // Unclosed fence still drops the opening fence and language tag.
        assert_eq!(
            strip_fences("Intro text.\n```html\n<p>partial"),
            "<p>partial"
        );
    }

    #[test]
    fn system_prompt_embeds_universe() {
        assert!(system_prompt("Mars 2090").ends_with("consistent with it:\nMars 2090"));
    }
}
