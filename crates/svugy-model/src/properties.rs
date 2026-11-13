//! Hold the properties a component declares about how it is drawn and where it points, the part a workflow may override.

use std::collections::BTreeMap;

use crate::model::{ByteRange, Shape};

/// Pair a written text value with where an imported source wrote it, so a span never outlives its text.
#[derive(Clone, Debug, PartialEq)]
pub struct SpannedText {
    pub(crate) text: String,
    pub(crate) span: Option<ByteRange>,
}

/// Describe how a component is drawn and where it points: caption, shape, height, style, link, code and metadata, each set or left unset.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentProperties {
    pub(crate) caption: Option<String>,
    pub(crate) shape: Option<Shape>,
    pub(crate) height: Option<f64>,
    pub(crate) style: BTreeMap<String, String>,
    pub(crate) link: Option<SpannedText>,
    pub(crate) code: Option<SpannedText>,
    pub(crate) metadata: BTreeMap<String, String>,
}

impl SpannedText {
    /// Pair `text` with `span`; `None` when no text was written, whatever the span.
    pub(crate) fn written(text: Option<String>, span: Option<ByteRange>) -> Option<Self> {
        let text = text?;
        let spanned_text = Self { text, span };
        Some(spanned_text)
    }

    /// Split `spanned_text` back into its text and span, both `None` when no text was written.
    pub(crate) fn unpaired(spanned_text: Option<Self>) -> (Option<String>, Option<ByteRange>) {
        let Some(Self { text, span }) = spanned_text else {
            return (None, None);
        };
        (Some(text), span)
    }
}

impl ComponentProperties {
    /// Create new properties, every one unset.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the secondary line drawn under the label.
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        let caption = caption.into();
        self.caption = Some(caption);
        self
    }

    /// Set the shape the component is drawn as, overriding the renderer's pick.
    pub fn with_shape(mut self, shape: Shape) -> Self {
        self.shape = Some(shape);
        self
    }

    /// Stand a leaf's top face `height` plan units above its floor instead of its primitive's default; validation bounds it.
    pub fn with_height(mut self, height: f64) -> Self {
        self.height = Some(height);
        self
    }

    /// Record a raw style entry; validation decides later whether the key is known.
    pub fn with_style(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        self.style.insert(key, value);
        self
    }

    /// Set the drill-down target (an element path) or URL the component points to, unresolved.
    pub fn with_link(self, link: impl Into<String>) -> Self {
        self.with_link_written_at(link, None)
    }

    /// Set the unresolved link, written at `span` of an imported source when there is one.
    pub fn with_link_written_at(
        mut self,
        link: impl Into<String>,
        span: Option<ByteRange>,
    ) -> Self {
        let text = link.into();
        let link = SpannedText { text, span };
        self.link = Some(link);
        self
    }

    /// Point the component at a place in the source tree, written `FILE` or `FILE#SYMBOL` relative to the repository root; validation checks its shape.
    pub fn with_code(self, code: impl Into<String>) -> Self {
        self.with_code_written_at(code, None)
    }

    /// Set the code reference, written at `span` of an imported source when there is one.
    pub fn with_code_written_at(
        mut self,
        code: impl Into<String>,
        span: Option<ByteRange>,
    ) -> Self {
        let text = code.into();
        let code = SpannedText { text, span };
        self.code = Some(code);
        self
    }

    /// Record a free-form metadata entry, carried into the rendered component's annotations.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        self.metadata.insert(key, value);
        self
    }

    /// Return these properties overridden by `overriding`: each property it sets replaces this one, style and metadata entry by entry.
    pub fn overridden_by(&self, overriding: &Self) -> Self {
        let caption = overriding.caption.clone().or_else(|| self.caption.clone());
        let shape = overriding.shape.or(self.shape);
        let height = overriding.height.or(self.height);
        let link = overriding.link.clone().or_else(|| self.link.clone());
        let code = overriding.code.clone().or_else(|| self.code.clone());
        let mut style = self.style.clone();
        let overriding_style = overriding.style.clone();
        style.extend(overriding_style);
        let mut metadata = self.metadata.clone();
        let overriding_metadata = overriding.metadata.clone();
        metadata.extend(overriding_metadata);
        Self {
            caption,
            shape,
            height,
            style,
            link,
            code,
            metadata,
        }
    }

    /// Return the secondary line drawn under the label, if set.
    pub fn caption(&self) -> Option<&str> {
        self.caption.as_deref()
    }

    /// Return the authored shape, or `None` to let the renderer pick (box for components, slab for containers).
    pub fn shape(&self) -> Option<Shape> {
        self.shape
    }

    /// Return the authored height, or `None` to stand at the primitive's default height.
    pub fn height(&self) -> Option<f64> {
        self.height
    }

    /// Return the raw style entries, keyed by style name.
    pub fn style(&self) -> &BTreeMap<String, String> {
        &self.style
    }

    /// Return the unresolved drill-down target or URL, if set.
    pub fn link(&self) -> Option<&str> {
        let link = self.link.as_ref()?;
        Some(&link.text)
    }

    /// Return where in an imported source the link value was written, as a byte range.
    pub fn link_span(&self) -> Option<ByteRange> {
        let link = self.link.as_ref()?;
        link.span
    }

    /// Return the unchecked `FILE#SYMBOL` place in the source tree, if set.
    pub fn code(&self) -> Option<&str> {
        let code = self.code.as_ref()?;
        Some(&code.text)
    }

    /// Return where in an imported source the code value was written, as a byte range.
    pub fn code_span(&self) -> Option<ByteRange> {
        let code = self.code.as_ref()?;
        code.span
    }

    /// Return the metadata entries, keyed by name.
    pub fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
    }
}
