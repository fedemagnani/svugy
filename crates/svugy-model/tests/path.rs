use svugy_model::path::{ElementPath, PathParseError};

fn parse(text: &str) -> ElementPath {
    text.parse().expect("well-formed path")
}

#[test]
fn a_quoted_id_containing_a_dot_stays_one_segment() {
    let path = parse(r#"backend."api.v2".handler"#);

    let segments = path.segments();
    assert_eq!(segments, ["backend", "api.v2", "handler"]);
}

#[test]
fn a_segment_containing_a_dot_is_displayed_quoted() {
    let path = ElementPath::from_segments(["backend", "api.v2"]);

    assert_eq!(path.to_string(), r#"backend."api.v2""#);
}

#[test]
fn display_then_parse_round_trips_any_segment_content() {
    let tricky_segments = [
        "plain",
        "",
        ".",
        "..",
        "\"",
        "\\",
        "\\\"",
        "a.b",
        " spaced id ",
        "quote\"inside",
        "é → ①",
        "\"already quoted\"",
        "trailing\\",
    ];
    for outer in tricky_segments {
        for inner in tricky_segments {
            let path = ElementPath::from_segments([outer, inner]);
            let displayed = path.to_string();
            let parsed: ElementPath = displayed.parse().expect("displayed paths parse");
            assert_eq!(parsed, path, "round trip of {displayed}");
        }
    }
}

#[test]
fn malformed_paths_are_rejected_with_their_offset() {
    let cases = [
        ("", PathParseError::Empty),
        ("a..b", PathParseError::EmptySegment { offset: 2 }),
        ("a.", PathParseError::EmptySegment { offset: 2 }),
        (r#""open"#, PathParseError::UnterminatedQuote { offset: 0 }),
        (
            r#"a"b"#,
            PathParseError::UnexpectedCharacter {
                character: '"',
                offset: 1,
            },
        ),
        (
            r#""a"b"#,
            PathParseError::UnexpectedCharacter {
                character: 'b',
                offset: 3,
            },
        ),
        (
            r#""\n""#,
            PathParseError::InvalidEscape {
                character: 'n',
                offset: 1,
            },
        ),
    ];
    for (text, expected_error) in cases {
        let parsed = text.parse::<ElementPath>();
        assert_eq!(parsed, Err(expected_error), "parsing {text:?}");
    }
}

#[test]
fn ancestors_are_the_proper_non_root_prefixes_outermost_first() {
    let path = parse("a.b.c");

    let ancestors: Vec<ElementPath> = path.ancestors().collect();

    assert_eq!(ancestors, [parse("a"), parse("a.b")]);
}

#[test]
fn lowest_common_ancestor_is_the_longest_shared_prefix() {
    let cousin = parse("a.b.c");
    let other_cousin = parse("a.d.e");
    let unrelated = parse("z");

    assert_eq!(cousin.lowest_common_ancestor(&other_cousin), parse("a"));
    assert_eq!(
        cousin.lowest_common_ancestor(&unrelated),
        ElementPath::root()
    );
}

#[test]
fn ancestry_is_proper_prefix_on_whole_segments() {
    let container = parse("a");
    let child = parse("a.b");
    let lookalike = parse("ab");

    assert!(container.is_ancestor_of(&child));
    assert!(!container.is_ancestor_of(&container));
    assert!(!container.is_ancestor_of(&lookalike));
    assert!(ElementPath::root().is_ancestor_of(&container));
}
