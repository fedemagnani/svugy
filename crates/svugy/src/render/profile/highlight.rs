//! Embed highlight.js knowing only the languages a document's code fences name, with its GitHub themes scoped to the description panel.

use std::collections::BTreeSet;
use std::str::FromStr;

use strum::{EnumMessage, EnumString, VariantArray};
use svugy_model::description::CodeFence;

use crate::render::writer::css::StyleSheet;

/// Declare the runtime's handle on the highlighter when the document embeds none.
const ABSENT_HIGHLIGHTER_DECLARATION: &str = "const svugyHighlighter = null;";
/// Select the description panel's body, the only place showing highlighted code.
const PANEL_BODY_SELECTOR: &str = "[data-svugy-contract] .svugy-panel-body";
/// Embed the highlight.js core, a function body returning the highlighter.
const CORE: &str = include_str!("../../../runtime/vendor/highlight/core.min.js");
/// Embed the GitHub theme for a dark canvas.
const DARK_THEME: &str = include_str!("../../../runtime/vendor/highlight/github-dark.min.css");
/// Embed the GitHub theme for a light canvas.
const LIGHT_THEME: &str = include_str!("../../../runtime/vendor/highlight/github.min.css");

/// Name a language the description panel highlights, parsed from any fence tag spelling it, case aside.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, EnumString, EnumMessage, VariantArray,
)]
#[strum(ascii_case_insensitive)]
pub(crate) enum HighlightLanguage {
    #[strum(serialize = "rs", serialize = "rust")]
    Rust,
    #[strum(serialize = "ts", serialize = "typescript")]
    TypeScript,
    #[strum(serialize = "js", serialize = "javascript")]
    JavaScript,
    #[strum(serialize = "py", serialize = "python")]
    Python,
    #[strum(serialize = "go")]
    Go,
    #[strum(serialize = "json")]
    Json,
    #[strum(serialize = "toml")]
    Toml,
    #[strum(serialize = "yaml", serialize = "yml")]
    Yaml,
    #[strum(serialize = "sql")]
    Sql,
    #[strum(serialize = "sh", serialize = "bash", serialize = "shell")]
    Shell,
}

/// Name a highlight.js grammar: the name it registers itself under and its source.
struct Grammar {
    registered_name: &'static str,
    source: &'static str,
}

/// Embeds highlight.js knowing only the languages a document's code fences name, nothing when they name none.
#[derive(Default)]
pub(crate) struct Highlighter {
    languages: BTreeSet<HighlightLanguage>,
}

impl Highlighter {
    /// Collect the languages the code fences of `descriptions` name.
    pub(crate) fn of_descriptions<'a>(descriptions: impl Iterator<Item = &'a str>) -> Self {
        let fence_tags = descriptions.flat_map(CodeFence::languages_in);
        let languages = fence_tags
            .filter_map(|fence_tag| HighlightLanguage::from_str(fence_tag).ok())
            .collect();
        Self { languages }
    }

    /// Declare the runtime's handle on the highlighter: `null` without languages, else the core, each language's grammar, and the map from every fence tag spelling a language to its grammar's name.
    pub(crate) fn declaration(&self) -> String {
        let is_highlighting_nothing = self.languages.is_empty();
        if is_highlighting_nothing {
            return ABSENT_HIGHLIGHTER_DECLARATION.to_owned();
        }
        let mut declaration = format!(
            "const svugyHighlighter = (() => {{\nconst hljs = (() => {{\n'use strict';\n{CORE}\n}})();\n"
        );
        let mut fence_tag_entries = Vec::new();
        for language in &self.languages {
            let grammar = language.grammar();
            declaration.push_str(grammar.source);
            declaration.push('\n');
            let registered_name = grammar.registered_name;
            for fence_tag in language.get_serializations() {
                let fence_tag_entry = format!("[{fence_tag:?}, {registered_name:?}]");
                fence_tag_entries.push(fence_tag_entry);
            }
        }
        let fence_tag_entry_list = fence_tag_entries.join(", ");
        let handle = format!(
            "const languageNameByFenceTag = new Map([{fence_tag_entry_list}]);\nreturn {{ hljs, languageNameByFenceTag }};\n}})();"
        );
        declaration.push_str(&handle);
        declaration
    }

    /// Push the GitHub themes without languages to highlight nothing, else nested under the panel's body: dark by default, light when the viewer prefers `light_scheme_query`.
    pub(crate) fn push_themes(
        &self,
        style_sheet: &mut StyleSheet,
        light_scheme_query: &'static str,
    ) {
        let is_highlighting_nothing = self.languages.is_empty();
        if is_highlighting_nothing {
            return;
        }
        style_sheet.push_static(PANEL_BODY_SELECTOR);
        style_sheet.push_static("{");
        style_sheet.push_static(DARK_THEME);
        style_sheet.push_static("}\n@media ");
        style_sheet.push_static(light_scheme_query);
        style_sheet.push_static("{");
        style_sheet.push_static(PANEL_BODY_SELECTOR);
        style_sheet.push_static("{");
        style_sheet.push_static(LIGHT_THEME);
        style_sheet.push_static("}}\n");
    }
}

