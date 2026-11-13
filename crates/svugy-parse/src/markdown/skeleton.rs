//! Read a diagram file's skeleton with comrak: its frontmatter, its sections, the nesting of its component items, its workflow and step headings with their prose, each item's verbatim line and the description under it.

use comrak::nodes::{AstNode, ListType, NodeCodeBlock, NodeHeading, NodeValue, Sourcepos};
use comrak::{Arena, Options, parse_document};

use super::ast::{Description, Span};
use super::diagnostic::Diagnostics;

/// Spell CommonMark's HTML block type of a comment, `<!-- … -->`, the one HTML a diagram file may hold outside prose.
const COMMENT_BLOCK_TYPE: u8 = 2;
/// Delimit the YAML frontmatter at the very top of the file.
const FRONTMATTER_DELIMITER: &str = "---";
/// Spell the bullet of a list of items; a `*` or `+` list is prose text.
const ITEM_BULLET: u8 = b'-';
/// Spell the heading marker every heading opens with, repeated once per level.
const HEADING_MARKER: char = '#';
/// Name the heading level of a document title, `#`.
const TITLE_LEVEL: u8 = 1;
/// Name the heading level of a section, `##`.
const SECTION_LEVEL: u8 = 2;
/// Name the heading level of a workflow, `###`.
const WORKFLOW_LEVEL: u8 = 3;
/// Name the heading level of a step, `####`.
const STEP_LEVEL: u8 = 4;
/// Spell the UTF-8 byte order mark some editors open a file with.
const BYTE_ORDER_MARK: char = '\u{feff}';
/// Spell the opening of an HTML comment, which the language ignores.
const COMMENT_OPENING: &str = "<!--";
/// Spell the closing of an HTML comment.
const COMMENT_CLOSING: &str = "-->";
/// Spell the info string of the fenced code block that holds a description.
pub(crate) const DESCRIPTION_INFO: &str = "description";
/// Spell the error of a block `## Components` cannot hold.
const NOT_A_BULLET_LIST: &str = "`## Components` holds a `- ` bullet list only";
/// Spell the error of text written outside a description block where only one may hold it.
const UNWRAPPED_DESCRIPTION: &str = "wrap a description in a ```description block";
/// Spell the error of a code block whose info string is not `description`.
const NOT_A_DESCRIPTION_BLOCK: &str = "the only code block allowed here is ```description";
/// Spell the error of text after a workflow's overrides or a step's items.
const TEXT_AFTER_ITEMS: &str =
    "text after the list: prose goes between the heading and the first item";
/// Spell the v3 error of a description block before `## Components`.
const MOVED_DIAGRAM_DESCRIPTION: &str = "a ```description block before `## Components` is v3: write it as the first workflow's prose, unfenced";

/// Hold a stretch of the source verbatim, with the byte offset it starts at.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceText<'src> {
    pub(crate) text: &'src str,
    pub(crate) offset: usize,
}

/// Hold a diagram file's skeleton: its frontmatter text, its component tree and its workflows.
#[derive(Debug, Default)]
pub(crate) struct Skeleton<'src> {
    pub(crate) frontmatter: Option<SourceText<'src>>,
    pub(crate) components: Vec<ComponentNode<'src>>,
    pub(crate) workflows: Vec<WorkflowSkeleton<'src>>,
}

/// Hold an item's verbatim line and the content of the description block under it.
#[derive(Debug)]
pub(crate) struct DescribedLine<'src> {
    pub(crate) line: SourceText<'src>,
    pub(crate) description: Option<Description>,
}

/// Nest a component item over its children's.
#[derive(Debug)]
pub(crate) struct ComponentNode<'src> {
    pub(crate) item: DescribedLine<'src>,
    pub(crate) is_indented_but_not_nested: bool,
    pub(crate) children: Vec<ComponentNode<'src>>,
}

