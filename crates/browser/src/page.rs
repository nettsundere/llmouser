//! Turning generated HTML into something safe to display, plus the chrome's
//! own pages (start page, error page) and tab-icon helpers.

use crate::i18n::{fill, Messages};

/// In-document network lockdown for generated pages: no remote subresources, no
/// fetch/XHR/WebSocket. Inline scripts/styles and data: URIs stay usable, so the
/// pages remain interactive and can embed their own images/fonts. Second layer on
/// top of the shell's webview-level request blocking.
pub const SITE_CSP: &str =
    "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
    img-src data:; font-src data:; media-src data:; frame-src about: data:";

/// Scheme used by the chrome's own pages to talk to the shell:
/// `llmouser://retry`, `llmouser://settings`.
pub const CHROME_SCHEME: &str = "llmouser";

pub fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn find_tag_end(html: &str, tag: &str) -> Option<usize> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find(&format!("<{tag}")) {
        let start = from + pos;
        let after = start + 1 + tag.len();
        // `<head>` or `<head lang=...>`, not `<header>`.
        let next = lower.as_bytes().get(after).copied();
        if matches!(
            next,
            Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r')
        ) {
            return lower[after..].find('>').map(|end| after + end + 1);
        }
        from = after;
    }
    None
}

/// Inject the CSP meta (and, when the host webview cannot take a base URL of
/// its own, a `<base>`) ahead of any generated content.
pub fn prepare_document(html: &str, base_url: Option<&str>) -> String {
    let mut tag = format!("<meta http-equiv=\"Content-Security-Policy\" content=\"{SITE_CSP}\">");
    if let Some(base) = base_url {
        tag.push_str(&format!("<base href=\"{}\">", base.replace('"', "&quot;")));
    }
    if let Some(end) = find_tag_end(html, "head") {
        return format!("{}{}{}", &html[..end], tag, &html[end..]);
    }
    if let Some(end) = find_tag_end(html, "html") {
        return format!("{}<head>{}</head>{}", &html[..end], tag, &html[end..]);
    }
    format!("{tag}{html}")
}

/// `<title>` text of the generated page, if any.
pub fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open_end = start + lower[start..].find('>')?;
    let close = open_end + lower[open_end..].find("</title>")?;
    let title = html[open_end + 1..close].trim();
    (!title.is_empty()).then(|| decode_basic_entities(title))
}

fn decode_basic_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Pull the LLM-generated SVG favicon (data URI) out of the page's `<head>`.
pub fn extract_icon(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find("<link") {
        let start = from + pos;
        let end = start + lower[start..].find('>')?;
        let tag = &html[start..=end];
        let tag_lower = &lower[start..=end];
        from = end + 1;
        if !tag_lower.contains("icon") {
            continue;
        }
        let Some(href_pos) = tag_lower.find("href") else {
            continue;
        };
        let rest = &tag[href_pos + 4..];
        let rest = rest
            .trim_start()
            .strip_prefix('=')
            .unwrap_or(rest)
            .trim_start();
        let value = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => rest[1..].split(q).next().unwrap_or(""),
            _ => rest.split([' ', '>']).next().unwrap_or(""),
        };
        if value.starts_with("data:image/svg+xml") {
            return Some(value.to_string());
        }
    }
    None
}

/// Host part of a URL, without `www.`, or the raw text when not a URL.
pub fn display_host(url: &str) -> String {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string());
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}

/// Fallback tab icon when the generated page has no usable favicon: a colored
/// tile with the site's first letter. Returns an SVG document.
pub fn letter_icon_svg(url: &str) -> String {
    let cleaned = display_host(url);
    let first = cleaned
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('?');
    let letter = if first.is_ascii_alphanumeric() {
        first
    } else {
        '?'
    };
    let mut hash: i32 = 0;
    for c in cleaned.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(c as i32);
    }
    let hue = hash.unsigned_abs() % 360;
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\">\
         <rect width=\"16\" height=\"16\" rx=\"3\" fill=\"hsl({hue},60%,45%)\"/>\
         <text x=\"8\" y=\"12\" font-family=\"system-ui,sans-serif\" font-size=\"10\" \
         font-weight=\"bold\" fill=\"#fff\" text-anchor=\"middle\">{letter}</text></svg>"
    )
}

