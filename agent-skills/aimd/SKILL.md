---
name: aimd
description: Safe structural Markdown and frontmatter inspection/editing with the aimd CLI. Use when Codex or another agent needs to outline Markdown, select exact heading paths, read or mutate sections, edit Obsidian-style frontmatter properties, update lists/maps, or validate structural hazards without fuzzy search or semantic Obsidian parsing.
---

# aimd

Use `aimd` for scoped Markdown edits by heading structure and explicit frontmatter property operations. Treat it as structural tooling: it preserves source text where possible, treats frontmatter as metadata for heading operations, and does not perform fuzzy search, semantic Obsidian parsing, summarization, embeddings, or project-wide indexing.

The command contract is still pre-release. Before relying on examples, run `aimd --help` and the relevant subcommand `--help` to confirm the installed binary matches this skill.

## Core Workflow

1. Run `aimd outline <file> --json` before selecting paths in unfamiliar files.
2. Use exact, case-sensitive heading paths from outline output, such as `Project > Release Plan > Checklist`.
3. Run `aimd check <file> --json` before writes when duplicate paths, skipped heading levels, or frontmatter hazards are likely.
4. Read before writing. Prefer `get --shallow` before `replace --shallow` when editing direct body text.
5. Preview important edits with `--dry-run` or `--stdout`; use `--backup` before mutating important files.
6. Stop on ambiguity errors. Do not guess between duplicate section paths or duplicate child headings.
7. Use `aimd fm ...` for document-start YAML frontmatter. Do not edit frontmatter with heading-path commands or ad hoc regexes.

## Command Patterns

Inspect structure:

```bash
aimd outline docs/project.md --json
aimd outline docs/project.md --json --max-level 2
aimd outline docs/project.md --json --root "Project > Release Plan"
```

Read a section:

```bash
aimd get docs/project.md "Project > Release Plan"
aimd get docs/project.md "Project > Release Plan" --shallow
aimd get docs/project.md --line 42 --json
```

Replace a shallow direct body while preserving child sections:

```bash
aimd get docs/project.md "Project > Release Plan" --shallow
aimd replace docs/project.md "Project > Release Plan" --shallow --file replacement-body.md --dry-run
aimd replace docs/project.md "Project > Release Plan" --shallow --file replacement-body.md --backup
```

For `replace --shallow`, provide body-only Markdown. If the replacement starts with a heading, stop and either remove the heading or use full subtree replacement.

Replace a full subtree:

```bash
aimd get docs/project.md "Project > Release Plan"
aimd replace docs/project.md "Project > Release Plan" --file replacement-section.md --dry-run
```

For full replacement, the replacement content must include the selected heading with the same level and text.

Append body content:

```bash
aimd append docs/project.md "Project > Release Plan" --content "Decision: keep the launch checklist separate." --dry-run
```

Append a child section:

```bash
aimd outline docs/project.md --json --root "Project > Release Plan"
aimd append-child docs/project.md "Project > Release Plan" --heading "Risks" --content "- Dependency review is pending." --after-child 1 --dry-run
```

Prefer `--after-child` or `--before-child` indexes from JSON output for agent workflows. Use `--after <child-heading>` or `--before <child-heading>` only when direct child headings are unique.

Validate:

```bash
aimd check docs/project.md --json
```

Read frontmatter properties:

```bash
aimd fm get docs/project.md --json
aimd fm get docs/project.md play_status --json
aimd fm get docs/project.md bandwidth_categories.continuity --json
```

Edit typed frontmatter values:

```bash
aimd fm set docs/project.md ib_session_ready --bool true --dry-run
aimd fm set docs/project.md last_played --date 2026-07-27 --dry-run
aimd fm set docs/project.md bandwidth_categories.continuity --int 3 --dry-run
aimd fm set docs/project.md bandwidth_categories --map-file /tmp/bandwidth.yaml --dry-run
```

Edit frontmatter lists:

