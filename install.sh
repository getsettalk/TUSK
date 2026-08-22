#!/usr/bin/env bash
# Chowk installer — symlinks the `chowk` CLI into your PATH.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-/opt/homebrew}"
TARGET="$PREFIX/bin/chowk"

if [[ ! -w "$PREFIX/bin" ]]; then
  echo "==> Linking with sudo (need write access to $PREFIX/bin)"
  sudo ln -sf "$REPO_DIR/bin/chowk" "$TARGET"
else
  ln -sf "$REPO_DIR/bin/chowk" "$TARGET"
fi

chmod +x "$REPO_DIR/bin/chowk"
echo "✓ Installed: $TARGET -> $REPO_DIR/bin/chowk"
echo "  Try:  chowk doctor"
