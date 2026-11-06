//! Resolve every color and style decision from theme tokens.

pub(crate) mod color;

use color::Oklab;
use svugy_model::color::{Rgb, Rgba};

use crate::render::writer::css::{CssDeclaration, CssIdentifier, CssValue};

const LEFT_FACE_DIMMING: f64 = 0.835;
const RIGHT_FACE_DIMMING: f64 = 0.72;
const CSS_CUSTOM_PROPERTY_PREFIX: &str = "--svugy-";

/// Names a themed color: a flat role, or a face or the ink of a shaded surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Token {
    Canvas,
    GridLine,
    FaceOutline,
    Text,
    TextMuted,
    ExternalOutline,
    SurfaceTop(Surface),
    SurfaceLeft(Surface),
    SurfaceRight(Surface),
    SurfaceInk(Surface),
}

/// Names a surface whose faces are shaded from one base color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Surface {
    Neutral,
    ContainerBlue,
}

/// Holds the base color of every token for one color scheme.
pub(crate) struct Palette {
    canvas: Rgb,
    grid_line: Rgb,
    face_outline: Rgb,
    text: Rgb,
    text_muted: Rgb,
    external_outline: Rgb,
    neutral: SurfaceColors,
    container_blue: SurfaceColors,
}

#[derive(Clone, Copy)]
struct SurfaceColors {
    base: Rgb,
    ink: Rgb,
}

/// Holds the top, left and right face shades of one base color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FaceShades {
    pub(crate) top: Rgb,
    pub(crate) left: Rgb,
    pub(crate) right: Rgb,
}

/// Paint every token for a dark canvas.
pub(crate) const DARK_PALETTE: Palette = Palette {
    canvas: Rgb::from_packed(0x0a0b10),
    grid_line: Rgb::from_packed(0x151722),
    face_outline: Rgb::from_packed(0x0a0b10),
    text: Rgb::from_packed(0xe6e8ee),
    text_muted: Rgb::from_packed(0x8b93a5),
    external_outline: Rgb::from_packed(0x5b6170),
    neutral: SurfaceColors::new(0x3a3f4b, 0xe6e8ee),
    container_blue: SurfaceColors::new(0x1b2436, 0x8aa0c8),
};

/// The dark palette mirrored in Oklab lightness (L ↦ 1.13 − L, hue kept).
pub(crate) const LIGHT_PALETTE: Palette = Palette {
    canvas: Rgb::from_packed(0xf6f8ff),
    grid_line: Rgb::from_packed(0xe1e4f4),
    face_outline: Rgb::from_packed(0xf6f8ff),
    text: Rgb::from_packed(0x14161a),
    text_muted: Rgb::from_packed(0x535a6a),
    external_outline: Rgb::from_packed(0x858b9b),
    neutral: SurfaceColors::new(0xacb2c0, 0x14161a),
    container_blue: SurfaceColors::new(0xc8d4ed, 0x3c4f72),
};

impl Token {
    /// Lists every token in a fixed order, for emitting custom-property declarations.
    pub(crate) fn all() -> Vec<Token> {
        let flat_tokens = [
            Token::Canvas,
            Token::GridLine,
            Token::FaceOutline,
            Token::Text,
            Token::TextMuted,
            Token::ExternalOutline,
        ];
        let surface_tokens = Surface::ALL.into_iter().flat_map(|surface| {
            [
                Token::SurfaceTop(surface),
                Token::SurfaceLeft(surface),
                Token::SurfaceRight(surface),
                Token::SurfaceInk(surface),
            ]
        });
        flat_tokens.into_iter().chain(surface_tokens).collect()
    }

    /// Names the CSS custom property that carries this token's color.
    pub(crate) fn css_custom_property(self) -> CssIdentifier {
        let property_name = match self {
            Token::Canvas => format!("{CSS_CUSTOM_PROPERTY_PREFIX}canvas"),
            Token::GridLine => format!("{CSS_CUSTOM_PROPERTY_PREFIX}grid-line"),
            Token::FaceOutline => format!("{CSS_CUSTOM_PROPERTY_PREFIX}face-outline"),
            Token::Text => format!("{CSS_CUSTOM_PROPERTY_PREFIX}text"),
            Token::TextMuted => format!("{CSS_CUSTOM_PROPERTY_PREFIX}text-muted"),
            Token::ExternalOutline => format!("{CSS_CUSTOM_PROPERTY_PREFIX}external-outline"),
            Token::SurfaceTop(surface) => {
                let slug = surface.slug();
                format!("{CSS_CUSTOM_PROPERTY_PREFIX}{slug}-top")
            }
            Token::SurfaceLeft(surface) => {
                let slug = surface.slug();
                format!("{CSS_CUSTOM_PROPERTY_PREFIX}{slug}-left")
            }
            Token::SurfaceRight(surface) => {
                let slug = surface.slug();
                format!("{CSS_CUSTOM_PROPERTY_PREFIX}{slug}-right")
            }
            Token::SurfaceInk(surface) => {
                let slug = surface.slug();
                format!("{CSS_CUSTOM_PROPERTY_PREFIX}{slug}-ink")
            }
        };
        CssIdentifier::escape(&property_name)
    }
}

impl Surface {
    pub(crate) const ALL: [Surface; 2] = [Surface::Neutral, Surface::ContainerBlue];

