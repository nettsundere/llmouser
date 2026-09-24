//! Address-bar input handling.

/// Placeholder in a search template replaced by the URL-encoded query.
pub const QUERY_PLACEHOLDER: &str = "{query}";

/// Default search template used when the typed text is not an address.
pub const DEFAULT_SEARCH_URL: &str = "https://www.google.com/search?q={query}";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AddressError {
    #[error("Address is empty")]
    Empty,
}

fn has_scheme(input: &str) -> bool {
    let mut chars = input.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    let rest: String = chars.collect();
    if let Some(idx) = rest.find("://") {
        rest[..idx]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
    } else {
        false
    }
}

/// Normalize a user-typed address into an absolute URL: an explicit scheme is
/// kept, anything else gets `https://` in front (this never searches).
pub fn normalize_url(input: &str) -> Result<String, AddressError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AddressError::Empty);
    }
    let raw = if has_scheme(trimmed) {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    // Canonical form (adds the root slash, lowercases the host) so the same page
    // always has the same URL, whichever way it was typed or linked.
    Ok(url::Url::parse(&raw).map(|u| u.to_string()).unwrap_or(raw))
}

/// True when the text reads like an address rather than a search query: it has a
/// scheme, or it is a single token containing a dot (or `localhost`).
pub fn looks_like_address(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }
    if has_scheme(trimmed) {
        return true;
    }
    if trimmed.chars().any(char::is_whitespace) {
        return false;
    }
    let host = trimmed.split(['/', '?', '#']).next().unwrap_or("");
    host == "localhost" || host.contains('.')
}

fn encode_query(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Turn address-bar text into the URL to generate: addresses are normalized,
/// everything else becomes a search on `search_template`.
pub fn resolve_input(input: &str, search_template: &str) -> Result<String, AddressError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AddressError::Empty);
    }
    if looks_like_address(trimmed) {
        return normalize_url(trimmed);
    }
    let template = if search_template.contains(QUERY_PLACEHOLDER) {
        search_template
    } else {
        DEFAULT_SEARCH_URL
    };
    Ok(template.replace(QUERY_PLACEHOLDER, &encode_query(trimmed)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_bare_hosts() {
        assert_eq!(
            normalize_url(" example.com ").unwrap(),
            "https://example.com/"
        );
        assert_eq!(normalize_url("HTTP://A.b/c").unwrap(), "http://a.b/c");
        assert_eq!(normalize_url("ftp://x").unwrap(), "ftp://x/");
        assert_eq!(
            normalize_url("not a url at all").unwrap(),
            "https://not a url at all"
        );
        assert_eq!(normalize_url("  "), Err(AddressError::Empty));
    }

    #[test]
    fn detects_addresses_versus_queries() {
        assert!(looks_like_address("example.com"));
        assert!(looks_like_address("example.com/path?q=1"));
        assert!(looks_like_address("https://anything at all"));
        assert!(looks_like_address("localhost"));
        assert!(!looks_like_address("cats"));
        assert!(!looks_like_address("best cat food"));
        assert!(!looks_like_address(""));
    }

    #[test]
    fn resolves_queries_through_the_template() {
        assert_eq!(
            resolve_input("best cat food", DEFAULT_SEARCH_URL).unwrap(),
            "https://www.google.com/search?q=best+cat+food"
        );
        assert_eq!(
            resolve_input("привет", "https://ya.ru/search?text={query}").unwrap(),
            "https://ya.ru/search?text=%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82"
        );
        // A template without the placeholder falls back to the default one.
        assert_eq!(
            resolve_input("x", "https://broken").unwrap(),
            "https://www.google.com/search?q=x"
        );
        assert_eq!(
            resolve_input("example.com", DEFAULT_SEARCH_URL).unwrap(),
            "https://example.com/"
        );
    }
}