/// Decode an SVG `data:` URI (the only icon form generated pages may use) into
/// the SVG source. Returns `None` for anything else.
pub fn svg_from_data_uri(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("data:image/svg+xml")?;
    let (meta, payload) = rest.split_once(',')?;
    if meta.contains("base64") {
        let bytes = base64_decode(payload.trim())?;
        String::from_utf8(bytes).ok()
    } else {
        Some(percent_decode(payload))
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &c in text.as_bytes() {
        if c == b'=' || c == b'\n' || c == b'\r' || c == b' ' {
            continue;
        }
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

const CHROME_PAGE_STYLE: &str = "\
  :root { color-scheme: light dark; }\
  body { margin: 0; min-height: 100vh; display: flex; align-items: center; justify-content: center;\
    font-family: system-ui, -apple-system, sans-serif; background: #ffffff; color: #1d1d1f; }\
  @media (prefers-color-scheme: dark) { body { background: #1e1e1e; color: #f2f2f2; } }\
  main { max-width: 480px; padding: 32px; text-align: center; }\
  h1 { font-size: 20px; font-weight: 600; margin: 0 0 8px; }\
  p { margin: 0 0 16px; font-size: 14px; color: #6e6e73; line-height: 1.4; overflow-wrap: anywhere; }\
  @media (prefers-color-scheme: dark) { p { color: #a0a0a5; } }\
  pre { white-space: pre-wrap; text-align: left; font-size: 12px; padding: 10px 12px; border-radius: 8px;\
    background: rgba(127,127,127,0.12); max-height: 200px; overflow: auto; }\
  a.button { display: inline-block; margin: 4px; padding: 7px 16px; border-radius: 8px; font-size: 14px;\
    text-decoration: none; color: #fff; background: #007aff; }\
  a.button.secondary { background: rgba(127,127,127,0.18); color: inherit; }";

fn chrome_page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{SITE_CSP}\">\
         <title>{}</title><style>{CHROME_PAGE_STYLE}</style></head>\
         <body><main>{body}</main></body></html>",
        escape_html(title)
    )
}

/// What an empty tab shows: nothing, like a browser. The body carries a marker
/// so tests can tell the blank page from a page that has not loaded yet.
pub fn start_page() -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{SITE_CSP}\">\
         <style>{CHROME_PAGE_STYLE}</style></head><body data-llmouser=\"start\"></body></html>"
    )
}

/// Failed generation: what went wrong, in words, plus a way out.
pub fn error_page(m: &Messages, url: &str, error: &str, offer_settings: bool) -> String {
    let title = fill(m.error_title, &[("url", url)]);
    let mut body = format!(
        "<h1 id=\"error-title\">{}</h1><pre id=\"error-detail\">{}</pre>\
         <a class=\"button\" id=\"retry\" href=\"{CHROME_SCHEME}://retry\">{}</a>",
        escape_html(&title),
        escape_html(error),
        escape_html(m.retry)
    );
    if offer_settings {
        body.push_str(&format!(
            "<a class=\"button secondary\" id=\"open-settings\" href=\"{CHROME_SCHEME}://settings\">{}</a>",
            escape_html(m.open_settings)
        ));
    }
    chrome_page(&title, &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[test]
    fn injects_after_head_or_creates_one() {
        let out = prepare_document(
            "<html><head><title>x</title></head></html>",
            Some("https://a/"),
        );
        assert!(out.starts_with("<html><head><meta http-equiv=\"Content-Security-Policy\""));
        assert!(out.contains("<base href=\"https://a/\"><title>x</title>"));

        let out = prepare_document("<HTML lang=en><body>hi</body></HTML>", None);
        assert!(out.starts_with("<HTML lang=en><head><meta"));
        assert!(!out.contains("<base"));

        let out = prepare_document("<header>x</header>plain", None);
        assert!(out.starts_with("<meta http-equiv"));
        assert!(out.ends_with("<header>x</header>plain"));

        let out = prepare_document("<html><head>", Some("https://q\"uote/"));
        assert!(out.contains("href=\"https://q&quot;uote/\""));
    }

    #[test]
    fn extracts_title_and_icon() {
        let html = "<html><head><TITLE lang=x> A &amp; B </TITLE>\
            <link rel=\"stylesheet\" href=\"x.css\">\
            <link rel='icon' href='data:image/svg+xml,%3Csvg/%3E'></head></html>";
        assert_eq!(extract_title(html).as_deref(), Some("A & B"));
        assert_eq!(
            extract_icon(html).as_deref(),
            Some("data:image/svg+xml,%3Csvg/%3E")
        );
        assert_eq!(extract_title("<title></title>"), None);
        assert_eq!(
            extract_icon("<link rel=icon href=https://x/favicon.ico>"),
            None
        );
        assert_eq!(
            extract_icon("<link rel=icon href=data:image/svg+xml;utf8,<svg/>>"),
            Some("data:image/svg+xml;utf8,<svg/".into())
        );
    }

    #[test]
    fn letter_icon_is_deterministic() {
        let a = letter_icon_svg("https://www.example.com/x");
        assert!(a.contains(">E</text>"));
        assert_eq!(a, letter_icon_svg("https://example.com"));
        assert!(letter_icon_svg("https://-dash.org").contains(">?</text>"));
        assert!(letter_icon_svg("").contains(">?</text>"));
        assert_eq!(display_host("not a url"), "not a url");
    }

    #[test]
    fn decodes_svg_data_uris() {
        assert_eq!(
            svg_from_data_uri("data:image/svg+xml,%3Csvg%20a=%22b%22/%3E").as_deref(),
            Some("<svg a=\"b\"/>")
        );
        assert_eq!(
            svg_from_data_uri("data:image/svg+xml;base64,PHN2Zy8+").as_deref(),
            Some("<svg/>")
        );
        assert_eq!(svg_from_data_uri("data:image/png;base64,AAAA"), None);
    }

    #[test]
    fn chrome_pages_link_to_shell_actions() {
        let m = Language::En.messages();
        let start = start_page();
        assert!(start.contains("data-llmouser=\"start\""));
        assert!(!start.contains("llmouser://"));

        let err = error_page(m, "https://x", "<boom>", true);
        assert!(err.contains("Couldn&#39;t load https://x"));
        assert!(err.contains("&lt;boom&gt;"));
        assert!(err.contains("llmouser://retry"));
        assert!(err.contains("llmouser://settings"));
        assert!(!error_page(m, "https://x", "e", false).contains("llmouser://settings"));
    }
}
