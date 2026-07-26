# aimd

`aimd` is planned structural Markdown tooling for agents and humans who need to inspect and edit large Markdown files without relying on fragile line-range rewrites.

The project is implemented as a Rust CLI plus core library. The CLI will expose heading-tree navigation, exact section reads, scoped replacements, append operations, and structural checks. The core library will own parsing, heading paths, frontmatter detection, range tracking, rewrite planning, and recoverable error types.

Implementation is pre-release but functional enough for local validation. The command contract below is the intended v0 surface and is covered by initial fixture tests.

## Why aimd

Markdown documents often grow into long-lived plans, runbooks, notes, release docs, and project handbooks. Agents can damage these files when they patch approximate line ranges or guess where a section starts and ends.

`aimd` is designed around a safer workflow:

1. Inspect the document structure.
2. Select exact, case-sensitive heading paths.
3. Read the target section before editing.
4. Apply shallow or subtree-scoped changes.
5. Use dry-run, stdout, backups, and checks before mutating important files.

Frontmatter is treated as document metadata, not as a heading section. Obsidian-style syntax, wikilinks, callouts, tags, and other Markdown-ish content are preserved as source text rather than parsed semantically.

## Planned Command Surface

```bash
aimd outline <file> [--json] [--max-level N] [--root <exact-path>]
aimd get <file> <exact-path> [--json] [--shallow]
aimd get <file> --line <line> [--json] [--shallow]
aimd replace <file> <exact-path> [--file <content-file> | --content <markdown>] [--shallow] [--dry-run] [--stdout] [--backup]
aimd append <file> <exact-path> [--file <content-file> | --content <markdown>] [--dry-run] [--stdout] [--backup]
aimd append-child <file> <exact-path> --heading <heading> [--file <content-file> | --content <markdown>] [--after <child-heading> | --before <child-heading> | --after-child <index> | --before-child <index>] [--dry-run] [--stdout] [--backup]
aimd check <file> [--json]
```

### `outline`

Print a compact heading tree or JSON section metadata. Agents should use `outline --json` before editing unfamiliar files so they can select canonical heading paths and child indexes.

### `get`

Return a section by exact heading path or by 1-based line lookup. By default, `get` returns the full subtree. With `--shallow`, it returns the heading and direct body while excluding child sections.

### `replace`

Replace either a full section subtree or, with `--shallow`, only the direct body under an existing heading. Full replacements must include a matching heading. Shallow replacements should be body-only content.

### `append`

Append body content to an existing section before its first child section, when children exist.

### `append-child`

Create a new child section under an existing section. Agent workflows should prefer `--after-child` and `--before-child` indexes from JSON outline output when placement ambiguity is possible.

### `check`

Report structural hazards such as duplicate exact paths, skipped heading levels, unterminated frontmatter, and ambiguous write targets.

## Agent Skill

The canonical first-party skill lives at `agent-skills/aimd/SKILL.md`.

Best-effort wrapper scaffolds are included for agent surfaces that support plugin-style skill distribution:

- Codex/OpenAI placeholder: `agent-plugins/openai-aimd/`
- Claude Code placeholder: `plugins/aimd/`

These wrappers intentionally include TODO metadata because marketplace and plugin packaging formats may need verification before publishing.

## Performance Expectations

`aimd` rewrites by byte ranges from the original source instead of re-rendering whole Markdown documents. v0 should comfortably handle large single-file notes and runbooks in normal CLI use. The initial fixture suite includes `fixtures/input/large-synthetic-note.md` as a reviewable benchmark-style document; future releases can expand this into a generated stress fixture once the range model is stable.

## Status

`aimd` is pre-release work. Command names, flags, JSON fields, and error codes describe the intended v0 contract and may change before the first public release.
