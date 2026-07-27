use aimd_core::{
    AimdError, Diagnostic, Document, FmSchema, FmSetValue, Frontmatter, Placement, PropertyPath,
    Section,
};
use chrono::Utc;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use serde_json::json;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "aimd", version, about = "Targeted structural Markdown edits")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Outline {
        file: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=6))]
        max_level: Option<u8>,
        #[arg(long)]
        root: Option<String>,
    },
    Get {
        file: PathBuf,
        exact_path: Option<String>,
        #[arg(long)]
        line: Option<usize>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        shallow: bool,
    },
    Replace(WriteArgs),
    Append(AppendArgs),
    AppendChild(AppendChildArgs),
    Fm {
        #[command(subcommand)]
        command: FmCommands,
    },
    Check {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Completions {
        shell: CompletionShell,
    },
}

#[derive(Debug, Subcommand)]
enum FmCommands {
    Get(FmGetArgs),
    Set(FmSetArgs),
    SetList(FmListSetArgs),
    AppendListItem(FmListAppendArgs),
    RemoveListItem(FmListRemoveArgs),
    Remove(FmRemoveArgs),
    Has(FmHasArgs),
    Check(FmCheckArgs),
    Normalize(FmNormalizeArgs),
}

#[derive(Debug, Parser)]
struct FmGetArgs {
    file: PathBuf,
    property_path: Option<String>,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    schema: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct FmSetArgs {
    file: PathBuf,
    property_path: String,
    #[arg(long)]
    value: Option<String>,
    #[arg(long = "str")]
    string: Option<String>,
    #[arg(long)]
    int: Option<i64>,
    #[arg(long)]
    float: Option<f64>,
    #[arg(long)]
    bool: Option<bool>,
    #[arg(long)]
    date: Option<String>,
    #[arg(long)]
    blank: bool,
    #[arg(long)]
    null: bool,
    #[arg(long = "map")]
    map_value: Option<String>,
    #[arg(long = "map-file")]
    map_file: Option<PathBuf>,
    #[arg(long)]
    schema: Option<PathBuf>,
    #[arg(long)]
    create: bool,
    #[arg(long, conflicts_with = "stdout")]
    dry_run: bool,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    backup: bool,
}

#[derive(Debug, Parser)]
struct FmListSetArgs {
    file: PathBuf,
    property_path: String,
    values: Vec<String>,
    #[arg(long)]
    schema: Option<PathBuf>,
    #[arg(long)]
    create: bool,
    #[arg(long, conflicts_with = "stdout")]
    dry_run: bool,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    backup: bool,
}

#[derive(Debug, Parser)]
struct FmListAppendArgs {
    file: PathBuf,
    property_path: String,
    values: Vec<String>,
    #[arg(long)]
    allow_duplicate: bool,
    #[arg(long)]
    schema: Option<PathBuf>,
    #[arg(long)]
    create: bool,
    #[arg(long, conflicts_with = "stdout")]
    dry_run: bool,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    backup: bool,
}

#[derive(Debug, Parser)]
struct FmListRemoveArgs {
    file: PathBuf,
    property_path: String,
    values: Vec<String>,
    #[arg(long)]
    schema: Option<PathBuf>,
    #[arg(long, conflicts_with = "stdout")]
    dry_run: bool,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    backup: bool,
}

#[derive(Debug, Parser)]
struct FmRemoveArgs {
    file: PathBuf,
    property_path: String,
    #[arg(long)]
    schema: Option<PathBuf>,
    #[arg(long, conflicts_with = "stdout")]
    dry_run: bool,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    backup: bool,
}

#[derive(Debug, Parser)]
struct FmHasArgs {
    file: PathBuf,
    property_path: String,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    schema: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct FmCheckArgs {
    file: PathBuf,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    schema: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct FmNormalizeArgs {
    file: PathBuf,
    #[arg(long)]
    schema: PathBuf,
    #[arg(long, conflicts_with = "stdout")]
    dry_run: bool,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    backup: bool,
}

#[derive(Debug, Parser)]
struct WriteArgs {
    file: PathBuf,
    exact_path: String,
    #[arg(long = "file")]
    file_content: Option<PathBuf>,
    #[arg(long)]
    content: Option<String>,
    #[arg(long)]
    shallow: bool,
    #[arg(long, conflicts_with = "stdout")]
    dry_run: bool,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    backup: bool,
}

#[derive(Debug, Parser)]
struct AppendArgs {
    file: PathBuf,
    exact_path: String,
    #[arg(long = "file")]
    file_content: Option<PathBuf>,
    #[arg(long)]
    content: Option<String>,
    #[arg(long, conflicts_with = "stdout")]
    dry_run: bool,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    backup: bool,
}

#[derive(Debug, Parser)]
struct AppendChildArgs {
    file: PathBuf,
    exact_path: String,
    #[arg(long)]
    heading: String,
    #[arg(long = "file")]
    file_content: Option<PathBuf>,
    #[arg(long)]
    content: Option<String>,
    #[arg(long)]
    after: Option<String>,
    #[arg(long)]
    before: Option<String>,
    #[arg(long)]
    after_child: Option<usize>,
    #[arg(long)]
    before_child: Option<usize>,
    #[arg(long, conflicts_with = "stdout")]
    dry_run: bool,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    backup: bool,
}

#[derive(Debug, Clone, ValueEnum)]
enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json_errors = cli.json_errors();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            if json_errors {
                let _ = print_error_json(&err);
            } else {
                let _ = writeln!(io::stderr(), "{err}");
            }
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Outline {
            file,
            json,
            max_level,
            root,
        } => {
            let source = read_file(&file)?;
            let doc = Document::parse(source);
            let root = root.map(|path| parse_path(&path));
            let sections = doc.outline(root.as_deref(), max_level)?;
            if json {
                print_json(&json!({
                    "file": file,
                    "frontmatter": doc.frontmatter,
                    "sections": sections,
                    "warnings": doc.warnings,
                }))?;
            } else {
                print_human_outline(&sections);
                print_warnings(&doc.warnings);
            }
        }
        Commands::Get {
            file,
            exact_path,
            line,
            json,
            shallow,
        } => {
            let source = read_file(&file)?;
            let doc = Document::parse(source);
            let content = match (exact_path, line) {
                (Some(_), Some(_)) => {
                    return Err(CliError::Aimd(simple_error(
                        "invalid_selector",
                        "Provide either a path or --line, not both.",
                    )));
                }
                (Some(path), None) => doc.get_path(&parse_path(&path), shallow)?,
                (None, Some(line)) => doc.get_line(line, shallow)?,
                (None, None) => {
                    return Err(CliError::Aimd(simple_error(
                        "invalid_selector",
                        "Provide an exact path or --line.",
                    )));
                }
            };
            if json {
                print_json(&content)?;
            } else {
                print!("{}", content.content);
            }
        }
        Commands::Replace(args) => {
            let source = read_file(&args.file)?;
            let payload = read_payload(args.file_content.as_deref(), args.content.as_deref())?;
            let doc = Document::parse(&source);
            let output = doc
                .replace(&parse_path(&args.exact_path), &payload, args.shallow)?
                .output;
            write_result(
                &args.file,
                &source,
                &output,
                args.dry_run,
                args.stdout,
                args.backup,
            )?;
        }
        Commands::Append(args) => {
            let source = read_file(&args.file)?;
            let payload = read_payload(args.file_content.as_deref(), args.content.as_deref())?;
            let doc = Document::parse(&source);
            let output = doc.append(&parse_path(&args.exact_path), &payload)?.output;
            write_result(
                &args.file,
                &source,
                &output,
                args.dry_run,
                args.stdout,
                args.backup,
            )?;
        }
        Commands::AppendChild(args) => {
            let source = read_file(&args.file)?;
            let payload = read_payload(args.file_content.as_deref(), args.content.as_deref())?;
            let placement = placement(&args)?;
            let doc = Document::parse(&source);
            let output = doc
                .append_child(
                    &parse_path(&args.exact_path),
                    &args.heading,
                    &payload,
                    placement,
                )?
                .output;
            write_result(
                &args.file,
                &source,
                &output,
                args.dry_run,
                args.stdout,
                args.backup,
            )?;
        }
        Commands::Fm { command } => run_fm(command)?,
        Commands::Check { file, json } => {
            let source = read_file(&file)?;
            let doc = Document::parse(source);
            if json {
                print_json(&json!({
                    "file": file,
                    "frontmatter": doc.frontmatter,
                    "diagnostics": doc.check(),
                }))?;
            } else {
                print_warnings(&doc.check());
            }
        }
        Commands::Completions { shell } => {
            let shell: Shell = shell.into();
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            generate(shell, &mut command, name, &mut io::stdout());
        }
    }
    Ok(())
}

impl Cli {
    fn json_errors(&self) -> bool {
        match &self.command {
            Commands::Outline { json, .. }
            | Commands::Get { json, .. }
            | Commands::Check { json, .. } => *json,
            Commands::Fm { command } => command.json_errors(),
            Commands::Replace(_)
            | Commands::Append(_)
            | Commands::AppendChild(_)
            | Commands::Completions { .. } => false,
        }
    }
}

impl FmCommands {
    fn json_errors(&self) -> bool {
        match self {
            FmCommands::Get(args) => args.json,
            FmCommands::Has(args) => args.json,
            FmCommands::Check(args) => args.json,
            FmCommands::Set(_)
            | FmCommands::SetList(_)
            | FmCommands::AppendListItem(_)
            | FmCommands::RemoveListItem(_)
            | FmCommands::Remove(_)
            | FmCommands::Normalize(_) => false,
        }
    }
}

fn run_fm(command: FmCommands) -> Result<(), CliError> {
    match command {
        FmCommands::Get(args) => {
            let source = read_file(&args.file)?;
            let doc = Document::parse(source);
            let schema = read_schema(args.schema.as_deref())?;
            let path = args
                .property_path
                .as_deref()
                .map(PropertyPath::parse)
                .transpose()?;
            let result = doc.fm_get(path.as_ref(), schema.as_ref())?;
            if args.json {
                print_json(&json!({
                    "file": args.file,
                    "frontmatter": result.frontmatter,
                    "properties": result.properties,
                    "diagnostics": result.diagnostics,
                }))?;
            } else {
                print_fm_properties(&result.properties);
                print_warnings(&result.diagnostics);
            }
        }
        FmCommands::Set(args) => {
            let source = read_file(&args.file)?;
            let schema = read_schema(args.schema.as_deref())?;
            let path = PropertyPath::parse(&args.property_path)?;
            let value = fm_set_value(&args)?;
            let doc = Document::parse(&source);
            let output = doc
                .fm_set(&path, value, args.create, schema.as_ref())?
                .output;
            write_result(
                &args.file,
                &source,
                &output,
                args.dry_run,
                args.stdout,
                args.backup,
            )?;
        }
        FmCommands::SetList(args) => {
            let source = read_file(&args.file)?;
            let schema = read_schema(args.schema.as_deref())?;
            let path = PropertyPath::parse(&args.property_path)?;
            let doc = Document::parse(&source);
            let output = doc
                .fm_set_list(&path, &args.values, args.create, schema.as_ref())?
                .output;
            write_result(
                &args.file,
                &source,
                &output,
                args.dry_run,
                args.stdout,
                args.backup,
            )?;
        }
        FmCommands::AppendListItem(args) => {
            let source = read_file(&args.file)?;
            let schema = read_schema(args.schema.as_deref())?;
            let path = PropertyPath::parse(&args.property_path)?;
            let doc = Document::parse(&source);
            let output = doc
                .fm_append_list_item(
                    &path,
                    &args.values,
                    args.allow_duplicate,
                    args.create,
                    schema.as_ref(),
                )?
                .output;
            write_result(
                &args.file,
                &source,
                &output,
                args.dry_run,
                args.stdout,
                args.backup,
            )?;
        }
        FmCommands::RemoveListItem(args) => {
            let source = read_file(&args.file)?;
            let schema = read_schema(args.schema.as_deref())?;
            let path = PropertyPath::parse(&args.property_path)?;
            let doc = Document::parse(&source);
            let output = doc
                .fm_remove_list_item(&path, &args.values, schema.as_ref())?
                .output;
            write_result(
                &args.file,
                &source,
                &output,
                args.dry_run,
                args.stdout,
                args.backup,
            )?;
        }
        FmCommands::Remove(args) => {
            let source = read_file(&args.file)?;
            let schema = read_schema(args.schema.as_deref())?;
            let path = PropertyPath::parse(&args.property_path)?;
            let doc = Document::parse(&source);
            let output = doc.fm_remove(&path, schema.as_ref())?.output;
            write_result(
                &args.file,
                &source,
                &output,
                args.dry_run,
                args.stdout,
                args.backup,
            )?;
        }
        FmCommands::Has(args) => {
            let source = read_file(&args.file)?;
            let schema = read_schema(args.schema.as_deref())?;
            let path = PropertyPath::parse(&args.property_path)?;
            let doc = Document::parse(source);
            doc.fm_has(&path, schema.as_ref())?;
            if args.json {
                print_json(&json!({
                    "file": args.file,
                    "present": true,
                }))?;
            }
        }
        FmCommands::Check(args) => {
            let source = read_file(&args.file)?;
            let schema = read_schema(args.schema.as_deref())?;
            let doc = Document::parse(source);
            let result = doc.fm_check(schema.as_ref())?;
            if args.json {
                print_json(&json!({
                    "file": args.file,
                    "frontmatter": result.frontmatter,
                    "properties": result.properties,
                    "diagnostics": result.diagnostics,
                }))?;
            } else {
                print_warnings(&result.diagnostics);
            }
        }
        FmCommands::Normalize(args) => {
            let source = read_file(&args.file)?;
            let schema = read_schema(Some(&args.schema))?.ok_or_else(|| {
                CliError::Aimd(simple_error(
                    "missing_schema",
                    "normalize requires --schema.",
                ))
            })?;
            let doc = Document::parse(&source);
            let output = doc.fm_normalize(&schema)?.output;
            write_result(
                &args.file,
                &source,
                &output,
                args.dry_run,
                args.stdout,
                args.backup,
            )?;
        }
    }
    Ok(())
}

fn parse_path(path: &str) -> Vec<String> {
    path.split('>')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn placement(args: &AppendChildArgs) -> Result<Placement, CliError> {
    let count = usize::from(args.after.is_some())
        + usize::from(args.before.is_some())
        + usize::from(args.after_child.is_some())
        + usize::from(args.before_child.is_some());
    if count > 1 {
        return Err(CliError::Aimd(simple_error(
            "conflicting_placement_flags",
            "Provide at most one placement flag.",
        )));
    }
    Ok(if let Some(heading) = &args.after {
        Placement::AfterChildHeading(heading.clone())
    } else if let Some(heading) = &args.before {
        Placement::BeforeChildHeading(heading.clone())
    } else if let Some(index) = args.after_child {
        Placement::AfterChildIndex(index)
    } else if let Some(index) = args.before_child {
        Placement::BeforeChildIndex(index)
    } else {
        Placement::End
    })
}

fn fm_set_value(args: &FmSetArgs) -> Result<FmSetValue, CliError> {
    let selected = usize::from(args.value.is_some())
        + usize::from(args.string.is_some())
        + usize::from(args.int.is_some())
        + usize::from(args.float.is_some())
        + usize::from(args.bool.is_some())
        + usize::from(args.date.is_some())
        + usize::from(args.blank)
        + usize::from(args.null)
        + usize::from(args.map_value.is_some())
        + usize::from(args.map_file.is_some());
    if selected != 1 {
        return Err(CliError::Aimd(simple_error(
            "conflicting_frontmatter_value_inputs",
            "Provide exactly one frontmatter value input.",
        )));
    }
    if let Some(value) = &args.value {
        return Ok(FmSetValue::String(value.clone()));
    }
    if let Some(value) = &args.string {
        return Ok(FmSetValue::String(value.clone()));
    }
    if let Some(value) = args.int {
        return Ok(FmSetValue::Int(value));
    }
    if let Some(value) = args.float {
        return Ok(FmSetValue::Float(value));
    }
    if let Some(value) = args.bool {
        return Ok(FmSetValue::Bool(value));
    }
    if let Some(value) = &args.date {
        return Ok(FmSetValue::Date(value.clone()));
    }
    if args.blank {
        return Ok(FmSetValue::Blank);
    }
    if args.null {
        return Ok(FmSetValue::Null);
    }
    if let Some(value) = &args.map_value {
        return Ok(FmSetValue::Map(parse_map_value(value)?));
    }
    if let Some(path) = &args.map_file {
        let payload = if path == Path::new("-") {
            read_stdin()?
        } else {
            fs::read_to_string(path).map_err(CliError::Io)?
        };
        return Ok(FmSetValue::Map(parse_map_value(&payload)?));
    }
    unreachable!("selected count requires one value input")
}

fn parse_map_value(payload: &str) -> Result<serde_json::Value, CliError> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) {
        if value.is_object() {
            return Ok(value);
        }
        return Err(CliError::Aimd(simple_error(
            "invalid_frontmatter_value",
            "--map expects a JSON or YAML object.",
        )));
    }
    let trimmed_payload = payload.trim();
    if trimmed_payload.starts_with('{') && trimmed_payload.ends_with('}') {
        let inner = &trimmed_payload[1..trimmed_payload.len() - 1];
        return parse_map_lines(inner.split(',').map(str::trim));
    }
    parse_map_lines(payload.lines().map(str::trim))
}

