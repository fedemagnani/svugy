//! Read a diagram file's skeleton with comrak: its frontmatter, its sections, the nesting of its component items and each item's verbatim line.

use comrak::nodes::{AstNode, ListType, NodeValue, Sourcepos};
use comrak::{Arena, Options, parse_document};

use super::ast::Span;
use super::diagnostic::Diagnostics;

/// Spell CommonMark's HTML block type of a comment, `<!-- … -->`, the one HTML a diagram file may hold.
const COMMENT_BLOCK_TYPE: u8 = 2;
/// Delimit the YAML frontmatter at the very top of the file.
const FRONTMATTER_DELIMITER: &str = "---";
/// Spell the heading marker every heading opens with, repeated once per level.
const HEADING_MARKER: char = '#';
/// Name the heading level of a section, `##`.
const SECTION_LEVEL: u8 = 2;
/// Name the heading level of a document title, `#`.
const TITLE_LEVEL: u8 = 1;
/// Spell the UTF-8 byte order mark some editors open a file with.
const BYTE_ORDER_MARK: char = '\u{feff}';
/// Spell the opening of an HTML comment, which the language ignores.
const COMMENT_OPENING: &str = "<!--";
/// Spell the closing of an HTML comment.
const COMMENT_CLOSING: &str = "-->";
/// Spell the error of a block a section cannot hold.
const NOT_A_BULLET_LIST: &str = "a section holds bullet lists only";
/// Spell the error of an item spanning more than its first line.
const NOT_ONE_LINE: &str = "a list item is one line";

/// Hold a stretch of the source verbatim, with the byte offset it starts at.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceText<'src> {
    pub(crate) text: &'src str,
    pub(crate) offset: usize,
}

/// Hold a diagram file's skeleton: its frontmatter text, its component tree and the verbatim lines of its edges and legend.
#[derive(Debug, Default)]
pub(crate) struct Skeleton<'src> {
    pub(crate) frontmatter: Option<SourceText<'src>>,
    pub(crate) components: Vec<ComponentNode<'src>>,
    pub(crate) edges: Vec<SourceText<'src>>,
    pub(crate) legend: Option<LegendSkeleton<'src>>,
}

/// Nest a component item's verbatim line over its children's.
#[derive(Debug)]
pub(crate) struct ComponentNode<'src> {
    pub(crate) line: SourceText<'src>,
    pub(crate) is_indented_but_not_nested: bool,
    pub(crate) children: Vec<ComponentNode<'src>>,
}

/// Hold the `## Legend` section's title and its items' verbatim lines.
#[derive(Debug)]
pub(crate) struct LegendSkeleton<'src> {
    pub(crate) title: Option<&'src str>,
    pub(crate) lines: Vec<SourceText<'src>>,
}

/// Name the section a block falls in, in the order known sections must come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Section {
    Preamble,
    Components,
    Edges,
    Legend,
    Unknown,
}

/// Read each comrak block against the source, turning source positions into byte offsets and items into verbatim lines.
struct BlockReader<'src, 'diagnostics> {
    source: &'src str,
    line_starts: Vec<usize>,
    diagnostics: &'diagnostics mut Diagnostics,
}

/// Collect the component items of one parent's lists, flagging a later list indented deeper than the first, which a comment split off.
#[derive(Default)]
struct ComponentLists<'src> {
    first_item_column: Option<usize>,
    nodes: Vec<ComponentNode<'src>>,
}

