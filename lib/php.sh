#!/usr/bin/env bash
# php.sh — php-fpm lifecycle and version switching.
#
# Model: exactly ONE php-fpm runs at a time (the active version), listening on a
# fixed unix socket ($CHOWK_RUN/php.sock). Every nginx site fastcgi_pass-es to
# that socket, so switching PHP version = restart php-fpm with a different
# binary — no nginx changes needed.

write_php_fpm_conf() {
  ensure_dirs
  local tpl="$CHOWK_LIB_DIR/../templates/php-fpm.conf.tmpl"
  render "$tpl" \
    CHOWK="$CHOWK_HOME" \
    USER="$USER" \
    GROUP="staff" \
    > "$CHOWK_ETC/php-fpm.conf"
}

start_php_fpm() {
  local v="${1:-$(get_active_php)}"
  if ! php_installed "$v"; then
    error "php@$v installed nahi hai. Chalao: chowk install"
    return 1
  fi
  stop_php_fpm >/dev/null 2>&1
  write_php_fpm_conf
  info "php-fpm start (php@$v)…"
  # -y : our self-contained FPM config (global + www pool, daemonize=yes)
  # -c : php.ini directory (brew's, so extensions load correctly)
  "$(php_fpm_binary "$v")" \
      -y "$CHOWK_ETC/php-fpm.conf" \
      -c "$BREW_PREFIX/etc/php/$v" 2>>"$CHOWK_LOGS/php-fpm.log" \
    && success "php-fpm running (php@$v)" \
    || { error "php-fpm start fail. Log: $CHOWK_LOGS/php-fpm.log"; return 1; }
}

stop_php_fpm() {
  if [[ -f "$CHOWK_RUN/php-fpm.pid" ]]; then
    local pid; pid="$(cat "$CHOWK_RUN/php-fpm.pid")"
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null && success "php-fpm stopped"
    fi
    rm -f "$CHOWK_RUN/php-fpm.pid"
  fi
}

cmd_php() {
  local sub="${1:-}"; shift || true
  case "$sub" in
    use)
      local v="${1:-}"
      [[ -z "$v" ]] && die "Usage: chowk php use <version>   (e.g. 8.2)"
      # accept "php@8.2" or "8.2"
      v="${v#php@}"
      php_installed "$v" || die "php@$v installed nahi. Chalao: chowk install"
      set_active_php "$v"
      success "Active PHP -> $v"
      start_php_fpm "$v"
      ;;
    list|ls|"")
      step "PHP versions"
      local active v; active="$(get_active_php)"
      for v in "${SUPPORTED_PHP[@]}"; do
        if php_installed "$v"; then
          if [[ "$v" == "$active" ]]; then
            printf '  %s php@%s  (active)\n' "${C_GREEN}●${C_RESET}" "$v"
          else
            printf '  %s php@%s\n' "${C_DIM}○${C_RESET}" "$v"
          fi
        else
          printf '  %s php@%s  %s(not installed)%s\n' "${C_DIM}○" "$v" "$C_DIM" "$C_RESET"
        fi
      done
      ;;
    current)
      echo "$(get_active_php)"
      ;;
    *)
      die "Unknown: chowk php $sub   (use: use | list | current)"
      ;;
  esac
}
