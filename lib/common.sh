#!/usr/bin/env bash
# common.sh — shared helpers, paths and configuration for Chowk.
# This file is sourced by bin/chowk and every lib/*.sh module.

# ---------------------------------------------------------------------------
# Colors / logging
# ---------------------------------------------------------------------------
if [[ -t 1 ]]; then
  C_RESET=$'\033[0m'; C_DIM=$'\033[2m'; C_BOLD=$'\033[1m'
  C_RED=$'\033[31m'; C_GREEN=$'\033[32m'; C_YELLOW=$'\033[33m'; C_BLUE=$'\033[34m'; C_CYAN=$'\033[36m'
else
  C_RESET=""; C_DIM=""; C_BOLD=""; C_RED=""; C_GREEN=""; C_YELLOW=""; C_BLUE=""; C_CYAN=""
fi

info()    { printf '%s\n' "${C_CYAN}==>${C_RESET} $*"; }
success() { printf '%s\n' "${C_GREEN}✓${C_RESET} $*"; }
warn()    { printf '%s\n' "${C_YELLOW}!${C_RESET} $*" >&2; }
error()   { printf '%s\n' "${C_RED}✗${C_RESET} $*" >&2; }
die()     { error "$*"; exit 1; }
step()    { printf '%s\n' "${C_BOLD}${C_BLUE}▸${C_RESET} ${C_BOLD}$*${C_RESET}"; }

confirm() {
  # confirm "Question?"  -> returns 0 on yes
  local prompt="${1:-Continue?}"
  local reply
  read -r -p "$prompt [y/N] " reply
  [[ "$reply" =~ ^[Yy]$ ]]
}

# ---------------------------------------------------------------------------
# Homebrew detection
# ---------------------------------------------------------------------------
require_brew() {
  if ! command -v brew >/dev/null 2>&1; then
    die "Homebrew nahi mila. Install karo: https://brew.sh"
  fi
}

brew_prefix() {
  brew --prefix 2>/dev/null
}

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
BREW_PREFIX="$(brew_prefix 2>/dev/null || echo /opt/homebrew)"
CHOWK_HOME="${CHOWK_HOME:-$HOME/.chowk}"
CHOWK_SITES="$CHOWK_HOME/sites"
CHOWK_LOGS="$CHOWK_HOME/logs"
CHOWK_RUN="$CHOWK_HOME/run"
CHOWK_APPS="$CHOWK_HOME/apps"
CHOWK_ETC="$CHOWK_HOME/etc"
CHOWK_WWW="${CHOWK_WWW:-$HOME/Sites}"          # default parent for project docroots
STATE_PHP="$CHOWK_ETC/php-version"             # remembers active php version

# The TLD used for local sites (e.g. myapp.test)
CHOWK_TLD="${CHOWK_TLD:-test}"

# PHP versions Chowk knows about (extend freely).
SUPPORTED_PHP=("8.2" "8.3")

ensure_dirs() {
  mkdir -p "$CHOWK_SITES" "$CHOWK_LOGS" "$CHOWK_RUN" "$CHOWK_APPS" "$CHOWK_ETC" "$CHOWK_WWW"
}

# ---------------------------------------------------------------------------
# Active PHP version state
# ---------------------------------------------------------------------------
get_active_php() {
  if [[ -f "$STATE_PHP" ]]; then
    cat "$STATE_PHP"
  else
    # newest supported (bash 3.2 has no negative indices)
    echo "${SUPPORTED_PHP[$(( ${#SUPPORTED_PHP[@]} - 1 ))]}"
  fi
}

set_active_php() {
  ensure_dirs
  printf '%s' "$1" > "$STATE_PHP"
}

php_binary() {   # php_binary 8.3  -> full path to that php
  echo "$BREW_PREFIX/opt/php@$1/bin/php"
}

php_fpm_binary() {
  echo "$BREW_PREFIX/opt/php@$1/sbin/php-fpm"
}

php_installed() {   # php_installed 8.2 -> 0 if brew formula present
  [[ -x "$(php_binary "$1")" ]]
}

# Render a template file, replacing {{TOKENS}}. Usage:
#   render tpl.file KEY=value KEY2=value2 ...
render() {
  local tpl="$1"; shift
  local content; content="$(cat "$tpl")"
  local pair k v
  for pair in "$@"; do
    k="${pair%%=*}"; v="${pair#*=}"
    # escape ampersands/pipes for sed replacement safety by using bash instead
    content="${content//\{\{$k\}\}/$v}"
  done
  printf '%s' "$content"
}
