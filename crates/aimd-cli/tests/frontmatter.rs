use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;
use yaml_rust2::{Yaml, YamlLoader};

const AIDYN_CLAIMS: &str = "---\nclaims:\n  - text: First claim\n    evidence: []\n  - text: Second claim\n    evidence: []\n---\nBody\n";
const COMMENTS_AND_BLANKS: &str =
    include_str!("fixtures/frontmatter/valid/comments-blank-lines.md");
const HARDENING_SCHEMA: &str = include_str!("fixtures/frontmatter/schemas/hardening-schema.yaml");
const COMMENTS_SET_STATUS: &str =
    include_str!("fixtures/frontmatter/golden/comments-set-status.md");

fn aimd() -> Command {
    Command::cargo_bin("aimd").unwrap()
}

fn write_temp(name: &str, content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempdir().unwrap();
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    (dir, path)
}

fn frontmatter_body(markdown: &str) -> &str {
    let rest = markdown
        .strip_prefix("---\r\n")
        .or_else(|| markdown.strip_prefix("---\n"))
        .unwrap();
    rest.split_once("\r\n---")
        .or_else(|| rest.split_once("\n---"))
        .unwrap()
        .0
}

fn yaml_doc(markdown: &str) -> Yaml {
    let docs = YamlLoader::load_from_str(frontmatter_body(markdown)).unwrap();
    docs.into_iter().next().unwrap()
}

fn assert_valid_yaml_frontmatter(markdown: &str) {
    YamlLoader::load_from_str(frontmatter_body(markdown)).unwrap();
}

fn assert_body_preserved(input: &str, output: &str) {
    let input_body = input
        .split_once("\n---\n")
        .or_else(|| input.split_once("\r\n---\r\n"))
        .unwrap()
        .1;
    assert!(
        output.ends_with(input_body) || output.ends_with(&format!("{input_body}\n")),
        "Markdown body changed unexpectedly.\ninput body:\n{input_body}\noutput:\n{output}"
    );
}

fn assert_yaml_kind(doc: &Yaml, key: &str, expected: &str) {
    let actual = match &doc[key] {
        Yaml::Array(_) => "list",
        Yaml::Hash(_) => "map",
        Yaml::Boolean(_) => "bool",
        Yaml::Integer(_) => "int",
        Yaml::Real(_) => "float",
        Yaml::String(value) if value.is_empty() => "blank",
        Yaml::String(_) => "string",
        Yaml::Null | Yaml::BadValue => "null",
        _ => "other",
    };
    assert_eq!(actual, expected, "unexpected YAML kind for {key}");
}

fn assert_no_invalid_inline_empty_list_children(markdown: &str) {
    for line in frontmatter_body(markdown).lines() {
        assert!(
            !line.contains("[]") || !line.ends_with("[]  - text:"),
            "inline empty list swallowed a sibling item: {line}"
        );
    }
    assert!(!frontmatter_body(markdown).contains("evidence: []\n    - "));
}

fn assert_claims_shape(markdown: &str) {
    let doc = yaml_doc(markdown);
    let claims = doc["claims"].as_vec().unwrap();
    assert_eq!(claims.len(), 2);
    assert_eq!(claims[0]["text"].as_str().unwrap(), "First claim");
    assert_eq!(claims[1]["text"].as_str().unwrap(), "Second claim");
    assert!(claims[0]["evidence"].as_vec().unwrap().is_empty());
    assert!(claims[1]["evidence"].as_vec().unwrap().is_empty());
}

