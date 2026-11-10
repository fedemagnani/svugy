//! Render a diagram, written in Markdown or d2, into the interactive SVG of the standard or sketch theme inside a browser, through four plain exports and no bindings generator: a searched render and a quick draft one.

use std::cell::RefCell;

use miette::Diagnostic;
use serde::Serialize;
use svugy::{
    DraftInteractiveSvg, InteractiveSvg, RenderSvg, SketchDraftInteractiveSvg, SketchInteractiveSvg,
};
use svugy_model::problem::ErrorSeverity;
use svugy_parse::d2::D2Language;
use svugy_parse::diagnostic::{LabelledDiagnostic, SourceLanguage};
use svugy_parse::markdown::MarkdownLanguage;

/// Name Markdown in a `render` call.
const MARKDOWN_LANGUAGE_BYTE: u8 = 0;

/// Name d2 in a `render` call.
const D2_LANGUAGE_BYTE: u8 = 1;

/// Name the standard theme in a `render` call.
const STANDARD_THEME_BYTE: u8 = 0;

/// Name the hand-drawn sketch theme in a `render` call.
const SKETCH_THEME_BYTE: u8 = 1;

thread_local! {
    /// Hold the one playground the exports share, as wasm runs on a single thread.
    static PLAYGROUND: RefCell<Playground> = {
        let playground = Playground::default();
        RefCell::new(playground)
    };
}

/// Size the input buffer to `length` bytes and return where the caller writes the diagram source.
#[unsafe(no_mangle)]
pub extern "C" fn input_buffer(length: usize) -> *mut u8 {
    PLAYGROUND.with_borrow_mut(|playground| playground.input_buffer(length))
}

/// Render the source in the input buffer, read in the language `language_byte` names (0 Markdown, 1 d2), drawn in the theme `theme_byte` names (0 standard, 1 sketch), laid out in its searched arrangement, and return the byte length of the JSON verdict.
#[unsafe(no_mangle)]
pub extern "C" fn render(language_byte: u8, theme_byte: u8) -> usize {
    PLAYGROUND.with_borrow_mut(|playground| {
        playground.render_input::<Searched>(language_byte, theme_byte)
    })
}

/// Render like `render`, laid out in the seed arrangement without searching: a quick draft to show while `render` runs.
#[unsafe(no_mangle)]
pub extern "C" fn render_draft(language_byte: u8, theme_byte: u8) -> usize {
    PLAYGROUND
        .with_borrow_mut(|playground| playground.render_input::<Draft>(language_byte, theme_byte))
}

/// Return the output pointer: where the JSON verdict of the last render starts.
#[unsafe(no_mangle)]
pub extern "C" fn output_pointer() -> *const u8 {
    PLAYGROUND.with_borrow(|playground| playground.output.as_ptr())
}

/// Name a theme's SVG flavour for each arrangement.
trait Theme {
    type Searched: RenderSvg;
    type Draft: RenderSvg;
}

/// Name the standard theme's flavours.
struct StandardTheme;

/// Name the hand-drawn sketch theme's flavours.
struct SketchTheme;

/// Pick the flavour a theme renders in this arrangement.
trait Arrangement {
    type Flavour<T: Theme>: RenderSvg;
}

/// Select the searched arrangement.
struct Searched;

/// Select the seed arrangement, drafted without searching.
struct Draft;

/// Hold the playground's source a page writes in and the JSON verdict it reads back, so no pointer ever crosses the boundary inward.
#[derive(Default)]
struct Playground {
    input: Vec<u8>,
    output: String,
}

/// Carry a render's verdict to the page: the SVG, absent while any diagnostic is an error, and every diagnostic.
#[derive(Serialize)]
struct Verdict {
    svg: Option<String>,
    diagnostics: Vec<LocatedDiagnostic>,
}

/// Place one diagnostic in the editor by its first label's 1-based line and column.
#[derive(Serialize)]
struct LocatedDiagnostic {
    is_error: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
}

impl Playground {
    /// Size the input buffer to `length` zeroed bytes and return where it starts.
    fn input_buffer(&mut self, length: usize) -> *mut u8 {
        self.input.clear();
        self.input.resize(length, 0);
        self.input.as_mut_ptr()
    }