impl HighlightLanguage {
    /// Return the vendored highlight.js grammar highlighting this language.
    fn grammar(self) -> Grammar {
        let (registered_name, source) = match self {
            Self::Rust => (
                "rust",
                include_str!("../../../runtime/vendor/highlight/languages/rust.min.js"),
            ),
            Self::TypeScript => (
                "typescript",
                include_str!("../../../runtime/vendor/highlight/languages/typescript.min.js"),
            ),
            Self::JavaScript => (
                "javascript",
                include_str!("../../../runtime/vendor/highlight/languages/javascript.min.js"),
            ),
            Self::Python => (
                "python",
                include_str!("../../../runtime/vendor/highlight/languages/python.min.js"),
            ),
            Self::Go => (
                "go",
                include_str!("../../../runtime/vendor/highlight/languages/go.min.js"),
            ),
            Self::Json => (
                "json",
                include_str!("../../../runtime/vendor/highlight/languages/json.min.js"),
            ),
            Self::Toml => (
                "ini",
                include_str!("../../../runtime/vendor/highlight/languages/ini.min.js"),
            ),
            Self::Yaml => (
                "yaml",
                include_str!("../../../runtime/vendor/highlight/languages/yaml.min.js"),
            ),
            Self::Sql => (
                "sql",
                include_str!("../../../runtime/vendor/highlight/languages/sql.min.js"),
            ),
            Self::Shell => (
                "bash",
                include_str!("../../../runtime/vendor/highlight/languages/bash.min.js"),
            ),
        };
        Grammar {
            registered_name,
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_languages_the_fences_name_are_embedded() {
        let component_description =
            "The shop owns its orders:\n```RS\nstruct Shop { orders: Orders }\n```";
        let edge_description =
            "```kotlin\nfun send() {}\n```\n```shell\ncurl api\n```\n```\nplain\n```";
        let descriptions = [component_description, edge_description].into_iter();
        let highlighter = Highlighter::of_descriptions(descriptions);
        let declaration = highlighter.declaration();
        let registered_grammar_count = declaration.matches("hljs.registerLanguage(").count();
        assert_eq!(
            registered_grammar_count, 2,
            "kotlin and a bare fence stay monospace, so only the rust and bash grammars are embedded"
        );
        let expected_fence_tags = r#"new Map([["rs", "rust"], ["rust", "rust"], ["sh", "bash"], ["bash", "bash"], ["shell", "bash"]])"#;
        let has_expected_fence_tags = declaration.contains(expected_fence_tags);
        assert!(
            has_expected_fence_tags,
            "the runtime resolves exactly the fence tags of the embedded languages, never a grammar's own aliases"
        );
        let prose_only = ["No code here, `inline` aside."].into_iter();
        let prose_only_highlighter = Highlighter::of_descriptions(prose_only);
        let prose_only_declaration = prose_only_highlighter.declaration();
        assert_eq!(
            prose_only_declaration, ABSENT_HIGHLIGHTER_DECLARATION,
            "a document without highlighted fences embeds no highlighter"
        );
    }

    #[test]
    fn vendored_sources_never_close_their_cdata_wrapper() {
        let cdata_end = "]]>";
        let every_language = HighlightLanguage::VARIANTS.iter().copied().collect();
        let highlighter = Highlighter {
            languages: every_language,
        };
        let declaration = highlighter.declaration();
        let mut style_sheet = StyleSheet::new();
        highlighter.push_themes(&mut style_sheet, "(prefers-color-scheme: light)");
        let themes = style_sheet.as_str();
        let is_declaration_free_of_cdata_end = !declaration.contains(cdata_end);
        let are_themes_free_of_cdata_end = !themes.contains(cdata_end);
        assert!(
            is_declaration_free_of_cdata_end,
            "a vendored highlight.js script contains {cdata_end}"
        );
        assert!(
            are_themes_free_of_cdata_end,
            "a vendored highlight.js theme contains {cdata_end}"
        );
    }
}
