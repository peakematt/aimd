---
name: aimd
description: Safe structural Markdown inspection and editing with the aimd CLI. Use when Codex or another agent needs to outline Markdown, select exact heading paths, read sections, replace shallow bodies or full subtrees, append body content, append child sections, or validate structural hazards without fuzzy search or semantic Obsidian parsing.
---

# aimd

Use `aimd` for scoped Markdown edits by heading structure. Treat it as structural tooling: it preserves source text where possible, treats frontmatter as metadata, and does not perform fuzzy search, semantic Obsidian parsing, summarization, embeddings, or project-wide indexing.

The command contract is still pre-release. Before relying on examples, run `aimd --help` and the relevant subcommand `--help` to confirm the installed binary matches this skill.

## Core Workflow

1. Run `aimd outline <file> --json` before selecting paths in unfamiliar files.
2. Use exact, case-sensitive heading paths from outline output, such as `Project > Release Plan > Checklist`.
3. Run `aimd check <file> --json` before writes when duplicate paths, skipped heading levels, or frontmatter hazards are likely.
4. Read before writing. Prefer `get --shallow` before `replace --shallow` when editing direct body text.
5. Preview important edits with `--dry-run` or `--stdout`; use `--backup` before mutating important files.
6. Stop on ambiguity errors. Do not guess between duplicate section paths or duplicate child headings.

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
aimd check --help
```