fn parse_map_lines<'a>(
    lines: impl Iterator<Item = &'a str>,
) -> Result<serde_json::Value, CliError> {
    let mut object = serde_json::Map::new();
    for trimmed in lines {
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            return Err(CliError::Aimd(simple_error(
                "invalid_frontmatter_value",
                "--map YAML input must be simple key/value pairs.",
            )));
        };
        object.insert(key.trim().to_string(), parse_scalar_json(value.trim()));
    }
    if object.is_empty() {
        return Err(CliError::Aimd(simple_error(
            "invalid_frontmatter_value",
            "--map expects at least one object key.",
        )));
    }
    Ok(serde_json::Value::Object(object))
}

fn parse_scalar_json(value: &str) -> serde_json::Value {
    let unquoted = strip_yaml_quotes(value);
    match value {
        "true" => serde_json::Value::Bool(true),
        "false" => serde_json::Value::Bool(false),
        "null" | "~" => serde_json::Value::Null,
        _ => value
            .parse::<i64>()
            .map(serde_json::Value::from)
            .or_else(|_| value.parse::<f64>().map(serde_json::Value::from))
            .unwrap_or(serde_json::Value::String(unquoted)),
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

fn read_schema(path: Option<&Path>) -> Result<Option<FmSchema>, CliError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let source = fs::read_to_string(path).map_err(CliError::Io)?;
    FmSchema::parse(&source).map(Some).map_err(CliError::Aimd)
}

fn read_file(path: &Path) -> Result<String, CliError> {
    fs::read_to_string(path).map_err(CliError::Io)
}

fn read_payload(file: Option<&Path>, content: Option<&str>) -> Result<String, CliError> {
    if file.is_some() && content.is_some() {
        return Err(CliError::Aimd(simple_error(
            "conflicting_content_inputs",
            "Provide exactly one of --file, --file -, piped stdin, or --content.",
        )));
    }
    if let Some(content) = content {
        return Ok(content.to_string());
    }
    if let Some(path) = file {
        if path == Path::new("-") {
            return read_stdin();
        }
        return fs::read_to_string(path).map_err(CliError::Io);
    }
    if io::stdin().is_terminal() {
        return Err(CliError::Aimd(simple_error(
            "missing_content",
            "Provide --file <content-file>, --file -, piped stdin, or --content <markdown>.",
        )));
    }
    read_stdin()
}

fn read_stdin() -> Result<String, CliError> {
    let mut payload = String::new();
    io::stdin()
        .read_to_string(&mut payload)
        .map_err(CliError::Io)?;
    Ok(payload)
}

fn write_result(
    path: &Path,
    before: &str,
    after: &str,
    dry_run: bool,
    stdout: bool,
    backup: bool,
) -> Result<(), CliError> {
    if dry_run {
        print_diff(path, before, after);
        return Ok(());
    }
    if stdout {
        print!("{after}");
        return Ok(());
    }
    if backup {
        write_backup(path, before)?;
    }
    fs::write(path, after).map_err(CliError::Io)
}

fn write_backup(path: &Path, content: &str) -> Result<(), CliError> {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("document.md");
    let backup_name = format!("{file_name}.{timestamp}.bak");
    let backup_path = path.with_file_name(backup_name);
    fs::write(backup_path, content).map_err(CliError::Io)
}

fn print_diff(path: &Path, before: &str, after: &str) {
    let diff = TextDiff::from_lines(before, after);
    println!("--- a/{}", path.display());
    println!("+++ b/{}", path.display());
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        print!("{sign}{change}");
    }
}

