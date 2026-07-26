# Agent Skill Installation

`aimd` includes a canonical skill and best-effort wrapper scaffolds for agent plugin distribution.

The skill and wrappers are placeholders while the CLI and marketplace packaging are still in progress. Verify the current plugin formats for each agent surface before publishing these directories as installable packages.

## Canonical Skill

Use this source as the authoritative skill text:

```text
agent-skills/aimd/SKILL.md
```

## Codex/OpenAI Placeholder

Best-effort plugin scaffold:

```text
agent-plugins/openai-aimd/.codex-plugin/plugin.json
agent-plugins/openai-aimd/skills/aimd/SKILL.md
```

Planned local authoring fallback:

```text
$skill-installer from a GitHub repository path, once the repository and skill path are published.
```

TODO: verify the current Codex/OpenAI plugin manifest shape before marketplace submission.

## Claude Code Placeholder

Best-effort plugin scaffold:

```text
plugins/aimd/.claude-plugin/plugin.json
plugins/aimd/.claude-plugin/marketplace.json
plugins/aimd/skills/aimd/SKILL.md
```

The planning spec mentions commands like:

```text
/plugin marketplace add peakematt/aimd
/plugin install aimd@<marketplace-name>
/reload-plugins
```

TODO: verify Claude Code's current repository-level marketplace layout before publishing. This scaffold keeps the placeholder marketplace file under `plugins/aimd/` because the current ownership scope does not include root `.claude-plugin/`.

## Project-Local Fallback

Teams can vendor the canonical skill into their own repository once `aimd` is released.

Suggested repo-local placements:

```text
.agents/skills/aimd/SKILL.md
.claude/skills/aimd/SKILL.md
```

Keep examples synthetic and public. Do not include private notes, personal logs, account data, proprietary documentation, or copyrighted source text.
