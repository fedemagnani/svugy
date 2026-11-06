//! Measure label text before layout.

mod bundled;
mod sized_face;

use kurbo::{BezPath, Point, Size};
use svugy_model::model::Element;

pub(crate) use bundled::BundledFont;

const BUNDLED_FAMILY: &str = "Inter";

/// Measures the line box a single-line label occupies.
pub(crate) trait TextMeasure {
    fn measure(&self, spec: &TextSpec) -> TextSize;
}

/// Outlines a single-line label as glyph paths, so it renders without fonts.
pub(crate) trait TextOutline {
    fn outline(&self, spec: &TextSpec, baseline_origin: Point, anchor: TextAnchor) -> BezPath;
}

/// Specifies a label and the font request it is set in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextSpec<'a> {
    pub(crate) text: &'a str,
    pub(crate) family: &'a str,
    pub(crate) weight: u16,
    pub(crate) size: f64,
    pub(crate) letter_spacing: f64,
}

impl<'a> TextSpec<'a> {
    pub(crate) fn new(text: &'a str, family: &'a str, weight: u16, size: f64) -> Self {
        Self {
            text,
            family,
            weight,
            size,
            letter_spacing: 0.0,
        }
    }

    pub(crate) fn with_letter_spacing(self, letter_spacing: f64) -> Self {
        Self {
            letter_spacing,
            ..self
        }
    }
}

/// Sizes a label's line box in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextSize {
    pub(crate) width: f64,
    pub(crate) line_height: f64,
    pub(crate) baseline_from_top: f64,
}

/// Names the part a label plays, which fixes the font both layout and geometry set it in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LabelRole {
    Title,
    Caption,
    ConnectionLabel,
    ContainerTag,
    DiagramTitle,
    DiagramSubtitle,
    LegendTitle,
    LegendLine,
}

impl LabelRole {
    /// Measure the line box of `text` set in this role's font.
    pub(crate) fn measure_text(self, text: &str, measure: &impl TextMeasure) -> TextSize {
        let spec = self.text_spec(text);
        measure.measure(&spec)
    }

    /// Specify `text` in this role's font.
    pub(crate) fn text_spec(self, text: &str) -> TextSpec<'_> {
        match self {
            Self::Title => TextSpec::new(text, BUNDLED_FAMILY, 650, 12.0),
            Self::Caption => TextSpec::new(text, BUNDLED_FAMILY, 450, 8.5),
            Self::ConnectionLabel => TextSpec::new(text, BUNDLED_FAMILY, 500, 10.5),
            Self::ContainerTag => {
                let unspaced = TextSpec::new(text, BUNDLED_FAMILY, 600, 9.0);
                unspaced.with_letter_spacing(0.3)
            }
            Self::DiagramTitle => TextSpec::new(text, BUNDLED_FAMILY, 700, 17.0),
            Self::DiagramSubtitle => TextSpec::new(text, BUNDLED_FAMILY, 400, 11.5),
            Self::LegendTitle => TextSpec::new(text, BUNDLED_FAMILY, 700, 12.5),
            Self::LegendLine => TextSpec::new(text, BUNDLED_FAMILY, 400, 11.0),
        }
    }
}

/// Measures an element's label block, its title over its caption, and spaces it: the one set of metrics layout sizes room by and geometry places labels by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LabelBlock {
    pub(crate) title: TextSize,
    pub(crate) caption: Option<TextSize>,
}

impl LabelBlock {
    /// Inset around a block lying on a top face, a little looser than the 3–4 a tightly drawn block leaves.
    const TOP_FACE_INSET: f64 = 6.0;
    /// Inset of a container's hanging tag from its slab's west corner, as the example hangs it, kept on the east too.
    const TAG_INSET: f64 = 9.0;
    /// Drop from the title's baseline to the caption's.
    pub(crate) const CAPTION_LINE_OFFSET: f64 = 13.0;
    /// Title baseline on a top face, from the face's centre.
    pub(crate) const TOP_FACE_TITLE_BASELINE: Point = Point::new(0.0, -2.0);
    /// Title baseline stood upright, from the top face's projected centre.
    pub(crate) const UPRIGHT_TITLE_BASELINE: Point = Point::new(0.0, 4.0);
    /// Tag baseline on a front face, from its west top corner: the tag inset along it, below the top edge.
    pub(crate) const HANGING_TAG_BASELINE: Point = Point::new(Self::TAG_INSET, 21.5);

    /// Measure `element`'s title in `title_role`'s font and its caption, if any, in the caption font.
    pub(crate) fn measure(
        element: &Element,
        title_role: LabelRole,
        measure: &impl TextMeasure,
    ) -> Self {
        let title_text = element.label();
        let title = title_role.measure_text(title_text, measure);
        let caption = element
            .caption()
            .map(|caption_text| LabelRole::Caption.measure_text(caption_text, measure));
        Self { title, caption }
    }

    /// Return the plan room the block needs lying on a top face: its widest line along x and both lines along y, inset on every side.
    pub(crate) fn top_face_room(&self) -> Size {
        let caption_width = self.caption.map_or(0.0, |caption| caption.width);
        let caption_line_height = self.caption.map_or(0.0, |caption| caption.line_height);
        let block_width = self.title.width.max(caption_width);
        let block_depth = self.title.line_height + caption_line_height;
        let inset_width = block_width + 2.0 * Self::TOP_FACE_INSET;
        let inset_depth = block_depth + 2.0 * Self::TOP_FACE_INSET;
        Size::new(inset_width, inset_depth)
    }

    /// Return the width the block needs hung along a front face, its widest line inset from both corners.
    pub(crate) fn hanging_width(&self) -> f64 {
        let caption_width = self.caption.map_or(0.0, |caption| caption.width);
        let block_width = self.title.width.max(caption_width);
        block_width + 2.0 * Self::TAG_INSET
    }
}

/// Anchors a label to its origin the way SVG's text-anchor does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextAnchor {
    Start,
    Middle,
}

impl TextAnchor {
    pub(crate) fn width_share_before_origin(self) -> f64 {
        match self {
            Self::Start => 0.0,
            Self::Middle => 0.5,
        }
    }
}
