//! Resolve an element's `link:` value into a drill-down target or a URL, the one place that decides which it is.

use crate::model::Model;
use crate::path::ElementPath;

const URL_CHARACTERS: [char; 4] = [':', '/', '?', '#'];

/// Names what a `link:` value points to: an element of the model, or anything else, left as a URL for the renderer's URL policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link<'a> {
    /// Names an existing element, the level a click enters.
    Element(ElementPath),
    /// Names no element; unsanitised, so only a URL policy may let it reach markup.
    Url(&'a str),
}

impl<'a> Link<'a> {
    /// Resolve `raw` against `model`: an element when it parses as the path of one, otherwise a URL.
    pub fn resolve(raw: &'a str, model: &Model) -> Self {
        let Ok(path) = raw.parse::<ElementPath>() else {
            return Self::Url(raw);
        };
        let is_element = model.element(&path).is_some();
        if is_element {
            return Self::Element(path);
        }
        Self::Url(raw)
    }

    /// Return the element path a URL is shaped like, with no `:`, `/`, `?` or `#`: the likely typo of a drill-down target.
    pub fn path_shaped_url(&self) -> Option<ElementPath> {
        let Self::Url(raw) = self else {
            return None;
        };
        let has_url_character = raw.contains(URL_CHARACTERS);
        if has_url_character {
            return None;
        }
        let Ok(path) = raw.parse::<ElementPath>() else {
            return None;
        };
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ModelBuilder;
    use crate::model::Element;

    fn backend_with_api() -> Model {
        let mut builder = ModelBuilder::new();
        let backend = builder.add_top_level("backend", Element::new("backend"));
        builder.add_child(&backend, "api", Element::new("API"));
        builder.build().model.expect("fixture model is valid")
    }

    #[test]
    fn a_link_naming_an_element_resolves_to_it_and_anything_else_stays_a_url() {
        let model = backend_with_api();
        let api = ElementPath::from_segments(["backend", "api"]);
        let cases = [
            ("backend.api", Link::Element(api)),
            ("backend.apii", Link::Url("backend.apii")),
            ("https://example.com", Link::Url("https://example.com")),
            ("./guide.html", Link::Url("./guide.html")),
            ("", Link::Url("")),
        ];
        for (raw, expected_link) in cases {
            let link = Link::resolve(raw, &model);
            assert_eq!(link, expected_link, "{raw:?}");
        }
    }

    #[test]
    fn only_urls_without_url_characters_are_shaped_like_an_element_path() {
        let model = backend_with_api();
        let typo = ElementPath::from_segments(["backend", "apii"]);
        let cases = [
            ("backend.apii", Some(typo)),
            ("backend.api", None),
            ("docs/guide.html", None),
            ("mailto:team@example.com", None),
            ("#section", None),
            ("..", None),
        ];
        for (raw, expected_path) in cases {
            let link = Link::resolve(raw, &model);
            let path_shaped = link.path_shaped_url();
            assert_eq!(path_shaped, expected_path, "{raw:?}");
        }
    }
}
