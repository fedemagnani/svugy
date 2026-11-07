//! Sanitise the links svugy_model's `Link::resolve` leaves as URLs into safe URLs.

/// List the schemes a link may use, lowercase.
const ALLOWED_SCHEMES: [&str; 3] = ["http", "https", "mailto"];
/// List the delimiters whose first occurrence tells whether a `:` ends a scheme.
const SCHEME_SEARCH_DELIMITERS: [char; 4] = [':', '/', '?', '#'];

/// Holds a `Link::Url` that passed the URL policy: an http, https or mailto URL, or a relative path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SafeUrl {
    value: String,
}

impl SafeUrl {
    /// Parse a `Link::Url` value into a safe URL: ASCII whitespace and controls stripped, an allowed scheme or a relative path accepted.
    pub(crate) fn parse(raw: &str) -> Option<SafeUrl> {
        let value: String = raw.chars().filter(Self::is_kept_in_url).collect();
        let is_blank = value.is_empty();
        if is_blank {
            return None;
        }
        let has_backslash = value.contains('\\');
        if has_backslash {
            return None;
        }
        let is_root_relative = value.starts_with('/');
        if is_root_relative {
            return None;
        }
        let Some(scheme) = Self::scheme_of(&value) else {
            let relative_url = SafeUrl { value };
            return Some(relative_url);
        };
        let lowercase_scheme = scheme.to_ascii_lowercase();
        let lowercase_scheme_name = lowercase_scheme.as_str();
        let is_allowed_scheme = ALLOWED_SCHEMES.contains(&lowercase_scheme_name);
        if is_allowed_scheme {
            let schemed_url = SafeUrl { value };
            return Some(schemed_url);
        }
        None
    }

    /// Keep every character but ASCII whitespace and controls: the printable ASCII graphics and everything beyond ASCII.
    fn is_kept_in_url(character: &char) -> bool {
        let is_ascii_graphic = character.is_ascii_graphic();
        let is_beyond_ascii = *character > '\u{7F}';
        is_ascii_graphic || is_beyond_ascii
    }

    /// Return what precedes a `:` that comes before any `/`, `?` or `#`.
    fn scheme_of(value: &str) -> Option<&str> {
        let delimiter_index = value.find(SCHEME_SEARCH_DELIMITERS)?;
        let is_scheme_delimiter = value[delimiter_index..].starts_with(':');
        is_scheme_delimiter.then(|| &value[..delimiter_index])
    }

    /// Borrow the safe URL as a str.
    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_links_are_rejected() {
        let hostile_links = [
            "javascript:alert(1)",
            " JaVaScRiPt:alert(1)",
            "java\nscript:alert(1)",
            "java\u{0}script:alert(1)",
            "\tjava script:alert(1)",
            "data:text/html,<script>",
            "vbscript:x",
            "a:b/c",
            "\\\\evil",
            "http:\\\\evil",
            "/abs",
            "//evil.com",
            " \n\t",
            "",
        ];
        for raw in hostile_links {
            assert_eq!(SafeUrl::parse(raw), None, "{raw:?}");
        }
    }

    #[test]
    fn allowed_links_are_kept_without_whitespace() {
        let cases = [
            ("https://example.com/a b", "https://example.com/ab"),
            ("HTTP://Example.com", "HTTP://Example.com"),
            ("mailto:ops@example.com", "mailto:ops@example.com"),
            ("docs/design.md", "docs/design.md"),
            ("docs/a:b", "docs/a:b"),
            ("page?x=a:b", "page?x=a:b"),
            ("#section", "#section"),
            (" relative\n", "relative"),
        ];
        for (raw, expected_value) in cases {
            let url = SafeUrl::parse(raw).expect("accepted");
            assert_eq!(url.as_str(), expected_value, "{raw:?}");
        }
    }
}