fn print_human_outline(sections: &[Section]) {
    let color = io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    for section in sections {
        print_section(section, color);
    }
}

fn print_section(section: &Section, color: bool) {
    let indent = "  ".repeat(section.path.len().saturating_sub(1));
    let hashes = "#".repeat(section.level as usize);
    if color {
        println!(
            "{}\u{1b}[36m{}\u{1b}[0m {} \u{1b}[2m:{}\u{1b}[0m",
            indent, hashes, section.heading, section.line_start
        );
    } else {
        println!(
            "{}{} {}:{}",
            indent, hashes, section.heading, section.line_start
        );
    }
    for child in &section.children {
        print_section(child, color);
    }
}

fn print_warnings(warnings: &[Diagnostic]) {
    for warning in warnings {
        if let Some(line) = warning.line {
            eprintln!("{}:{}: {}", warning.code, line, warning.message);
        } else {
            eprintln!("{}: {}", warning.code, warning.message);
        }
    }
}

fn print_fm_properties(properties: &[aimd_core::FmProperty]) {
    for property in properties {
        let path = property.path.join(".");
        match &property.value {
            serde_json::Value::String(value) => println!("{path}: {value}"),
            value => println!("{path}: {value}"),
        }
    }
}

fn print_json(value: &impl serde::Serialize) -> Result<(), CliError> {
    serde_json::to_writer_pretty(io::stdout(), value).map_err(CliError::Json)?;
    println!();
    Ok(())
}

