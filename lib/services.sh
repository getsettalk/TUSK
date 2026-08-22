#!/usr/bin/env bash
# services.sh — start / stop / status orchestration for nginx, php-fpm, mariadb.

# --- dashboard landing page ------------------------------------------------
seed_dashboard() {
  local d="$CHOWK_APPS/dashboard"
  mkdir -p "$d"
  cat > "$d/index.html" <<'HTML'
<!doctype html><meta charset="utf-8"><title>Chowk</title>
<style>
  body{font-family:-apple-system,system-ui,sans-serif;background:#0f1115;color:#e6e6e6;
       display:grid;place-items:center;height:100vh;margin:0}
  .card{text-align:center}h1{font-size:3rem;margin:0}
  code{background:#1c1f26;padding:.2em .5em;border-radius:6px}
  a{color:#5eb1ff}
</style>
<div class=card>
  <h1>चौक · Chowk</h1>
  <p>Your local dev environment is running.</p>
  <p><code>chowk site add myapp</code> to create a site.</p>
  <p><a href="http://pma.test">phpMyAdmin</a></p>
</div>
HTML
}

# --- nginx config ----------------------------------------------------------
write_nginx_main_conf() {
  ensure_dirs
  local tpl="$CHOWK_LIB_DIR/../templates/nginx-main.conf.tmpl"
  render "$tpl" \
    CHOWK="$CHOWK_HOME" \
    BREW="$BREW_PREFIX" \
    USER="$USER" \
    > "$CHOWK_ETC/nginx.conf"
}

nginx_conf() { echo "$CHOWK_ETC/nginx.conf"; }

start_nginx() {
  [[ -f "$(nginx_conf)" ]] || write_nginx_main_conf
  info "nginx start (port 80 ke liye sudo chahiye)…"
  if "$BREW_PREFIX/bin/nginx" -t -c "$(nginx_conf)" >/dev/null 2>&1; then
    sudo "$BREW_PREFIX/bin/nginx" -c "$(nginx_conf)" 2>/dev/null && success "nginx running" \
      || warn "nginx pehle se chal raha hoga (ya port 80 busy). 'chowk restart' try karo."
  else
    error "nginx config test fail:"
    "$BREW_PREFIX/bin/nginx" -t -c "$(nginx_conf)"
    return 1
  fi
}

stop_nginx() {
  if [[ -f "$CHOWK_RUN/nginx.pid" ]]; then
    sudo "$BREW_PREFIX/bin/nginx" -c "$(nginx_conf)" -s stop 2>/dev/null && success "nginx stopped" \
      || warn "nginx stop nahi hua (shayad already stopped)."
  else
    warn "nginx nahi chal raha."
  fi
}

reload_nginx() {
  if [[ -f "$CHOWK_RUN/nginx.pid" ]]; then
    "$BREW_PREFIX/bin/nginx" -t -c "$(nginx_conf)" >/dev/null 2>&1 \
      && sudo "$BREW_PREFIX/bin/nginx" -c "$(nginx_conf)" -s reload 2>/dev/null \
      && success "nginx reloaded"
  fi
}

# --- mariadb ---------------------------------------------------------------
start_mariadb() {
  info "MariaDB start…"
  brew services start mariadb >/dev/null 2>&1 && success "MariaDB running (127.0.0.1:3306)" \
    || warn "MariaDB start nahi hua."
}

stop_mariadb() {
  brew services stop mariadb >/dev/null 2>&1 && success "MariaDB stopped" \
    || warn "MariaDB stop nahi hua."
}

# --- combined --------------------------------------------------------------
cmd_start() {
  require_brew
  ensure_dirs
  seed_dashboard
  step "Starting Chowk services"
  start_mariadb
  start_php_fpm "$(get_active_php)"
  start_nginx
  echo
  cmd_status
}

cmd_stop() {
  step "Stopping Chowk services"
  stop_nginx
  stop_php_fpm
  stop_mariadb
}

cmd_restart() {
  cmd_stop
  echo
  cmd_start
}

cmd_status() {
  step "Chowk status"
  # php-fpm
  if [[ -f "$CHOWK_RUN/php-fpm.pid" ]] && kill -0 "$(cat "$CHOWK_RUN/php-fpm.pid")" 2>/dev/null; then
    success "php-fpm: running  (php@$(get_active_php), socket: $CHOWK_RUN/php.sock)"
  else
    warn "php-fpm: stopped"
  fi
  # nginx
  if [[ -f "$CHOWK_RUN/nginx.pid" ]] && kill -0 "$(cat "$CHOWK_RUN/nginx.pid")" 2>/dev/null; then
    success "nginx: running  (http://localhost)"
  else
    warn "nginx: stopped"
  fi
  # mariadb
  if brew services list 2>/dev/null | grep -qE '^mariadb\s+started'; then
    success "mariadb: running  (127.0.0.1:3306)"
  else
    warn "mariadb: stopped"
  fi
}