/// Hold a workflow as written: its heading with its prose and overrides, then its steps.
#[derive(Debug)]
pub(crate) struct WorkflowSkeleton<'src> {
    pub(crate) part: HeadedPart<'src>,
    pub(crate) steps: Vec<HeadedPart<'src>>,
}

/// Hold what a workflow or step heading opens: the heading's text, its prose, then the items of its list.
#[derive(Debug)]
pub(crate) struct HeadedPart<'src> {
    pub(crate) heading: SourceText<'src>,
    pub(crate) prose: Option<Description>,
    pub(crate) items: Vec<DescribedLine<'src>>,
}

/// Name the `##` section a block falls in, the known ones in the order they must come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Section {
    Preamble,
    Components,
    Workflows,
    Skipped,
}

/// Classify a block outside a heading: a comment the language ignores, a bullet list of items, a description block, another code block, or any other block.
#[derive(Debug, Clone, Copy)]
enum BlockRole<'node> {
    Comment,
    BulletList,
    Description(&'node NodeCodeBlock),
    OtherCodeBlock(&'node NodeCodeBlock),
    Unwrapped,
}

/// Read each comrak block against the source, turning source positions into byte offsets and items into verbatim lines and descriptions.
struct BlockReader<'src, 'diagnostics> {
    source: &'src str,
    line_starts: Vec<usize>,
    diagnostics: &'diagnostics mut Diagnostics,
}

/// Hold an item as read: its line and description, and the bullet lists nested after them.
struct ItemBlocks<'src, 'node> {
    item: DescribedLine<'src>,
    nested_lists: Vec<&'node AstNode<'node>>,
}

/// Collect the component items of one parent's lists, flagging a later list indented deeper than the first, which a comment split off.
#[derive(Default)]
struct ComponentLists<'src> {
    first_item_column: Option<usize>,
    nodes: Vec<ComponentNode<'src>>,
}

/// Collect the workflows of `## Workflows` with their steps, closing the open heading's prose where its list or the next heading starts.
#[derive(Default)]
struct WorkflowParts<'src> {
    workflows: Vec<WorkflowSkeleton<'src>>,
    open_prose: Option<OpenProse>,
}

/// Track the prose under the last heading while no list or heading has closed it: its first line and the comments read in it so far.
struct OpenProse {
    first_line: usize,
    comments: Vec<Span>,
}