    fn slug(self) -> &'static str {
        match self {
            Surface::Neutral => "neutral",
            Surface::ContainerBlue => "container-blue",
        }
    }
}

impl Palette {
    /// Resolves a token to the literal color a profile inlines.
    pub(crate) fn literal(&self, token: Token) -> Rgb {
        match token {
            Token::Canvas => self.canvas,
            Token::GridLine => self.grid_line,
            Token::FaceOutline => self.face_outline,
            Token::Text => self.text,
            Token::TextMuted => self.text_muted,
            Token::ExternalOutline => self.external_outline,
            Token::SurfaceTop(surface) => self.face_shades(surface).top,
            Token::SurfaceLeft(surface) => self.face_shades(surface).left,
            Token::SurfaceRight(surface) => self.face_shades(surface).right,
            Token::SurfaceInk(surface) => self.surface_colors(surface).ink,
        }
    }

    /// Declare every token's custom property as this palette's literal, the rule one color scheme applies.
    pub(crate) fn custom_property_declarations(&self) -> Vec<CssDeclaration> {
        let tokens = Token::all();
        tokens
            .into_iter()
            .map(|token| {
                let property = token.css_custom_property();
                let literal = self.literal(token);
                let opaque_literal = Rgba::opaque(literal);
                let value = CssValue::color(opaque_literal);
                CssDeclaration { property, value }
            })
            .collect()
    }

    fn face_shades(&self, surface: Surface) -> FaceShades {
        let surface_colors = self.surface_colors(surface);
        FaceShades::from_base(surface_colors.base)
    }

    fn surface_colors(&self, surface: Surface) -> SurfaceColors {
        match surface {
            Surface::Neutral => self.neutral,
            Surface::ContainerBlue => self.container_blue,
        }
    }
}

impl SurfaceColors {
    const fn new(packed_base: u32, packed_ink: u32) -> Self {
        let base = Rgb::from_packed(packed_base);
        let ink = Rgb::from_packed(packed_ink);
        SurfaceColors { base, ink }
    }
}

impl FaceShades {
    /// Derive the side faces by dimming the base in Oklab: top = base, left = 0.835·base, right = 0.72·base.
    pub(crate) fn from_base(base: Rgb) -> Self {
        let base_oklab = Oklab::from_rgb(base);
        let left_oklab = base_oklab.dimmed(LEFT_FACE_DIMMING);
        let right_oklab = base_oklab.dimmed(RIGHT_FACE_DIMMING);
        let left = left_oklab.to_rgb();
        let right = right_oklab.to_rgb();
        FaceShades {
            top: base,
            left,
            right,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fmt::Write as _;

    use super::*;

    const EXAMPLE_TOP_FACES: [u32; 8] = [
        0x3478f6, 0x9945ff, 0x14f195, 0xf2a93b, 0x3a3f4b, 0x1b2436, 0x334566, 0x17271f,
    ];

    #[test]
    fn face_shades_of_the_example_colors() {
        let mut table = String::new();
        for packed_top in EXAMPLE_TOP_FACES {
            let base = Rgb::from_packed(packed_top);
            let FaceShades { top, left, right } = FaceShades::from_base(base);
            let _infallible = writeln!(table, "{top} {left} {right}");
        }
        insta::assert_snapshot!(table, @r"
        #3478f6 #275dc2 #1e4b9e
        #9945ff #7734c9 #6129a4
        #14f195 #0dbe74 #099b5e
        #f2a93b #be842c #9c6b22
        #3a3f4b #2b2f39 #22252d
        #1b2436 #131a28 #0d131f
        #334566 #26344f #1d293f
        #17271f #101c16 #0b1510
        ");
    }

    #[test]
    fn face_shades_darken_from_top_to_right() {
        for surface in Surface::ALL {
            let top = DARK_PALETTE.literal(Token::SurfaceTop(surface));
            let left = DARK_PALETTE.literal(Token::SurfaceLeft(surface));
            let right = DARK_PALETTE.literal(Token::SurfaceRight(surface));
            let top_lightness = Oklab::from_rgb(top).lightness;
            let left_lightness = Oklab::from_rgb(left).lightness;
            let right_lightness = Oklab::from_rgb(right).lightness;
            assert!(top_lightness > left_lightness, "{surface:?}");
            assert!(left_lightness > right_lightness, "{surface:?}");
        }
    }

    #[test]
    fn top_face_resolves_to_the_exact_base() {
        let neutral_top = DARK_PALETTE.literal(Token::SurfaceTop(Surface::Neutral));
        assert_eq!(neutral_top.to_string(), "#3a3f4b");
        let canvas = DARK_PALETTE.literal(Token::Canvas);
        assert_eq!(canvas.to_string(), "#0a0b10");
    }

    #[test]
    fn every_token_has_a_distinct_css_custom_property() {
        let tokens = Token::all();
        let property_names: BTreeSet<String> = tokens
            .iter()
            .map(|token| token.css_custom_property().as_str().to_owned())
            .collect();
        assert_eq!(property_names.len(), tokens.len());
        let blue_left = Token::SurfaceLeft(Surface::ContainerBlue).css_custom_property();
        assert_eq!(blue_left.as_str(), "--svugy-container-blue-left");
    }

    #[test]
    fn every_token_resolves_in_both_palettes() {
        for token in Token::all() {
            let _light = LIGHT_PALETTE.literal(token);
            let _dark = DARK_PALETTE.literal(token);
        }
    }
}
