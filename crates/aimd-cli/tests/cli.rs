use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

const BASIC: &str = include_str!("../../../fixtures/input/basic-nested.md");
const FRONTMATTER: &str = include_str!("../../../fixtures/input/frontmatter.md");
const DUPLICATE_PATHS: &str = include_str!("../../../fixtures/input/duplicate-paths.md");
const DUPLICATE_CHILD_HEADINGS: &str =
    include_str!("../../../fixtures/input/duplicate-child-headings.md");
const H6_BOUNDARY: &str = include_str!("../../../fixtures/input/h6-boundary.md");
const REPLACE_SHALLOW: &str = include_str!("../../../fixtures/expected/replace-shallow.md");
const APPEND_CHILD_AFTER: &str = include_str!("../../../fixtures/expected/append-child-after.md");
const FM_BLOCK_MAP: &str = "---\nplay_status:\n  - planned\nbandwidth_categories:\n  continuity: 2\n  route: 3\n  session: 1\n  time: 2\nib_session_ready: false\n---\n\n# Game\nBody.\n";
const FM_FLOW_MAP: &str =
    "---\nbandwidth_categories: { continuity: 2, route: 3, session: 1, time: 2 }\n---\n\n# Game\n";
const FM_SCHEMA: &str = "name: game-note\nversion: 1\nfields:\n  play_status:\n    type: list\n    required: true\n    order: 10\n  ib_session_ready:\n    type: bool\n    required: true\n    order: 20\n  bandwidth_categories:\n    type: map\n    style: block\n    required: true\n    order: 30\n    fields:\n      continuity:\n        type: int\n";

fn aimd() -> Command {
    Command::cargo_bin("aimd").unwrap()
}

fn write_temp(name: &str, content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().unwrap();
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    (dir, path)
}

fn lf(content: &str) -> String {
    content.replace("\r\n", "\n")
}

#[test]
fn outline_json_reports_frontmatter_and_filters_max_level() {
    let (_dir, path) = write_temp("frontmatter.md", FRONTMATTER);

    aimd()
        .args([
            "outline",
            path.to_str().unwrap(),
            "--json",
            "--max-level",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"present\": true"))
        .stdout(predicate::str::contains(
            "\"heading\": \"Project Handbook\"",
        ))
        .stdout(predicate::str::contains("\"Release Plan\"").not());
}

#[test]
fn outline_root_scopes_to_exact_path() {
    let (_dir, path) = write_temp("basic.md", BASIC);

    aimd()
        .args([
            "outline",
            path.to_str().unwrap(),
            "--json",
            "--root",
            "Project Handbook > Release Plan",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"heading\": \"Release Plan\""))
        .stdout(predicate::str::contains("\"heading\": \"Operations Runbook\"").not());
}