```bash
aimd fm set-list docs/project.md play_status planned --dry-run
aimd fm append-list-item docs/project.md ib_challenges "[[Backlog Blitz v3]]" --dry-run
aimd fm remove-list-item docs/project.md ib_challenges "[[Old Challenge]]" --dry-run
```

Use `set-list` to replace the whole list. Use `append-list-item` and `remove-list-item` to add or remove matching list values. Appends avoid duplicates by default unless `--allow-duplicate` is supplied. Removal deletes all matching scalar values from a supported list.

Check and normalize with a schema:

```bash
aimd fm check docs/project.md --schema schemas/game-note.yaml --json
aimd fm normalize docs/project.md --schema schemas/game-note.yaml --dry-run
```

Flow-style YAML maps can be read and checked, but nested flow-map mutation may fail with `unsafe_frontmatter_rewrite`. Replace the whole map with `--map-file` when that is intentional.

`aimd fm` validates generated frontmatter as YAML before writing. It preserves unrelated body bytes, comments, blank lines, ordering, CRLF line endings, and final-newline policy where the supported source-range model can prove the edit. Schema normalization inserts required lists as `[]`, required maps as `{}` or block maps with required child placeholders, and scalar defaults conservatively. It can read and preserve sequence-of-maps shapes, including inline empty lists like `evidence: []`, but it does not support mutation paths inside nested sequences. Avoid editing frontmatter with anchors, aliases, merge keys, custom tags, complex keys, multiline scalars, duplicate keys, or multiple YAML document markers; mutating commands should refuse those with stable diagnostics instead of rewriting them.

## Synthetic Example Shape

Use public, synthetic examples like this when demonstrating or testing the skill:

```md
# Project
Intro.

## Release Plan
Current release notes.

### Checklist
- Confirm fixtures are synthetic.

## Operations
Runbook notes.
```

Never use private notes, account data, proprietary documentation, personal logs, or copyrighted text as fixtures.

## Failure Recovery

- `section_not_found`: run `outline --json` and select an exact path from output.
- `duplicate_section_path`: stop; ask for a more specific document shape or change the document to remove ambiguity before writing.
- `line_in_frontmatter`: treat the line as metadata, not an editable heading section.
- `line_outside_section`: use `outline --json` and choose a heading section.
- `heading_in_shallow_replacement`: provide body-only content or use full `replace`.
- `replacement_heading_mismatch`: make the replacement heading match the selected section exactly.
- `duplicate_child_heading`: use `--after-child` or `--before-child` indexes from JSON output.
- `invalid_child_index`: rerun `outline --json --root <path>` and use the current direct-child index.
- `missing_content` or `conflicting_content_inputs`: provide exactly one content source.
- `frontmatter_missing`: use `--create` only when inserting a new document-start frontmatter block is intended.
- `frontmatter_property_not_found`: run `aimd fm get <file> --json` and select an existing property path, or use `--create` on supported writes.
- `frontmatter_schema_type_mismatch`: change the command/value type or update the schema before writing.
- `invalid_frontmatter_value`: provide a scalar/list/map value matching the command shape.
- `invalid_yaml_frontmatter`: fix the existing frontmatter syntax before attempting a mutation; writes are blocked.
- `unsupported_yaml_construct`: preserve the file and edit manually or simplify the YAML shape before using `aimd fm` mutation commands.
- `unsupported_frontmatter_path`: select a supported top-level key or direct child map key; nested sequence item mutation is intentionally refused.
- `duplicate_frontmatter_key`: remove or reconcile duplicate keys before mutating frontmatter.
- `unsafe_rewrite`, `parse_error`, or `io_error`: stop, preserve the file, and inspect diagnostics before retrying.

## Review Step

When updating this skill or examples, compare command patterns against the current CLI contract:

```bash
aimd --help
aimd outline --help
aimd get --help
aimd replace --help
aimd append --help
aimd append-child --help
aimd fm --help
aimd fm set --help
aimd fm append-list-item --help
aimd check --help
```