impl<'src> Skeleton<'src> {
    /// Read the skeleton of `source`, recording every block out of place and every item that is not one line.
    pub(crate) fn read(source: &'src str, diagnostics: &mut Diagnostics) -> Self {
        let content_start = Self::content_start(source);
        let content = &source[content_start..];
        let arena = Arena::new();
        let mut options = Options::default();
        let frontmatter_delimiter = FRONTMATTER_DELIMITER.to_owned();
        options.extension.front_matter_delimiter = Some(frontmatter_delimiter);
        let root = parse_document(&arena, content, &options);
        let mut reader = BlockReader::new(source, content_start, diagnostics);
        let mut skeleton = Self::default();
        let mut components = ComponentLists::default();
        let mut section = Section::Preamble;
        let mut last_known_section = Section::Preamble;
        let mut components_heading_span = None;
        let mut has_component_list = false;
        for block in root.children() {
            let ast = block.data.borrow();
            let block_span = reader.span(ast.sourcepos);
            match &ast.value {
                NodeValue::FrontMatter(frontmatter) => {
                    let frontmatter_body =
                        Self::frontmatter_body(source, content_start, frontmatter);
                    skeleton.frontmatter = Some(frontmatter_body);
                }
                NodeValue::HtmlBlock(html) if html.block_type == COMMENT_BLOCK_TYPE => {}
                NodeValue::Heading(heading) => {
                    let heading_line = reader.first_line(ast.sourcepos);
                    let heading_text = Self::heading_text(heading_line.text);
                    let heading_span = heading_line.span();
                    let opened_section = reader.read_heading(
                        heading_text,
                        heading_span,
                        heading.level,
                        last_known_section,
                    );
                    let Some(opened_section) = opened_section else {
                        continue;
                    };
                    let is_known_section = opened_section != Section::Unknown;
                    if is_known_section {
                        last_known_section = opened_section;
                    }
                    let is_components = opened_section == Section::Components;
                    if is_components {
                        components_heading_span.get_or_insert(heading_span);
                    }
                    let is_legend = opened_section == Section::Legend;
                    if is_legend {
                        let title = Self::legend_title(heading_text);
                        let lines = Vec::new();
                        skeleton.legend = Some(LegendSkeleton { title, lines });
                    }
                    section = opened_section;
                }
                NodeValue::List(list) if list.list_type == ListType::Bullet => match section {
                    Section::Components => {
                        has_component_list = true;
                        components.push_list(block, &mut reader);
                    }
                    Section::Edges => {
                        let edge_lines = reader.read_flat_list(block);
                        skeleton.edges.extend(edge_lines);
                    }
                    Section::Legend => {
                        let legend_lines = reader.read_flat_list(block);
                        if let Some(legend) = skeleton.legend.as_mut() {
                            legend.lines.extend(legend_lines);
                        }
                    }
                    Section::Preamble => {
                        let message = "a list belongs under a section: `## Components`, `## Edges` or `## Legend`";
                        reader.diagnostics.record_error(message, block_span);
                    }
                    Section::Unknown => {}
                },
                _other_block => match section {
                    Section::Preamble => {
                        let message = "text before the first section goes in a comment, `<!-- … -->`, or in the frontmatter `subtitle`";
                        reader.diagnostics.record_error(message, block_span);
                    }
                    Section::Components | Section::Edges | Section::Legend => {
                        reader
                            .diagnostics
                            .record_error(NOT_A_BULLET_LIST, block_span);
                    }
                    Section::Unknown => {}
                },
            }
        }
        match (components_heading_span, has_component_list) {
            (None, _has_component_list) => {
                let file_start = Span::from(0..0);
                let message = "a diagram needs a `## Components` section";
                reader.diagnostics.record_error(message, file_start);
            }
            (Some(heading_span), false) => {
                let message = "`## Components` holds at least one component";
                reader.diagnostics.record_error(message, heading_span);
            }
            (Some(_heading_span), true) => {}
        }
        skeleton.components = components.nodes;
        skeleton
    }

    /// Return where the diagram's content starts: after a UTF-8 byte order mark and any blank lines, so frontmatter after them still opens the file.
    fn content_start(source: &str) -> usize {
        let after_byte_order_mark = source.strip_prefix(BYTE_ORDER_MARK).unwrap_or(source);
        let mut content_start = source.len() - after_byte_order_mark.len();
        for line in after_byte_order_mark.split_inclusive('\n') {
            let is_content = !line.trim().is_empty();
            if is_content {
                break;
            }
            content_start += line.len();
        }
        content_start
    }