    /// Serialise the verdict on the input buffer's source, read in the language `language_byte` names and drawn in the theme `theme_byte` names in arrangement `A`, into the output, returning its byte length.
    fn render_input<A: Arrangement>(&mut self, language_byte: u8, theme_byte: u8) -> usize {
        let source = String::from_utf8_lossy(&self.input);
        let verdict = Verdict::of_theme_byte::<A>(theme_byte, language_byte, &source);
        self.output = serde_json::to_string(&verdict).expect("a verdict serialises");
        self.output.len()
    }
}

impl Theme for StandardTheme {
    type Searched = InteractiveSvg;
    type Draft = DraftInteractiveSvg;
}

impl Theme for SketchTheme {
    type Searched = SketchInteractiveSvg;
    type Draft = SketchDraftInteractiveSvg;
}

impl Arrangement for Searched {
    type Flavour<T: Theme> = T::Searched;
}

impl Arrangement for Draft {
    type Flavour<T: Theme> = T::Draft;
}

impl Verdict {
    /// Judge `source` drawn in the theme `theme_byte` names in arrangement `A`, or report the byte when it names none.
    fn of_theme_byte<A: Arrangement>(theme_byte: u8, language_byte: u8, source: &str) -> Self {
        match theme_byte {
            STANDARD_THEME_BYTE => {
                Self::of_language_byte::<A::Flavour<StandardTheme>>(language_byte, source)
            }
            SKETCH_THEME_BYTE => {
                Self::of_language_byte::<A::Flavour<SketchTheme>>(language_byte, source)
            }
            _unknown_theme_byte => Self::unknown_theme(theme_byte),
        }
    }

    /// Judge `source` in the language `language_byte` names, rendering by `R`, or report the byte when it names none.
    fn of_language_byte<R: RenderSvg>(language_byte: u8, source: &str) -> Self {
        match language_byte {
            MARKDOWN_LANGUAGE_BYTE => Self::of::<MarkdownLanguage, R>(source),
            D2_LANGUAGE_BYTE => Self::of::<D2Language, R>(source),
            _unknown_language_byte => Self::unknown_language(language_byte),
        }
    }

    /// Judge `source` in the language `L`: the SVG `R` renders when nothing is an error, and every diagnostic located in the source.
    fn of<L: SourceLanguage, R: RenderSvg>(source: &str) -> Self {
        let import = L::import(source);
        let diagnostics = import
            .diagnostics()
            .into_iter()
            .map(|diagnostic| LocatedDiagnostic::locate(diagnostic, source))
            .collect();
        let admitted_model = import.admitted_model();
        let svg = admitted_model.map(R::render);
        Self { svg, diagnostics }
    }

    /// Withhold the SVG behind one error naming the `theme_byte` no theme answers to.
    fn unknown_theme(theme_byte: u8) -> Self {
        let message = format!(
            "no theme has byte {theme_byte}: pass {STANDARD_THEME_BYTE} for standard or {SKETCH_THEME_BYTE} for sketch"
        );
        Self::withheld_by(message)
    }

    /// Withhold the SVG behind one error naming the `language_byte` no language answers to.
    fn unknown_language(language_byte: u8) -> Self {
        let message = format!(
            "no language has byte {language_byte}: pass {MARKDOWN_LANGUAGE_BYTE} for Markdown or {D2_LANGUAGE_BYTE} for d2"
        );
        Self::withheld_by(message)
    }

    /// Withhold the SVG behind one unlocated error carrying `message`.
    fn withheld_by(message: String) -> Self {
        let unlocated_error = LocatedDiagnostic {
            is_error: true,
            message,
            help: None,
            line: None,
            column: None,
        };
        let diagnostics = vec![unlocated_error];
        Self {
            svg: None,
            diagnostics,
        }
    }
}

