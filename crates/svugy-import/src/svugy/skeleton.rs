//! Read a diagram file's skeleton with comrak: its frontmatter, its sections, the nesting of its component items and each item's verbatim line.

use comrak::nodes::{AstNode, ListType, NodeValue, Sourcepos};
use comrak::{Arena, Options, parse_document};

use super::ast::Span;
use super::diagnostic::Diagnostics;

/// Spell CommonMark's HTML block type of a comment, `<!-- … -->`, the one HTML a diagram file may hold.
const COMMENT_BLOCK_TYPE: u8 = 2;
/// Delimit the YAML frontmatter at the very top of the file.
const FRONTMATTER_DELIMITER: &str = "---";
/// Spell the heading marker of a section.
const SECTION_MARKER: &str = "##";
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
        let arena = Arena::new();
        let mut options = Options::default();
        let frontmatter_delimiter = FRONTMATTER_DELIMITER.to_owned();
        options.extension.front_matter_delimiter = Some(frontmatter_delimiter);
        let root = parse_document(&arena, source, &options);
        let mut reader = BlockReader::new(source, diagnostics);
        let mut skeleton = Self::default();
        let mut components = ComponentLists::default();
        let mut section = Section::Preamble;
        let mut has_components_section = false;
        for block in root.children() {
            let ast = block.data.borrow();
            let block_span = reader.span(ast.sourcepos);
            match &ast.value {
                NodeValue::FrontMatter(frontmatter) => {
                    let frontmatter_body = Self::frontmatter_body(source, frontmatter);
                    skeleton.frontmatter = Some(frontmatter_body);
                }
                NodeValue::HtmlBlock(html) if html.block_type == COMMENT_BLOCK_TYPE => {}
                NodeValue::Heading(heading) if heading.level == 2 => {
                    let heading_line = reader.first_line(ast.sourcepos);
                    let next_section = reader.read_section_heading(heading_line, section);
                    has_components_section |= next_section == Section::Components;
                    let is_legend = next_section == Section::Legend;
                    if is_legend {
                        let title = Self::legend_title(heading_line.text);
                        let lines = Vec::new();
                        skeleton.legend = Some(LegendSkeleton { title, lines });
                    }
                    section = next_section;
                }
                NodeValue::List(list) if list.list_type == ListType::Bullet => match section {
                    Section::Components => components.push_list(block, &mut reader),
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
                        reader.diagnostics.record(message, block_span);
                    }
                    Section::Unknown => {}
                },
                _other_block => reader.diagnostics.record(NOT_A_BULLET_LIST, block_span),
            }
        }
        let has_no_component = components.nodes.is_empty();
        if has_no_component {
            let message = if has_components_section {
                "`## Components` holds at least one component"
            } else {
                "a diagram needs a `## Components` section"
            };
            let file_start = Span::from(0..0);
            reader.diagnostics.record(message, file_start);
        }
        skeleton.components = components.nodes;
        skeleton
    }

    /// Return the YAML between the frontmatter's delimiter lines, which open the file.
    fn frontmatter_body(source: &'src str, frontmatter: &str) -> SourceText<'src> {
        let frontmatter_length = frontmatter.len();
        let frontmatter_text = &source[..frontmatter_length];
        let opening_line_end = frontmatter_text.find('\n').map_or(0, |newline| newline + 1);
        let closing_start = frontmatter_text
            .rfind(FRONTMATTER_DELIMITER)
            .unwrap_or(frontmatter_length);
        let body_end = closing_start.max(opening_line_end);
        let text = &frontmatter_text[opening_line_end..body_end];
        SourceText {
            text,
            offset: opening_line_end,
        }
    }

    /// Return the title of a `## Legend: TITLE` heading, `None` for a bare `## Legend`.
    fn legend_title(heading_line: &'src str) -> Option<&'src str> {
        let (_legend_keyword, title) = heading_line.split_once(':')?;
        let title = title.trim();
        Some(title)
    }
}

impl<'src, 'diagnostics> BlockReader<'src, 'diagnostics> {
    /// Create a reader of blocks parsed from `source`, indexing where each of its lines starts.
    fn new(source: &'src str, diagnostics: &'diagnostics mut Diagnostics) -> Self {
        let newline_ends = source
            .match_indices('\n')
            .map(|(newline, _newline_text)| newline + 1);
        let line_starts = std::iter::once(0).chain(newline_ends).collect();
        Self {
            source,
            line_starts,
            diagnostics,
        }
    }

    /// Return the section a heading opens, recording an unknown heading and a known one out of order.
    fn read_section_heading(
        &mut self,
        heading_line: SourceText<'src>,
        previous: Section,
    ) -> Section {
        let heading_text = heading_line.text.trim_start_matches(SECTION_MARKER).trim();
        let section_name = heading_text.split(':').next().unwrap_or_default().trim();
        let heading_span = heading_line.span();
        let section = match section_name {
            "Components" => Section::Components,
            "Edges" => Section::Edges,
            "Legend" => Section::Legend,
            _unknown_name => {
                let message = format!(
                    "unknown section `## {heading_text}`: expected `## Components`, `## Edges` or `## Legend`"
                );
                self.diagnostics.record(message, heading_span);
                return Section::Unknown;
            }
        };
        let is_out_of_order = section <= previous && previous != Section::Unknown;
        if is_out_of_order {
            let message =
                "sections come once each, in the order `## Components`, `## Edges`, `## Legend`";
            self.diagnostics.record(message, heading_span);
        }
        section
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
                self.diagnostics.record(message, nested_span);
            }
            lines.push(line);
        }
        lines
    }

    /// Return the verbatim first line of `item`, recording an item with no text on its marker's line or with more than one line.
    fn read_item_line<'node>(&mut self, item: &'node AstNode<'node>) -> Option<SourceText<'src>> {
        let item_ast = item.data.borrow();
        let item_span = self.span(item_ast.sourcepos);
        let first_block = item.first_child();
        let paragraph = first_block.filter(|block| {
            let block_ast = block.data.borrow();
            matches!(block_ast.value, NodeValue::Paragraph)
        });
        let Some(paragraph) = paragraph else {
            self.diagnostics.record("an empty list item", item_span);
            return None;
        };
        let paragraph_sourcepos = paragraph.data.borrow().sourcepos;
        let is_multiline = paragraph_sourcepos.end.line > paragraph_sourcepos.start.line;
        if is_multiline {
            let paragraph_span = self.span(paragraph_sourcepos);
            self.diagnostics.record(NOT_ONE_LINE, paragraph_span);
            return None;
        }
        let line = self.first_line(paragraph_sourcepos);
        Some(line)
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
                    reader.diagnostics.record(NOT_ONE_LINE, nested_span);
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

impl<'src> SourceText<'src> {
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
