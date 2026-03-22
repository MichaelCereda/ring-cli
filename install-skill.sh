#!/bin/sh
set -e

# install-skill.sh — installs the ring-cli configuration-builder skill for Claude Code
# Usage: curl -fsSL https://raw.githubusercontent.com/MichaelCereda/ring-cli/master/install-skill.sh | sh

REPO="MichaelCereda/ring-cli"
BRANCH="master"
SKILL_NAME="configuration-builder"
SKILLS_DIR="$HOME/.claude/skills/$SKILL_NAME"

info()  { printf '[info]  %s\n' "$*"; }
warn()  { printf '[warn]  %s\n' "$*" >&2; }
error() { printf '[error] %s\n' "$*" >&2; exit 1; }

need_cmd() {
    if ! command -v "$1" > /dev/null 2>&1; then
        error "Required command not found: $1"
    fi
}

need_cmd curl
need_cmd mkdir

SKILL_URL="https://raw.githubusercontent.com/$REPO/$BRANCH/plugin/skills/$SKILL_NAME/SKILL.md"

info "Installing ring-cli skill: $SKILL_NAME"
info "Target: $SKILLS_DIR/SKILL.md"

mkdir -p "$SKILLS_DIR"

if curl -fsSL "$SKILL_URL" -o "$SKILLS_DIR/SKILL.md"; then
    info "Skill installed successfully!"
    info ""
    info "The /ring-cli:configuration-builder skill is now available in Claude Code."
    info "Use it in any project to generate ring-cli configurations from natural language."
else
    error "Failed to download skill from $SKILL_URL"
fi