#[test]
fn fm_check_accepts_aidyn_nested_empty_list_claims_fixture() {
    let (_dir, path) = write_temp("claims.md", AIDYN_CLAIMS);

    aimd()
        .args(["fm", "check", path.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("invalid_yaml_frontmatter").not());

    assert_claims_shape(&fs::read_to_string(path).unwrap());
}

#[test]
fn fm_get_does_not_flatten_sequence_of_maps() {
    let (_dir, path) = write_temp("claims.md", AIDYN_CLAIMS);

    aimd()
        .args(["fm", "get", path.to_str().unwrap(), "claims", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"kind\": \"list\""))
        .stdout(predicate::str::contains("\"evidence\": []"))
        .stdout(predicate::str::contains("\"Second claim\""));
}

#[test]
fn fm_unrelated_set_preserves_sequence_of_maps_with_inline_empty_lists() {
    let (_dir, path) = write_temp("claims.md", AIDYN_CLAIMS);

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "status",
            "--str",
            "reviewed",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert_claims_shape(&updated);
    assert_no_invalid_inline_empty_list_children(&updated);
    assert!(updated.contains("status: reviewed"));
    assert!(updated.ends_with("Body\n"));
}

#[test]
fn fm_append_to_inline_empty_list_rewrites_as_valid_block_list() {
    let input = "---\nevidence: []\n---\nBody\n";
    let (_dir, path) = write_temp("inline-empty.md", input);

    aimd()
        .args([
            "fm",
            "append-list-item",
            path.to_str().unwrap(),
            "evidence",
            "source note",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert_eq!(
        yaml_doc(&updated)["evidence"].as_vec().unwrap()[0]
            .as_str()
            .unwrap(),
        "source note"
    );
    assert!(!frontmatter_body(&updated).contains("evidence: []\n  - "));
}

#[test]
fn fm_remove_final_list_item_leaves_empty_sequence() {
    let input = "---\nevidence:\n  - first\n---\nBody\n";
    let (_dir, path) = write_temp("list.md", input);

    aimd()
        .args([
            "fm",
            "remove-list-item",
            path.to_str().unwrap(),
            "evidence",
            "first",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert!(yaml_doc(&updated)["evidence"].as_vec().unwrap().is_empty());
    assert!(frontmatter_body(&updated).contains("evidence: []"));
}

#[test]
fn fm_string_quoting_preserves_indicator_like_values_as_strings() {
    let values = [
        "@aidyn",
        "#tag",
        ":value",
        "!bang",
        "&anchorish",
        "*aliasish",
        "{brace}",
        "[bracket]",
        "%directiveish",
        "true",
        "false",
        "null",
        "2026-07-28",
        "value: with colon",
        "value # with hash",
        " leading",
        "trailing ",
    ];

    for value in values {
        let (_dir, path) = write_temp("strings.md", "---\nexisting: true\n---\nBody\n");
        aimd()
            .args(["fm", "set", path.to_str().unwrap(), "field", "--str", value])
            .assert()
            .success();

        let updated = fs::read_to_string(path).unwrap();
        assert_valid_yaml_frontmatter(&updated);
        assert_eq!(yaml_doc(&updated)["field"].as_str().unwrap(), value);
    }
}

#[test]
fn fm_mutation_rejects_invalid_existing_yaml_without_writing() {
    let input = "---\nhandle: @aidyn\n---\nBody\n";
    let (_dir, path) = write_temp("invalid.md", input);

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "status",
            "--str",
            "reviewed",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid_yaml_frontmatter"));

    assert_eq!(fs::read_to_string(path).unwrap(), input);
}

#[test]
fn fm_check_reports_valid_but_unsupported_yaml_constructs() {
    let unsupported = [
        ("anchor.md", "---\nbase: &base value\n---\nBody\n"),
        (
            "alias.md",
            "---\nbase: &base value\ncopy: *base\n---\nBody\n",
        ),
        (
            "merge.md",
            "---\nbase: &base\n  status: planned\nmerged:\n  <<: *base\n---\nBody\n",
        ),
        ("tag.md", "---\ncreated: !custom value\n---\nBody\n"),
        ("complex-key.md", "---\n? [one, two]\n: value\n---\nBody\n"),
        (
            "literal.md",
            "---\ndescription: |\n  multiline\n---\nBody\n",
        ),
        ("documents.md", "---\ntitle: One\n...\n---\nBody\n"),
    ];

    for (name, input) in unsupported {
        let (_dir, path) = write_temp(name, input);
        aimd()
            .args(["fm", "check", path.to_str().unwrap(), "--json"])
            .assert()
            .success()
            .stdout(predicate::str::contains("unsupported_yaml_construct"));
    }
}

#[test]
fn fm_mutation_rejects_unsupported_yaml_constructs_without_writing() {
    let input = "---\ndescription: |\n  multiline\n---\nBody\n";
    let (_dir, path) = write_temp("literal.md", input);

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "status",
            "--str",
            "reviewed",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported_yaml_construct"));

    assert_eq!(fs::read_to_string(path).unwrap(), input);
}

#[test]
fn fm_mutation_rejects_duplicate_keys_without_writing() {
    let input = "---\ntags: [one]\ntags: [two]\n---\nBody\n";
    let (_dir, path) = write_temp("duplicate.md", input);

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "status",
            "--str",
            "reviewed",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate_frontmatter_key"));

    assert_eq!(fs::read_to_string(path).unwrap(), input);
}

#[test]
fn fm_nested_sequence_mutation_path_fails_without_writing() {
    let (_dir, path) = write_temp("claims.md", AIDYN_CLAIMS);

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "claims.0.text",
            "--str",
            "Changed",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported_frontmatter_path"));

    assert_eq!(fs::read_to_string(path).unwrap(), AIDYN_CLAIMS);
}

#[test]
fn fm_dry_run_stdout_and_write_produce_same_frontmatter_shape() {
    let input = "---\ntags: []\n---\nBody\n";
    let (_dir, dry_path) = write_temp("dry.md", input);
    let (_dir2, write_path) = write_temp("write.md", input);

    let dry = aimd()
        .args([
            "fm",
            "append-list-item",
            dry_path.to_str().unwrap(),
            "tags",
            "agent",
            "--stdout",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    aimd()
        .args([
            "fm",
            "append-list-item",
            write_path.to_str().unwrap(),
            "tags",
            "agent",
        ])
        .assert()
        .success();

    let stdout_output = String::from_utf8(dry).unwrap();
    assert_eq!(stdout_output, fs::read_to_string(write_path).unwrap());
    assert_valid_yaml_frontmatter(&stdout_output);
}

#[test]
fn fm_has_and_remove_cover_supported_property_paths() {
    let input = "---\ntitle: Sample\nmetadata:\n  owner: Sage\n  priority: 2\ntags:\n  - alpha\n---\nBody\n";
    let (_dir, path) = write_temp("paths.md", input);

    aimd()
        .args([
            "fm",
            "has",
            path.to_str().unwrap(),
            "metadata.owner",
            "--json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"present\": true"));

    aimd()
        .args([
            "fm",
            "has",
            path.to_str().unwrap(),
            "metadata.missing",
            "--json",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("frontmatter_property_not_found"));

    aimd()
        .args(["fm", "remove", path.to_str().unwrap(), "metadata.priority"])
        .assert()
        .success();

    let updated = fs::read_to_string(&path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert!(updated.contains("metadata:\n  owner: Sage\n"));
    assert!(!updated.contains("priority"));

    aimd()
        .args(["fm", "remove", path.to_str().unwrap(), "tags"])
        .assert()
        .success();

    let updated = fs::read_to_string(path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert!(!updated.contains("tags:"));
    assert_body_preserved(input, &updated);
}

#[test]
fn fm_has_distinguishes_unsupported_nested_sequence_paths() {
    let (_dir, path) = write_temp("claims.md", AIDYN_CLAIMS);

    aimd()
        .args([
            "fm",
            "has",
            path.to_str().unwrap(),
            "claims.0.text",
            "--json",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported_frontmatter_path"));
}

#[test]
fn fm_set_list_nested_map_path_writes_yaml_sequence_shape() {
    let input = "---\nmetadata:\n  owner: Sage\n---\nBody\n";
    let (_dir, empty_path) = write_temp("empty-nested-list.md", input);

    aimd()
        .args([
            "fm",
            "set-list",
            empty_path.to_str().unwrap(),
            "metadata.evidence",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(empty_path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert!(
        yaml_doc(&updated)["metadata"]["evidence"]
            .as_vec()
            .unwrap()
            .is_empty()
    );
    assert!(updated.contains("  evidence: []"));

    let (_dir2, values_path) = write_temp("nested-list.md", input);
    aimd()
        .args([
            "fm",
            "set-list",
            values_path.to_str().unwrap(),
            "metadata.evidence",
            "one",
            "true",
            "value: with colon",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(values_path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    let doc = yaml_doc(&updated);
    let evidence = doc["metadata"]["evidence"]
        .as_vec()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(evidence, ["one", "true", "value: with colon"]);
    assert!(updated.contains("  evidence: [one, \"true\", \"value: with colon\"]"));
}

#[test]
fn fm_nested_map_set_preserves_yaml_map_shape() {
    let input = "---\nmetadata:\n  owner: Sage\n---\nBody\n";
    let (_dir, inline_path) = write_temp("nested-map-inline.md", input);

    aimd()
        .args([
            "fm",
            "set",
            inline_path.to_str().unwrap(),
            "metadata.details",
            "--map",
            "{ count: 2 }",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(inline_path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert_eq!(
        yaml_doc(&updated)["metadata"]["details"]["count"]
            .as_i64()
            .unwrap(),
        2
    );
    assert!(updated.contains("  details: { count: 2 }"));

    let (dir, file_path) = write_temp("nested-map-file.md", input);
    let payload = dir.path().join("details.yaml");
    fs::write(&payload, "active: true\ncount: 2\n").unwrap();
    aimd()
        .args([
            "fm",
            "set",
            file_path.to_str().unwrap(),
            "metadata.details",
            "--map-file",
            payload.to_str().unwrap(),
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(file_path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert!(
        yaml_doc(&updated)["metadata"]["details"]["active"]
            .as_bool()
            .unwrap()
    );
}

#[test]
fn fm_nested_map_list_append_and_remove_operate_on_child_lists() {
    let input = "---\nmetadata:\n  evidence: []\n---\nBody\n";
    let (_dir, path) = write_temp("nested-list-ops.md", input);

    aimd()
        .args([
            "fm",
            "append-list-item",
            path.to_str().unwrap(),
            "metadata.evidence",
            "source",
            "true",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(&path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert_eq!(
        yaml_doc(&updated)["metadata"]["evidence"]
            .as_vec()
            .unwrap()
            .len(),
        2
    );
    assert!(updated.contains("  evidence: [source, \"true\"]"));

    aimd()
        .args([
            "fm",
            "remove-list-item",
            path.to_str().unwrap(),
            "metadata.evidence",
            "source",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    let doc = yaml_doc(&updated);
    let evidence = doc["metadata"]["evidence"]
        .as_vec()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(evidence, ["true"]);
}

#[test]
fn fm_schema_normalize_inserts_required_shapes_and_nested_fields() {
    let (dir, path) = write_temp("missing.md", "---\ntitle: Sample\n---\nBody\n");
    let schema = dir.path().join("schema.yaml");
    fs::write(&schema, HARDENING_SCHEMA).unwrap();

    aimd()
        .args([
            "fm",
            "normalize",
            path.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(&path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    let doc = yaml_doc(&updated);
    assert_yaml_kind(&doc, "play_status", "list");
    assert_yaml_kind(&doc, "bandwidth_categories", "map");
    assert_eq!(
        doc["bandwidth_categories"]["continuity"].as_i64().unwrap(),
        0
    );
    assert!(!doc["ib_session_ready"].as_bool().unwrap());

    let (_dir2, nested_path) = write_temp(
        "nested-missing.md",
        "---\nbandwidth_categories:\n  route: 3\n---\nBody\n",
    );
    aimd()
        .args([
            "fm",
            "normalize",
            nested_path.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(nested_path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert!(updated.contains("bandwidth_categories:\n  route: 3\n  continuity: 0\n"));
}

#[test]
fn fm_schema_checks_blank_and_null_policy() {
    let schema = "name: blank-null\nversion: 1\nfields:\n  blank_field:\n    type: blank\n    required: true\n  null_field:\n    type: null\n    required: true\n";
    let (dir, path) = write_temp(
        "blank-null.md",
        "---\nblank_field: null\nnull_field: \n---\nBody\n",
    );
    let schema_path = dir.path().join("schema.yaml");
    fs::write(&schema_path, schema).unwrap();

    aimd()
        .args([
            "fm",
            "check",
            path.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("frontmatter_unexpected_null"))
        .stdout(predicate::str::contains("frontmatter_blank_null_mismatch"));

    let (_dir2, missing_path) =
        write_temp("missing-blank-null.md", "---\ntitle: Sample\n---\nBody\n");
    aimd()
        .args([
            "fm",
            "normalize",
            missing_path.to_str().unwrap(),
            "--schema",
            schema_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(missing_path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert!(updated.contains("blank_field: \n"));
    assert!(updated.contains("null_field: null\n"));
}

#[test]
fn fm_nested_duplicate_keys_are_reported_and_block_mutation() {
    let input = "---\nmetadata:\n  owner: Sage\n  owner: Aidyn\n---\nBody\n";
    let (_dir, path) = write_temp("nested-duplicate.md", input);

    aimd()
        .args(["fm", "check", path.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("duplicate_frontmatter_key"))
        .stdout(predicate::str::contains("\"metadata\""))
        .stdout(predicate::str::contains("\"owner\""));

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "status",
            "--str",
            "reviewed",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate_frontmatter_key"));

    assert_eq!(fs::read_to_string(path).unwrap(), input);
}

#[test]
fn fm_schema_checks_wrong_types_and_refuses_required_removal() {
    let (dir, path) = write_temp(
        "wrong-types.md",
        "---\nplay_status: planned\nbandwidth_categories:\n  continuity:\n    - nope\nib_session_ready: false\n---\nBody\n",
    );
    let schema = dir.path().join("schema.yaml");
    fs::write(&schema, HARDENING_SCHEMA).unwrap();

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
        .stdout(predicate::str::contains("frontmatter_schema_type_mismatch"));

    aimd()
        .args([
            "fm",
            "remove",
            path.to_str().unwrap(),
            "play_status",
            "--schema",
            schema.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("frontmatter_required_key_missing"));

    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "---\nplay_status: planned\nbandwidth_categories:\n  continuity:\n    - nope\nib_session_ready: false\n---\nBody\n"
    );
}

#[test]
fn fm_preserves_comments_blank_lines_order_body_crlf_and_final_newline() {
    let (_dir, path) = write_temp("comments.md", COMMENTS_AND_BLANKS);

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "status",
            "--str",
            "reviewed",
        ])
        .assert()
        .success();

    let updated = fs::read_to_string(&path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert_eq!(updated, COMMENTS_SET_STATUS);
    assert_body_preserved(COMMENTS_AND_BLANKS, &updated);

    let crlf = "---\r\ntitle: CRLF\r\n---\r\nBody\r\n";
    let (_dir2, crlf_path) = write_temp("crlf.md", crlf);
    aimd()
        .args(["fm", "set-list", crlf_path.to_str().unwrap(), "tags", "one"])
        .assert()
        .success();
    let updated = fs::read(crlf_path).unwrap();
    assert!(updated.windows(2).any(|window| window == b"\r\n"));
    assert!(!String::from_utf8(updated).unwrap().contains("tags:\n"));

    let no_final_newline = "---\ntitle: No final newline\n---\nBody";
    let (_dir3, no_final_path) = write_temp("no-final.md", no_final_newline);
    aimd()
        .args([
            "fm",
            "set",
            no_final_path.to_str().unwrap(),
            "status",
            "--str",
            "done",
        ])
        .assert()
        .success();
    let updated = fs::read_to_string(no_final_path).unwrap();
    assert!(updated.ends_with('\n'));
    assert!(updated.contains("Body"));
}

#[test]
fn fm_invalid_yaml_fixture_matrix_reports_stable_diagnostics() {
    let invalid = [
        (
            "bad-indent.md",
            include_str!("fixtures/frontmatter/invalid/bad-indentation.md"),
            "invalid_yaml_frontmatter",
        ),
        (
            "inline-empty-child.md",
            "---\nevidence: []\n  - text: Sibling accidentally indented as child\n---\nBody\n",
            "invalid_yaml_frontmatter",
        ),
        (
            "quote.md",
            include_str!("fixtures/frontmatter/invalid/unterminated-quote.md"),
            "invalid_yaml_frontmatter",
        ),
        (
            "tab.md",
            include_str!("fixtures/frontmatter/invalid/tab-indentation.md"),
            "invalid_yaml_frontmatter",
        ),
        (
            "multi-doc.md",
            "---\ntitle: One\n...\n---\nBody\n",
            "unsupported_yaml_construct",
        ),
    ];

    for (name, input, diagnostic) in invalid {
        let (_dir, path) = write_temp(name, input);
        aimd()
            .args(["fm", "check", path.to_str().unwrap(), "--json"])
            .assert()
            .success()
            .stdout(predicate::str::contains(diagnostic));
    }

    let (_dir, path) = write_temp("missing-close.md", "---\ntitle: Missing close\nBody\n");
    aimd()
        .args(["fm", "check", path.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("malformed_frontmatter"));
}

#[test]
fn fm_unsupported_construct_fixture_matrix_refuses_mutation() {
    let input = include_str!("fixtures/frontmatter/unsupported/anchor-alias-merge.md");
    let (_dir, path) = write_temp("unsupported.md", input);

    aimd()
        .args(["fm", "check", path.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("unsupported_yaml_construct"));

    aimd()
        .args([
            "fm",
            "set",
            path.to_str().unwrap(),
            "status",
            "--str",
            "reviewed",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported_yaml_construct"));

    assert_eq!(fs::read_to_string(path).unwrap(), input);
}

#[test]
fn fm_list_append_and_remove_duplicate_semantics_are_explicit() {
    let input = "---\ntags:\n  - one\n  - one\n  - two\n---\nBody\n";
    let (_dir, path) = write_temp("duplicates.md", input);

    aimd()
        .args([
            "fm",
            "append-list-item",
            path.to_str().unwrap(),
            "tags",
            "two",
        ])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(&path)
            .unwrap()
            .matches("  - two")
            .count(),
        1
    );

    aimd()
        .args([
            "fm",
            "append-list-item",
            path.to_str().unwrap(),
            "tags",
            "two",
            "--allow-duplicate",
        ])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(&path)
            .unwrap()
            .matches("  - two")
            .count(),
        2
    );

    aimd()
        .args([
            "fm",
            "remove-list-item",
            path.to_str().unwrap(),
            "tags",
            "one",
        ])
        .assert()
        .success();
    let updated = fs::read_to_string(path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert!(!updated.contains("  - one"));
    assert_eq!(updated.matches("  - two").count(), 2);
}

#[test]
fn fm_rejects_unsupported_nested_sequence_mutations_without_writing() {
    let (_dir, append_path) = write_temp("append-claims.md", AIDYN_CLAIMS);
    aimd()
        .args([
            "fm",
            "append-list-item",
            append_path.to_str().unwrap(),
            "claims.0.evidence",
            "source",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported_frontmatter_path"));
    assert_eq!(fs::read_to_string(append_path).unwrap(), AIDYN_CLAIMS);

    let (_dir2, remove_path) = write_temp("remove-claims.md", AIDYN_CLAIMS);
    aimd()
        .args([
            "fm",
            "remove",
            remove_path.to_str().unwrap(),
            "claims.0.text",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported_frontmatter_path"));
    assert_eq!(fs::read_to_string(remove_path).unwrap(), AIDYN_CLAIMS);
}

#[test]
fn fm_map_file_stdin_and_json_payloads_preserve_yaml_shape() {
    let (_dir, stdin_path) = write_temp("stdin-map.md", "---\nexisting: true\n---\nBody\n");
    aimd()
        .args([
            "fm",
            "set",
            stdin_path.to_str().unwrap(),
            "metadata",
            "--map-file",
            "-",
        ])
        .write_stdin("owner: Sage\ncount: 2\n")
        .assert()
        .success();
    let updated = fs::read_to_string(stdin_path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert_yaml_kind(&yaml_doc(&updated), "metadata", "map");

    let (_dir2, json_path) = write_temp("json-map.md", "---\nexisting: true\n---\nBody\n");
    aimd()
        .args([
            "fm",
            "set",
            json_path.to_str().unwrap(),
            "metadata",
            "--map",
            "{\"owner\":\"Sage\",\"active\":true}",
        ])
        .assert()
        .success();
    let updated = fs::read_to_string(json_path).unwrap();
    assert_valid_yaml_frontmatter(&updated);
    assert!(yaml_doc(&updated)["metadata"]["active"].as_bool().unwrap());
}

#[test]
fn fm_dry_run_stdout_and_write_match_for_set_list_and_remove() {
    let input = "---\ntags:\n  - one\n  - two\n---\nBody\n";
    let (_dir, stdout_path) = write_temp("stdout.md", input);
    let (_dir2, write_path) = write_temp("write.md", input);

    let stdout = aimd()
        .args([
            "fm",
            "remove-list-item",
            stdout_path.to_str().unwrap(),
            "tags",
            "one",
            "--stdout",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    aimd()
        .args([
            "fm",
            "remove-list-item",
            write_path.to_str().unwrap(),
            "tags",
            "one",
        ])
        .assert()
        .success();

    let stdout_output = String::from_utf8(stdout).unwrap();
    assert_eq!(stdout_output, fs::read_to_string(write_path).unwrap());
    assert_valid_yaml_frontmatter(&stdout_output);
}
