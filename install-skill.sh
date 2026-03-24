#!/bin/sh
set -e

# install-skill.sh — installs the stampo configuration-builder skill for Claude Code
# Usage: curl -fsSL https://raw.githubusercontent.com/MichaelCereda/stampo/master/install-skill.sh | sh

REPO="MichaelCereda/stampo"
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

info "Installing stampo skill: $SKILL_NAME"
info "Target: $SKILLS_DIR/SKILL.md"

mkdir -p "$SKILLS_DIR"

if curl -fsSL "$SKILL_URL" -o "$SKILLS_DIR/SKILL.md"; then
    info "Skill installed successfully!"
    info ""
    info "The /stampo:configuration-builder skill is now available in Claude Code."
    info "Use it in any project to generate stampo configurations from natural language."
else
    error "Failed to download skill from $SKILL_URL"
fi
