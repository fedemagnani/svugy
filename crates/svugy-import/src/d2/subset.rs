//! Filter the d2 syntax tree down to the v1 subset, recording each construct it drops as a warning and each key d2 rejects as an error.

use svugy_model::style::AnyStyleKey;

use super::ast::{
    D2Document, EdgeDeclaration, EdgeLink, Field, FieldAssignment, KeyPath, KeySegment, Keyword,
    Scalar, ShapeDeclaration, Span, Statement, StyleProperty, SvugyProperty, SvugyValue,
};
use super::diagnostic::{Construct, Diagnostics};
use super::parse::{
    SyntaxArray, SyntaxAssignment, SyntaxDeclaration, SyntaxHead, SyntaxKey, SyntaxLink,
    SyntaxStatement, SyntaxValue,
};

/// Reject an entry of the `svugy` variable that is not a plain `key: value`.
const SVUGY_ENTRY_ERROR: &str = "the `svugy` variable holds only `key: value` entries";

/// Reject an item of a `svugy` list that is not a plain value.
const SVUGY_LIST_ITEM_ERROR: &str = "a list in the `svugy` variable holds only plain values";

/// Filter syntax statements to the v1 subset, recording a warning for every construct dropped and an error for every key d2 itself rejects.
pub(crate) struct SubsetFilter<'sink> {
    diagnostics: &'sink mut Diagnostics,
}

/// Tell what a key declares: a shape path, or a field from its first reserved keyword on (`a.b.style.fill`).
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

impl<'sink> SubsetFilter<'sink> {
    /// Filter a parsed d2 document to the v1 subset, recording the warnings for everything else and the errors for rejected keys.
    pub(crate) fn filter_document(
        statements: Vec<SyntaxStatement>,
        diagnostics: &'sink mut Diagnostics,
    ) -> D2Document {
        let mut filter = Self { diagnostics };
        let statements = filter.filter_statements(statements);
        D2Document { statements }
    }

    /// Filter every statement of a list, keeping those the subset holds.
    fn filter_statements(&mut self, statements: Vec<SyntaxStatement>) -> Vec<Statement> {
        statements
            .into_iter()
            .filter_map(|statement| self.filter_statement(statement))
            .collect()
    }

    /// Filter one statement, warning on edge references and spreads.
    fn filter_statement(&mut self, statement: SyntaxStatement) -> Option<Statement> {
        match statement {
            SyntaxStatement::Declaration(declaration) => self.filter_declaration(declaration),
            SyntaxStatement::EdgeReference(span) => {
                self.diagnostics
                    .record_warning(Construct::EdgeReference, span);
                None
            }
            SyntaxStatement::Spread { value, span } => {
                let is_import = matches!(value, SyntaxValue::Import(_import_span));
                let construct = if is_import {
                    Construct::Import
                } else {
                    Construct::Variables
                };
                self.diagnostics.record_warning(construct, span);
                None
            }
        }
    }

    /// Filter a declaration by its head: a key or an edge chain.
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