impl<'src> Skeleton<'src> {
    /// Read the skeleton of `source`, recording every block out of place and each required section missing or empty.
    pub(crate) fn read(source: &'src str, diagnostics: &mut Diagnostics) -> Self {
        let content_start = Self::content_start(source);
        let content = &source[content_start..];
        let arena = Arena::new();
        let mut options = Options::default();
        let frontmatter_delimiter = FRONTMATTER_DELIMITER.to_owned();
        options.extension.front_matter_delimiter = Some(frontmatter_delimiter);
        let root = parse_document(&arena, content, &options);
        let mut reader = BlockReader::new(source, content_start, diagnostics);
        let mut frontmatter = None;
        let mut components = ComponentLists::default();
        let mut workflow_parts = WorkflowParts::default();
        let mut section = Section::Preamble;
        let mut last_known_section = Section::Preamble;
        let mut components_heading_span = None;
        let mut workflows_heading_span = None;
        let mut has_component_list = false;
        for block in root.children() {
            let ast = block.data.borrow();
            let block_span = reader.span(ast.sourcepos);
            let has_workflow = !workflow_parts.workflows.is_empty();
            match &ast.value {
                NodeValue::FrontMatter(frontmatter_text) => {
                    let frontmatter_body =
                        Self::frontmatter_body(source, content_start, frontmatter_text);
                    frontmatter = Some(frontmatter_body);
                }
                NodeValue::Heading(NodeHeading {
                    level,
                    setext: false,
                    ..
                }) => {
                    let heading_text = reader.heading_text(ast.sourcepos);
                    let heading_line = ast.sourcepos.start.line;
                    let is_in_workflows = section == Section::Workflows;
                    match *level {
                        SECTION_LEVEL => {
                            workflow_parts.close_open_prose(&reader, heading_line);
                            section = reader.open_section(heading_text, last_known_section);
                            let is_known_section = section != Section::Skipped;
                            if is_known_section {
                                last_known_section = section;
                            }
                            let heading_span = heading_text.span();
                            match section {
                                Section::Components => {
                                    components_heading_span.get_or_insert(heading_span);
                                }
                                Section::Workflows => {
                                    workflows_heading_span.get_or_insert(heading_span);
                                }
                                Section::Preamble | Section::Skipped => {}
                            }
                        }
                        WORKFLOW_LEVEL if is_in_workflows => {
                            workflow_parts.open_workflow(&reader, heading_text, heading_line);
                        }
                        STEP_LEVEL if is_in_workflows && has_workflow => {
                            workflow_parts.open_step(&reader, heading_text, heading_line);
                        }
                        level => reader.reject_heading(level, block_span),
                    }
                }
                block_value => {
                    let block_role = BlockRole::of(block_value);
                    match (block_role, section, has_workflow) {
                        (block_role, Section::Workflows, true) => {
                            workflow_parts.read_block(&mut reader, block, block_role);
                        }
                        (BlockRole::Comment, _any_section, _has_workflow) => {}
                        (_any_role, Section::Skipped, _has_workflow) => {}
                        (BlockRole::Description(_code_block), Section::Preamble, _has_workflow) => {
                            reader
                                .diagnostics
                                .record_error(MOVED_DIAGRAM_DESCRIPTION, block_span);
                        }
                        (_other_role, Section::Preamble, _has_workflow) => {
                            let message = "nothing but the frontmatter goes before `## Components`: write prose under a workflow or step heading";
                            reader.diagnostics.record_error(message, block_span);
                        }
                        (BlockRole::BulletList, Section::Components, _has_workflow) => {
                            has_component_list = true;
                            components.push_list(block, &mut reader);
                        }
                        (_non_list_role, Section::Components, _has_workflow) => {
                            reader
                                .diagnostics
                                .record_error(NOT_A_BULLET_LIST, block_span);
                        }
                        (_any_role, Section::Workflows, false) => {
                            let message =
                                "`## Workflows` holds workflows, each opened by `### ID: TITLE`";
                            reader.diagnostics.record_error(message, block_span);
                        }
                    }
                }
            }
        }
        let line_after_source = reader.line_starts.len() + 1;
        workflow_parts.close_open_prose(&reader, line_after_source);
        let has_workflow = !workflow_parts.workflows.is_empty();
        let required_sections = [
            (
                components_heading_span,
                has_component_list,
                "a diagram needs a `## Components` section",
                "`## Components` holds at least one component",
            ),
            (
                workflows_heading_span,
                has_workflow,
                "a diagram needs a `## Workflows` section",
                "`## Workflows` holds at least one workflow",
            ),
        ];
        for (heading_span, is_filled, missing_message, empty_message) in required_sections {
            match (heading_span, is_filled) {
                (None, _is_filled) => {
                    let file_start = Span::from(0..0);
                    reader.diagnostics.record_error(missing_message, file_start);
                }
                (Some(heading_span), false) => {
                    reader.diagnostics.record_error(empty_message, heading_span);
                }
                (Some(_heading_span), true) => {}
            }
        }
        Self {
            frontmatter,
            components: components.nodes,
            workflows: workflow_parts.workflows,
        }
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
}

impl Skeleton<'_> {
    /// Tell whether `fenced_description`, `description` in a description block under an item, reads back as `description` with a child item after it still nested.
    pub(crate) fn reads_back_as_description(description: &str, fenced_description: &str) -> bool {
        let source = format!(
            "## Components\n- item\n{fenced_description}  - child\n\n## Workflows\n### w: t\n"
        );
        let mut diagnostics = Diagnostics::default();
        let skeleton = Skeleton::read(&source, &mut diagnostics);
        let has_errors = !diagnostics.errors.is_empty();
        if has_errors {
            return false;
        }
        let Some(node) = skeleton.components.first() else {
            return false;
        };
        let has_one_child = node.children.len() == 1;
        let read_description = node.item.description.as_ref();
        let read_text = read_description.map(|read_description| read_description.text.as_str());
        let is_read_back_verbatim = read_text == Some(description);
        has_one_child && is_read_back_verbatim
    }

