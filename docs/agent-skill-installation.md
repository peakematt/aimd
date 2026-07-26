# Agent Skill Installation

`aimd` includes a canonical skill, release-packaged skill archives, and best-effort wrapper scaffolds for agent plugin distribution.

The skill package is meant to be easy for a human or agent to inspect before installing. Marketplace wrapper metadata is still placeholder scaffolding until the exact Codex/OpenAI and Claude plugin marketplace schemas are verified.

## Canonical Skill

Use this source as the authoritative skill text:

```text
agent-skills/aimd/SKILL.md
```

## Release Asset Install

Every versioned GitHub Release should include these agent skill assets:

```text
aimd-agent-skill-vX.Y.Z.tar.gz
aimd-agent-skill-vX.Y.Z.zip
```

Each archive contains one top-level skill folder:

```text
aimd/
  SKILL.md
  README.md
  install.sh
```

Example install flow for an agent or human:

```bash
AIMD_VERSION=v0.1.0
curl -LO "https://github.com/peakematt/aimd/releases/download/${AIMD_VERSION}/aimd-agent-skill-${AIMD_VERSION}.tar.gz"
tar -xzf "aimd-agent-skill-${AIMD_VERSION}.tar.gz"
sed -n '1,180p' aimd/SKILL.md
sh aimd/install.sh --target both
aimd --help
```

Use `--target codex` or `--target claude` to install only one runtime. The installer copies the skill into:

```text
${CODEX_HOME:-$HOME/.codex}/skills/aimd
${CLAUDE_HOME:-$HOME/.claude}/skills/aimd
```

For OpenAI hosted environments, the zip asset has the required single top-level skill folder shape and can be uploaded as a skill bundle.

Local checkout test:

```bash
scripts/install-agent-skill.sh --target both --source agent-skills/aimd --dry-run
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