    /// Filter a key declaration into a shape declaration or a field assignment, by its key's role.
    fn filter_key_declaration(
        &mut self,
        key: SyntaxKey,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Statement> {
        let path = self.filter_plain_path(key)?;
        let key_role = KeyRole::of(path);
        match key_role {
            KeyRole::Shape(path) => self.filter_shape_declaration(path, assignment, span),
            KeyRole::Field(field_key) => self.filter_field_assignment(field_key, assignment, span),
        }
    }

    /// Keep a key without globs or filters as a path, counting its leading `_`; the subset cannot expand globs or filters.
    fn filter_plain_path(&mut self, key: SyntaxKey) -> Option<KeyPath> {
        let has_glob_segment = key.segments.iter().any(|segment| segment.is_glob);
        let is_glob_or_filter = has_glob_segment || key.is_filtered;
        if is_glob_or_filter {
            self.diagnostics.record_warning(Construct::Glob, key.span);
            return None;
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
            let misplaced_span = misplaced_parent_reference.segment.span;
            self.diagnostics.record_error(message, misplaced_span);
            return None;
        }
        let is_only_parent_references = named_segments.is_empty();
        if is_only_parent_references {
            let message = "a key needs a name after its `_` parent references";
            self.diagnostics.record_error(message, key.span);
            return None;
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

    /// Filter a shape declaration, its value kept as its label and its map as its body; a `null` deletion is warned on and dropped.
    fn filter_shape_declaration(
        &mut self,
        path: KeyPath,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Statement> {
        let SyntaxAssignment { value, map } = assignment;
        let label = self.filter_label(value, span)?;
        let body = map
            .map(|map| self.filter_statements(map))
            .unwrap_or_default();
        let shape = ShapeDeclaration {
            path,
            label,
            body,
            span,
        };
        let statement = Statement::Shape(shape);
        Some(statement)
    }

    /// Filter a declaration's value into its optional label, or `None` for a `null` deletion, warned on so its declaration is dropped.
    fn filter_label(&mut self, value: Option<SyntaxValue>, span: Span) -> Option<Option<Scalar>> {
        if let Some(SyntaxValue::Null(_null_span)) = value {
            self.diagnostics.record_warning(Construct::Null, span);
            return None;
        }
        let label = value.and_then(|value| self.filter_scalar(value));
        Some(label)
    }

    /// Filter a field assignment by its keyword, warning on the classes, boards and reserved fields the subset drops.
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
                self.diagnostics.record_warning(Construct::Classes, span);
                return None;
            }
            Keyword::Layers | Keyword::Scenarios | Keyword::Steps => {
                let board_keyword = keyword.name();
                let construct = Construct::Board(board_keyword);
                self.diagnostics.record_warning(construct, span);
                return None;
            }
            reserved_keyword => {
                let reserved_name = reserved_keyword.name();
                let construct = Construct::ReservedField(reserved_name);
                self.diagnostics.record_warning(construct, span);
                return None;
            }
        };
        let assignment = FieldAssignment {
            target,
            field,
            span,
        };
        let statement = Statement::Field(assignment);
        Some(statement)
    }

    /// Filter a scalar field `keyword: value`; a sub-key (`label.near`) or a value-less form is dropped, a map beside the value is ignored.
    fn filter_scalar_field(
        &mut self,
        keyword: Keyword,
        rest: Vec<KeySegment>,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Scalar> {
        let has_sub_key = !rest.is_empty();
        if has_sub_key {
            self.warn_sub_keyed_form(keyword, &rest, span);
            return None;
        }
        let value = self.take_value_warning_on_map(keyword, assignment, span)?;
        self.filter_scalar(value)
    }

    /// Warn on the sub-keyed form `keyword.sub.key: …`, spelled with its sub-keys.
    fn warn_sub_keyed_form(&mut self, keyword: Keyword, rest: &[KeySegment], span: Span) {
        let keyword_name = keyword.name();
        let rest_names = rest.iter().map(|segment| segment.name.as_str());
        let names: Vec<&str> = std::iter::once(keyword_name).chain(rest_names).collect();
        let dotted_name = names.join(".");
        let construct = Construct::FieldForm(dotted_name);
        self.diagnostics.record_warning(construct, span);
    }

    /// Take the value of `keyword: value`, warning on a map beside it and on a form without one, which is dropped.
    fn take_value_warning_on_map(
        &mut self,
        keyword: Keyword,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<SyntaxValue> {
        let SyntaxAssignment { value, map } = assignment;
        let Some(value) = value else {
            self.warn_field_form(keyword, span);
            return None;
        };
        let has_map_beside_value = map.is_some();
        if has_map_beside_value {
            self.warn_field_form(keyword, span);
        }
        Some(value)
    }

    /// Take the map of `keyword: { … }`, warning on a value beside it and on a form without one, which is dropped.
    fn take_map_warning_on_value(
        &mut self,
        keyword: Keyword,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Vec<SyntaxStatement>> {
        let SyntaxAssignment { value, map } = assignment;
        let Some(map) = map else {
            self.warn_field_form(keyword, span);
            return None;
        };
        let has_value_beside_map = value.is_some();
        if has_value_beside_map {
            self.warn_field_form(keyword, span);
        }
        Some(map)
    }

    /// Warn on a form of the field `keyword` that the subset does not take.
    fn warn_field_form(&mut self, keyword: Keyword, span: Span) {
        let keyword_name = keyword.name().to_owned();
        let field_form = Construct::FieldForm(keyword_name);
        self.diagnostics.record_warning(field_form, span);
    }

    /// Keep `style.name: value` and `style: { name: value; … }` as style properties.
    fn filter_style_field(
        &mut self,
        mut rest: Vec<KeySegment>,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Field> {
        let sub_key_property_name = rest.pop();
        let is_deeper_than_one_property = !rest.is_empty();
        if is_deeper_than_one_property {
            self.warn_field_form(Keyword::Style, span);
            return None;
        }
        let Some(name) = sub_key_property_name else {
            return self.filter_style_map(assignment, span);
        };
        let value = self.take_value_warning_on_map(Keyword::Style, assignment, span)?;
        let property = self.filter_style_entry(name, value, span)?;
        let properties = vec![property];
        let style_field = Field::Style(properties);
        Some(style_field)
    }

    /// Filter the map of `style: { … }` into its style properties.
    fn filter_style_map(&mut self, assignment: SyntaxAssignment, span: Span) -> Option<Field> {
        let map = self.take_map_warning_on_value(Keyword::Style, assignment, span)?;
        let properties = map
            .into_iter()
            .filter_map(|statement| self.filter_style_property(statement))
            .collect();
        let style_field = Field::Style(properties);
        Some(style_field)
    }

    /// Keep one `name: value` line of a `style: { … }` map.
    fn filter_style_property(&mut self, statement: SyntaxStatement) -> Option<StyleProperty> {
        let SyntaxStatement::Declaration(declaration) = statement else {
            let span = statement.span();
            self.warn_field_form(Keyword::Style, span);
            return None;
        };
        let SyntaxDeclaration {
            head,
            assignment,
            span,
        } = declaration;
        let SyntaxHead::Key(key) = head else {
            self.warn_field_form(Keyword::Style, span);
            return None;
        };
        let path = self.filter_plain_path(key)?;
        let has_parent_reference = path.parent_references > 0;
        if has_parent_reference {
            self.warn_field_form(Keyword::Style, span);
            return None;
        }
        let Ok([name]) = <[KeySegment; 1]>::try_from(path.segments) else {
            self.warn_field_form(Keyword::Style, span);
            return None;
        };
        let value = self.take_value_warning_on_map(Keyword::Style, assignment, span)?;
        self.filter_style_entry(name, value, span)
    }

    /// Keep `name: value` as a style property only when `svugy_model::style` knows `name`, as d2 rejects unknown style keys.
    fn filter_style_entry(
        &mut self,
        name: KeySegment,
        value: SyntaxValue,
        span: Span,
    ) -> Option<StyleProperty> {
        let Some(_style_key) = AnyStyleKey::find(&name.name) else {
            let unknown_style_key = Construct::UnknownStyleKey(name.name);
            self.diagnostics.record_warning(unknown_style_key, span);
            return None;
        };
        let value = self.filter_scalar(value)?;
        let property = StyleProperty { name, value, span };
        Some(property)
    }

    /// Keep the `svugy` variable of a `vars: { … }` map as the carrier; every other variable is warned on, as the subset substitutes none.
    fn filter_vars_field(
        &mut self,
        rest: Vec<KeySegment>,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Field> {
        let has_sub_key = !rest.is_empty();
        if has_sub_key {
            self.warn_sub_keyed_form(Keyword::Vars, &rest, span);
            return None;
        }
        let map = self.take_map_warning_on_value(Keyword::Vars, assignment, span)?;
        let properties: Vec<SvugyProperty> = map
            .into_iter()
            .flat_map(|statement| self.collect_vars_entry(statement))
            .collect();
        let is_carrier_absent = properties.is_empty();
        if is_carrier_absent {
            return None;
        }
        let svugy_field = Field::Svugy(properties);
        Some(svugy_field)
    }

    /// Collect the carrier properties of a `vars` entry when it is the `svugy` variable, warning on any other variable.
    fn collect_vars_entry(&mut self, statement: SyntaxStatement) -> Vec<SvugyProperty> {
        let span = statement.span();
        let Some((key, assignment)) = statement.into_key_declaration() else {
            self.diagnostics.record_warning(Construct::Variables, span);
            return Vec::new();
        };
        let first_segment = key.segments.first();
        let is_user_variable = first_segment
            .is_none_or(|first_segment| first_segment.segment.name != SvugyProperty::VARIABLE);
        if is_user_variable {
            self.diagnostics.record_warning(Construct::Variables, span);
            return Vec::new();
        }
        let Some(path) = self.filter_plain_path(key) else {
            return Vec::new();
        };
        let below_variable_segments = path.segments.into_iter().skip(1);
        let attribute_path = below_variable_segments
            .map(|segment| segment.name)
            .collect();
        self.collect_svugy_properties(attribute_path, assignment, span)
    }

    /// Collect every `key: value` below `attribute_path` as a carrier property addressed by its full path; anything else there is an error.
    fn collect_svugy_properties(
        &mut self,
        attribute_path: Vec<String>,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Vec<SvugyProperty> {
        let mut properties = Vec::new();
        let SyntaxAssignment { value, map } = assignment;
        let carried_value = value.and_then(|value| self.filter_carried_value(value));
        if let Some(value) = carried_value {
            let property_attribute_path = attribute_path.clone();
            let property = SvugyProperty {
                attribute_path: property_attribute_path,
                value,
                span,
            };
            properties.push(property);
        }
        let Some(map) = map else {
            return properties;
        };
        for statement in map {
            let entry_span = statement.span();
            let Some((key, entry_assignment)) = statement.into_key_declaration() else {
                self.diagnostics.record_error(SVUGY_ENTRY_ERROR, entry_span);
                continue;
            };
            let Some(path) = self.filter_plain_path(key) else {
                continue;
            };
            let has_parent_reference = path.parent_references > 0;
            if has_parent_reference {
                self.diagnostics.record_error(SVUGY_ENTRY_ERROR, entry_span);
                continue;
            }
            let entry_names = path.segments.into_iter().map(|segment| segment.name);
            let mut entry_attribute_path = attribute_path.clone();
            entry_attribute_path.extend(entry_names);
            let entry_properties =
                self.collect_svugy_properties(entry_attribute_path, entry_assignment, entry_span);
            properties.extend(entry_properties);
        }
        properties
    }

    /// Keep a carried value: a scalar as itself, an array as a list of its scalar items, each other item an error.
    fn filter_carried_value(&mut self, value: SyntaxValue) -> Option<SvugyValue> {
        let SyntaxValue::Array(array) = value else {
            let scalar = self.filter_scalar(value)?;
            let carried_scalar = SvugyValue::Scalar(scalar);
            return Some(carried_scalar);
        };
        let SyntaxArray { items, span } = array;
        let item_count = items.len();
        let mut scalar_items = Vec::with_capacity(item_count);
        for item in items {
            let SyntaxValue::Scalar(scalar) = item else {
                let item_span = item.span();
                self.diagnostics
                    .record_error(SVUGY_LIST_ITEM_ERROR, item_span);
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

    /// Filter an edge chain, its value kept as its label and its map as its fields; a `null` deletion is warned on and dropped.
    fn filter_edge_declaration(
        &mut self,
        source: SyntaxKey,
        links: Vec<SyntaxLink>,
        assignment: SyntaxAssignment,
        span: Span,
    ) -> Option<Statement> {
        let source = self.filter_plain_path(source)?;
        let link_count = links.len();
        let mut kept_links = Vec::with_capacity(link_count);
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
        let label = self.filter_label(value, span)?;
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
        let statement = Statement::Edge(edge);
        Some(statement)
    }

    /// Keep the fields of an edge's map; anything declaring shapes or edges there is dropped.
    fn filter_edge_fields(&mut self, map: Vec<SyntaxStatement>) -> Vec<Field> {
        let statements = self.filter_statements(map);
        let statement_count = statements.len();
        let mut fields = Vec::with_capacity(statement_count);
        for statement in statements {
            let Statement::Field(assignment) = statement else {
                let declaration_span = statement.span();
                self.diagnostics
                    .record_warning(Construct::DeclarationInEdgeMap, declaration_span);
                continue;
            };
            let has_target_segment = !assignment.target.segments.is_empty();
            let has_parent_reference = assignment.target.parent_references > 0;
            let has_target = has_target_segment || has_parent_reference;
            if has_target {
                self.diagnostics
                    .record_warning(Construct::DeclarationInEdgeMap, assignment.span);
                continue;
            }
            fields.push(assignment.field);
        }
        fields
    }

    /// Filter a value down to a scalar, warning on every other kind of value.
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
        self.diagnostics.record_warning(construct, span);
        None
    }
}

impl KeyRole {
    /// Tell the role of `path` from its first reserved keyword, a shape when it holds none.
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
