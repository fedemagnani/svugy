//! Parse the color values a style may hold into 8-bit sRGB, the one color grammar every crate shares.

use std::fmt;

const TRANSPARENT_KEYWORD: &str = "transparent";
const OPAQUE_ALPHA_DIGITS: &str = "ff";
const RGB_HEX_DIGIT_COUNT: usize = 6;
const RGBA_HEX_DIGIT_COUNT: usize = 8;
const SHORT_HEX_DIGIT_COUNTS: [usize; 2] = [3, 4];
/// Every CSS named color with its sRGB value, sorted by name.
pub const NAMED_COLORS: [(&str, Rgb); 148] = [
    ("aliceblue", Rgb::from_packed(0xf0f8ff)),
    ("antiquewhite", Rgb::from_packed(0xfaebd7)),
    ("aqua", Rgb::from_packed(0x00ffff)),
    ("aquamarine", Rgb::from_packed(0x7fffd4)),
    ("azure", Rgb::from_packed(0xf0ffff)),
    ("beige", Rgb::from_packed(0xf5f5dc)),
    ("bisque", Rgb::from_packed(0xffe4c4)),
    ("black", Rgb::from_packed(0x000000)),
    ("blanchedalmond", Rgb::from_packed(0xffebcd)),
    ("blue", Rgb::from_packed(0x0000ff)),
    ("blueviolet", Rgb::from_packed(0x8a2be2)),
    ("brown", Rgb::from_packed(0xa52a2a)),
    ("burlywood", Rgb::from_packed(0xdeb887)),
    ("cadetblue", Rgb::from_packed(0x5f9ea0)),
    ("chartreuse", Rgb::from_packed(0x7fff00)),
    ("chocolate", Rgb::from_packed(0xd2691e)),
    ("coral", Rgb::from_packed(0xff7f50)),
    ("cornflowerblue", Rgb::from_packed(0x6495ed)),
    ("cornsilk", Rgb::from_packed(0xfff8dc)),
    ("crimson", Rgb::from_packed(0xdc143c)),
    ("cyan", Rgb::from_packed(0x00ffff)),
    ("darkblue", Rgb::from_packed(0x00008b)),
    ("darkcyan", Rgb::from_packed(0x008b8b)),
    ("darkgoldenrod", Rgb::from_packed(0xb8860b)),
    ("darkgray", Rgb::from_packed(0xa9a9a9)),
    ("darkgreen", Rgb::from_packed(0x006400)),
    ("darkgrey", Rgb::from_packed(0xa9a9a9)),
    ("darkkhaki", Rgb::from_packed(0xbdb76b)),
    ("darkmagenta", Rgb::from_packed(0x8b008b)),
    ("darkolivegreen", Rgb::from_packed(0x556b2f)),
    ("darkorange", Rgb::from_packed(0xff8c00)),
    ("darkorchid", Rgb::from_packed(0x9932cc)),
    ("darkred", Rgb::from_packed(0x8b0000)),
    ("darksalmon", Rgb::from_packed(0xe9967a)),
    ("darkseagreen", Rgb::from_packed(0x8fbc8f)),
    ("darkslateblue", Rgb::from_packed(0x483d8b)),
    ("darkslategray", Rgb::from_packed(0x2f4f4f)),
    ("darkslategrey", Rgb::from_packed(0x2f4f4f)),
    ("darkturquoise", Rgb::from_packed(0x00ced1)),
    ("darkviolet", Rgb::from_packed(0x9400d3)),
    ("deeppink", Rgb::from_packed(0xff1493)),
    ("deepskyblue", Rgb::from_packed(0x00bfff)),
    ("dimgray", Rgb::from_packed(0x696969)),
    ("dimgrey", Rgb::from_packed(0x696969)),
    ("dodgerblue", Rgb::from_packed(0x1e90ff)),
    ("firebrick", Rgb::from_packed(0xb22222)),
    ("floralwhite", Rgb::from_packed(0xfffaf0)),
    ("forestgreen", Rgb::from_packed(0x228b22)),
    ("fuchsia", Rgb::from_packed(0xff00ff)),
    ("gainsboro", Rgb::from_packed(0xdcdcdc)),
    ("ghostwhite", Rgb::from_packed(0xf8f8ff)),
    ("gold", Rgb::from_packed(0xffd700)),
    ("goldenrod", Rgb::from_packed(0xdaa520)),
    ("gray", Rgb::from_packed(0x808080)),
    ("green", Rgb::from_packed(0x008000)),
    ("greenyellow", Rgb::from_packed(0xadff2f)),
    ("grey", Rgb::from_packed(0x808080)),
    ("honeydew", Rgb::from_packed(0xf0fff0)),
    ("hotpink", Rgb::from_packed(0xff69b4)),
    ("indianred", Rgb::from_packed(0xcd5c5c)),
    ("indigo", Rgb::from_packed(0x4b0082)),
    ("ivory", Rgb::from_packed(0xfffff0)),
    ("khaki", Rgb::from_packed(0xf0e68c)),
    ("lavender", Rgb::from_packed(0xe6e6fa)),
    ("lavenderblush", Rgb::from_packed(0xfff0f5)),
    ("lawngreen", Rgb::from_packed(0x7cfc00)),
    ("lemonchiffon", Rgb::from_packed(0xfffacd)),
    ("lightblue", Rgb::from_packed(0xadd8e6)),
    ("lightcoral", Rgb::from_packed(0xf08080)),
    ("lightcyan", Rgb::from_packed(0xe0ffff)),
    ("lightgoldenrodyellow", Rgb::from_packed(0xfafad2)),
    ("lightgray", Rgb::from_packed(0xd3d3d3)),
    ("lightgreen", Rgb::from_packed(0x90ee90)),
    ("lightgrey", Rgb::from_packed(0xd3d3d3)),
    ("lightpink", Rgb::from_packed(0xffb6c1)),
    ("lightsalmon", Rgb::from_packed(0xffa07a)),
    ("lightseagreen", Rgb::from_packed(0x20b2aa)),
    ("lightskyblue", Rgb::from_packed(0x87cefa)),
    ("lightslategray", Rgb::from_packed(0x778899)),
    ("lightslategrey", Rgb::from_packed(0x778899)),
    ("lightsteelblue", Rgb::from_packed(0xb0c4de)),
    ("lightyellow", Rgb::from_packed(0xffffe0)),
    ("lime", Rgb::from_packed(0x00ff00)),
    ("limegreen", Rgb::from_packed(0x32cd32)),
    ("linen", Rgb::from_packed(0xfaf0e6)),
    ("magenta", Rgb::from_packed(0xff00ff)),
    ("maroon", Rgb::from_packed(0x800000)),
    ("mediumaquamarine", Rgb::from_packed(0x66cdaa)),
    ("mediumblue", Rgb::from_packed(0x0000cd)),
    ("mediumorchid", Rgb::from_packed(0xba55d3)),
    ("mediumpurple", Rgb::from_packed(0x9370db)),
    ("mediumseagreen", Rgb::from_packed(0x3cb371)),
    ("mediumslateblue", Rgb::from_packed(0x7b68ee)),
    ("mediumspringgreen", Rgb::from_packed(0x00fa9a)),
    ("mediumturquoise", Rgb::from_packed(0x48d1cc)),
    ("mediumvioletred", Rgb::from_packed(0xc71585)),
    ("midnightblue", Rgb::from_packed(0x191970)),
    ("mintcream", Rgb::from_packed(0xf5fffa)),
    ("mistyrose", Rgb::from_packed(0xffe4e1)),
    ("moccasin", Rgb::from_packed(0xffe4b5)),
    ("navajowhite", Rgb::from_packed(0xffdead)),
    ("navy", Rgb::from_packed(0x000080)),
    ("oldlace", Rgb::from_packed(0xfdf5e6)),
    ("olive", Rgb::from_packed(0x808000)),
    ("olivedrab", Rgb::from_packed(0x6b8e23)),
    ("orange", Rgb::from_packed(0xffa500)),
    ("orangered", Rgb::from_packed(0xff4500)),
    ("orchid", Rgb::from_packed(0xda70d6)),
    ("palegoldenrod", Rgb::from_packed(0xeee8aa)),
    ("palegreen", Rgb::from_packed(0x98fb98)),
    ("paleturquoise", Rgb::from_packed(0xafeeee)),
    ("palevioletred", Rgb::from_packed(0xdb7093)),
    ("papayawhip", Rgb::from_packed(0xffefd5)),
    ("peachpuff", Rgb::from_packed(0xffdab9)),
    ("peru", Rgb::from_packed(0xcd853f)),
    ("pink", Rgb::from_packed(0xffc0cb)),
    ("plum", Rgb::from_packed(0xdda0dd)),
    ("powderblue", Rgb::from_packed(0xb0e0e6)),
    ("purple", Rgb::from_packed(0x800080)),
    ("rebeccapurple", Rgb::from_packed(0x663399)),
    ("red", Rgb::from_packed(0xff0000)),
    ("rosybrown", Rgb::from_packed(0xbc8f8f)),
    ("royalblue", Rgb::from_packed(0x4169e1)),
    ("saddlebrown", Rgb::from_packed(0x8b4513)),
    ("salmon", Rgb::from_packed(0xfa8072)),
    ("sandybrown", Rgb::from_packed(0xf4a460)),
    ("seagreen", Rgb::from_packed(0x2e8b57)),
    ("seashell", Rgb::from_packed(0xfff5ee)),
    ("sienna", Rgb::from_packed(0xa0522d)),
    ("silver", Rgb::from_packed(0xc0c0c0)),
    ("skyblue", Rgb::from_packed(0x87ceeb)),
    ("slateblue", Rgb::from_packed(0x6a5acd)),
    ("slategray", Rgb::from_packed(0x708090)),
    ("slategrey", Rgb::from_packed(0x708090)),
    ("snow", Rgb::from_packed(0xfffafa)),
    ("springgreen", Rgb::from_packed(0x00ff7f)),
    ("steelblue", Rgb::from_packed(0x4682b4)),
    ("tan", Rgb::from_packed(0xd2b48c)),
    ("teal", Rgb::from_packed(0x008080)),
    ("thistle", Rgb::from_packed(0xd8bfd8)),
    ("tomato", Rgb::from_packed(0xff6347)),
    ("turquoise", Rgb::from_packed(0x40e0d0)),
    ("violet", Rgb::from_packed(0xee82ee)),
    ("wheat", Rgb::from_packed(0xf5deb3)),
    ("white", Rgb::from_packed(0xffffff)),
    ("whitesmoke", Rgb::from_packed(0xf5f5f5)),
    ("yellow", Rgb::from_packed(0xffff00)),
    ("yellowgreen", Rgb::from_packed(0x9acd32)),
];