    /// Tell whether `prose`, written under a heading and followed by a blank line and a list, reads back as `prose` with the list still after it.
    pub(crate) fn reads_back_as_prose(prose: &str) -> bool {
        let source = format!("## Components\n- a\n\n## Workflows\n### w: t\n{prose}\n\n- a ()\n");
        let mut diagnostics = Diagnostics::default();
        let skeleton = Skeleton::read(&source, &mut diagnostics);
        let has_errors = !diagnostics.errors.is_empty();
        if has_errors {
            return false;
        }
        let Some(workflow) = skeleton.workflows.first() else {
            return false;
        };
        let has_one_item = workflow.part.items.len() == 1;
        let read_prose = workflow.part.prose.as_ref();
        let read_text = read_prose.map(|read_prose| read_prose.text.as_str());
        let is_read_back_verbatim = read_text == Some(prose);
        has_one_item && is_read_back_verbatim
    }
}

impl<'src> WorkflowParts<'src> {
    /// Open a workflow at `heading`, on `heading_line`, after closing the open prose.
    fn open_workflow(
        &mut self,
        reader: &BlockReader<'src, '_>,
        heading: SourceText<'src>,
        heading_line: usize,
    ) {
        self.close_open_prose(reader, heading_line);
        let part = HeadedPart::new(heading);
        let steps = Vec::new();
        let workflow = WorkflowSkeleton { part, steps };
        self.workflows.push(workflow);
        let open_prose = OpenProse::under_heading(heading_line);
        self.open_prose = Some(open_prose);
    }

    /// Open a step of the last workflow at `heading`, on `heading_line`, after closing the open prose.
    fn open_step(
        &mut self,
        reader: &BlockReader<'src, '_>,
        heading: SourceText<'src>,
        heading_line: usize,
    ) {
        self.close_open_prose(reader, heading_line);
        let Some(last_workflow) = self.workflows.last_mut() else {
            return;
        };
        let step = HeadedPart::new(heading);
        last_workflow.steps.push(step);
        let open_prose = OpenProse::under_heading(heading_line);
        self.open_prose = Some(open_prose);
    }

    /// Read `block` into the open part: comments left out of its prose, items from its list, recording text after the prose, a description block in it and a code fence never closed in it.
    fn read_block<'node>(
        &mut self,
        reader: &mut BlockReader<'src, '_>,
        block: &'node AstNode<'node>,
        block_role: BlockRole<'_>,
    ) {
        let sourcepos = block.data.borrow().sourcepos;
        let block_span = reader.span(sourcepos);
        let is_past_prose = self.open_prose.is_none();
        match block_role {
            BlockRole::BulletList => {
                self.close_open_prose(reader, sourcepos.start.line);
                let items = reader.read_flat_list(block);
                let Some(open_part) = self.open_part_mut() else {
                    return;
                };
                open_part.items.extend(items);
            }
            BlockRole::Comment if is_past_prose => {}
            _prose_role if is_past_prose => {
                reader
                    .diagnostics
                    .record_error(TEXT_AFTER_ITEMS, block_span);
            }
            BlockRole::Description(_code_block) => {
                let message = "workflow and step prose is unfenced: drop the ```description fence";
                reader.diagnostics.record_error(message, block_span);
            }
            BlockRole::OtherCodeBlock(code_block) => {
                let is_unclosed = code_block.fenced && !code_block.closed;
                if is_unclosed {
                    let message =
                        "a code block in prose needs a closing fence as long as its opening one";
                    reader.diagnostics.record_error(message, block_span);
                }
            }
            BlockRole::Comment | BlockRole::Unwrapped => {
                let block_comments = reader.comments_in(block);
                let Some(open_prose) = &mut self.open_prose else {
                    return;
                };
                open_prose.comments.extend(block_comments);
            }
        }
    }

    /// Close the open prose before the 1-based `end_line`, storing it in the open part.
    fn close_open_prose(&mut self, reader: &BlockReader<'src, '_>, end_line: usize) {
        let Some(open_prose) = self.open_prose.take() else {
            return;
        };
        let prose = reader.prose(open_prose, end_line);
        let Some(open_part) = self.open_part_mut() else {
            return;
        };
        open_part.prose = prose;
    }

    /// Return the part the next block falls in: the last workflow's last step, else its own heading part.
    fn open_part_mut(&mut self) -> Option<&mut HeadedPart<'src>> {
        let last_workflow = self.workflows.last_mut()?;
        let last_step = last_workflow.steps.last_mut();
        let open_part = last_step.unwrap_or(&mut last_workflow.part);
        Some(open_part)
    }
}