    /// Return the YAML between the frontmatter's delimiter lines, which open the content at `content_start`.
    fn frontmatter_body(
        source: &'src str,
        content_start: usize,
        frontmatter: &str,
    ) -> SourceText<'src> {
        let frontmatter_end = content_start + frontmatter.len();
        let frontmatter_text = &source[content_start..frontmatter_end];
        let opening_line_end = frontmatter_text.find('\n').map_or(0, |newline| newline + 1);
        let closing_start = frontmatter_text
            .rfind(FRONTMATTER_DELIMITER)
            .unwrap_or(frontmatter_text.len());
        let body_end = closing_start.max(opening_line_end);
        let text = &frontmatter_text[opening_line_end..body_end];
        let offset = content_start + opening_line_end;
        SourceText { text, offset }
    }

    /// Return a heading's text without its opening `#` markers and its CommonMark closing sequence, ` ##`.
    fn heading_text(heading_line: &'src str) -> &'src str {
        let without_opening = heading_line.trim_start_matches(HEADING_MARKER).trim();
        let without_closing_markers = without_opening.trim_end_matches(HEADING_MARKER);
        let has_closing_sequence = without_closing_markers.ends_with([' ', '\t']);
        if has_closing_sequence {
            return without_closing_markers.trim_end();
        }
        without_opening
    }

    /// Return the title of a `## Legend: TITLE` heading's text, `None` for a bare `## Legend`.
    fn legend_title(heading_text: &'src str) -> Option<&'src str> {
        let (_legend_keyword, title) = heading_text.split_once(':')?;
        let title = title.trim();
        Some(title)
    }
}

impl<'src, 'diagnostics> BlockReader<'src, 'diagnostics> {
    /// Create a reader of blocks parsed from the content of `source` starting at `content_start`, indexing where each of its lines starts.
    fn new(
        source: &'src str,
        content_start: usize,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        let content = &source[content_start..];
        let newline_ends = content
            .match_indices('\n')
            .map(|(newline, _newline_text)| content_start + newline + 1);
        let line_starts = std::iter::once(content_start).chain(newline_ends).collect();
        Self {
            source,
            line_starts,
            diagnostics,
        }
    }

    /// Return the section a heading opens, `None` when it opens none, recording a heading that is not a known `##` section and a known one out of order.
    fn read_heading(
        &mut self,
        heading_text: &str,
        heading_span: Span,
        level: u8,
        last_known_section: Section,
    ) -> Option<Section> {
        let section_name = heading_text.split(':').next().unwrap_or_default().trim();
        let known_section = Section::named(section_name);
        let Some(section) = known_section else {
            let message = match level {
                SECTION_LEVEL => format!(
                    "unknown section `## {heading_text}`: expected `## Components`, `## Edges` or `## Legend`"
                ),
                TITLE_LEVEL => "the title goes in the frontmatter `title`".to_owned(),
                _other_level => NOT_A_BULLET_LIST.to_owned(),
            };
            self.diagnostics.record_error(message, heading_span);
            let is_section_level = level == SECTION_LEVEL;
            return is_section_level.then_some(Section::Unknown);
        };
        let is_wrong_level = level != SECTION_LEVEL;
        if is_wrong_level {
            let message = format!("use `## {section_name}`");
            self.diagnostics.record_error(message, heading_span);
        }
        let is_out_of_order = section <= last_known_section;
        if is_out_of_order {
            let message =
                "sections come once each, in the order `## Components`, `## Edges`, `## Legend`";
            self.diagnostics.record_error(message, heading_span);
        }
        Some(section)
    }

    /// Read each item of a list that does not nest, as the edges and the legend are, into its verbatim line.
    fn read_flat_list<'node>(&mut self, list: &'node AstNode<'node>) -> Vec<SourceText<'src>> {
        let mut lines = Vec::new();
        for item in list.children() {
            let Some(line) = self.read_item_line(item) else {
                continue;
            };
            let nested_blocks = item.children().skip(1);
            for nested_block in nested_blocks {
                let nested_ast = nested_block.data.borrow();
                let nested_span = self.span(nested_ast.sourcepos);
                let is_nested_list = matches!(nested_ast.value, NodeValue::List(_list));
                let message = if is_nested_list {
                    "only `## Components` items nest"
                } else {
                    NOT_ONE_LINE
                };
                self.diagnostics.record_error(message, nested_span);
            }
            lines.push(line);
        }
        lines
    }

    /// Return the verbatim first line of `item` without its trailing comments, recording an item with no text on its marker's line or with more than one line.
    fn read_item_line<'node>(&mut self, item: &'node AstNode<'node>) -> Option<SourceText<'src>> {
        let item_ast = item.data.borrow();
        let item_span = self.span(item_ast.sourcepos);
        let first_block = item.first_child();
        let paragraph = first_block.filter(|block| {
            let block_ast = block.data.borrow();
            matches!(block_ast.value, NodeValue::Paragraph)
        });
        let Some(paragraph) = paragraph else {
            self.diagnostics
                .record_error("an empty list item", item_span);
            return None;
        };
        let paragraph_sourcepos = paragraph.data.borrow().sourcepos;
        let is_multiline = paragraph_sourcepos.end.line > paragraph_sourcepos.start.line;
        if is_multiline {
            let paragraph_span = self.span(paragraph_sourcepos);
            self.diagnostics.record_error(NOT_ONE_LINE, paragraph_span);
            return None;
        }
        let line = self.first_line(paragraph_sourcepos);
        let item_line = line.without_trailing_comments();
        Some(item_line)
    }

    /// Return the source from where `sourcepos` starts to the end of that line, trailing whitespace dropped.
    fn first_line(&self, sourcepos: Sourcepos) -> SourceText<'src> {
        let start = self.offset(sourcepos.start.line, sourcepos.start.column);
        let rest_of_source = &self.source[start..];
        let line_length = rest_of_source.find('\n').unwrap_or(rest_of_source.len());
        let line_text = &rest_of_source[..line_length];
        let text = line_text.trim_end();
        SourceText {
            text,
            offset: start,
        }
    }

    /// Return the byte range `sourcepos` covers, its end column inclusive.
    fn span(&self, sourcepos: Sourcepos) -> Span {
        let start = self.offset(sourcepos.start.line, sourcepos.start.column);
        let last_byte = self.offset(sourcepos.end.line, sourcepos.end.column);
        let end = (last_byte + 1).max(start);
        Span::from(start..end)
    }

    /// Return the byte offset of the 1-based `line` and byte `column`.
    fn offset(&self, line: usize, column: usize) -> usize {
        let line_index = line.saturating_sub(1);
        let line_start = self
            .line_starts
            .get(line_index)
            .copied()
            .unwrap_or(self.source.len());
        let column_offset = column.saturating_sub(1);
        let offset = line_start + column_offset;
        offset.min(self.source.len())
    }
}