impl LocatedDiagnostic {
    /// Locate `diagnostic` in `source` by its first label, carrying its severity, message and help.
    fn locate(diagnostic: &dyn Diagnostic, source: &str) -> Self {
        let is_error = diagnostic.is_error();
        let message = diagnostic.to_string();
        let help = diagnostic.help().map(|help| help.to_string());
        let label_offset = diagnostic.first_label_offset();
        let line_and_column = label_offset.map(|offset| Self::line_and_column(source, offset));
        let (line, column) = line_and_column.unzip();
        Self {
            is_error,
            message,
            help,
            line,
            column,
        }
    }

    /// Count lines and characters before byte `offset`: line = 1 + newlines before it, column = 1 + characters since the last newline.
    fn line_and_column(source: &str, offset: usize) -> (usize, usize) {
        let source_length = source.len();
        let clamped_offset = offset.min(source_length);
        let before = source.get(..clamped_offset).unwrap_or(source);
        let line = 1 + before.matches('\n').count();
        let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
        let line_before_offset = &before[line_start..];
        let column = 1 + line_before_offset.chars().count();
        (line, column)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    const MARKDOWN_SAMPLE: &str = include_str!("../../../docs/diagrams/pipeline.md");
    const D2_SAMPLE: &str = include_str!("../../../docs/src/playground/sample.d2");

    fn verdict_of(language_byte: u8, source: &str) -> Value {
        let verdict = Verdict::of_language_byte::<InteractiveSvg>(language_byte, source);
        serde_json::to_value(&verdict).expect("the verdict is JSON")
    }

    #[test]
    fn every_playground_sample_renders_the_interactive_svg_with_no_diagnostics() {
        let samples = [
            ("Markdown", MARKDOWN_LANGUAGE_BYTE, MARKDOWN_SAMPLE),
            ("d2", D2_LANGUAGE_BYTE, D2_SAMPLE),
        ];
        for (language, language_byte, sample) in samples {
            let verdict = verdict_of(language_byte, sample);

            let svg = verdict["svg"].as_str().unwrap_or_default();
            let diagnostics = &verdict["diagnostics"];
            let has_interactive_svg = svg.contains("<script>");
            let has_no_diagnostics = diagnostics.as_array().is_some_and(Vec::is_empty);
            assert!(
                has_interactive_svg && has_no_diagnostics,
                "the playground opens the {language} sample on an interactive SVG with no diagnostics, got {diagnostics}"
            );
        }
    }

    #[test]
    fn an_error_withholds_the_svg_and_points_at_its_line_and_column() {
        let verdict = verdict_of(MARKDOWN_LANGUAGE_BYTE, "## Components\n- a (colour=red)\n");

        let diagnostics = verdict["diagnostics"].as_array().expect("a list");
        let first = &diagnostics[0];
        assert!(verdict["svg"].is_null());
        assert_eq!(first["is_error"], true);
        assert_eq!(first["line"], 2);
        assert!(first["column"].as_u64().expect("a column") > 1);
    }

    #[test]
    fn a_warning_still_renders() {
        let verdict = verdict_of(
            MARKDOWN_LANGUAGE_BYTE,
            "## Components\n- a (shape=cylinder)\n  - b\n",
        );

        let diagnostics = verdict["diagnostics"].as_array().expect("a list");
        assert!(verdict["svg"].is_string());
        assert_eq!(diagnostics[0]["is_error"], false);
    }

    #[test]
    fn exports_render_what_the_input_buffer_holds_in_the_language_and_theme_named() {
        let source = b"## Components\n- a\n- b\n\n## Edges\n- a -> b\n";
        let buffer = input_buffer(source.len());
        let input = PLAYGROUND.with_borrow_mut(|playground| playground.input.as_mut_ptr());
        assert_eq!(buffer, input);
        PLAYGROUND.with_borrow_mut(|playground| playground.input.copy_from_slice(source));

        let output_length = render(MARKDOWN_LANGUAGE_BYTE, SKETCH_THEME_BYTE);

        let output = PLAYGROUND.with_borrow(|playground| playground.output.clone());
        assert_eq!(output.len(), output_length);
        let output_start = PLAYGROUND.with_borrow(|playground| playground.output.as_ptr());
        assert_eq!(output_pointer(), output_start);
        assert!(output.contains("\"svg\":\"<svg "));
        assert!(output.contains("font-family:Excalifont"));
    }
}