fn print_error_json(err: &CliError) -> Result<(), serde_json::Error> {
    match err {
        CliError::Aimd(err) => serde_json::to_writer_pretty(io::stderr(), err)?,
        CliError::Io(err) => serde_json::to_writer_pretty(
            io::stderr(),
            &json!({
                "error": "io_error",
                "hint": err.to_string(),
            }),
        )?,
        CliError::Json(err) => serde_json::to_writer_pretty(
            io::stderr(),
            &json!({
                "error": "io_error",
                "hint": err.to_string(),
            }),
        )?,
    }
    eprintln!();
    Ok(())
}

fn simple_error(code: &str, hint: &str) -> AimdError {
    AimdError {
        error: code.to_string(),
        selector: None,
        line: None,
        hint: Some(hint.to_string()),
        matches: Vec::new(),
    }
}

impl From<CompletionShell> for Shell {
    fn from(value: CompletionShell) -> Self {
        match value {
            CompletionShell::Bash => Shell::Bash,
            CompletionShell::Elvish => Shell::Elvish,
            CompletionShell::Fish => Shell::Fish,
            CompletionShell::PowerShell => Shell::PowerShell,
            CompletionShell::Zsh => Shell::Zsh,
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    Aimd(#[from] AimdError),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

#[allow(dead_code)]
fn _frontmatter_used_for_rustdoc(_: Frontmatter) {}