#[test]
fn get_shallow_excludes_child_sections() {
    let (_dir, path) = write_temp("basic.md", BASIC);

    aimd()
        .args([
            "get",
            path.to_str().unwrap(),
            "Project Handbook",
            "--shallow",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("# Project Handbook"))
        .stdout(predicate::str::contains("Intro paragraph."))
        .stdout(predicate::str::contains("## Release Plan").not());
}

#[test]
fn get_line_rejects_frontmatter() {
    let (_dir, path) = write_temp("frontmatter.md", FRONTMATTER);

    aimd()
        .args(["get", path.to_str().unwrap(), "--line", "2", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("line_in_frontmatter"));
}

#[test]
fn full_replace_requires_matching_heading() {
    let (_dir, path) = write_temp("basic.md", BASIC);

    aimd()
        .args([
            "replace",
            path.to_str().unwrap(),
            "Project Handbook > Release Plan",
            "--content",
            "## Different\nNope\n",
            "--stdout",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("replacement_heading_mismatch"));
}

#[test]
fn replace_shallow_preserves_children_and_unrelated_sections() {
    let (_dir, path) = write_temp("basic.md", BASIC);

    aimd()
        .args([
            "replace",
            path.to_str().unwrap(),
            "Project Handbook > Release Plan",
            "--content",
            "Updated body.\n",
            "--shallow",
        ])
        .assert()
        .success();

    assert_eq!(lf(&fs::read_to_string(path).unwrap()), lf(REPLACE_SHALLOW));
}

#[test]
fn shallow_replace_preserves_frontmatter_bytes() {
    let (_dir, path) = write_temp("frontmatter.md", FRONTMATTER);

    aimd()
        .args([
            "replace",
            path.to_str().unwrap(),
            "Project Handbook > Release Plan",
            "--content",
            "Updated release body.",
            "--shallow",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(path).unwrap();
    assert!(lf(&updated).starts_with("---\ntitle: Project Handbook\ntags: [docs]\n---\n"));
    assert!(updated.contains("Updated release body."));
}

#[test]
fn append_reads_piped_stdin() {
    let (_dir, path) = write_temp("basic.md", BASIC);

    aimd()
        .args([
            "append",
            path.to_str().unwrap(),
            "Project Handbook > Release Plan",
            "--stdout",
        ])
        .write_stdin("Piped note.")
        .assert()
        .success()
        .stdout(predicate::str::contains("Piped note."));
}

#[test]
fn backup_writes_timestamped_original_sibling() {
    let (dir, path) = write_temp("basic.md", BASIC);

    aimd()
        .args([
            "replace",
            path.to_str().unwrap(),
            "Project Handbook > Release Plan",
            "--content",
            "Updated body.",
            "--shallow",
            "--backup",
        ])
        .assert()
        .success();

    let backups = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".bak"))
        .collect::<Vec<_>>();
    assert_eq!(backups.len(), 1);
    assert_eq!(fs::read_to_string(backups[0].path()).unwrap(), BASIC);
}

#[test]
fn crlf_line_endings_are_preserved_for_rewrites() {
    let (_dir, path) = write_temp(
        "crlf.md",
        "# Project\r\nIntro.\r\n\r\n## Release Plan\r\nBody.\r\n",
    );

    aimd()
        .args([
            "replace",
            path.to_str().unwrap(),
            "Project > Release Plan",
            "--content",
            "Updated body.",
            "--shallow",
        ])
        .assert()
        .success();

    let updated = fs::read(path).unwrap();
    assert!(updated.windows(2).any(|window| window == b"\r\n"));
    assert!(
        !String::from_utf8(updated)
            .unwrap()
            .contains("Updated body.\n")
    );
}

#[test]
fn malformed_frontmatter_is_reported_by_check() {
    let (_dir, path) = write_temp("malformed.md", "---\ntitle: Missing close\n# Not Section\n");

    aimd()
        .args(["check", path.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("malformed_frontmatter"));
}

#[test]
fn append_child_can_insert_after_index() {
    let (_dir, path) = write_temp("basic.md", BASIC);

    aimd()
        .args([
            "append-child",
            path.to_str().unwrap(),
            "Project Handbook > Release Plan",
            "--heading",
            "Notes",
            "--content",
            "Child body.",
            "--after-child",
            "0",
        ])
        .assert()
        .success();

    assert_eq!(
        lf(&fs::read_to_string(path).unwrap()),
        lf(APPEND_CHILD_AFTER)
    );
}

#[test]
fn duplicate_exact_path_is_ambiguous() {
    let (_dir, path) = write_temp("dup.md", DUPLICATE_PATHS);

    aimd()
        .args(["get", path.to_str().unwrap(), "Project > Notes", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate_section_path"));
}

#[test]
fn json_mode_reports_structured_section_errors() {
    let (_dir, path) = write_temp("basic.md", BASIC);

    aimd()
        .args(["get", path.to_str().unwrap(), "Missing", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("\"error\": \"section_not_found\""))
        .stderr(predicate::str::contains("\"selector\""))
        .stderr(predicate::str::contains("\"Missing\""))
        .stderr(predicate::str::contains("\"hint\""));
}

#[test]
fn json_mode_reports_structured_io_errors() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("missing.md");

    aimd()
        .args(["outline", path.to_str().unwrap(), "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("\"error\": \"io_error\""))
        .stderr(predicate::str::contains("\"hint\""));
}

#[test]
fn duplicate_child_heading_is_ambiguous_for_human_placement() {
    let (_dir, path) = write_temp("dup-child.md", DUPLICATE_CHILD_HEADINGS);

    aimd()
        .args([
            "append-child",
            path.to_str().unwrap(),
            "Project",
            "--heading",
            "Notes",
            "--content",
            "Body",
            "--after",
            "Alpha",
            "--stdout",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate_child_heading"));
}

#[test]
fn h6_parent_cannot_have_child() {
    let (_dir, path) = write_temp("h6.md", H6_BOUNDARY);

    aimd()
        .args([
            "append-child",
            path.to_str().unwrap(),
            "Leaf",
            "--heading",
            "Too Deep",
            "--content",
            "Body",
            "--stdout",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot_append_child_to_h6"));
}

#[test]
fn conflicting_content_inputs_return_spec_error_code() {
    let (_dir, path) = write_temp("basic.md", BASIC);

    aimd()
        .args([
            "append",
            path.to_str().unwrap(),
            "Project Handbook",
            "--file",
            "-",
            "--content",
            "Body",
            "--stdout",
        ])
        .write_stdin("stdin body")
        .assert()
        .failure()
        .stderr(predicate::str::contains("conflicting_content_inputs"));
}

#[test]
fn conflicting_placement_flags_return_spec_error_code() {
    let (_dir, path) = write_temp("basic.md", BASIC);

    aimd()
        .args([
            "append-child",
            path.to_str().unwrap(),
            "Project Handbook > Release Plan",
            "--heading",
            "Notes",
            "--content",
            "Body",
            "--after-child",
            "0",
            "--before-child",
            "0",
            "--stdout",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("conflicting_placement_flags"));
}

#[test]
fn check_reports_skipped_levels() {
    let (_dir, path) = write_temp(
        "skipped.md",
        include_str!("../../../fixtures/input/skipped-levels.md"),
    );

    aimd()
        .args(["check", path.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("skipped_heading_level"));
}

#[test]
fn completions_generate_shell_script() {
    aimd()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_aimd"));
}

#[test]
fn fm_get_reports_nested_map_properties_as_json() {
    let (_dir, path) = write_temp("game.md", FM_BLOCK_MAP);

    aimd()
        .args([
            "fm",
            "get",
            path.to_str().unwrap(),
            "bandwidth_categories.continuity",
            "--json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"path\": ["))
        .stdout(predicate::str::contains("\"bandwidth_categories\""))
        .stdout(predicate::str::contains("\"continuity\""))
        .stdout(predicate::str::contains("\"value\": 2"));
}

#[test]
fn fm_set_nested_map_entry_preserves_siblings() {
    let (_dir, path) = write_temp("game.md", FM_BLOCK_MAP);

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "bandwidth_categories.continuity",
            "--int",
            "5",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(path).unwrap();
    assert!(updated.contains("  continuity: 5"));
    assert!(updated.contains("  route: 3"));
    assert!(updated.contains("# Game\nBody."));
}

#[test]
fn fm_set_map_file_replaces_entire_map_with_block_style() {
    let (dir, path) = write_temp("game.md", FM_BLOCK_MAP);
    let payload = dir.path().join("map.yaml");
    fs::write(&payload, "continuity: 7\nroute: 4\nsession: 2\n").unwrap();

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "bandwidth_categories",
            "--map-file",
            payload.to_str().unwrap(),
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(path).unwrap();
    assert!(updated.contains("bandwidth_categories:\n  continuity: 7\n  route: 4\n  session: 2\n"));
    assert!(!updated.contains("  time: 2"));
}

#[test]
fn fm_set_map_accepts_inline_yaml_flow_map() {
    let (_dir, path) = write_temp("game.md", FM_BLOCK_MAP);

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "bandwidth_categories",
            "--map",
            "{ continuity: 7, route: 4 }",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(path).unwrap();
    assert!(updated.contains("bandwidth_categories:\n  continuity: 7\n  route: 4\n"));
    assert!(!updated.contains("  time: 2"));
}

#[test]
fn fm_list_item_append_and_remove_preserve_list_shape() {
    let (_dir, path) = write_temp("game.md", FM_BLOCK_MAP);

    aimd()
        .args([
            "fm",
            "append-list-item",
            path.to_str().unwrap(),
            "play_status",
            "planned",
            "active",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(&path).unwrap();
    assert_eq!(updated.matches("  - planned").count(), 1);
    assert!(updated.contains("  - active"));

    aimd()
        .args([
            "fm",
            "remove-list-item",
            path.to_str().unwrap(),
            "play_status",
            "planned",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(path).unwrap();
    assert!(!updated.contains("  - planned"));
    assert!(updated.contains("  - active"));
}

#[test]
fn fm_set_list_without_values_renders_empty_yaml_list() {
    let (_dir, path) = write_temp("plain.md", "# Game\nBody.\n");

    aimd()
        .args([
            "fm",
            "set-list",
            path.to_str().unwrap(),
            "play_status",
            "--create",
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "---\nplay_status: []\n---\n\n# Game\nBody.\n"
    );

    aimd()
        .args(["fm", "get", path.to_str().unwrap(), "play_status", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"kind\": \"list\""))
        .stdout(predicate::str::contains("\"value\": []"));
}

#[test]
fn fm_create_inserts_frontmatter_block() {
    let (_dir, path) = write_temp("plain.md", "# Game\nBody.\n");

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "ib_session_ready",
            "--bool",
            "true",
            "--create",
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "---\nib_session_ready: true\n---\n\n# Game\nBody.\n"
    );
}

#[test]
fn fm_set_quotes_yaml_indicator_strings() {
    let (_dir, path) = write_temp("plain.md", "# Game\nBody.\n");

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "handle",
            "--str",
            "@aidyn",
            "--create",
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "---\nhandle: \"@aidyn\"\n---\n\n# Game\nBody.\n"
    );

    aimd()
        .args(["fm", "check", path.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("invalid_yaml_frontmatter").not());
}

#[test]
fn fm_check_reports_invalid_yaml_frontmatter() {
    let (_dir, path) = write_temp("invalid.md", "---\nhandle: @aidyn\n---\n\n# Game\n");

    aimd()
        .args(["fm", "check", path.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("invalid_yaml_frontmatter"));
}

#[test]
fn fm_schema_check_reports_required_and_type_mismatches() {
    let (dir, path) = write_temp("game.md", "---\nplay_status: planned\n---\n\n# Game\n");
    let schema = dir.path().join("schema.yaml");
    fs::write(&schema, FM_SCHEMA).unwrap();

    aimd()
        .args([
            "fm",
            "check",
            path.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("frontmatter_schema_type_mismatch"))
        .stdout(predicate::str::contains("frontmatter_required_key_missing"));
}

#[test]
fn fm_flow_map_reads_but_nested_mutation_fails_safely() {
    let (_dir, path) = write_temp("flow.md", FM_FLOW_MAP);

    aimd()
        .args([
            "fm",
            "get",
            path.to_str().unwrap(),
            "bandwidth_categories.continuity",
            "--json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"value\": 2"));

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "bandwidth_categories.continuity",
            "--int",
            "4",
            "--stdout",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsafe_frontmatter_rewrite"));
}

#[test]
fn fm_dry_run_prints_diff_and_does_not_mutate() {
    let (_dir, path) = write_temp("game.md", FM_BLOCK_MAP);

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "ib_session_ready",
            "--bool",
            "true",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("-ib_session_ready: false"))
        .stdout(predicate::str::contains("+ib_session_ready: true"));

    assert!(
        fs::read_to_string(path)
            .unwrap()
            .contains("ib_session_ready: false")
    );
}

#[test]
fn fm_normalize_inserts_required_schema_fields() {
    let (dir, path) = write_temp("game.md", "---\nplay_status:\n  - planned\n---\n\n# Game\n");
    let schema = dir.path().join("schema.yaml");
    fs::write(&schema, FM_SCHEMA).unwrap();

    aimd()
        .args([
            "fm",
            "normalize",
            path.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("ib_session_ready: false"))
        .stdout(predicate::str::contains("bandwidth_categories:\n"))
        .stdout(predicate::str::contains("play_status: []").not());

    assert!(
        !fs::read_to_string(path)
            .unwrap()
            .contains("ib_session_ready")
    );
}

#[test]
fn fm_normalize_inserts_empty_yaml_list_for_required_lists() {
    let (dir, path) = write_temp(
        "game.md",
        "---\nib_session_ready: false\nbandwidth_categories:\n  continuity: 1\n---\n\n# Game\n",
    );
    let schema = dir.path().join("schema.yaml");
    fs::write(&schema, FM_SCHEMA).unwrap();

    aimd()
        .args([
            "fm",
            "normalize",
            path.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
            "--stdout",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("play_status: []"));
}
