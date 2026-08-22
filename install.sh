#!/bin/bash
# Tusk installer for macOS.
#
# Installs the latest Tusk.app from GitHub Releases straight into /Applications
# and strips the download quarantine, so you never hit the unsigned-app
# "Tusk is damaged and can't be opened" Gatekeeper error.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/getsettalk/tusk/main/install.sh | bash
set -euo pipefail

REPO="getsettalk/tusk"
APP="/Applications/Tusk.app"

echo "==> Finding the latest Tusk release…"
DMG_URL=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep -o '"browser_download_url": *"[^"]*\.dmg"' \
  | head -1 | cut -d'"' -f4)

if [ -z "${DMG_URL:-}" ]; then
  echo "Could not find a .dmg in the latest release. Check https://github.com/${REPO}/releases" >&2
  exit 1
fi
echo "    $DMG_URL"

TMP="$(mktemp -d)"
trap 'hdiutil detach "$MNT" -quiet 2>/dev/null || true; rm -rf "$TMP"' EXIT

echo "==> Downloading…"
curl -fsSL "$DMG_URL" -o "$TMP/Tusk.dmg"

echo "==> Mounting…"
MNT="$(hdiutil attach "$TMP/Tusk.dmg" -nobrowse -noverify -noautoopen | grep -o '/Volumes/.*' | head -1)"

echo "==> Installing to /Applications…"
rm -rf "$APP"
cp -R "$MNT/Tusk.app" /Applications/

echo "==> Removing quarantine + ad-hoc signing…"
xattr -cr "$APP" 2>/dev/null || true
codesign --force --deep --sign - "$APP" 2>/dev/null || true

echo "✓ Tusk installed. Opening…"
open "$APP"
echo "  If it still won't open: right-click Tusk in Applications → Open."
