//! Filter the d2 syntax tree down to the v1 subset, recording each construct it drops as a warning and each key d2 rejects as an error.

use svugy_model::path::ElementPath;
use svugy_model::style::StyleKey;

use super::ast::{
    D2Document, EdgeDeclaration, EdgeLink, Field, FieldAssignment, KeyPath, KeySegment, Scalar,
    ShapeDeclaration, Span, Statement, StyleProperty, SvugyProperty, SvugyValue,
};
use super::diagnostic::{Construct, SyntaxError, UnsupportedConstruct};
use super::parse::{
    SyntaxArray, SyntaxAssignment, SyntaxDeclaration, SyntaxHead, SyntaxKey, SyntaxLink,
    SyntaxStatement, SyntaxValue,
};

/// The d2 variable whose entries carry what d2 cannot say, e.g. `vars: { svugy: { caption: … } }`.
const SVUGY_VARIABLE: &str = "svugy";

/// Reject an entry of the `svugy` variable that is not a plain `key: value`.
const SVUGY_ENTRY_ERROR: &str = "the `svugy` variable holds only `key: value` entries";

/// Reject an item of a `svugy` list that is not a plain value.
const SVUGY_LIST_ITEM_ERROR: &str = "a list in the `svugy` variable holds only plain values";

/// Filter syntax statements to the v1 subset, accumulating a warning for every construct dropped and an error for every key d2 itself rejects.
pub(crate) struct SubsetFilter {
    warnings: Vec<UnsupportedConstruct>,
    errors: Vec<SyntaxError>,
}

/// What a key declares: a shape path, or a field from its first reserved keyword on (`a.b.style.fill`).
enum KeyRole {
    Shape(KeyPath),
    Field(FieldKey),
}

/// Split a field key into the shape it targets, its reserved keyword and the sub-keys after it.
struct FieldKey {
    target: KeyPath,
    keyword: Keyword,
    rest: Vec<KeySegment>,
}

/// Name a d2 reserved keyword outside `style` (d2ast/keywords.go at v0.9.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Keyword {
    Shape,
    Label,
    Direction,
    Link,
    Style,
    Vars,
    Class,
    Classes,
    Layers,
    Scenarios,
    Steps,
    Icon,
    Constraint,
    Tooltip,
    Near,
    Width,
    Height,
    Top,
    Left,
    GridRows,
    GridColumns,
    GridGap,
    VerticalGap,
    HorizontalGap,
    SourceArrowhead,
    TargetArrowhead,
}

