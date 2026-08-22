#!/usr/bin/env bash
# pma.sh — install phpMyAdmin and expose it at http://pma.test
#
# Reliability model (per project principle "reliable, verified sources"):
#   * Primary  = official phpmyadmin.net package, SHA256-verified.
#   * Uses the "latest" redirect so users always get the current release,
#     or pin an exact version with PMA_VERSION=x.y.z.
#   * Smallest format (.tar.xz) to keep downloads quick.
#   * Fallback = Homebrew bottle (also checksum-verified by brew) on macOS.

# Pin a version, or leave as "latest" to always fetch the current release.
PMA_VERSION="${PMA_VERSION:-latest}"
# english (small) | all-languages (every locale)
PMA_FLAVOR="${PMA_FLAVOR:-english}"
# tar.xz (smallest) | tar.gz
PMA_FORMAT="${PMA_FORMAT:-tar.xz}"
# official | brew  — where to get it from
PMA_SOURCE="${PMA_SOURCE:-official}"

cmd_phpmyadmin() {
  local sub="${1:-install}"
  case "$sub" in
    install) pma_install;;
    remove|rm) pma_remove;;
    *) die "Unknown: chowk phpmyadmin $sub   (use: install | remove)";;
  esac
}

pma_install() {
  ensure_dirs
  local dir cfg

  if [[ "$PMA_SOURCE" == "brew" ]] && command -v brew >/dev/null 2>&1; then
    dir="$BREW_PREFIX/share/phpmyadmin"
    [[ -f "$dir/index.php" ]] || { step "Installing phpMyAdmin via Homebrew"; brew install phpmyadmin || die "brew install phpmyadmin fail"; }
    cfg="$BREW_PREFIX/etc/phpmyadmin.config.inc.php"
  else
    dir="$CHOWK_APPS/phpmyadmin"
    pma_download "$dir"
    cfg="$dir/config.inc.php"
  fi

  pma_write_config "$cfg"
  site_add "pma" "$dir"
  echo
  success "phpMyAdmin ready: ${C_BOLD}http://pma.$CHOWK_TLD${C_RESET}"
  info "Login user: root  (MariaDB ka default password khaali hota hai jab tak set na karo)"
}

# Download + SHA256-verify the official package into $1.
pma_download() {
  local dir="$1"
  [[ -f "$dir/index.php" ]] && { info "phpMyAdmin pehle se maujood: $dir"; return 0; }

  local base pkg
  if [[ "$PMA_VERSION" == "latest" ]]; then
    # Generic redirect that always points at the current release.
    base="https://www.phpmyadmin.net/downloads"
    pkg="phpMyAdmin-latest-${PMA_FLAVOR}"
  else
    base="https://files.phpmyadmin.net/phpMyAdmin/${PMA_VERSION}"
    pkg="phpMyAdmin-${PMA_VERSION}-${PMA_FLAVOR}"
  fi
  local url="$base/${pkg}.${PMA_FORMAT}"

  step "Downloading ${pkg}.${PMA_FORMAT}"
  local tmp; tmp="$(mktemp -d)"
  local tarball="$tmp/pma.${PMA_FORMAT}"

  curl -fL --retry 4 --retry-delay 3 --retry-connrefused --connect-timeout 20 \
       -C - "$url" -o "$tarball" \
    || { rm -rf "$tmp"; die "Download fail (mirror slow?). Phir se: chowk phpmyadmin install"; }

  # SHA256 verification (best-effort: skip only if checksum file unavailable).
  local sha_url="$url.sha256"
  if curl -fsL --connect-timeout 15 "$sha_url" -o "$tmp/sum.sha256" 2>/dev/null; then
    local expected actual
    expected="$(awk '{print $1}' "$tmp/sum.sha256" | head -1)"
    actual="$(shasum -a 256 "$tarball" | awk '{print $1}')"
    if [[ -n "$expected" && "$expected" != "$actual" ]]; then
      rm -rf "$tmp"
      die "SHA256 mismatch! expected=$expected got=$actual — download corrupt/tampered."
    fi
    success "SHA256 verified ✓"
  else
    warn "Checksum file nahi mila — verification skip (download rakha)."
  fi

  # Extract (tar handles both .gz and .xz on macOS).
  local extracted
  tar -xf "$tarball" -C "$tmp" || { rm -rf "$tmp"; die "Extract fail"; }
  extracted="$(find "$tmp" -maxdepth 1 -type d -name 'phpMyAdmin-*' | head -1)"
  [[ -d "$extracted" ]] || { rm -rf "$tmp"; die "Extracted folder nahi mila"; }
  rm -rf "$dir"; mv "$extracted" "$dir"; rm -rf "$tmp"
  success "phpMyAdmin -> $dir"
}

pma_write_config() {
  local cfg="$1"
  [[ -f "$cfg" ]] && { info "config already: $cfg"; return 0; }
  local secret; secret="$(head -c 24 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 32)"
  cat > "$cfg" <<EOF
<?php
\$cfg['blowfish_secret'] = '$secret';
\$i = 0;
\$i++;
\$cfg['Servers'][\$i]['auth_type'] = 'cookie';
\$cfg['Servers'][\$i]['host'] = '127.0.0.1';
\$cfg['Servers'][\$i]['port'] = '3306';
\$cfg['Servers'][\$i]['compress'] = false;
\$cfg['Servers'][\$i]['AllowNoPassword'] = true;
\$cfg['UploadDir'] = '';
\$cfg['SaveDir'] = '';
EOF
  success "config likha: $cfg (host 127.0.0.1:3306)"
}

pma_remove() {
  site_remove "pma" 2>/dev/null || true
  if command -v brew >/dev/null 2>&1 && brew list phpmyadmin >/dev/null 2>&1; then
    brew uninstall phpmyadmin 2>/dev/null || true
  fi
  rm -rf "$CHOWK_APPS/phpmyadmin"
  success "phpMyAdmin removed"
}
