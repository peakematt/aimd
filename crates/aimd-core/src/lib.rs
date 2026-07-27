mod frontmatter_ops;

pub use frontmatter_ops::{
    FmCheck, FmMutation, FmProperty, FmSchema, FmSetValue, FmValueKind, PropertyPath,
};

use serde::Serialize;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Serialize)]
pub struct Document {
    #[serde(skip)]
    source: String,
    #[serde(skip)]
    newline: String,
    #[serde(skip)]
    line_starts: Vec<usize>,
    pub frontmatter: Frontmatter,
    pub sections: Vec<Section>,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Frontmatter {
    pub present: bool,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub byte_start: Option<usize>,
    pub byte_end: Option<usize>,
    pub malformed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Section {
    pub path: Vec<String>,
    pub level: u8,
    pub heading: String,
    pub child_index: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub body_line_start: Option<usize>,
    pub body_line_end: Option<usize>,
    pub byte_start: usize,
    pub byte_end: usize,
    pub body_byte_start: usize,
    pub body_byte_end: usize,
    pub child_count: usize,
    pub has_body: bool,
    pub children: Vec<Section>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub line: Option<usize>,
    pub path: Option<Vec<String>>,
    pub severity: DiagnosticSeverity,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Mutation {
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SectionContent {
    pub selector: Selector,
    pub path: Vec<String>,
    pub shallow: bool,
    pub line_start: usize,
    pub line_end: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Selector {
    Path { path: Vec<String> },
    Line { line: usize },
}

#[derive(Debug, Clone, Serialize, Error)]
#[error("{error}")]
pub struct AimdError {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub matches: Vec<ErrorMatch>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorMatch {
    pub line_start: usize,
    pub line_end: usize,
    pub path: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Placement {
    End,
    BeforeChildIndex(usize),
    AfterChildIndex(usize),
    BeforeChildHeading(String),
    AfterChildHeading(String),
}

#[derive(Debug, Clone)]
struct Line<'a> {
    text: &'a str,
    start: usize,
    end: usize,
    number: usize,
}

#[derive(Debug, Clone)]
struct HeadingRecord {
    level: u8,
    heading: String,
    byte_start: usize,
    heading_end: usize,
    line_start: usize,
}

#[derive(Debug, Clone)]
struct Node {
    heading: HeadingRecord,
    parent: Option<usize>,
    children: Vec<usize>,
    child_index: usize,
    byte_end: usize,
}

impl Document {
    pub fn parse(source: impl Into<String>) -> Self {
        let source = source.into();
        let newline = detect_newline(&source).to_string();
        let lines = lines(&source);
        let line_starts = lines.iter().map(|line| line.start).collect::<Vec<_>>();
        let mut warnings = Vec::new();
        let (frontmatter, scan_start_line) = frontmatter(&source, &lines, &mut warnings);
        let headings = scan_headings(&lines, scan_start_line);
        let nodes = build_nodes(&source, headings);
        let sections = materialize_sections(&source, &line_starts, &nodes, None, Vec::new());
        warnings.extend(check_duplicate_paths(&sections));
        warnings.extend(check_skipped_levels(&sections));

        Self {
            source,
            newline,
            line_starts,
            frontmatter,
            sections,
            warnings,
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn newline(&self) -> &str {
        &self.newline
    }

    pub fn outline(
        &self,
        root: Option<&[String]>,
        max_level: Option<u8>,
    ) -> Result<Vec<Section>, AimdError> {
        let source = match root {
            Some(path) => vec![self.resolve_unique(path)?.clone()],
            None => self.sections.clone(),
        };
        Ok(filter_sections(&source, max_level))
    }

    pub fn get_path(&self, path: &[String], shallow: bool) -> Result<SectionContent, AimdError> {
        let section = self.resolve_unique(path)?;
        Ok(self.section_content(
            section,
            Selector::Path {
                path: path.to_vec(),
            },
            shallow,
        ))
    }

    pub fn get_line(&self, line: usize, shallow: bool) -> Result<SectionContent, AimdError> {
        if line == 0 || line > self.line_count() {
            return Err(error("invalid_line").line(line));
        }
        if self.frontmatter.contains_line(line) {
            return Err(error("line_in_frontmatter").line(line));
        }
        let section = find_line_section(&self.sections, line)
            .ok_or_else(|| error("line_outside_section").line(line))?;
        Ok(self.section_content(section, Selector::Line { line }, shallow))
    }

    pub fn replace(
        &self,
        path: &[String],
        content: &str,
        shallow: bool,
    ) -> Result<Mutation, AimdError> {
        let section = self.resolve_unique(path)?;
        if shallow {
            if starts_with_heading(content) {
                return Err(error("heading_in_shallow_replacement")
                    .hint("Use replace without --shallow to replace a subtree, or provide body-only content."));
            }
            let has_children = section.child_count > 0;
            return Ok(Mutation {
                output: self.splice(
                    section.body_byte_start,
                    section.body_byte_end,
                    &format_shallow_body(content, &self.newline, has_children),
                ),
            });
        }

        let replacement_heading = first_heading(content).ok_or_else(|| {
            error("replacement_heading_mismatch")
                .selector(path)
                .hint("Full replacement content must start with the selected section heading.")
        })?;
        if replacement_heading.level != section.level
            || replacement_heading.heading != section.heading
        {
            return Err(error("replacement_heading_mismatch")
                .selector(path)
                .hint("Replacement heading level and text must match the selected section."));
        }
        Ok(Mutation {
            output: self.splice(
                section.byte_start,
                section.byte_end,
                &normalize_payload(content, &self.newline, true),
            ),
        })
    }

    pub fn append(&self, path: &[String], content: &str) -> Result<Mutation, AimdError> {
        let section = self.resolve_unique(path)?;
        Ok(Mutation {
            output: self.insert_at(
                section.body_byte_end,
                &format_body_append(&self.source, section.body_byte_end, content, &self.newline),
            ),
        })
    }

    pub fn append_child(
        &self,
        path: &[String],
        heading: &str,
        content: &str,
        placement: Placement,
    ) -> Result<Mutation, AimdError> {
        let section = self.resolve_unique(path)?;
        let heading = heading.trim();
        if heading.is_empty() || starts_with_heading(heading) {
            return Err(
                error("invalid_heading").hint("append-child --heading expects plain heading text.")
            );
        }
        if section.level >= 6 {
            return Err(error("cannot_append_child_to_h6").selector(path));
        }
        let insert_at = child_insert_byte(section, &placement)?;
        let level = section.level + 1;
        let hashes = "#".repeat(level as usize);
        let mut child = format!("{hashes} {heading}{}", self.newline);
        let body = normalize_payload(content, &self.newline, false);
        if !body.trim().is_empty() {
            child.push_str(&self.newline);
            child.push_str(&body);
            if !child.ends_with(&self.newline) {
                child.push_str(&self.newline);
            }
        }
        Ok(Mutation {
            output: self.insert_at(
                insert_at,
                &format_section_insert(&self.source, insert_at, &child, &self.newline),
            ),
        })
    }

    pub fn check(&self) -> Vec<Diagnostic> {
        self.warnings.clone()
    }

    fn section_content(
        &self,
        section: &Section,
        selector: Selector,
        shallow: bool,
    ) -> SectionContent {
        let (byte_start, byte_end, line_end) = if shallow {
            (
                section.byte_start,
                section.body_byte_end,
                self.line_for_end(section.body_byte_end)
                    .unwrap_or(section.line_start),
            )
        } else {
            (section.byte_start, section.byte_end, section.line_end)
        };
        SectionContent {
            selector,
            path: section.path.clone(),
            shallow,
            line_start: section.line_start,
            line_end,
            byte_start,
            byte_end,
            content: self.source[byte_start..byte_end].to_string(),
        }
    }

    fn resolve_unique(&self, path: &[String]) -> Result<&Section, AimdError> {
        if path.is_empty() {
            return Err(error("invalid_selector")
                .hint("Selectors must contain at least one heading segment."));
        }
        let matches = flatten_sections(&self.sections)
            .into_iter()
            .filter(|section| section.path == path)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(error("section_not_found")
                .selector(path)
                .hint("Run outline --json to discover exact heading paths.")),
            [section] => Ok(section),
            many => Err(error("duplicate_section_path")
                .selector(path)
                .matches(many.iter().map(|section| ErrorMatch {
                    line_start: section.line_start,
                    line_end: section.line_end,
                    path: section.path.clone(),
                }))),
        }
    }

    fn splice(&self, start: usize, end: usize, replacement: &str) -> String {
        let mut output = String::with_capacity(
            self.source.len() - (end - start) + replacement.len() + self.newline.len(),
        );
        output.push_str(&self.source[..start]);
        output.push_str(replacement);
        output.push_str(&self.source[end..]);
        ensure_final_newline(output, &self.newline)
    }

    fn insert_at(&self, byte: usize, content: &str) -> String {
        self.splice(byte, byte, content)
    }

    fn line_count(&self) -> usize {
        if self.source.is_empty() {
            0
        } else {
            self.line_starts.len()
        }
    }

    fn line_for_end(&self, end: usize) -> Option<usize> {
        if end == 0 {
            None
        } else {
            Some(line_for_byte(&self.line_starts, end.saturating_sub(1)))
        }
    }
}

impl Frontmatter {
    fn contains_line(&self, line: usize) -> bool {
        matches!(
            (self.line_start, self.line_end),
            (Some(start), Some(end)) if line >= start && line <= end
        )
    }
}

impl AimdError {
    fn selector(mut self, path: &[String]) -> Self {
        self.selector = Some(path.to_vec());
        self
    }

    fn line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    fn matches(mut self, matches: impl IntoIterator<Item = ErrorMatch>) -> Self {
        self.matches = matches.into_iter().collect();
        self
    }
}

fn error(code: &str) -> AimdError {
    AimdError {
        error: code.to_string(),
        selector: None,
        line: None,
        hint: None,
        matches: Vec::new(),
    }
}

fn detect_newline(source: &str) -> &str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn lines(source: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (number, part) in source.split_inclusive('\n').enumerate() {
        let end = start + part.len();
        let content_end =
            end - usize::from(part.ends_with('\n')) - usize::from(part.ends_with("\r\n"));
        lines.push(Line {
            text: &source[start..content_end],
            start,
            end,
            number: number + 1,
        });
        start = end;
    }
    if start < source.len() {
        lines.push(Line {
            text: &source[start..],
            start,
            end: source.len(),
            number: lines.len() + 1,
        });
    }
    lines
}

fn frontmatter(
    source: &str,
    lines: &[Line<'_>],
    warnings: &mut Vec<Diagnostic>,
) -> (Frontmatter, usize) {
    let Some(first) = lines.first() else {
        return (empty_frontmatter(), 0);
    };
    if first.text.trim() != "---" || first.start != 0 {
        return (empty_frontmatter(), 0);
    }
    for (index, line) in lines.iter().enumerate().skip(1) {
        if line.text.trim() == "---" {
            return (
                Frontmatter {
                    present: true,
                    line_start: Some(1),
                    line_end: Some(line.number),
                    byte_start: Some(0),
                    byte_end: Some(line.end),
                    malformed: false,
                },
                index + 1,
            );
        }
    }
    warnings.push(Diagnostic {
        code: "malformed_frontmatter".to_string(),
        message: "Document starts with frontmatter delimiter but has no closing delimiter."
            .to_string(),
        line: Some(1),
        path: None,
        severity: DiagnosticSeverity::Warning,
    });
    (
        Frontmatter {
            present: true,
            line_start: Some(1),
            line_end: Some(lines.len()),
            byte_start: Some(0),
            byte_end: Some(source.len()),
            malformed: true,
        },
        lines.len(),
    )
}

fn empty_frontmatter() -> Frontmatter {
    Frontmatter {
        present: false,
        line_start: None,
        line_end: None,
        byte_start: None,
        byte_end: None,
        malformed: false,
    }
}

fn scan_headings(lines: &[Line<'_>], start_line: usize) -> Vec<HeadingRecord> {
    let mut headings = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut html_block: Option<String> = None;

    for line in lines.iter().skip(start_line) {
        if let Some((ch, len)) = fence {
            if is_fence_close(line.text, ch, len) {
                fence = None;
            }
            continue;
        }
        if let Some(close) = &html_block {
            if line.text.to_ascii_lowercase().contains(close) {
                html_block = None;
            }
            continue;
        }
        if let Some((ch, len)) = fence_open(line.text) {
            fence = Some((ch, len));
            continue;
        }
        if let Some(close) = html_block_close(line.text) {
            html_block = Some(close);
            continue;
        }
        if is_indented_code(line.text) {
            continue;
        }
        if let Some((level, heading)) = parse_atx_heading(line.text) {
            headings.push(HeadingRecord {
                level,
                heading,
                byte_start: line.start,
                heading_end: line.end,
                line_start: line.number,
            });
        }
    }
    headings
}

fn build_nodes(source: &str, headings: Vec<HeadingRecord>) -> Vec<Node> {
    let mut nodes = Vec::<Node>::new();
    let mut stack = Vec::<usize>::new();
    for heading in headings {
        while let Some(last) = stack.last().copied() {
            if nodes[last].heading.level < heading.level {
                break;
            }
            stack.pop();
        }
        let parent = stack.last().copied();
        let child_index = parent.map_or_else(
            || nodes.iter().filter(|node| node.parent.is_none()).count(),
            |parent| nodes[parent].children.len(),
        );
        let index = nodes.len();
        nodes.push(Node {
            heading,
            parent,
            children: Vec::new(),
            child_index,
            byte_end: source.len(),
        });
        if let Some(parent) = parent {
            nodes[parent].children.push(index);
        }
        stack.push(index);
    }

    for index in 0..nodes.len() {
        let level = nodes[index].heading.level;
        let end = nodes
            .iter()
            .skip(index + 1)
            .find(|node| node.heading.level <= level)
            .map_or(source.len(), |node| node.heading.byte_start);
        nodes[index].byte_end = end;
    }
    nodes
}

fn materialize_sections(
    source: &str,
    line_starts: &[usize],
    nodes: &[Node],
    parent: Option<usize>,
    parent_path: Vec<String>,
) -> Vec<Section> {
    nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.parent == parent)
        .map(|(index, node)| {
            let mut path = parent_path.clone();
            path.push(node.heading.heading.clone());
            let children =
                materialize_sections(source, line_starts, nodes, Some(index), path.clone());
            let body_byte_start = node.heading.heading_end;
            let body_byte_end = children
                .first()
                .map_or(node.byte_end, |child| child.byte_start);
            let has_body = !source[body_byte_start..body_byte_end].trim().is_empty();
            let body_line_start = if body_byte_start < body_byte_end {
                Some(line_for_byte(line_starts, body_byte_start))
            } else {
                None
            };
            let body_line_end = if body_byte_start < body_byte_end {
                Some(line_for_byte(line_starts, body_byte_end.saturating_sub(1)))
            } else {
                None
            };
            Section {
                path,
                level: node.heading.level,
                heading: node.heading.heading.clone(),
                child_index: node.child_index,
                line_start: node.heading.line_start,
                line_end: line_for_byte(
                    line_starts,
                    node.byte_end
                        .saturating_sub(1)
                        .min(source.len().saturating_sub(1)),
                ),
                body_line_start,
                body_line_end,
                byte_start: node.heading.byte_start,
                byte_end: node.byte_end,
                body_byte_start,
                body_byte_end,
                child_count: children.len(),
                has_body,
                children,
            }
        })
        .collect()
}

fn filter_sections(sections: &[Section], max_level: Option<u8>) -> Vec<Section> {
    sections
        .iter()
        .filter_map(|section| {
            if max_level.is_some_and(|max| section.level > max) {
                return None;
            }
            let mut section = section.clone();
            section.children = filter_sections(&section.children, max_level);
            section.child_count = section.children.len();
            Some(section)
        })
        .collect()
}

fn flatten_sections(sections: &[Section]) -> Vec<&Section> {
    let mut flattened = Vec::new();
    for section in sections {
        flattened.push(section);
        flattened.extend(flatten_sections(&section.children));
    }
    flattened
}

fn find_line_section(sections: &[Section], line: usize) -> Option<&Section> {
    for section in sections {
        if line >= section.line_start && line <= section.line_end {
            if let Some(child) = find_line_section(&section.children, line) {
                return Some(child);
            }
            return Some(section);
        }
    }
    None
}

fn line_for_byte(line_starts: &[usize], byte: usize) -> usize {
    line_starts.partition_point(|start| *start <= byte)
}

fn check_duplicate_paths(sections: &[Section]) -> Vec<Diagnostic> {
    let mut map: HashMap<Vec<String>, Vec<&Section>> = HashMap::new();
    for section in flatten_sections(sections) {
        map.entry(section.path.clone()).or_default().push(section);
    }
    map.into_iter()
        .filter(|(_, sections)| sections.len() > 1)
        .flat_map(|(path, sections)| {
            sections.into_iter().map(move |section| Diagnostic {
                code: "duplicate_section_path".to_string(),
                message: "Duplicate exact heading path.".to_string(),
                line: Some(section.line_start),
                path: Some(path.clone()),
                severity: DiagnosticSeverity::Warning,
            })
        })
        .collect()
}

fn check_skipped_levels(sections: &[Section]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    fn walk(section: &Section, diagnostics: &mut Vec<Diagnostic>) {
        for child in &section.children {
            if child.level > section.level + 1 {
                diagnostics.push(Diagnostic {
                    code: "skipped_heading_level".to_string(),
                    message: "Heading level skips one or more intermediate levels.".to_string(),
                    line: Some(child.line_start),
                    path: Some(child.path.clone()),
                    severity: DiagnosticSeverity::Warning,
                });
            }
            walk(child, diagnostics);
        }
    }
    for section in sections {
        walk(section, &mut diagnostics);
    }
    diagnostics
}

fn child_insert_byte(section: &Section, placement: &Placement) -> Result<usize, AimdError> {
    match placement {
        Placement::End => Ok(section
            .children
            .last()
            .map_or(section.body_byte_end, |child| child.byte_end)),
        Placement::BeforeChildIndex(index) => section
            .children
            .get(*index)
            .map(|child| child.byte_start)
            .ok_or_else(|| error("invalid_child_index")),
        Placement::AfterChildIndex(index) => section
            .children
            .get(*index)
            .map(|child| child.byte_end)
            .ok_or_else(|| error("invalid_child_index")),
        Placement::BeforeChildHeading(heading) | Placement::AfterChildHeading(heading) => {
            let matches = section
                .children
                .iter()
                .filter(|child| child.heading == *heading)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [] => Err(error("section_not_found")
                    .hint("No direct child heading matches placement selector.")),
                [child] if matches!(placement, Placement::BeforeChildHeading(_)) => {
                    Ok(child.byte_start)
                }
                [child] => Ok(child.byte_end),
                many => Err(
                    error("duplicate_child_heading").matches(many.iter().map(|section| {
                        ErrorMatch {
                            line_start: section.line_start,
                            line_end: section.line_end,
                            path: section.path.clone(),
                        }
                    })),
                ),
            }
        }
    }
}

fn format_body_append(source: &str, byte: usize, content: &str, newline: &str) -> String {
    let content = normalize_payload(content, newline, false);
    if content.trim().is_empty() {
        return String::new();
    }
    let mut output = String::new();
    if needs_leading_blank(source, byte) {
        output.push_str(newline);
    }
    output.push_str(&content);
    if !output.ends_with(newline) {
        output.push_str(newline);
    }
    if needs_trailing_blank(source, byte) {
        output.push_str(newline);
    }
    output
}

fn format_shallow_body(content: &str, newline: &str, has_children: bool) -> String {
    let mut output = normalize_payload(content, newline, true);
    if has_children && !output.is_empty() && !output.ends_with(&format!("{newline}{newline}")) {
        output.push_str(newline);
    }
    output
}

fn format_section_insert(source: &str, byte: usize, content: &str, newline: &str) -> String {
    let mut output = String::new();
    if needs_leading_blank(source, byte) {
        output.push_str(newline);
    }
    output.push_str(content);
    if !output.ends_with(newline) {
        output.push_str(newline);
    }
    if needs_trailing_blank(source, byte) {
        output.push_str(newline);
    }
    output
}

fn needs_leading_blank(source: &str, byte: usize) -> bool {
    if byte == 0 {
        return false;
    }
    let before = &source[..byte];
    !before.ends_with("\n\n") && !before.ends_with("\r\n\r\n")
}

fn needs_trailing_blank(source: &str, byte: usize) -> bool {
    if byte >= source.len() {
        return false;
    }
    let after = &source[byte..];
    !after.starts_with('\n') && !after.starts_with("\r\n")
}

fn normalize_payload(content: &str, newline: &str, final_newline: bool) -> String {
    let mut output = content.replace("\r\n", "\n").replace('\r', "\n");
    if newline != "\n" {
        output = output.replace('\n', newline);
    }
    if final_newline {
        ensure_final_newline(output, newline)
    } else {
        output
    }
}

fn ensure_final_newline(mut output: String, newline: &str) -> String {
    if !output.is_empty() && !output.ends_with(newline) {
        output.push_str(newline);
    }
    output
}

fn starts_with_heading(content: &str) -> bool {
    content.lines().next().and_then(parse_atx_heading).is_some()
}

fn first_heading(content: &str) -> Option<HeadingRecord> {
    let line = content.lines().next()?;
    let (level, heading) = parse_atx_heading(line)?;
    Some(HeadingRecord {
        level,
        heading,
        byte_start: 0,
        heading_end: line.len(),
        line_start: 1,
    })
}

fn parse_atx_heading(line: &str) -> Option<(u8, String)> {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces > 0 {
        return None;
    }
    let trimmed = &line[leading_spaces..];
    let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let after_hashes = &trimmed[hashes..];
    if !after_hashes.is_empty() && !after_hashes.starts_with(char::is_whitespace) {
        return None;
    }
    let mut text = after_hashes.trim().to_string();
    if let Some(stripped) = strip_closing_hashes(&text) {
        text = stripped;
    }
    Some((hashes as u8, text))
}

fn strip_closing_hashes(text: &str) -> Option<String> {
    let trimmed_end = text.trim_end();
    let hash_start = trimmed_end.rfind(|ch| ch != '#')?;
    if hash_start + 1 == trimmed_end.len() {
        return None;
    }
    let before = &trimmed_end[..=hash_start];
    if before.ends_with(char::is_whitespace) {
        Some(before.trim_end().to_string())
    } else {
        None
    }
}

fn fence_open(line: &str) -> Option<(char, usize)> {
    let trimmed = trim_up_to_three_spaces(line)?;
    let ch = trimmed.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let len = trimmed.chars().take_while(|c| *c == ch).count();
    (len >= 3).then_some((ch, len))
}

fn is_fence_close(line: &str, ch: char, len: usize) -> bool {
    let Some(trimmed) = trim_up_to_three_spaces(line) else {
        return false;
    };
    let close_len = trimmed.chars().take_while(|c| *c == ch).count();
    close_len >= len && trimmed[close_len..].trim().is_empty()
}

fn trim_up_to_three_spaces(line: &str) -> Option<&str> {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    (leading_spaces <= 3).then_some(&line[leading_spaces..])
}

fn is_indented_code(line: &str) -> bool {
    line.starts_with("    ") || line.starts_with('\t')
}

fn html_block_close(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('<') {
        return None;
    }
    if trimmed.starts_with("<!--") {
        return (!trimmed.contains("-->")).then(|| "-->".to_string());
    }
    for tag in [
        "article",
        "aside",
        "blockquote",
        "details",
        "div",
        "figure",
        "footer",
        "header",
        "main",
        "nav",
        "section",
        "script",
        "style",
        "table",
    ] {
        let open = format!("<{tag}");
        if trimmed
            .get(..open.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&open))
            && !trimmed.to_ascii_lowercase().contains(&format!("</{tag}>"))
        {
            return Some(format!("</{tag}>"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_string()).collect()
    }

    #[test]
    fn parses_nested_headings_and_ignores_fenced_headings() {
        let doc = Document::parse("# A\nBody\n\n```md\n## Nope\n```\n\n## B\nText\n");
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].children.len(), 1);
        assert_eq!(doc.sections[0].children[0].heading, "B");
    }

    #[test]
    fn invalid_fence_close_keeps_headings_inside_code_ignored() {
        let doc = Document::parse("# A\n```md\n```not a close\n## Still Code\n```\n## Real\n");
        assert_eq!(doc.sections[0].children.len(), 1);
        assert_eq!(doc.sections[0].children[0].heading, "Real");
    }

    #[test]
    fn ignores_indented_and_html_block_headings() {
        let doc =
            Document::parse("# A\n- item\n  ## List Text\n<DIV>\n## Html Text\n</DIV>\n## Real\n");
        assert_eq!(doc.sections[0].children.len(), 1);
        assert_eq!(doc.sections[0].children[0].heading, "Real");
    }

    #[test]
    fn resolves_shallow_content() {
        let doc = Document::parse("# A\nIntro\n\n## B\nText\n");
        let got = doc.get_path(&path(&["A"]), true).unwrap();
        assert_eq!(got.content, "# A\nIntro\n\n");
    }

    #[test]
    fn rejects_ambiguous_paths() {
        let doc = Document::parse("# A\n## B\nOne\n# A\n## B\nTwo\n");
        let err = doc.get_path(&path(&["A", "B"]), false).unwrap_err();
        assert_eq!(err.error, "duplicate_section_path");
        assert_eq!(err.matches.len(), 2);
    }

    #[test]
    fn replaces_shallow_body_without_children() {
        let doc = Document::parse("# A\nOld\n\n## B\nChild\n");
        let output = doc.replace(&path(&["A"]), "New\n", true).unwrap().output;
        assert_eq!(output, "# A\nNew\n\n## B\nChild\n");
    }
}