impl<'src> HeadedPart<'src> {
    /// Create a new part opened by `heading`, holding no prose or item yet.
    fn new(heading: SourceText<'src>) -> Self {
        Self {
            heading,
            prose: None,
            items: Vec::new(),
        }
    }
}

impl OpenProse {
    /// Open the prose under the heading on `heading_line`, starting on the next line.
    fn under_heading(heading_line: usize) -> Self {
        let first_line = heading_line + 1;
        Self {
            first_line,
            comments: Vec::new(),
        }
    }
}

impl<'src, 'diagnostics> BlockReader<'src, 'diagnostics> {
    /// Create a reader of blocks parsed from the content of `source` starting at `content_start`, indexing where each of its lines starts.
    fn new(
        source: &'src str,
        content_start: usize,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        let source_bytes = source.as_bytes();
        let content_bytes = &source_bytes[content_start..];
        let line_break_ends = content_bytes
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| {
                let next_byte = content_bytes.get(index + 1);
                let is_lone_carriage_return = *byte == b'\r' && next_byte != Some(&b'\n');
                let ends_line_break = *byte == b'\n' || is_lone_carriage_return;
                ends_line_break.then_some(content_start + index + 1)
            });
        let line_starts = std::iter::once(content_start)
            .chain(line_break_ends)
            .collect();
        Self {
            source,
            line_starts,
            diagnostics,
        }
    }

    /// Return the section a `##` heading opens, recording a heading that is not a known section and a known one out of order; an unknown section's blocks are skipped.
    fn open_section(
        &mut self,
        heading_text: SourceText<'_>,
        last_known_section: Section,
    ) -> Section {
        let heading_span = heading_text.span();
        let section_name = heading_text.text;
        let Some(section) = Section::named(section_name) else {
            let message = Section::unknown_section_message(section_name);
            self.diagnostics.record_error(message, heading_span);
            return Section::Skipped;
        };
        let is_out_of_order = section <= last_known_section;
        if is_out_of_order {
            let message = "sections come once each, in the order `## Components`, `## Workflows`";
            self.diagnostics.record_error(message, heading_span);
        }
        section
    }

    /// Record a heading of `level` where it opens nothing.
    fn reject_heading(&mut self, level: u8, heading_span: Span) {
        let message = match level {
            TITLE_LEVEL => {
                "the diagram's title goes in the first workflow's heading: `### ID: TITLE`"
            }
            WORKFLOW_LEVEL => "a workflow heading `### ID: TITLE` goes under `## Workflows`",
            STEP_LEVEL => {
                "a step heading `#### ID: TITLE` goes under a workflow heading `### ID: TITLE`"
            }
            _other_level => {
                "headings are `##` for a section, `###` for a workflow and `####` for a step"
            }
        };
        self.diagnostics.record_error(message, heading_span);
    }

    /// Return the byte ranges of the HTML comments `block` holds: each `<!-- … -->` of its comment blocks, text after a `-->` kept, and each of its inline comments.
    fn comments_in<'node>(&self, block: &'node AstNode<'node>) -> Vec<Span> {
        let mut comments = Vec::new();
        for node in block.descendants() {
            let ast = node.data.borrow();
            let node_span = self.span(ast.sourcepos);
            match &ast.value {
                NodeValue::HtmlBlock(html) if html.block_type == COMMENT_BLOCK_TYPE => {
                    let block_comments = self.comments_spelled_in(node_span);
                    comments.extend(block_comments);
                }
                NodeValue::HtmlInline(html) if html.starts_with(COMMENT_OPENING) => {
                    comments.push(node_span);
                }
                _other_node => {}
            }
        }
        comments
    }

    /// Return the byte ranges of the `<!-- … -->` comments spelled in `span` of the source, an unclosed one running to its end.
    fn comments_spelled_in(&self, span: Span) -> Vec<Span> {
        let mut comments = Vec::new();
        let mut search_start = span.start;
        loop {
            let unsearched_text = &self.source[search_start..span.end];
            let Some(opening_index) = unsearched_text.find(COMMENT_OPENING) else {
                break;
            };
            let comment_start = search_start + opening_index;
            let body_start = comment_start + COMMENT_OPENING.len();
            let comment_body_onward = &self.source[body_start..span.end];
            let closing_index = comment_body_onward.find(COMMENT_CLOSING);
            let comment_end = closing_index.map_or(span.end, |closing_index| {
                body_start + closing_index + COMMENT_CLOSING.len()
            });
            let comment = Span::from(comment_start..comment_end);
            comments.push(comment);
            search_start = comment_end;
        }
        comments
    }

    /// Return the prose from `open_prose`'s first line to before the 1-based `end_line`: verbatim but for its comments, each taking one of the spaces around it, line breaks as `\n`, blank lines trimmed at both ends; `None` when blank.
    fn prose(&self, open_prose: OpenProse, end_line: usize) -> Option<Description> {
        let OpenProse {
            first_line,
            comments,
        } = open_prose;
        let prose_start = self.offset(first_line, 1);
        let prose_end = self.offset(end_line, 1);
        let mut uncommented = String::new();
        let mut kept_start = prose_start;
        for comment in comments {
            let comment_start = comment.start.clamp(kept_start, prose_end);
            let kept_text = &self.source[kept_start..comment_start];
            uncommented.push_str(kept_text);
            let comment_end = comment.end.clamp(comment_start, prose_end);
            let after_comment = &self.source[comment_end..prose_end];
            let is_spaced_before = uncommented.ends_with(' ');
            let is_spaced_after = after_comment.starts_with(' ');
            let is_spaced_on_both_sides = is_spaced_before && is_spaced_after;
            kept_start = if is_spaced_on_both_sides {
                comment_end + 1
            } else {
                comment_end
            };
        }
        let kept_tail = &self.source[kept_start..prose_end];
        uncommented.push_str(kept_tail);
        let with_line_feeds = uncommented.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = with_line_feeds.split('\n').collect();
        let is_written = |line: &&str| !line.trim().is_empty();
        let first_written = lines.iter().position(is_written)?;
        let last_written = lines.iter().rposition(is_written)?;
        let written_lines = &lines[first_written..=last_written];
        let text = written_lines.join("\n");
        let prose_source = &self.source[prose_start..prose_end];
        let leading_blank_length = prose_source.len() - prose_source.trim_start().len();
        let span_start = prose_start + leading_blank_length;
        let span_end = prose_start + prose_source.trim_end().len();
        let span = Span::from(span_start..span_end);
        let prose = Description { text, span };
        Some(prose)
    }

    /// Read each item of a list that does not nest, as overrides and step items are, into its line and description, recording a list nested under one.
    fn read_flat_list<'node>(&mut self, list: &'node AstNode<'node>) -> Vec<DescribedLine<'src>> {
        let mut items = Vec::new();
        for item in list.children() {
            let Some(item_blocks) = self.read_item(item) else {
                continue;
            };
            let ItemBlocks { item, nested_lists } = item_blocks;
            for nested_list in nested_lists {
                let nested_sourcepos = nested_list.data.borrow().sourcepos;
                let nested_span = self.span(nested_sourcepos);
                self.diagnostics
                    .record_error("only `## Components` items nest", nested_span);
            }
            items.push(item);
        }
        items
    }

    /// Read `item` into its first line, the description block under it and the bullet lists nested after them, recording any other block and a description after a nested list; `None` for an item with no text on its marker's line.
    fn read_item<'node>(&mut self, item: &'node AstNode<'node>) -> Option<ItemBlocks<'src, 'node>> {
        let item_sourcepos = item.data.borrow().sourcepos;
        let first_block = item.first_child();
        let paragraph = first_block.filter(|block| {
            let block_ast = block.data.borrow();
            matches!(block_ast.value, NodeValue::Paragraph)
        });
        let Some(paragraph) = paragraph else {
            let item_span = self.span(item_sourcepos);
            self.diagnostics
                .record_error("an empty list item", item_span);
            return None;
        };
        let paragraph_sourcepos = paragraph.data.borrow().sourcepos;
        let first_line = self.first_line(paragraph_sourcepos);
        let line = first_line.without_trailing_comments();
        let is_multiline = paragraph_sourcepos.end.line > paragraph_sourcepos.start.line;
        if is_multiline {
            let second_line = paragraph_sourcepos.start.line + 1;
            let continuation_start = self.offset(second_line, 1);
            let paragraph_span = self.span(paragraph_sourcepos);
            let continuation_span = Span::from(continuation_start..paragraph_span.end);
            self.diagnostics
                .record_error(UNWRAPPED_DESCRIPTION, continuation_span);
        }
        let mut description = None;
        let mut nested_lists = Vec::new();
        let nested_blocks = item.children().skip(1);
        for nested_block in nested_blocks {
            let nested_ast = nested_block.data.borrow();
            let nested_sourcepos = nested_ast.sourcepos;
            let nested_span = self.span(nested_sourcepos);
            let block_role = BlockRole::of(&nested_ast.value);
            match block_role {
                BlockRole::Comment => {}
                BlockRole::BulletList => nested_lists.push(nested_block),
                BlockRole::Description(code_block) => {
                    let has_nested_lists = !nested_lists.is_empty();
                    if has_nested_lists {
                        let message = "a description goes between its item's line and the items nested under it";
                        self.diagnostics.record_error(message, nested_span);
                        continue;
                    }
                    self.fill_description(&mut description, code_block, nested_sourcepos);
                }
                BlockRole::OtherCodeBlock(_code_block) => {
                    self.diagnostics
                        .record_error(NOT_A_DESCRIPTION_BLOCK, nested_span);
                }
                BlockRole::Unwrapped => {
                    self.diagnostics
                        .record_error(UNWRAPPED_DESCRIPTION, nested_span);
                }
            }
        }
        let item = DescribedLine { line, description };
        Some(ItemBlocks { item, nested_lists })
    }

    /// Read the content of the description block at `sourcepos` into `description`, its line breaks as `\n`, recording a block never closed and a second block.
    fn fill_description(
        &mut self,
        description: &mut Option<Description>,
        code_block: &NodeCodeBlock,
        sourcepos: Sourcepos,
    ) {
        let block_span = self.span(sourcepos);
        let is_unclosed = !code_block.closed;
        if is_unclosed {
            let message = "a ```description block needs a closing fence as long as its opening one";
            self.diagnostics.record_error(message, block_span);
        }
        let has_description = description.is_some();
        if has_description {
            let message = "a second ```description block; merge it into the first";
            self.diagnostics.record_error(message, block_span);
            return;
        }
        let literal = code_block.literal.replace("\r\n", "\n").replace('\r', "\n");
        let content = literal.strip_suffix('\n').unwrap_or(&literal);
        let text = content.to_owned();
        let read_description = Description {
            text,
            span: block_span,
        };
        *description = Some(read_description);
    }

    /// Return a heading's text without its opening `#` markers, its CommonMark closing sequence ` ##` and the comments it ends with.
    fn heading_text(&self, sourcepos: Sourcepos) -> SourceText<'src> {
        let heading_line = self.first_line(sourcepos);
        let line_text = heading_line.text;
        let without_opening = line_text.trim_start_matches(HEADING_MARKER).trim_start();
        let opening_length = line_text.len() - without_opening.len();
        let without_closing_markers = without_opening.trim_end_matches(HEADING_MARKER);
        let has_closing_sequence = without_closing_markers.ends_with([' ', '\t']);
        let text = if has_closing_sequence {
            without_closing_markers.trim_end()
        } else {
            without_opening
        };
        let offset = heading_line.offset + opening_length;
        let heading_text = SourceText { text, offset };
        heading_text.without_trailing_comments()
    }

    /// Return the source from where `sourcepos` starts to the end of that line, trailing whitespace dropped.
    fn first_line(&self, sourcepos: Sourcepos) -> SourceText<'src> {
        let start = self.offset(sourcepos.start.line, sourcepos.start.column);
        let line_end = self.line_end(sourcepos.start.line);
        let line_text = &self.source[start..line_end.max(start)];
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

    /// Return the byte offset where the 1-based `line` ends, before its line break.
    fn line_end(&self, line: usize) -> usize {
        let line_start = self.offset(line, 1);
        let rest_of_source = &self.source[line_start..];
        let line_length = rest_of_source
            .find(['\r', '\n'])
            .unwrap_or(rest_of_source.len());
        line_start + line_length
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
            let Some(item_blocks) = reader.read_item(item) else {
                continue;
            };
            let ItemBlocks { item, nested_lists } = item_blocks;
            let mut children = Self::default();
            for nested_list in nested_lists {
                children.push_list(nested_list, reader);
            }
            let node = ComponentNode {
                item,
                is_indented_but_not_nested,
                children: children.nodes,
            };
            self.nodes.push(node);
        }
    }
}