impl<'src> ComponentLists<'src> {
    /// Read the items of `list`, one more list of this parent, nesting each item's own lists under it.
    fn push_list<'node>(
        &mut self,
        list: &'node AstNode<'node>,
        reader: &mut BlockReader<'src, '_>,
    ) {
        for (item_index, item) in list.children().enumerate() {
            let item_column = item.data.borrow().sourcepos.start.column;
            let first_item_column = *self.first_item_column.get_or_insert(item_column);
            let is_list_start = item_index == 0;
            let is_indented_but_not_nested = is_list_start && item_column > first_item_column;
            let Some(node) = Self::read_item(item, is_indented_but_not_nested, reader) else {
                continue;
            };
            self.nodes.push(node);
        }
    }

    /// Read a component item into its line and the items of every list nested under it.
    fn read_item<'node>(
        item: &'node AstNode<'node>,
        is_indented_but_not_nested: bool,
        reader: &mut BlockReader<'src, '_>,
    ) -> Option<ComponentNode<'src>> {
        let line = reader.read_item_line(item)?;
        let mut children = Self::default();
        let nested_blocks = item.children().skip(1);
        for nested_block in nested_blocks {
            let nested_ast = nested_block.data.borrow();
            match &nested_ast.value {
                NodeValue::List(list) if list.list_type == ListType::Bullet => {
                    children.push_list(nested_block, reader);
                }
                NodeValue::HtmlBlock(html) if html.block_type == COMMENT_BLOCK_TYPE => {}
                _other_block => {
                    let nested_span = reader.span(nested_ast.sourcepos);
                    reader.diagnostics.record_error(NOT_ONE_LINE, nested_span);
                }
            }
        }
        Some(ComponentNode {
            line,
            is_indented_but_not_nested,
            children: children.nodes,
        })
    }
}

impl Section {
    /// Return the known section a heading names, whatever its level.
    fn named(section_name: &str) -> Option<Self> {
        match section_name {
            "Components" => Some(Self::Components),
            "Edges" => Some(Self::Edges),
            "Legend" => Some(Self::Legend),
            _unknown_name => None,
        }
    }
}

impl<'src> SourceText<'src> {
    /// Return the text without the HTML comments, `<!-- … -->`, it ends with.
    fn without_trailing_comments(self) -> Self {
        let mut text = self.text;
        while let Some(before_closing) = text.strip_suffix(COMMENT_CLOSING) {
            let Some(comment_start) = before_closing.rfind(COMMENT_OPENING) else {
                break;
            };
            text = text[..comment_start].trim_end();
        }
        Self {
            text,
            offset: self.offset,
        }
    }

    /// Return the byte range the text covers in the source.
    pub(crate) fn span(&self) -> Span {
        let end = self.offset + self.text.len();
        Span::from(self.offset..end)
    }

    /// Return the part of the text `span`, a range of the whole source inside this text, covers.
    pub(crate) fn slice(&self, span: Span) -> &'src str {
        let start = span.start - self.offset;
        let end = span.end - self.offset;
        &self.text[start..end]
    }
}
