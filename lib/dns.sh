#!/usr/bin/env bash
# dns.sh — wildcard *.test -> 127.0.0.1 via dnsmasq + macOS resolver.

cmd_dns() {
  local sub="${1:-setup}"
  case "$sub" in
    setup) dns_setup;;
    status) dns_status;;
    *) die "Unknown: chowk dns $sub   (use: setup | status)";;
  esac
}

dns_setup() {
  require_brew
  brew list dnsmasq >/dev/null 2>&1 || die "dnsmasq installed nahi. Chalao: chowk install"

  step "Setting up *.$CHOWK_TLD -> 127.0.0.1"

  # dnsmasq config
  local dnsmasq_conf="$BREW_PREFIX/etc/dnsmasq.conf"
  local line="address=/.$CHOWK_TLD/127.0.0.1"
  if ! grep -qxF "$line" "$dnsmasq_conf" 2>/dev/null; then
    printf '\n# Added by Chowk\n%s\n' "$line" | sudo tee -a "$dnsmasq_conf" >/dev/null
    success "dnsmasq config updated"
  else
    info "dnsmasq config already set"
  fi
  sudo brew services restart dnsmasq >/dev/null 2>&1 && success "dnsmasq restarted"

  # macOS resolver
  sudo mkdir -p /etc/resolver
  printf 'nameserver 127.0.0.1\n' | sudo tee "/etc/resolver/$CHOWK_TLD" >/dev/null
  success "/etc/resolver/$CHOWK_TLD written"

  echo
  success "Ho gaya. Ab koi bhi *.${CHOWK_TLD} domain 127.0.0.1 par resolve hoga."
  info "Test: ping -c1 anything.$CHOWK_TLD"
}

dns_status() {
  step "DNS status"
  [[ -f "/etc/resolver/$CHOWK_TLD" ]] && success "resolver present" || warn "resolver missing"
  if brew services list 2>/dev/null | grep -qE '^dnsmasq\s+started'; then
    success "dnsmasq running"
  else
    warn "dnsmasq stopped"
  fi
}