/// Holds an 8-bit sRGB color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// Holds an 8-bit sRGB color with straight 8-bit alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rgba {
    pub rgb: Rgb,
    pub alpha: u8,
}

impl Rgb {
    pub const fn from_packed(packed: u32) -> Self {
        Rgb {
            red: (packed >> 16) as u8,
            green: (packed >> 8) as u8,
            blue: packed as u8,
        }
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Rgb { red, green, blue } = *self;
        write!(formatter, "#{red:02x}{green:02x}{blue:02x}")
    }
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba {
        rgb: Rgb::from_packed(0x000000),
        alpha: 0,
    };

    pub const fn opaque(rgb: Rgb) -> Self {
        Rgba {
            rgb,
            alpha: u8::MAX,
        }
    }

    pub const fn from_packed(packed: u32) -> Self {
        let rgb = Rgb::from_packed(packed >> 8);
        let alpha = packed as u8;
        Rgba { rgb, alpha }
    }

    /// Parse a hex color of 3, 4, 6 or 8 digits, a CSS named color or `transparent`, ignoring case.
    pub fn parse(text: &str) -> Option<Self> {
        let lowercase = text.to_ascii_lowercase();
        let is_transparent = lowercase == TRANSPARENT_KEYWORD;
        if is_transparent {
            return Some(Rgba::TRANSPARENT);
        }
        let named_color_index =
            NAMED_COLORS.binary_search_by_key(&lowercase.as_str(), |&(name, _rgb)| name);
        if let Ok(named_color_index) = named_color_index {
            let (_name, named_rgb) = NAMED_COLORS[named_color_index];
            let named_color = Rgba::opaque(named_rgb);
            return Some(named_color);
        }
        Self::parse_hex(&lowercase)
    }

