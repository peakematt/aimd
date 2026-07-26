#!/usr/bin/env sh
set -eu

usage() {
  cat <<'USAGE'
Install the aimd agent skill for local agent runtimes.

Usage:
  install-agent-skill.sh [--target codex|claude|both] [--source DIR] [--dry-run]

Defaults:
  --target both
  --source auto-detected from the current checkout or extracted release asset

Install locations:
  codex  -> ${CODEX_HOME:-$HOME/.codex}/skills/aimd
  claude -> ${CLAUDE_HOME:-$HOME/.claude}/skills/aimd

Review SKILL.md before installing third-party skills.
USAGE
}

target="both"
source_dir=""
dry_run="false"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --target)
      target="${2:-}"
      shift 2
      ;;
    --source)
      source_dir="${2:-}"
      shift 2
      ;;
    --dry-run)
      dry_run="true"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$target" in
  codex|claude|both) ;;
  *)
    echo "--target must be codex, claude, or both" >&2
    exit 2
    ;;
esac

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

if [ -z "$source_dir" ]; then
  for candidate in \
    "$script_dir" \
    "$script_dir/../agent-skills/aimd" \
    "$PWD/agent-skills/aimd" \
    "$PWD/aimd"
  do
    if [ -f "$candidate/SKILL.md" ]; then
      source_dir="$candidate"
      break
    fi
  done
fi

if [ -z "$source_dir" ] || [ ! -f "$source_dir/SKILL.md" ]; then
  echo "Could not find aimd/SKILL.md. Pass --source DIR." >&2
  exit 1
fi

install_one() {
  runtime="$1"
  case "$runtime" in
    codex)
      root="${CODEX_HOME:-$HOME/.codex}/skills"
      ;;
    claude)
      root="${CLAUDE_HOME:-$HOME/.claude}/skills"
      ;;
    *)
      echo "Unknown runtime: $runtime" >&2
      exit 2
      ;;
  esac

  dest="$root/aimd"
  echo "Installing aimd skill for $runtime:"
  echo "  source: $source_dir"
  echo "  dest:   $dest"

  if [ "$dry_run" = "true" ]; then
    return 0
  fi

  mkdir -p "$dest"
  cp "$source_dir/SKILL.md" "$dest/SKILL.md"
}

case "$target" in
  codex)
    install_one codex
    ;;
  claude)
    install_one claude
    ;;
  both)
    install_one codex
    install_one claude
    ;;
esac

if command -v aimd >/dev/null 2>&1; then
  aimd --help >/dev/null
  echo "aimd binary check passed."
else
  echo "Warning: aimd is not on PATH yet. Install the CLI binary before relying on the skill." >&2
fi

if [ "$dry_run" = "true" ]; then
  echo "Dry run complete. No files were installed."
else
  echo "Installed. Restart agents that do not live-reload skills."
fi
