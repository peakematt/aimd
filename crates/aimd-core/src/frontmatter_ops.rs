use crate::{AimdError, Diagnostic, DiagnosticSeverity, Document, Frontmatter, Mutation};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize)]
pub struct FmCheck {
    pub frontmatter: Frontmatter,
    pub properties: Vec<FmProperty>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FmProperty {
    pub path: Vec<String>,
    pub value: Value,
    pub kind: FmValueKind,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FmValueKind {
    String,
    Int,
    Float,
    Bool,
    Date,
    Blank,
    Null,
    List,
    Map,
    Any,
}

#[derive(Debug, Clone)]
pub struct FmMutation {
    pub output: String,
}

impl From<FmMutation> for Mutation {
    fn from(value: FmMutation) -> Self {
        Self {
            output: value.output,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PropertyPath(Vec<String>);

impl PropertyPath {
    pub fn parse(path: &str) -> Result<Self, AimdError> {
        let segments = path
            .split('.')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if segments.is_empty() {
            return Err(fm_error("invalid_property_path")
                .hint("Property paths must contain at least one segment."));
        }
        Ok(Self(segments))
    }

    fn segments(&self) -> &[String] {
        &self.0
    }

    fn top(&self) -> &str {
        &self.0[0]
    }
}

#[derive(Debug, Clone)]
pub enum FmSetValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Date(String),
    Blank,
    Null,
    Map(Value),
}

impl FmSetValue {
    fn kind(&self) -> FmValueKind {
        match self {
            Self::String(_) => FmValueKind::String,
            Self::Int(_) => FmValueKind::Int,
            Self::Float(_) => FmValueKind::Float,
            Self::Bool(_) => FmValueKind::Bool,
            Self::Date(_) => FmValueKind::Date,
            Self::Blank => FmValueKind::Blank,
            Self::Null => FmValueKind::Null,
            Self::Map(_) => FmValueKind::Map,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FmSchema {
    pub name: Option<String>,
    pub version: Option<u64>,
    pub fields: Vec<FmSchemaField>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FmSchemaField {
    pub path: Vec<String>,
    pub kind: FmValueKind,
    pub required: bool,
    pub order: Option<i64>,
    pub style: Option<String>,
}

#[derive(Debug, Clone)]
struct ParsedSchema {
    public: FmSchema,
    fields: BTreeMap<Vec<String>, FmSchemaField>,
}

impl FmSchema {
    pub fn parse(source: &str) -> Result<Self, AimdError> {
        Ok(parse_schema(source)?.public)
    }
}

impl Document {
    pub fn fm_check(&self, schema: Option<&FmSchema>) -> Result<FmCheck, AimdError> {
        let mut diagnostics = Vec::new();
        if self.frontmatter.malformed {
            diagnostics.push(diagnostic(
                "malformed_frontmatter",
                "Document starts with frontmatter delimiter but has no closing delimiter.",
                self.frontmatter.line_start,
                None,
                DiagnosticSeverity::Error,
            ));
            return Ok(FmCheck {
                frontmatter: self.frontmatter.clone(),
                properties: Vec::new(),
                diagnostics,
            });
        }
        let Some(block) = self.fm_block()? else {
            diagnostics.push(diagnostic(
                "frontmatter_missing",
                "Document has no YAML frontmatter block.",
                None,
                None,
                DiagnosticSeverity::Warning,
            ));
            return Ok(FmCheck {
                frontmatter: self.frontmatter.clone(),
                properties: Vec::new(),
                diagnostics,
            });
        };
        let parsed = parse_frontmatter_block(&self.source, &block);
        diagnostics.extend(parsed.diagnostics.clone());
        if let Some(schema) = schema {
            diagnostics.extend(schema_diagnostics(&parsed, schema));
        }
        Ok(FmCheck {
            frontmatter: self.frontmatter.clone(),
            properties: parsed.properties(),
            diagnostics,
        })
    }

    pub fn fm_get(
        &self,
        path: Option<&PropertyPath>,
        schema: Option<&FmSchema>,
    ) -> Result<FmCheck, AimdError> {
        let mut check = self.fm_check(schema)?;
        if let Some(path) = path {
            check
                .properties
                .retain(|property| property.path == path.segments());
            if check.properties.is_empty() {
                return Err(fm_error("frontmatter_property_not_found")
                    .selector(path.segments())
                    .hint("Run aimd fm get <file> --json to inspect available properties."));
            }
        }
        Ok(check)
    }

    pub fn fm_has(&self, path: &PropertyPath, schema: Option<&FmSchema>) -> Result<(), AimdError> {
        self.fm_get(Some(path), schema).map(|_| ())
    }

    pub fn fm_set(
        &self,
        path: &PropertyPath,
        value: FmSetValue,
        create: bool,
        schema: Option<&FmSchema>,
    ) -> Result<FmMutation, AimdError> {
        validate_schema_write(path, value.kind(), schema)?;
        let rendered = render_set_value(path.top(), &value, &self.newline)?;
        let output = self.write_property(path, &value, &rendered, create, schema)?;
        Ok(FmMutation { output })
    }

    pub fn fm_set_list(
        &self,
        path: &PropertyPath,
        values: &[String],
        create: bool,
        schema: Option<&FmSchema>,
    ) -> Result<FmMutation, AimdError> {
        validate_schema_write(path, FmValueKind::List, schema)?;
        let rendered = render_list(path.top(), values, &self.newline);
        let output = self.write_property(
            path,
            &FmSetValue::Map(Value::Array(
                values.iter().cloned().map(Value::String).collect(),
            )),
            &rendered,
            create,
            schema,
        )?;
        Ok(FmMutation { output })
    }

    pub fn fm_append_list_item(
        &self,
        path: &PropertyPath,
        values: &[String],
        allow_duplicate: bool,
        create: bool,
        schema: Option<&FmSchema>,
    ) -> Result<FmMutation, AimdError> {
        validate_schema_write(path, FmValueKind::List, schema)?;
        let Some(block) = self.ensure_mutable_block(create)? else {
            let rendered = render_list(path.top(), values, &self.newline);
            return Ok(FmMutation {
                output: create_frontmatter(&self.source, &rendered, &self.newline),
            });
        };
        let parsed = parse_frontmatter_block(&self.source, &block);
        let Some(prop) = parsed.find_top(path.top()) else {
            if !create {
                return Err(fm_error("frontmatter_property_not_found").selector(path.segments()));
            }
            return Ok(FmMutation {
                output: self.insert_property(
                    &block,
                    &render_list(path.top(), values, &self.newline),
                    schema,
                ),
            });
        };
        if prop.kind != FmValueKind::List {
            return Err(fm_error("frontmatter_schema_type_mismatch")
                .selector(path.segments())
                .hint("append-list-item requires an existing YAML list or --create."));
        }
        let existing = prop
            .list_items
            .iter()
            .map(|item| item.value.clone())
            .collect::<BTreeSet<_>>();
        let to_add = values
            .iter()
            .filter(|value| allow_duplicate || !existing.contains(*value))
            .cloned()
            .collect::<Vec<_>>();
        if to_add.is_empty() {
            return Ok(FmMutation {
                output: self.source.clone(),
            });
        }
        let insert_at = prop
            .list_items
            .last()
            .map_or(prop.range_end, |item| item.range_end);
        let mut insertion = String::new();
        for value in to_add {
            insertion.push_str("  - ");
            insertion.push_str(&quote_yaml_string(&value));
            insertion.push_str(&self.newline);
        }
        Ok(FmMutation {
            output: self.splice(insert_at, insert_at, &insertion),
        })
    }

    pub fn fm_remove_list_item(
        &self,
        path: &PropertyPath,
        values: &[String],
        schema: Option<&FmSchema>,
    ) -> Result<FmMutation, AimdError> {
        validate_schema_write(path, FmValueKind::List, schema)?;
        let block = self.require_mutable_block()?;
        let parsed = parse_frontmatter_block(&self.source, &block);
        let prop = parsed
            .find_top(path.top())
            .ok_or_else(|| fm_error("frontmatter_property_not_found").selector(path.segments()))?;
        if prop.kind != FmValueKind::List {
            return Err(fm_error("frontmatter_schema_type_mismatch")
                .selector(path.segments())
                .hint("remove-list-item requires an existing YAML list."));
        }
        let remove = values.iter().cloned().collect::<BTreeSet<_>>();
        let mut output = self.source.clone();
        for item in prop.list_items.iter().rev() {
            if remove.contains(&item.value) {
                output.replace_range(item.range_start..item.range_end, "");
            }
        }
        Ok(FmMutation {
            output: ensure_final_newline(output, &self.newline),
        })
    }

    pub fn fm_remove(
        &self,
        path: &PropertyPath,
        schema: Option<&FmSchema>,
    ) -> Result<FmMutation, AimdError> {
        if schema_field(path, schema).is_some_and(|field| field.required) {
            return Err(fm_error("frontmatter_required_key_missing")
                .selector(path.segments())
                .hint("Refusing to remove a schema-required frontmatter field."));
        }
        let block = self.require_mutable_block()?;
        let parsed = parse_frontmatter_block(&self.source, &block);
        let prop = parsed
            .find_top(path.top())
            .ok_or_else(|| fm_error("frontmatter_property_not_found").selector(path.segments()))?;
        let (start, end) = if path.segments().len() == 1 {
            (prop.range_start, prop.range_end)
        } else {
            let child = prop.find_child(&path.segments()[1]).ok_or_else(|| {
                fm_error("frontmatter_property_not_found").selector(path.segments())
            })?;
            (child.range_start, child.range_end)
        };
        Ok(FmMutation {
            output: self.splice(start, end, ""),
        })
    }

    pub fn fm_normalize(&self, schema: &FmSchema) -> Result<FmMutation, AimdError> {
        let block = self.require_mutable_block()?;
        let parsed = parse_frontmatter_block(&self.source, &block);
        let parsed_schema = ParsedSchema::from_schema(schema.clone());
        let mut output = self.source.clone();
        let mut missing = parsed_schema
            .public
            .fields
            .iter()
            .filter(|field| {
                field.required && field.path.len() == 1 && parsed.find_path(&field.path).is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        missing.sort_by_key(|field| field.order.unwrap_or(i64::MAX));
        if missing.is_empty() {
            return Ok(FmMutation { output });
        }
        let mut insertion = String::new();
        for field in missing {
            insertion.push_str(&render_schema_placeholder(&field, &self.newline));
        }
        output.replace_range(block.content_end..block.content_end, &insertion);
        Ok(FmMutation {
            output: ensure_final_newline(output, &self.newline),
        })
    }

    fn ensure_mutable_block(&self, create: bool) -> Result<Option<FmBlock>, AimdError> {
        if self.frontmatter.malformed {
            return Err(fm_error("malformed_frontmatter"));
        }
        if let Some(block) = self.fm_block()? {
            return Ok(Some(block));
        }
        if create {
            Ok(None)
        } else {
            Err(fm_error("frontmatter_missing")
                .hint("Use --create to insert a new frontmatter block."))
        }
    }

    fn require_mutable_block(&self) -> Result<FmBlock, AimdError> {
        if self.frontmatter.malformed {
            return Err(fm_error("malformed_frontmatter"));
        }
        self.fm_block()?.ok_or_else(|| {
            fm_error("frontmatter_missing").hint("Use --create when setting a value.")
        })
    }

    fn write_property(
        &self,
        path: &PropertyPath,
        value: &FmSetValue,
        rendered_top_property: &str,
        create: bool,
        schema: Option<&FmSchema>,
    ) -> Result<String, AimdError> {
        let Some(block) = self.ensure_mutable_block(create)? else {
            return Ok(create_frontmatter(
                &self.source,
                rendered_top_property,
                &self.newline,
            ));
        };
        let parsed = parse_frontmatter_block(&self.source, &block);
        if path.segments().len() == 1 {
            if let Some(prop) = parsed.find_top(path.top()) {
                return Ok(self.splice(prop.range_start, prop.range_end, rendered_top_property));
            }
            return Ok(self.insert_property(&block, rendered_top_property, schema));
        }
        let child_key = &path.segments()[1];
        let Some(prop) = parsed.find_top(path.top()) else {
            if !create {
                return Err(fm_error("frontmatter_property_not_found").selector(path.segments()));
            }
            let mut rendered = String::new();
            rendered.push_str(path.top());
            rendered.push(':');
            rendered.push_str(&self.newline);
            rendered.push_str("  ");
            rendered.push_str(child_key);
            rendered.push_str(": ");
            rendered.push_str(&render_inline_value(value));
            rendered.push_str(&self.newline);
            return Ok(self.insert_property(&block, &rendered, schema));
        };
        if prop.flow_map {
            return Err(fm_error("unsafe_frontmatter_rewrite")
                .selector(path.segments())
                .hint("Flow-style map mutation is not source-preserving; replace the full map with --map-file or convert it manually."));
        }
        if prop.kind != FmValueKind::Map {
            return Err(fm_error("frontmatter_schema_type_mismatch").selector(path.segments()));
        }
        let child_line = format!(
            "  {child_key}: {}{}",
            render_inline_value(value),
            self.newline
        );
        if let Some(child) = prop.find_child(child_key) {
            Ok(self.splice(child.range_start, child.range_end, &child_line))
        } else {
            Ok(self.splice(prop.range_end, prop.range_end, &child_line))
        }
    }

    fn insert_property(
        &self,
        block: &FmBlock,
        rendered: &str,
        schema: Option<&FmSchema>,
    ) -> String {
        let insert_at =
            insertion_byte(&self.source, block, rendered, schema).unwrap_or(block.content_end);
        self.splice(insert_at, insert_at, rendered)
    }

    fn fm_block(&self) -> Result<Option<FmBlock>, AimdError> {
        if !self.frontmatter.present || self.frontmatter.malformed {
            return Ok(None);
        }
        let (Some(byte_start), Some(byte_end), Some(line_start), Some(line_end)) = (
            self.frontmatter.byte_start,
            self.frontmatter.byte_end,
            self.frontmatter.line_start,
            self.frontmatter.line_end,
        ) else {
            return Ok(None);
        };
        let open_end = self
            .source
            .get(byte_start..)
            .and_then(|tail| tail.find('\n').map(|index| byte_start + index + 1))
            .unwrap_or(byte_end);
        let closing_start = *self
            .line_starts
            .get(line_end.saturating_sub(1))
            .ok_or_else(|| {
                fm_error("parse_error").hint("Could not locate frontmatter closing delimiter.")
            })?;
        Ok(Some(FmBlock {
            content_start: open_end,
            content_end: closing_start,
            line_start,
        }))
    }
}

impl ParsedSchema {
    fn from_schema(public: FmSchema) -> Self {
        let fields = public
            .fields
            .iter()
            .map(|field| (field.path.clone(), field.clone()))
            .collect();
        Self { public, fields }
    }
}

#[derive(Debug, Clone)]
struct FmBlock {
    content_start: usize,
    content_end: usize,
    line_start: usize,
}

#[derive(Debug, Clone)]
struct ParsedFrontmatter {
    props: Vec<Prop>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
struct Prop {
    key: String,
    range_start: usize,
    range_end: usize,
    line_start: usize,
    line_end: usize,
    kind: FmValueKind,
    value: Value,
    children: Vec<Child>,
    list_items: Vec<ListItem>,
    flow_map: bool,
}

#[derive(Debug, Clone)]
struct Child {
    key: String,
    range_start: usize,
    range_end: usize,
    line_start: usize,
    line_end: usize,
    kind: FmValueKind,
    value: Value,
}

#[derive(Debug, Clone)]
struct ListItem {
    value: String,
    range_start: usize,
    range_end: usize,
}

#[derive(Debug, Clone)]
struct FmLine<'a> {
    text: &'a str,
    start: usize,
    end: usize,
    number: usize,
}

impl ParsedFrontmatter {
    fn find_top(&self, key: &str) -> Option<&Prop> {
        self.props.iter().find(|prop| prop.key == key)
    }

    fn find_path(&self, path: &[String]) -> Option<FmValueKind> {
        let prop = self.find_top(path.first()?)?;
        if path.len() == 1 {
            return Some(prop.kind);
        }
        prop.find_child(&path[1]).map(|child| child.kind)
    }

    fn properties(&self) -> Vec<FmProperty> {
        let mut output = Vec::new();
        for prop in &self.props {
            output.push(FmProperty {
                path: vec![prop.key.clone()],
                value: prop.value.clone(),
                kind: prop.kind,
                line_start: prop.line_start,
                line_end: prop.line_end,
            });
            for child in &prop.children {
                output.push(FmProperty {
                    path: vec![prop.key.clone(), child.key.clone()],
                    value: child.value.clone(),
                    kind: child.kind,
                    line_start: child.line_start,
                    line_end: child.line_end,
                });
            }
        }
        output
    }
}

impl Prop {
    fn find_child(&self, key: &str) -> Option<&Child> {
        self.children.iter().find(|child| child.key == key)
    }
}

fn parse_frontmatter_block(source: &str, block: &FmBlock) -> ParsedFrontmatter {
    let lines = fm_lines(source, block);
    let mut props = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen = BTreeSet::new();
    let mut index = 0;
    while index < lines.len() {
        let line = &lines[index];
        if line.text.trim().is_empty() || line.text.trim_start().starts_with('#') {
            index += 1;
            continue;
        }
        if is_indented(line.text) {
            diagnostics.push(diagnostic(
                "unsupported_yaml_construct",
                "Indented frontmatter content appears before a top-level key.",
                Some(line.number),
                None,
                DiagnosticSeverity::Warning,
            ));
            index += 1;
            continue;
        }
        let Some((key, inline)) = split_key_value(line.text) else {
            diagnostics.push(diagnostic(
                "unsupported_yaml_construct",
                "Frontmatter line is not a simple key/value pair.",
                Some(line.number),
                None,
                DiagnosticSeverity::Warning,
            ));
            index += 1;
            continue;
        };
        if !seen.insert(key.to_string()) {
            diagnostics.push(diagnostic(
                "duplicate_frontmatter_key",
                "Duplicate top-level frontmatter key.",
                Some(line.number),
                Some(vec![key.to_string()]),
                DiagnosticSeverity::Warning,
            ));
        }
        let mut end_index = index + 1;
        while end_index < lines.len()
            && (is_indented(lines[end_index].text)
                || lines[end_index].text.trim().is_empty()
                || lines[end_index].text.trim_start().starts_with('#'))
        {
            end_index += 1;
        }
        let nested = &lines[index + 1..end_index];
        let prop = parse_prop(key, inline, line, nested, end_index, &lines, block);
        props.push(prop);
        index = end_index;
    }
    ParsedFrontmatter { props, diagnostics }
}

fn parse_prop(
    key: &str,
    inline: &str,
    line: &FmLine<'_>,
    nested: &[FmLine<'_>],
    end_index: usize,
    lines: &[FmLine<'_>],
    block: &FmBlock,
) -> Prop {
    let range_end = lines
        .get(end_index)
        .map_or(block.content_end, |next| next.start);
    if inline.trim().is_empty() {
        if nested
            .iter()
            .any(|line| line.text.trim_start().starts_with("- "))
        {
            let items = nested
                .iter()
                .filter_map(|line| {
                    let text = line.text.trim_start();
                    text.strip_prefix("- ").map(|value| ListItem {
                        value: parse_scalar(value.trim())
                            .0
                            .as_str()
                            .unwrap_or(value.trim())
                            .to_string(),
                        range_start: line.start,
                        range_end: line.end,
                    })
                })
                .collect::<Vec<_>>();
            let values = items
                .iter()
                .map(|item| Value::String(item.value.clone()))
                .collect::<Vec<_>>();
            return Prop {
                key: key.to_string(),
                range_start: line.start,
                range_end,
                line_start: line.number,
                line_end: lines
                    .get(end_index.saturating_sub(1))
                    .map_or(line.number, |line| line.number),
                kind: FmValueKind::List,
                value: Value::Array(values),
                children: Vec::new(),
                list_items: items,
                flow_map: false,
            };
        }
        let children = nested
            .iter()
            .filter_map(|line| parse_child(line))
            .collect::<Vec<_>>();
        if !children.is_empty() {
            let mut map = Map::new();
            for child in &children {
                map.insert(child.key.clone(), child.value.clone());
            }
            return Prop {
                key: key.to_string(),
                range_start: line.start,
                range_end,
                line_start: line.number,
                line_end: lines
                    .get(end_index.saturating_sub(1))
                    .map_or(line.number, |line| line.number),
                kind: FmValueKind::Map,
                value: Value::Object(map),
                children,
                list_items: Vec::new(),
                flow_map: false,
            };
        }
        return Prop {
            key: key.to_string(),
            range_start: line.start,
            range_end,
            line_start: line.number,
            line_end: line.number,
            kind: FmValueKind::Blank,
            value: Value::String(String::new()),
            children: Vec::new(),
            list_items: Vec::new(),
            flow_map: false,
        };
    }
    if let Some(map) = parse_flow_map(inline.trim()) {
        let children = map
            .iter()
            .map(|(key, value)| Child {
                key: key.clone(),
                range_start: line.start,
                range_end: line.end,
                line_start: line.number,
                line_end: line.number,
                kind: kind_for_value(value),
                value: value.clone(),
            })
            .collect::<Vec<_>>();
        return Prop {
            key: key.to_string(),
            range_start: line.start,
            range_end,
            line_start: line.number,
            line_end: line.number,
            kind: FmValueKind::Map,
            value: Value::Object(map),
            children,
            list_items: Vec::new(),
            flow_map: true,
        };
    }
    if let Some(values) = parse_inline_list(inline.trim()) {
        return Prop {
            key: key.to_string(),
            range_start: line.start,
            range_end,
            line_start: line.number,
            line_end: line.number,
            kind: FmValueKind::List,
            value: Value::Array(values.into_iter().map(Value::String).collect()),
            children: Vec::new(),
            list_items: Vec::new(),
            flow_map: false,
        };
    }
    let (value, kind) = parse_scalar(inline.trim());
    Prop {
        key: key.to_string(),
        range_start: line.start,
        range_end,
        line_start: line.number,
        line_end: line.number,
        kind,
        value,
        children: Vec::new(),
        list_items: Vec::new(),
        flow_map: false,
    }
}

fn parse_child(line: &FmLine<'_>) -> Option<Child> {
    let text = line.text.strip_prefix("  ")?;
    if text.starts_with(' ') || text.trim_start().starts_with("- ") {
        return None;
    }
    let (key, value) = split_key_value(text)?;
    let (value, kind) = parse_scalar(value.trim());
    Some(Child {
        key: key.to_string(),
        range_start: line.start,
        range_end: line.end,
        line_start: line.number,
        line_end: line.number,
        kind,
        value,
    })
}

fn fm_lines<'a>(source: &'a str, block: &FmBlock) -> Vec<FmLine<'a>> {
    let mut output = Vec::new();
    let mut start = block.content_start;
    let mut number = block.line_start + 1;
    for part in source[block.content_start..block.content_end].split_inclusive('\n') {
        let end = start + part.len();
        let content_end =
            end - usize::from(part.ends_with('\n')) - usize::from(part.ends_with("\r\n"));
        output.push(FmLine {
            text: &source[start..content_end],
            start,
            end,
            number,
        });
        start = end;
        number += 1;
    }
    if start < block.content_end {
        output.push(FmLine {
            text: &source[start..block.content_end],
            start,
            end: block.content_end,
            number,
        });
    }
    output
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim();
    if key.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    Some((key, value.trim_start()))
}

fn parse_scalar(value: &str) -> (Value, FmValueKind) {
    let stripped = strip_yaml_quotes(value);
    if value.is_empty() {
        return (Value::String(String::new()), FmValueKind::Blank);
    }
    if value == "null" || value == "~" {
        return (Value::Null, FmValueKind::Null);
    }
    if value == "true" {
        return (Value::Bool(true), FmValueKind::Bool);
    }
    if value == "false" {
        return (Value::Bool(false), FmValueKind::Bool);
    }
    if let Ok(value) = value.parse::<i64>() {
        return (json!(value), FmValueKind::Int);
    }
    if let Ok(value) = value.parse::<f64>() {
        return (json!(value), FmValueKind::Float);
    }
    if looks_like_date(value) {
        return (Value::String(value.to_string()), FmValueKind::Date);
    }
    (Value::String(stripped), FmValueKind::String)
}

fn parse_flow_map(value: &str) -> Option<Map<String, Value>> {
    let inner = value.strip_prefix('{')?.strip_suffix('}')?;
    let mut map = Map::new();
    if inner.trim().is_empty() {
        return Some(map);
    }
    for part in inner.split(',') {
        let (key, value) = split_key_value(part.trim())?;
        map.insert(key.to_string(), parse_scalar(value.trim()).0);
    }
    Some(map)
}

fn parse_inline_list(value: &str) -> Option<Vec<String>> {
    let inner = value.strip_prefix('[')?.strip_suffix(']')?;
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    Some(
        inner
            .split(',')
            .map(|value| strip_yaml_quotes(value.trim()))
            .collect(),
    )
}

fn render_set_value(key: &str, value: &FmSetValue, newline: &str) -> Result<String, AimdError> {
    if let FmSetValue::Map(value) = value {
        let object = value.as_object().ok_or_else(|| {
            fm_error("invalid_frontmatter_value").hint("--map expects a YAML or JSON object.")
        })?;
        let mut output = format!("{key}:{newline}");
        for (child_key, value) in object {
            output.push_str("  ");
            output.push_str(child_key);
            output.push_str(": ");
            output.push_str(&render_json_inline_value(value));
            output.push_str(newline);
        }
        return Ok(output);
    }
    Ok(format!("{key}: {}{newline}", render_inline_value(value)))
}

fn render_list(key: &str, values: &[String], newline: &str) -> String {
    let mut output = format!("{key}:{newline}");
    for value in values {
        output.push_str("  - ");
        output.push_str(&quote_yaml_string(value));
        output.push_str(newline);
    }
    output
}

fn render_inline_value(value: &FmSetValue) -> String {
    match value {
        FmSetValue::String(value) => quote_yaml_string(value),
        FmSetValue::Int(value) => value.to_string(),
        FmSetValue::Float(value) => value.to_string(),
        FmSetValue::Bool(value) => value.to_string(),
        FmSetValue::Date(value) => value.to_string(),
        FmSetValue::Blank => String::new(),
        FmSetValue::Null => "null".to_string(),
        FmSetValue::Map(value) => render_json_inline_value(value),
    }
}

fn render_json_inline_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => quote_yaml_string(value),
        _ => quote_yaml_string(&value.to_string()),
    }
}

fn quote_yaml_string(value: &str) -> String {
    if value.is_empty()
        || value != value.trim()
        || value.contains(':')
        || value.contains('#')
        || value.contains("[[")
        || matches!(value, "true" | "false" | "null" | "~")
        || looks_like_date(value)
    {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

fn strip_yaml_quotes(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

fn looks_like_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

fn is_indented(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

fn kind_for_value(value: &Value) -> FmValueKind {
    match value {
        Value::Null => FmValueKind::Null,
        Value::Bool(_) => FmValueKind::Bool,
        Value::Number(number) if number.is_i64() || number.is_u64() => FmValueKind::Int,
        Value::Number(_) => FmValueKind::Float,
        Value::String(value) if value.is_empty() => FmValueKind::Blank,
        Value::String(value) if looks_like_date(value) => FmValueKind::Date,
        Value::String(_) => FmValueKind::String,
        Value::Array(_) => FmValueKind::List,
        Value::Object(_) => FmValueKind::Map,
    }
}

fn parse_schema(source: &str) -> Result<ParsedSchema, AimdError> {
    let mut name = None;
    let mut version = None;
    let mut fields = BTreeMap::<Vec<String>, FmSchemaField>::new();
    let mut current: Option<Vec<String>> = None;
    let mut current_child_parent: Option<String> = None;
    for line in source.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.chars().take_while(|ch| *ch == ' ').count();
        let trimmed = line.trim();
        if indent == 0 {
            if let Some((key, value)) = split_key_value(trimmed) {
                match key {
                    "name" => name = Some(strip_yaml_quotes(value)),
                    "version" => version = value.parse::<u64>().ok(),
                    _ => {}
                }
            }
            continue;
        }
        if indent == 2 && trimmed.ends_with(':') {
            let key = trimmed.trim_end_matches(':').to_string();
            current = Some(vec![key.clone()]);
            current_child_parent = None;
            fields.entry(vec![key]).or_insert(FmSchemaField {
                path: current.clone().unwrap(),
                kind: FmValueKind::Any,
                required: false,
                order: None,
                style: None,
            });
            continue;
        }
        if indent == 4 && trimmed == "fields:" {
            current_child_parent = current.as_ref().and_then(|path| path.first().cloned());
            continue;
        }
        if indent == 6 && trimmed.ends_with(':') {
            if let Some(parent) = &current_child_parent {
                let path = vec![parent.clone(), trimmed.trim_end_matches(':').to_string()];
                current = Some(path.clone());
                fields.entry(path.clone()).or_insert(FmSchemaField {
                    path,
                    kind: FmValueKind::Any,
                    required: false,
                    order: None,
                    style: None,
                });
            }
            continue;
        }
        if let Some(path) = current.clone()
            && let Some((key, value)) = split_key_value(trimmed)
        {
            let field = fields.get_mut(&path).expect("schema field exists");
            match key {
                "type" => field.kind = parse_kind(value),
                "required" => field.required = value == "true",
                "order" => field.order = value.parse::<i64>().ok(),
                "style" => field.style = Some(value.to_string()),
                _ => {}
            }
        }
    }
    let public = FmSchema {
        name,
        version,
        fields: fields.values().cloned().collect(),
    };
    Ok(ParsedSchema { public, fields })
}

fn parse_kind(value: &str) -> FmValueKind {
    match value {
        "string" => FmValueKind::String,
        "int" => FmValueKind::Int,
        "float" => FmValueKind::Float,
        "bool" => FmValueKind::Bool,
        "date" => FmValueKind::Date,
        "list" => FmValueKind::List,
        "map" => FmValueKind::Map,
        _ => FmValueKind::Any,
    }
}

fn schema_diagnostics(parsed: &ParsedFrontmatter, schema: &FmSchema) -> Vec<Diagnostic> {
    let schema = ParsedSchema::from_schema(schema.clone());
    let mut diagnostics = Vec::new();
    for field in schema.fields.values() {
        match parsed.find_path(&field.path) {
            Some(kind) if field.kind != FmValueKind::Any && kind != field.kind => {
                diagnostics.push(diagnostic(
                    "frontmatter_schema_type_mismatch",
                    "Frontmatter property type does not match schema.",
                    None,
                    Some(field.path.clone()),
                    DiagnosticSeverity::Warning,
                ));
            }
            Some(_) => {}
            None if field.required => diagnostics.push(diagnostic(
                "frontmatter_required_key_missing",
                "Required frontmatter property is missing.",
                None,
                Some(field.path.clone()),
                DiagnosticSeverity::Warning,
            )),
            None => {}
        }
    }
    diagnostics
}

fn validate_schema_write(
    path: &PropertyPath,
    kind: FmValueKind,
    schema: Option<&FmSchema>,
) -> Result<(), AimdError> {
    let Some(field) = schema_field(path, schema) else {
        return Ok(());
    };
    if field.kind != FmValueKind::Any && field.kind != kind {
        return Err(fm_error("frontmatter_schema_type_mismatch")
            .selector(path.segments())
            .hint("Write value type does not match schema."));
    }
    Ok(())
}

fn schema_field<'a>(
    path: &PropertyPath,
    schema: Option<&'a FmSchema>,
) -> Option<&'a FmSchemaField> {
    schema?
        .fields
        .iter()
        .find(|field| field.path == path.segments())
}

fn insertion_byte(
    source: &str,
    block: &FmBlock,
    rendered: &str,
    schema: Option<&FmSchema>,
) -> Option<usize> {
    let key = rendered.split_once(':')?.0;
    let field = schema?.fields.iter().find(|field| field.path == [key])?;
    let order = field.order?;
    let parsed = parse_frontmatter_block(source, block);
    for prop in parsed.props {
        let prop_order = schema?
            .fields
            .iter()
            .find(|field| field.path == [prop.key.as_str()])
            .and_then(|field| field.order);
        if prop_order.is_some_and(|prop_order| prop_order > order) {
            return Some(prop.range_start);
        }
    }
    None
}

fn render_schema_placeholder(field: &FmSchemaField, newline: &str) -> String {
    let key = field.path.last().cloned().unwrap_or_default();
    match field.kind {
        FmValueKind::List => format!("{key}:{newline}"),
        FmValueKind::Map => format!("{key}:{newline}"),
        FmValueKind::Bool => format!("{key}: false{newline}"),
        FmValueKind::Int => format!("{key}: 0{newline}"),
        FmValueKind::Float => format!("{key}: 0.0{newline}"),
        FmValueKind::Null => format!("{key}: null{newline}"),
        _ => format!("{key}: {newline}"),
    }
}

fn create_frontmatter(source: &str, rendered: &str, newline: &str) -> String {
    let mut output = String::new();
    output.push_str("---");
    output.push_str(newline);
    output.push_str(rendered);
    output.push_str("---");
    output.push_str(newline);
    output.push_str(newline);
    output.push_str(source);
    ensure_final_newline(output, newline)
}

fn ensure_final_newline(mut output: String, newline: &str) -> String {
    if !output.ends_with(newline) {
        output.push_str(newline);
    }
    output
}

fn diagnostic(
    code: &str,
    message: &str,
    line: Option<usize>,
    path: Option<Vec<String>>,
    severity: DiagnosticSeverity,
) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        message: message.to_string(),
        line,
        path,
        severity,
    }
}

fn fm_error(code: &str) -> AimdError {
    AimdError {
        error: code.to_string(),
        selector: None,
        line: None,
        hint: None,
        matches: Vec::new(),
    }
}