impl SubsetFilter {
    /// Keep the v1 subset of a parsed d2 file and return the warnings for everything else and the errors for rejected keys.
    pub(crate) fn filter_document(
        statements: Vec<SyntaxStatement>,
    ) -> (D2Document, Vec<UnsupportedConstruct>, Vec<SyntaxError>) {
        let mut filter = Self {
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        let statements = filter.filter_statements(statements);
        let document = D2Document { statements };
        (document, filter.warnings, filter.errors)
    }

    fn filter_statements(&mut self, statements: Vec<SyntaxStatement>) -> Vec<Statement> {
        statements
            .into_iter()
            .filter_map(|statement| self.filter_statement(statement))
            .collect()
    }

    fn filter_statement(&mut self, statement: SyntaxStatement) -> Option<Statement> {
        match statement {
            SyntaxStatement::Declaration(declaration) => self.filter_declaration(declaration),
            SyntaxStatement::EdgeReference(span) => {
                self.drop_construct(Construct::EdgeReference, span)
            }
            SyntaxStatement::Spread { value, span } => {
                let is_import = matches!(value, SyntaxValue::Import(_import_span));
                let construct = if is_import {
                    Construct::Import
                } else {
                    Construct::Variables
                };
                self.drop_construct(construct, span)
            }
        }
    }

    fn filter_declaration(&mut self, declaration: SyntaxDeclaration) -> Option<Statement> {
        let SyntaxDeclaration {
            head,
            assignment,
            span,
        } = declaration;
        match head {
            SyntaxHead::Key(key) => self.filter_key_declaration(key, assignment, span),
            SyntaxHead::Edges { source, links } => {
                self.filter_edge_declaration(source, links, assignment, span)
            }
        }
    }

    fn filter_key_declaration(
        &mut self,
        key: SyntaxKey,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Statement> {
        let path = self.filter_plain_path(key)?;
        match KeyRole::of(path) {
            KeyRole::Shape(path) => self.filter_shape_declaration(path, assignment, span),
            KeyRole::Field(field_key) => self.filter_field_assignment(field_key, assignment, span),
        }
    }

    /// Keep a key without globs or filters as a path, counting its leading `_`; the subset cannot expand globs or filters.
    fn filter_plain_path(&mut self, key: SyntaxKey) -> Option<KeyPath> {
        let has_glob_segment = key.segments.iter().any(|segment| segment.is_glob);
        let is_glob_or_filter = has_glob_segment || key.is_filtered;
        if is_glob_or_filter {
            return self.drop_construct(Construct::Glob, key.span);
        }
        let mut syntax_segments = key.segments;
        let parent_references = syntax_segments
            .iter()
            .take_while(|segment| segment.is_parent_reference)
            .count();
        let named_segments = syntax_segments.split_off(parent_references);
        let misplaced_parent_reference = named_segments
            .iter()
            .find(|segment| segment.is_parent_reference);
        if let Some(misplaced_parent_reference) = misplaced_parent_reference {
            let message = "`_` names the parent scope only at the start of a key, e.g. `_.x`";
            return self.reject(message, misplaced_parent_reference.segment.span);
        }
        let is_only_parent_references = named_segments.is_empty();
        if is_only_parent_references {
            let message = "a key needs a name after its `_` parent references";
            return self.reject(message, key.span);
        }
        let segments = named_segments
            .into_iter()
            .map(|segment| segment.segment)
            .collect();
        let path = KeyPath {
            parent_references,
            segments,
            span: key.span,
        };
        Some(path)
    }

    fn filter_shape_declaration(
        &mut self,
        path: KeyPath,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Statement> {
        let SyntaxAssignment { value, map } = assignment;
        if let Some(SyntaxValue::Null(_null_span)) = value {
            return self.drop_construct(Construct::Null, span);
        }
        let label = value.and_then(|value| self.filter_scalar(value));
        let body = map
            .map(|map| self.filter_statements(map))
            .unwrap_or_default();
        let shape = ShapeDeclaration {
            path,
            label,
            body,
            span,
        };
        Some(Statement::Shape(shape))
    }

    fn filter_field_assignment(
        &mut self,
        field_key: FieldKey,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Statement> {
        let FieldKey {
            target,
            keyword,
            rest,
        } = field_key;
        let field = match keyword {
            Keyword::Shape => self
                .filter_scalar_field(keyword, rest, assignment, span)
                .map(Field::Shape)?,
            Keyword::Label => self
                .filter_scalar_field(keyword, rest, assignment, span)
                .map(Field::Label)?,
            Keyword::Direction => self
                .filter_scalar_field(keyword, rest, assignment, span)
                .map(Field::Direction)?,
            Keyword::Link => self
                .filter_scalar_field(keyword, rest, assignment, span)
                .map(Field::Link)?,
            Keyword::Style => self.filter_style_field(rest, assignment, span)?,
            Keyword::Vars => self.filter_vars_field(rest, assignment, span)?,
            Keyword::Class | Keyword::Classes => {
                return self.drop_construct(Construct::Classes, span);
            }
            Keyword::Layers | Keyword::Scenarios | Keyword::Steps => {
                let board_keyword = keyword.name().to_owned();
                let construct = Construct::Board(board_keyword);
                return self.drop_construct(construct, span);
            }
            reserved_keyword => {
                let reserved_name = reserved_keyword.name().to_owned();
                let construct = Construct::ReservedField(reserved_name);
                return self.drop_construct(construct, span);
            }
        };
        let assignment = FieldAssignment {
            target,
            field,
            span,
        };
        Some(Statement::Field(assignment))
    }

    /// Keep `keyword: value`; a sub-key (`label.near`) or a value-less form is dropped, a map beside the value is ignored.
    fn filter_scalar_field(
        &mut self,
        keyword: Keyword,
        rest: Vec<KeySegment>,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Scalar> {
        let SyntaxAssignment { value, map } = assignment;
        let keyword_name = keyword.name();
        let has_sub_key = !rest.is_empty();
        if has_sub_key {
            let dotted_name = Self::dotted_name(keyword_name, &rest);
            let construct = Construct::FieldForm(dotted_name);
            return self.drop_construct(construct, span);
        }
        let Some(value) = value else {
            let construct = Construct::FieldForm(keyword_name.to_owned());
            return self.drop_construct(construct, span);
        };
        let has_map_beside_value = map.is_some();
        if has_map_beside_value {
            let construct = Construct::FieldForm(keyword_name.to_owned());
            self.warn(construct, span);
        }
        self.filter_scalar(value)
    }

    /// Keep `style.name: value` and `style: { name: value; … }` as style properties.
    fn filter_style_field(
        &mut self,
        rest: Vec<KeySegment>,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Field> {
        let SyntaxAssignment { value, map } = assignment;
        let mut rest = rest;
        let dotted_property_name = rest.pop();
        let is_deeper_than_one_property = !rest.is_empty();
        if is_deeper_than_one_property {
            let style_form = Construct::style_form();
            return self.drop_construct(style_form, span);
        }
        let Some(name) = dotted_property_name else {
            return self.filter_style_map(value, map, span);
        };
        let Some(value) = value else {
            let style_form = Construct::style_form();
            return self.drop_construct(style_form, span);
        };
        let has_map_beside_value = map.is_some();
        if has_map_beside_value {
            let style_form = Construct::style_form();
            self.warn(style_form, span);
        }
        let property = self.filter_style_entry(name, value, span)?;
        let properties = vec![property];
        Some(Field::Style(properties))
    }

    fn filter_style_map(
        &mut self,
        value: Option<SyntaxValue>,
        map: Option<Vec<SyntaxStatement>>,
        span: Span,
    ) -> Option<Field> {
        let Some(map) = map else {
            let style_form = Construct::style_form();
            return self.drop_construct(style_form, span);
        };
        let has_value_beside_map = value.is_some();
        if has_value_beside_map {
            let style_form = Construct::style_form();
            self.warn(style_form, span);
        }
        let properties = map
            .into_iter()
            .filter_map(|statement| self.filter_style_property(statement))
            .collect();
        Some(Field::Style(properties))
    }

    /// Keep one `name: value` line of a `style: { … }` map.
    fn filter_style_property(&mut self, statement: SyntaxStatement) -> Option<StyleProperty> {
        let SyntaxStatement::Declaration(declaration) = statement else {
            let span = statement.span();
            let style_form = Construct::style_form();
            return self.drop_construct(style_form, span);
        };
        let SyntaxDeclaration {
            head,
            assignment,
            span,
        } = declaration;
        let SyntaxHead::Key(key) = head else {
            let style_form = Construct::style_form();
            return self.drop_construct(style_form, span);
        };
        let path = self.filter_plain_path(key)?;
        let has_parent_reference = path.parent_references > 0;
        if has_parent_reference {
            let style_form = Construct::style_form();
            return self.drop_construct(style_form, span);
        }
        let Ok([name]) = <[KeySegment; 1]>::try_from(path.segments) else {
            let style_form = Construct::style_form();
            return self.drop_construct(style_form, span);
        };
        let SyntaxAssignment { value, map } = assignment;
        let Some(value) = value else {
            let style_form = Construct::style_form();
            return self.drop_construct(style_form, span);
        };
        let has_map_beside_value = map.is_some();
        if has_map_beside_value {
            let style_form = Construct::style_form();
            self.warn(style_form, span);
        }
        self.filter_style_entry(name, value, span)
    }

    /// Keep `name: value` as a style property only when `svugy_model::style` knows `name`, as d2 rejects unknown style keys.
    fn filter_style_entry(
        &mut self,
        name: KeySegment,
        value: SyntaxValue,
        span: Span,
    ) -> Option<StyleProperty> {
        let Some(_style_key) = StyleKey::find(&name.name) else {
            let unknown_style_key = Construct::UnknownStyleKey(name.name);
            return self.drop_construct(unknown_style_key, span);
        };
        let value = self.filter_scalar(value)?;
        Some(StyleProperty { name, value, span })
    }

    /// Keep the `svugy` variable of a `vars: { … }` map as the carrier; every other variable is warned on, as the subset substitutes none.
    fn filter_vars_field(
        &mut self,
        rest: Vec<KeySegment>,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Field> {
        let SyntaxAssignment { value, map } = assignment;
        let vars_name = Keyword::Vars.name();
        let has_sub_key = !rest.is_empty();
        if has_sub_key {
            let dotted_name = Self::dotted_name(vars_name, &rest);
            let construct = Construct::FieldForm(dotted_name);
            return self.drop_construct(construct, span);
        }
        let Some(map) = map else {
            let construct = Construct::FieldForm(vars_name.to_owned());
            return self.drop_construct(construct, span);
        };
        let has_value_beside_map = value.is_some();
        if has_value_beside_map {
            let construct = Construct::FieldForm(vars_name.to_owned());
            self.warn(construct, span);
        }
        let mut properties = Vec::new();
        for statement in map {
            self.collect_vars_entry(statement, &mut properties);
        }
        let is_carrier_absent = properties.is_empty();
        if is_carrier_absent {
            return None;
        }
        Some(Field::Svugy(properties))
    }

    /// Collect the carrier properties of a `vars` entry when it is the `svugy` variable, warning on any other variable.
    fn collect_vars_entry(
        &mut self,
        statement: SyntaxStatement,
        properties: &mut Vec<SvugyProperty>,
    ) {
        let span = statement.span();
        let Some((key, assignment)) = statement.into_key_declaration() else {
            return self.warn(Construct::Variables, span);
        };
        let first_segment = key.segments.first();
        let is_user_variable =
            first_segment.is_none_or(|first_segment| first_segment.segment.name != SVUGY_VARIABLE);
        if is_user_variable {
            return self.warn(Construct::Variables, span);
        }
        let Some(path) = self.filter_plain_path(key) else {
            return;
        };
        let name = Self::carrier_name(&path.segments);
        self.collect_svugy_properties(name, assignment, span, properties);
    }

    /// Collect every `key: value` below the key `name` as a carrier property named by its full path; anything else there is an error.
    fn collect_svugy_properties(
        &mut self,
        name: String,
        assignment: SyntaxAssignment,
        span: Span,
        properties: &mut Vec<SvugyProperty>,
    ) {
        let SyntaxAssignment { value, map } = assignment;
        let carried_value = value.and_then(|value| self.filter_carried_value(value));
        if let Some(value) = carried_value {
            let property = SvugyProperty {
                name: name.clone(),
                value,
                span,
            };
            properties.push(property);
        }
        let Some(map) = map else {
            return;
        };
        for statement in map {
            let entry_span = statement.span();
            let Some((key, entry_assignment)) = statement.into_key_declaration() else {
                self.error(SVUGY_ENTRY_ERROR, entry_span);
                continue;
            };
            let Some(path) = self.filter_plain_path(key) else {
                continue;
            };
            let has_parent_reference = path.parent_references > 0;
            if has_parent_reference {
                self.error(SVUGY_ENTRY_ERROR, entry_span);
                continue;
            }
            let entry_path_name = Self::carrier_name(&path.segments);
            let entry_name = format!("{name}.{entry_path_name}");
            self.collect_svugy_properties(entry_name, entry_assignment, entry_span, properties);
        }
    }

    /// Keep a carried value: a scalar as itself, an array as a list of its scalar items, each other item an error.
    fn filter_carried_value(&mut self, value: SyntaxValue) -> Option<SvugyValue> {
        let SyntaxValue::Array(array) = value else {
            let scalar = self.filter_scalar(value)?;
            let carried_scalar = SvugyValue::Scalar(scalar);
            return Some(carried_scalar);
        };
        let SyntaxArray { items, span } = array;
        let mut scalar_items = Vec::with_capacity(items.len());
        for item in items {
            let SyntaxValue::Scalar(scalar) = item else {
                let item_span = item.span();
                self.error(SVUGY_LIST_ITEM_ERROR, item_span);
                continue;
            };
            scalar_items.push(scalar);
        }
        let list = SvugyValue::List {
            items: scalar_items,
            span,
        };
        Some(list)
    }

    /// Spell carrier key segments as a model path does, quoting a segment that holds a dot, so `desk.sqlite` and `"desk.sqlite"` stay apart.
    fn carrier_name(segments: &[KeySegment]) -> String {
        let names = segments.iter().map(|segment| segment.name.as_str());
        let carrier_path = ElementPath::from_segments(names);
        carrier_path.to_string()
    }

    fn filter_edge_declaration(
        &mut self,
        source: SyntaxKey,
        links: Vec<SyntaxLink>,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Statement> {
        let source = self.filter_plain_path(source)?;
        let mut kept_links = Vec::with_capacity(links.len());
        for link in links {
            let target = self.filter_plain_path(link.target)?;
            let kept_link = EdgeLink {
                operator: link.operator,
                operator_span: link.operator_span,
                target,
            };
            kept_links.push(kept_link);
        }
        let SyntaxAssignment { value, map } = assignment;
        if let Some(SyntaxValue::Null(_null_span)) = value {
            return self.drop_construct(Construct::Null, span);
        }
        let label = value.and_then(|value| self.filter_scalar(value));
        let fields = map
            .map(|map| self.filter_edge_fields(map))
            .unwrap_or_default();
        let edge = EdgeDeclaration {
            source,
            links: kept_links,
            label,
            fields,
            span,
        };
        Some(Statement::Edge(edge))
    }

    /// Keep the fields of an edge's map; anything declaring shapes or edges there is dropped.
    fn filter_edge_fields(&mut self, map: Vec<SyntaxStatement>) -> Vec<Field> {
        let statements = self.filter_statements(map);
        let mut fields = Vec::with_capacity(statements.len());
        for statement in statements {
            let Statement::Field(assignment) = statement else {
                self.warn(Construct::DeclarationInEdgeMap, statement.span());
                continue;
            };
            let has_target_segment = !assignment.target.segments.is_empty();
            let has_parent_reference = assignment.target.parent_references > 0;
            let has_target = has_target_segment || has_parent_reference;
            if has_target {
                self.warn(Construct::DeclarationInEdgeMap, assignment.span);
                continue;
            }
            fields.push(assignment.field);
        }
        fields
    }

    fn filter_scalar(&mut self, value: SyntaxValue) -> Option<Scalar> {
        let (construct, span) = match value {
            SyntaxValue::Scalar(scalar) => return Some(scalar),
            SyntaxValue::Null(span) => (Construct::Null, span),
            SyntaxValue::BlockString(span) => (Construct::BlockString, span),
            SyntaxValue::Import(span) => (Construct::Import, span),
            SyntaxValue::Substitution(span) => (Construct::Variables, span),
            SyntaxValue::Array(array) => (Construct::Array, array.span),
            SyntaxValue::Map(span) => (Construct::ArrayMap, span),
        };
        self.drop_construct(construct, span)
    }

    fn drop_construct<T>(&mut self, construct: Construct, span: Span) -> Option<T> {
        self.warn(construct, span);
        None
    }

    fn warn(&mut self, construct: Construct, span: Span) {
        let warning = UnsupportedConstruct::new(construct, span);
        self.warnings.push(warning);
    }

    /// Record an error and drop the statement it sits in.
    fn reject<T>(&mut self, message: &str, span: Span) -> Option<T> {
        self.error(message, span);
        None
    }

    fn error(&mut self, message: &str, span: Span) {
        let error = SyntaxError::new(message.to_owned(), span);
        self.errors.push(error);
    }

    fn dotted_name(head_name: &str, rest: &[KeySegment]) -> String {
        let rest_names = rest.iter().map(|segment| segment.name.as_str());
        let names: Vec<&str> = std::iter::once(head_name).chain(rest_names).collect();
        names.join(".")
    }
}

impl KeyRole {
    fn of(path: KeyPath) -> Self {
        let keyword_position = path
            .segments
            .iter()
            .enumerate()
            .find_map(|(index, segment)| {
                let keyword = Keyword::recognise(&segment.name)?;
                Some((index, keyword))
            });
        let Some((keyword_index, keyword)) = keyword_position else {
            return Self::Shape(path);
        };
        let mut target_segments = path.segments;
        let mut rest = target_segments.split_off(keyword_index);
        let keyword_segment = rest.remove(0);
        let last_target_segment = target_segments.last();
        let target_end = last_target_segment.map_or(keyword_segment.span.start, |last_segment| {
            last_segment.span.end
        });
        let target_span = Span::new(path.span.start, target_end);
        let target = KeyPath {
            parent_references: path.parent_references,
            segments: target_segments,
            span: target_span,
        };
        let field_key = FieldKey {
            target,
            keyword,
            rest,
        };
        Self::Field(field_key)
    }
}

impl Keyword {
    /// Spell every keyword as d2 does, the one table both directions read.
    const SPELLINGS: [(&'static str, Keyword); 26] = [
        ("shape", Self::Shape),
        ("label", Self::Label),
        ("direction", Self::Direction),
        ("link", Self::Link),
        ("style", Self::Style),
        ("vars", Self::Vars),
        ("class", Self::Class),
        ("classes", Self::Classes),
        ("layers", Self::Layers),
        ("scenarios", Self::Scenarios),
        ("steps", Self::Steps),
        ("icon", Self::Icon),
        ("constraint", Self::Constraint),
        ("tooltip", Self::Tooltip),
        ("near", Self::Near),
        ("width", Self::Width),
        ("height", Self::Height),
        ("top", Self::Top),
        ("left", Self::Left),
        ("grid-rows", Self::GridRows),
        ("grid-columns", Self::GridColumns),
        ("grid-gap", Self::GridGap),
        ("vertical-gap", Self::VerticalGap),
        ("horizontal-gap", Self::HorizontalGap),
        ("source-arrowhead", Self::SourceArrowhead),
        ("target-arrowhead", Self::TargetArrowhead),
    ];

    /// Recognise `name` as a keyword case-insensitively, as d2 does.
    fn recognise(name: &str) -> Option<Self> {
        let lowercase_name = name.to_ascii_lowercase();
        let mut spellings = Self::SPELLINGS.iter();
        let spelling = spellings.find(|(spelled_name, _keyword)| *spelled_name == lowercase_name);
        spelling.map(|(_spelled_name, keyword)| *keyword)
    }

    pub(crate) fn name(self) -> &'static str {
        let mut spellings = Self::SPELLINGS.iter();
        let spelling = spellings.find(|(_spelled_name, keyword)| *keyword == self);
        let (spelled_name, _keyword) = spelling.expect("every keyword has a spelling");
        spelled_name
    }
}

impl Field {
    /// Name the keyword this field was written under.
    pub(crate) fn keyword(&self) -> Keyword {
        match self {
            Self::Shape(_shape) => Keyword::Shape,
            Self::Label(_label) => Keyword::Label,
            Self::Direction(_direction) => Keyword::Direction,
            Self::Link(_link) => Keyword::Link,
            Self::Style(_properties) => Keyword::Style,
            Self::Svugy(_properties) => Keyword::Vars,
        }
    }
}

impl SyntaxStatement {
    /// Read a map entry as a key declaration, or `None` for an edge, a spread or an edge reference.
    fn into_key_declaration(self) -> Option<(SyntaxKey, SyntaxAssignment)> {
        let Self::Declaration(declaration) = self else {
            return None;
        };
        let SyntaxHead::Key(key) = declaration.head else {
            return None;
        };
        Some((key, declaration.assignment))
    }
}

impl Construct {
    fn style_form() -> Self {
        Self::FieldForm("style".to_owned())
    }
}
