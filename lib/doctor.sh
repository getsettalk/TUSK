#!/usr/bin/env bash
# doctor.sh — diagnose the environment.

cmd_doctor() {
  step "Chowk doctor"
  local ok=1

  # Homebrew
  if command -v brew >/dev/null 2>&1; then
    success "Homebrew: $(brew --version | head -1)  (prefix: $BREW_PREFIX)"
  else
    error "Homebrew missing — https://brew.sh"; ok=0
  fi

  # PHP versions
  local v
  for v in "${SUPPORTED_PHP[@]}"; do
    if php_installed "$v"; then
      success "php@$v: $("$(php_binary "$v")" -r 'echo PHP_VERSION;' 2>/dev/null)"
    else
      warn "php@$v not installed  (run: chowk install)"
    fi
  done

  # MariaDB
  if command -v "$BREW_PREFIX/opt/mariadb/bin/mysql" >/dev/null 2>&1 || brew list mariadb >/dev/null 2>&1; then
    success "MariaDB installed"
  else
    warn "MariaDB not installed  (run: chowk install)"
  fi

  # Nginx
  if brew list nginx >/dev/null 2>&1; then
    success "Nginx installed"
  else
    warn "Nginx not installed  (run: chowk install)"
  fi

  # dnsmasq + resolver
  if brew list dnsmasq >/dev/null 2>&1; then
    success "dnsmasq installed"
  else
    warn "dnsmasq not installed (optional, for *.$CHOWK_TLD pretty URLs)"
  fi
  if [[ -f "/etc/resolver/$CHOWK_TLD" ]]; then
    success "/etc/resolver/$CHOWK_TLD present (.$CHOWK_TLD domains resolve locally)"
  else
    warn "/etc/resolver/$CHOWK_TLD missing  (run: chowk dns setup)"
  fi

  # Active version + running services
  echo
  info "Active PHP: $(get_active_php)"
  info "Config home: $CHOWK_HOME"
  info "Sites root: $CHOWK_WWW"

  echo
  [[ $ok -eq 1 ]] && success "Base tooling looks good." || error "Fix the items above."
}
