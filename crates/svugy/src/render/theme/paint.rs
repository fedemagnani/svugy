//! Name a color by theme token or by literal.

use svugy_model::color::Rgba;

use super::Token;

/// Names a color by theme token or by literal, or paints nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Paint {
    Token(Token),
    Literal(Rgba),
    None,
}

impl Paint {
    /// Paint `color` as a literal, or nothing when it is fully transparent.
    pub(crate) fn from_color(color: Rgba) -> Self {
        let is_transparent = color.is_transparent();
        if is_transparent {
            return Paint::None;
        }
        Paint::Literal(color)
    }
}
