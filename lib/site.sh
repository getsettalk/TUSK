#!/usr/bin/env bash
# site.sh — create / list / remove local sites (nginx vhosts).

_site_conf() { echo "$CHOWK_SITES/$1.conf"; }

cmd_site() {
  local sub="${1:-}"; shift || true
  case "$sub" in
    add|create) site_add "$@";;
    list|ls|"") site_list;;
    remove|rm|delete) site_remove "$@";;
    *) die "Unknown: chowk site $sub   (use: add | list | remove)";;
  esac
}

# chowk site add <name> [docroot]
site_add() {
  local name="${1:-}"; local docroot="${2:-}"
  [[ -z "$name" ]] && die "Usage: chowk site add <name> [docroot]"
  name="${name%.$CHOWK_TLD}"   # strip .test if user typed it

  if [[ -z "$docroot" ]]; then
    docroot="$CHOWK_WWW/$name"
    mkdir -p "$docroot"
    if [[ ! -e "$docroot/index.php" && -z "$(ls -A "$docroot" 2>/dev/null)" ]]; then
      printf '<?php phpinfo();\n' > "$docroot/index.php"
      info "Placeholder index.php banaya: $docroot/index.php"
    fi
  fi
  docroot="$(cd "$docroot" 2>/dev/null && pwd)" || die "Docroot nahi mila: $docroot"

  local server_name="$name.$CHOWK_TLD"
  local tpl="$CHOWK_LIB_DIR/../templates/nginx-site.conf.tmpl"
  render "$tpl" \
    SERVER_NAME="$server_name" \
    DOCROOT="$docroot" \
    NAME="$name" \
    CHOWK="$CHOWK_HOME" \
    BREW="$BREW_PREFIX" \
    > "$(_site_conf "$name")"

  success "Site added: ${C_BOLD}http://$server_name${C_RESET}"
  info "Docroot: $docroot"
  reload_nginx 2>/dev/null || warn "nginx reload nahi hua — 'chowk restart' chalao."
  [[ -f "/etc/resolver/$CHOWK_TLD" ]] || warn ".$CHOWK_TLD resolve karne ke liye ek baar 'chowk dns setup' chalao."
}

site_list() {
  step "Sites"
  local f name found=0
  for f in "$CHOWK_SITES"/*.conf; do
    [[ -e "$f" ]] || continue
    found=1
    name="$(basename "$f" .conf)"
    local root; root="$(grep -m1 -E '^[[:space:]]*root ' "$f" | sed -E 's/^[[:space:]]*root[[:space:]]+//; s/;.*$//')"
    printf '  %s http://%s.%s  %s→ %s%s\n' "${C_GREEN}●${C_RESET}" "$name" "$CHOWK_TLD" "$C_DIM" "$root" "$C_RESET"
  done
  if [[ $found -eq 0 ]]; then
    info "Abhi koi site nahi. Banao:  chowk site add myapp"
  fi
}

site_remove() {
  local name="${1:-}"
  [[ -z "$name" ]] && die "Usage: chowk site remove <name>"
  name="${name%.$CHOWK_TLD}"
  local conf; conf="$(_site_conf "$name")"
  [[ -f "$conf" ]] || die "Site nahi mili: $name"
  rm -f "$conf"
  success "Site removed: $name.$CHOWK_TLD  (files delete nahi kiye)"
  reload_nginx 2>/dev/null || true
}
