#!/usr/bin/env bash
# install.sh — install the underlying components via Homebrew.

cmd_install() {
  require_brew
  ensure_dirs
  step "Installing Chowk components via Homebrew"

  local formulae=()
  local v
  for v in "${SUPPORTED_PHP[@]}"; do
    php_installed "$v" || formulae+=("php@$v")
  done
  brew list mariadb >/dev/null 2>&1 || formulae+=("mariadb")
  brew list nginx   >/dev/null 2>&1 || formulae+=("nginx")
  brew list dnsmasq >/dev/null 2>&1 || formulae+=("dnsmasq")

  if [[ ${#formulae[@]} -eq 0 ]]; then
    success "Sab kuch pehle se installed hai."
  else
    info "Ye formulae install honge: ${formulae[*]}"
    if confirm "Aage badhein?"; then
      brew install "${formulae[@]}" || die "brew install fail hua."
    else
      warn "Install cancel kiya."
      return 1
    fi
  fi

  # Seed the main nginx config include + fastcgi params from brew if needed.
  write_nginx_main_conf
  set_active_php "$(get_active_php)"

  echo
  success "Install ho gaya."
  info "Ab chalao:  chowk start"
  info "Pretty URLs ke liye (ek baar): chowk dns setup"
}
