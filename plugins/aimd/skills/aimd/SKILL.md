---
name: aimd
description: Safe structural Markdown and frontmatter inspection/editing with the aimd CLI. Use when Claude Code or another agent needs to outline Markdown, select exact heading paths, read or mutate sections, edit Obsidian-style frontmatter properties, update lists/maps, or validate structural hazards without fuzzy search or semantic Obsidian parsing.
---

# aimd

Use `aimd` for scoped Markdown edits by heading structure and explicit frontmatter property operations. Treat it as structural tooling: it preserves source text where possible, treats frontmatter as metadata for heading operations, and does not perform fuzzy search, semantic Obsidian parsing, summarization, embeddings, or project-wide indexing.

The canonical skill source is `agent-skills/aimd/SKILL.md`. Keep this wrapper copy synchronized with that file until the plugin packaging workflow can reference the canonical file directly.