impl Section {
    /// Return the known section a `##` heading names.
    fn named(section_name: &str) -> Option<Self> {
        match section_name {
            "Components" => Some(Self::Components),
            "Workflows" => Some(Self::Workflows),
            _unknown_name => None,
        }
    }

    /// Spell the error of a `##` heading naming no known section, a v3 one with its v4 fix.
    fn unknown_section_message(section_name: &str) -> String {
        match section_name {
            "Edges" => "`## Edges` is v3: write each edge in a step: `### main: TITLE`, then `#### main: TITLE`".to_owned(),
            legend_name if legend_name == "Legend" || legend_name.starts_with("Legend:") => {
                "`## Legend` is v3 and removed: say what each entry meant in step prose or edge descriptions".to_owned()
            }
            _unknown_name => format!(
                "unknown section `## {section_name}`: expected `## Components` or `## Workflows`"
            ),
        }
    }
}

impl<'node> BlockRole<'node> {
    /// Classify a block written outside a heading or nested in an item; a `-` bullet list always holds items.
    fn of(value: &'node NodeValue) -> Self {
        match value {
            NodeValue::HtmlBlock(html) if html.block_type == COMMENT_BLOCK_TYPE => Self::Comment,
            NodeValue::List(list)
                if list.list_type == ListType::Bullet && list.bullet_char == ITEM_BULLET =>
            {
                Self::BulletList
            }
            NodeValue::CodeBlock(code_block) if code_block.info == DESCRIPTION_INFO => {
                Self::Description(code_block)
            }
            NodeValue::CodeBlock(code_block) => Self::OtherCodeBlock(code_block),
            _other_block => Self::Unwrapped,
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
