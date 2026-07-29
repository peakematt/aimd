use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;
use yaml_rust2::{Yaml, YamlLoader};

const AIDYN_CLAIMS: &str = "---\nclaims:\n  - text: First claim\n    evidence: []\n  - text: Second claim\n    evidence: []\n---\nBody\n";

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
    let rest = markdown.strip_prefix("---\n").unwrap();
    rest.split_once("\n---").unwrap().0
}

fn yaml_doc(markdown: &str) -> Yaml {
    let docs = YamlLoader::load_from_str(frontmatter_body(markdown)).unwrap();
    docs.into_iter().next().unwrap()
}

fn assert_valid_yaml_frontmatter(markdown: &str) {
    YamlLoader::load_from_str(frontmatter_body(markdown)).unwrap();
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