    /// Parse `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` by first spelling it as `#rrggbbaa`.
    fn parse_hex(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        let has_non_hex_digit = digits.bytes().any(|digit| !digit.is_ascii_hexdigit());
        if has_non_hex_digit {
            return None;
        }
        let is_short_form = SHORT_HEX_DIGIT_COUNTS.contains(&digits.len());
        let long_digits: String = if is_short_form {
            digits.chars().flat_map(|digit| [digit, digit]).collect()
        } else {
            digits.to_owned()
        };
        let is_alpha_omitted = long_digits.len() == RGB_HEX_DIGIT_COUNT;
        let rgba_digits = if is_alpha_omitted {
            long_digits + OPAQUE_ALPHA_DIGITS
        } else {
            long_digits
        };
        let is_diff_rgba_length = rgba_digits.len() != RGBA_HEX_DIGIT_COUNT;
        if is_diff_rgba_length {
            return None;
        }
        let packed = u32::from_str_radix(&rgba_digits, 16).ok()?;
        let color = Rgba::from_packed(packed);
        Some(color)
    }

    pub fn is_opaque(self) -> bool {
        self.alpha == u8::MAX
    }

    pub fn is_transparent(self) -> bool {
        self.alpha == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors_parse_in_every_css_length() {
        let cases = [
            ("#3478F6", 0x3478f6ff),
            ("#0af", 0x00aaffff),
            ("#3478f680", 0x3478f680),
            ("#0af8", 0x00aaff88),
            ("#00000000", 0x00000000),
        ];
        for (text, packed) in cases {
            let parsed = Rgba::parse(text).expect("valid");
            assert_eq!(parsed, Rgba::from_packed(packed), "{text:?}");
        }
        let blue = Rgba::parse("#3478f6").expect("valid");
        assert_eq!(blue.rgb.to_string(), "#3478f6");
    }

    #[test]
    fn named_colors_and_transparent_parse_ignoring_case() {
        let red = Rgba::parse("Red").expect("named");
        assert_eq!(red, Rgba::from_packed(0xff0000ff));
        let rebecca_purple = Rgba::parse("rebeccapurple").expect("named");
        assert_eq!(rebecca_purple, Rgba::from_packed(0x663399ff));
        let transparent = Rgba::parse("TRANSPARENT").expect("keyword");
        assert!(transparent.is_transparent());
    }

    #[test]
    fn named_colors_are_sorted_for_binary_search() {
        let is_sorted = NAMED_COLORS.windows(2).all(|pair| pair[0].0 < pair[1].0);
        assert!(is_sorted);
    }

    #[test]
    fn malformed_colors_are_rejected() {
        let malformed = [
            "",
            "#",
            "3478f6",
            "#3478f",
            "#3478f6a",
            "#12",
            "#12345",
            "#123456789",
            "#+12345",
            "#ggg",
            "#12 34 5",
            "bleu",
            "none",
            "5",
        ];
        for text in malformed {
            assert_eq!(Rgba::parse(text), None, "{text:?}");
        }
    }
}
