//! Sanitise the links svugy_model's `Link::resolve` leaves as URLs into safe URLs.

const ALLOWED_SCHEMES: [&str; 3] = ["http", "https", "mailto"];
const SCHEME_SEARCH_DELIMITERS: [char; 4] = [':', '/', '?', '#'];

/// Holds a `Link::Url` that passed the URL policy: an http, https or mailto URL, or a relative path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SafeUrl {
    value: String,
}

/// Names the URL policy rule a link value broke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UrlRejection {
    Empty,
    Backslash,
    RootRelativePath,
    DisallowedScheme,
}

impl SafeUrl {
    /// Strip ASCII whitespace and controls from a `Link::Url` value, then accept an allowed scheme or a relative path.
    pub(crate) fn parse(raw: &str) -> Result<SafeUrl, UrlRejection> {
        let value: String = raw.chars().filter(Self::is_kept_in_url).collect();
        if value.is_empty() {
            return Err(UrlRejection::Empty);
        }
        let has_backslash = value.contains('\\');
        if has_backslash {
            return Err(UrlRejection::Backslash);
        }
        let is_root_relative = value.starts_with('/');
        if is_root_relative {
            return Err(UrlRejection::RootRelativePath);
        }
        let Some(scheme) = Self::scheme_of(&value) else {
            return Ok(SafeUrl { value });
        };
        let lowercase_scheme = scheme.to_ascii_lowercase();
        let is_disallowed_scheme = !ALLOWED_SCHEMES.contains(&lowercase_scheme.as_str());
        if is_disallowed_scheme {
            return Err(UrlRejection::DisallowedScheme);
        }
        Ok(SafeUrl { value })
    }

    fn is_kept_in_url(character: &char) -> bool {
        let is_ascii_whitespace = character.is_ascii_whitespace();
        let is_ascii_control = character.is_ascii_control();
        let is_ascii_noise = is_ascii_whitespace || is_ascii_control;
        !is_ascii_noise
    }

    /// Return what precedes a `:` that comes before any `/`, `?` or `#`.
    fn scheme_of(value: &str) -> Option<&str> {
        let delimiter_index = value.find(SCHEME_SEARCH_DELIMITERS)?;
        let is_scheme_delimiter = value[delimiter_index..].starts_with(':');
        is_scheme_delimiter.then(|| &value[..delimiter_index])
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_links_are_rejected() {
        let cases = [
            ("javascript:alert(1)", UrlRejection::DisallowedScheme),
            (" JaVaScRiPt:alert(1)", UrlRejection::DisallowedScheme),
            ("java\nscript:alert(1)", UrlRejection::DisallowedScheme),
            ("java\u{0}script:alert(1)", UrlRejection::DisallowedScheme),
            ("\tjava script:alert(1)", UrlRejection::DisallowedScheme),
            ("data:text/html,<script>", UrlRejection::DisallowedScheme),
            ("vbscript:x", UrlRejection::DisallowedScheme),
            ("a:b/c", UrlRejection::DisallowedScheme),
            ("\\\\evil", UrlRejection::Backslash),
            ("http:\\\\evil", UrlRejection::Backslash),
            ("/abs", UrlRejection::RootRelativePath),
            ("//evil.com", UrlRejection::RootRelativePath),
            (" \n\t", UrlRejection::Empty),
            ("", UrlRejection::Empty),
        ];
        for (raw, expected_rejection) in cases {
            assert_eq!(SafeUrl::parse(raw), Err(expected_rejection), "{raw:?}");
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
