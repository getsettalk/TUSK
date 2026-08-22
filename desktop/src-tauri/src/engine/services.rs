//! Service orchestration: php-fpm, the chosen web server (nginx or apache),
//! and MariaDB. Privileged ports (<1024) are started via a macOS admin prompt.

use super::*;
use serde::Serialize;
use std::fs;

pub fn php_sock() -> String {
    format!("{}/php.sock", run_dir().display())
}

#[derive(Serialize, Clone)]
pub struct Status {
    pub php_fpm: bool,
    pub php_version: String,
    pub web_server: String,
    pub web_running: bool,
    pub mariadb: bool,
    pub http_port: u16,
}

pub fn status() -> Status {
    let state = load_state();
    Status {
        php_fpm: pid_alive(&run_dir().join("php-fpm.pid")),
        php_version: state.active_php.clone(),
        web_server: state.web_server.clone(),
        web_running: web_running(&state.web_server),
        mariadb: mariadb_running(),
        http_port: state.http_port,
    }
}

// ---------------------------------------------------------------------------
// php-fpm
// ---------------------------------------------------------------------------

fn write_php_fpm_conf() -> Result<String, String> {
    ensure_dirs();
    let user = std::env::var("USER").unwrap_or_else(|_| "nobody".into());
    let conf = format!(
        "[global]\n\
         pid = {run}/php-fpm.pid\n\
         error_log = {logs}/php-fpm.log\n\
         daemonize = yes\n\n\
         [www]\n\
         user = {user}\n\
         group = staff\n\
         listen = {sock}\n\
         listen.owner = {user}\n\
         listen.group = staff\n\
         listen.mode = 0660\n\
         pm = dynamic\n\
         pm.max_children = 10\n\
         pm.start_servers = 2\n\
         pm.min_spare_servers = 1\n\
         pm.max_spare_servers = 3\n\
         pm.max_requests = 500\n",
        run = run_dir().display(),
        logs = logs_dir().display(),
        user = user,
        sock = php_sock(),
    );
    let path = etc_dir().join("php-fpm.conf");
    fs::write(&path, conf).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

pub fn restart_php_fpm(version: &str) -> Result<String, String> {
    stop_php_fpm();
    let conf = write_php_fpm_conf()?;
    if !php::is_installed(version) {
        return Err(format!("php@{version} not installed"));
    }
    // -y = our self-contained fpm config; -c = brew's php.ini dir (extensions).
    run(
        &php::php_fpm_bin(version),
        &["-y", &conf, "-c", &php::php_ini_dir(version)],
    )?;
    Ok(format!("php-fpm running (php@{version})"))
}

pub fn stop_php_fpm() {
    let pid_file = run_dir().join("php-fpm.pid");
    if let Ok(s) = fs::read_to_string(&pid_file) {
        let _ = run("kill", &[s.trim()]);
    }
    let _ = fs::remove_file(pid_file);
}

// ---------------------------------------------------------------------------
// MariaDB (user-level via brew services, no sudo)
// ---------------------------------------------------------------------------

pub fn mariadb_running() -> bool {
    run(&brew_bin(), &["services", "list"])
        .map(|s| {
            s.lines()
                .any(|l| l.starts_with("mariadb") && l.contains("started"))
        })
        .unwrap_or(false)
}

pub fn start_mariadb() -> Result<String, String> {
    run(&brew_bin(), &["services", "start", "mariadb"])
}
pub fn stop_mariadb() -> Result<String, String> {
    run(&brew_bin(), &["services", "stop", "mariadb"])
}

// ---------------------------------------------------------------------------
// Web server: nginx / apache
// ---------------------------------------------------------------------------

fn nginx_conf_path() -> String { etc_dir().join("nginx.conf").display().to_string() }
fn httpd_conf_path() -> String { etc_dir().join("httpd.conf").display().to_string() }

/// Robust running check: pid file OR a live process matching our config path
/// (so it still works if the pid file was lost/emptied).
pub fn web_running(server: &str) -> bool {
    let (pidf, conf, bin) = if server == "apache" {
        ("httpd.pid", httpd_conf_path(), "httpd")
    } else {
        ("nginx.pid", nginx_conf_path(), "nginx")
    };
    pid_alive(&run_dir().join(pidf))
        || run_ok("pgrep", &["-f", &format!("{}.*{}", bin, conf)])
}

/// Start the selected web server on a CLEAN slate: kill any existing nginx AND
/// httpd (ours) first, then start the chosen one — one admin prompt does both,
/// so switching servers or double-clicking Start never leaves duplicates or a
/// port-80 conflict.
pub fn start_web() -> Result<String, String> {
    let state = load_state();
    seed_dashboard();
    sites::generate_vhosts(&state.web_server, state.http_port, &state.tld)?;
    let brew = brew_prefix();
    let nconf = nginx_conf_path();
    let hconf = httpd_conf_path();

    let start_cmd = match state.web_server.as_str() {
        "apache" => {
            if !brew_installed("httpd") {
                return Err("Apache (httpd) not installed. Install it from the Services tab.".into());
            }
            write_apache_conf(state.http_port)?;
            run(&format!("{brew}/bin/httpd"), &["-f", &hconf, "-t"])?; // validate
            format!("{brew}/bin/httpd -f {hconf} -k start")
        }
        _ => {
            write_nginx_conf(state.http_port)?;
            run(&format!("{brew}/bin/nginx"), &["-t", "-c", &nconf])?; // validate
            format!("{brew}/bin/nginx -c {nconf}")
        }
    };
    // Clean any existing instances of BOTH servers, then start the selected one.
    let script = format!(
        "pkill -f 'nginx.*{nconf}' 2>/dev/null; pkill -f 'httpd.*{hconf}' 2>/dev/null; sleep 1; {start_cmd}"
    );
    if state.http_port < 1024 {
        run_privileged(&script)?;
    } else {
        run("sh", &["-c", &script])?;
    }
    Ok(format!("{} started", state.web_server))
}

/// Stop BOTH web servers (whichever is running) and clear pid files.
pub fn stop_web() -> Result<String, String> {
    let state = load_state();
    let brew = brew_prefix();
    let nconf = nginx_conf_path();
    let hconf = httpd_conf_path();
    let script = format!(
        "{brew}/bin/nginx -c {nconf} -s stop 2>/dev/null; \
         pkill -f 'nginx.*{nconf}' 2>/dev/null; \
         pkill -f 'httpd.*{hconf}' 2>/dev/null; true"
    );
    if state.http_port < 1024 {
        run_privileged(&script)?;
    } else {
        let _ = run("sh", &["-c", &script]);
    }
    let _ = std::fs::remove_file(run_dir().join("nginx.pid"));
    let _ = std::fs::remove_file(run_dir().join("httpd.pid"));
    Ok("web server stopped".into())
}

// --- nginx -----------------------------------------------------------------

fn write_nginx_conf(port: u16) -> Result<String, String> {
    ensure_dirs();
    let brew = brew_prefix();
    // On a privileged port nginx starts as root and its workers would drop to
    // `nobody`, which can't traverse the user's 0750 home dir (→ 404/502).
    // Pinning workers to the real user fixes static files AND php-fpm socket access.
    let user_line = if port < 1024 {
        let u = std::env::var("USER").unwrap_or_else(|_| "nobody".into());
        format!("user {u} staff;\n")
    } else {
        String::new()
    };
    let conf = format!(
        "{user_line}\
         worker_processes auto;\n\
         pid {run}/nginx.pid;\n\
         error_log {logs}/nginx-error.log;\n\
         events {{ worker_connections 1024; }}\n\
         http {{\n\
         \x20 include {brew}/etc/nginx/mime.types;\n\
         \x20 default_type application/octet-stream;\n\
         \x20 sendfile on;\n\
         \x20 keepalive_timeout 65;\n\
         \x20 client_max_body_size 128M;\n\
         \x20 access_log {logs}/nginx-access.log;\n\
         \x20 include {etc}/nginx-sites/*.conf;\n\
         \x20 server {{\n\
         \x20\x20\x20 listen {port} default_server;\n\
         \x20\x20\x20 server_name localhost;\n\
         \x20\x20\x20 root {apps}/dashboard;\n\
         \x20\x20\x20 index index.html index.php;\n\
         \x20\x20\x20 location / {{ try_files $uri $uri/ =404; }}\n\
         \x20\x20\x20 location ~ \\.php$ {{ fastcgi_pass unix:{sock}; include {brew}/etc/nginx/fastcgi_params; fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name; }}\n\
         \x20 }}\n\
         }}\n",
        run = run_dir().display(),
        logs = logs_dir().display(),
        etc = etc_dir().display(),
        apps = apps_dir().display(),
        brew = brew,
        port = port,
        sock = php_sock(),
    );
    let path = etc_dir().join("nginx.conf");
    fs::write(&path, conf).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

// --- apache ----------------------------------------------------------------

fn write_apache_conf(port: u16) -> Result<String, String> {
    ensure_dirs();
    let brew = brew_prefix();
    let user = std::env::var("USER").unwrap_or_else(|_| "nobody".into());
    // brew httpd keeps modules under opt/httpd/lib/httpd/modules; ServerRoot
    // makes the relative LoadModule paths resolve.
    let modules = [
        "mpm_event_module lib/httpd/modules/mod_mpm_event.so",
        "authz_core_module lib/httpd/modules/mod_authz_core.so",
        "authz_host_module lib/httpd/modules/mod_authz_host.so",
        "dir_module lib/httpd/modules/mod_dir.so",
        "mime_module lib/httpd/modules/mod_mime.so",
        "log_config_module lib/httpd/modules/mod_log_config.so",
        "unixd_module lib/httpd/modules/mod_unixd.so",
        "proxy_module lib/httpd/modules/mod_proxy.so",
        "proxy_fcgi_module lib/httpd/modules/mod_proxy_fcgi.so",
        "rewrite_module lib/httpd/modules/mod_rewrite.so",
        "headers_module lib/httpd/modules/mod_headers.so",
    ];
    let mut load = String::new();
    for m in modules {
        load.push_str(&format!("LoadModule {}\n", m));
    }
    // Run as the real user only when we'll be root (privileged port).
    let user_dir = if port < 1024 {
        format!("User {user}\nGroup staff\n")
    } else {
        String::new()
    };
    let conf = format!(
        "ServerRoot \"{brew}/opt/httpd\"\n\
         Listen {port}\n\
         {load}\
         ServerName localhost\n\
         PidFile \"{run}/httpd.pid\"\n\
         ErrorLog \"{logs}/apache-error.log\"\n\
         {user_dir}\
         DocumentRoot \"{apps}/dashboard\"\n\
         DirectoryIndex index.php index.html\n\
         <Directory />\n\x20 AllowOverride none\n\x20 Require all denied\n</Directory>\n\
         <Directory \"{apps}\">\n\x20 AllowOverride All\n\x20 Require all granted\n</Directory>\n\
         <Directory \"{home}/Sites\">\n\x20 AllowOverride All\n\x20 Require all granted\n</Directory>\n\
         <FilesMatch \\.php$>\n\x20 SetHandler \"proxy:unix:{sock}|fcgi://localhost/\"\n</FilesMatch>\n\
         IncludeOptional {etc}/apache-sites/*.conf\n",
        brew = brew,
        port = port,
        load = load,
        run = run_dir().display(),
        logs = logs_dir().display(),
        user_dir = user_dir,
        apps = apps_dir().display(),
        home = home().display(),
        sock = php_sock(),
        etc = etc_dir().display(),
    );
    let path = etc_dir().join("httpd.conf");
    fs::write(&path, conf).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}


// ---------------------------------------------------------------------------
// Per-service model (for the compact service list in the UI)
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct Service {
    pub key: String,     // stable id: php | web | mariadb | redis | mailpit
    pub label: String,   // display name
    pub version: String, // installed version, if known
    pub installed: bool,
    pub running: bool,
    pub port: String,    // shown next to the row
}

/// The list of services shown in the dashboard, each independently toggleable.
pub fn list_services() -> Vec<Service> {
    let state = load_state();
    // Take ONE snapshot each instead of spawning a brew process per service.
    let vers = brew_versions_map();
    let started = brew_started_set();
    let inst = |f: &str| vers.contains_key(f);
    let ver = |f: &str| vers.get(f).cloned().unwrap_or_default();

    let web_formula = if state.web_server == "apache" { "httpd" } else { "nginx" };
    let php_formula = format!("php@{}", state.active_php);

    vec![
        Service {
            key: "web".into(),
            label: if state.web_server == "apache" { "Apache".into() } else { "Nginx".into() },
            version: ver(web_formula),
            installed: inst(web_formula),
            running: web_running(&state.web_server),
            port: state.http_port.to_string(),
        },
        Service {
            key: "php".into(),
            label: format!("PHP {}", state.active_php),
            version: ver(&php_formula),
            installed: inst(&php_formula),
            running: pid_alive(&run_dir().join("php-fpm.pid")),
            port: "fpm".into(),
        },
        Service {
            key: "mariadb".into(),
            label: "MySQL (MariaDB)".into(),
            version: ver("mariadb"),
            installed: inst("mariadb"),
            running: started.contains("mariadb"),
            port: "3306".into(),
        },
        Service {
            key: "redis".into(),
            label: "Redis".into(),
            version: ver("redis"),
            installed: inst("redis"),
            running: started.contains("redis"),
            port: "6379".into(),
        },
        Service {
            key: "mailpit".into(),
            label: "Mailpit".into(),
            version: ver("mailpit"),
            installed: inst("mailpit"),
            running: started.contains("mailpit"),
            port: "8025".into(),
        },
    ]
}


/// Start or stop a single service by key.
pub fn set_service(key: &str, on: bool) -> Result<String, String> {
    let state = load_state();
    match key {
        "php" => {
            if on { restart_php_fpm(&state.active_php)?; } else { stop_php_fpm(); }
        }
        "web" => {
            if on { start_web()?; } else { stop_web()?; }
        }
        "mariadb" => {
            if on { start_mariadb()?; } else { stop_mariadb()?; }
        }
        "redis" => {
            run(&brew_bin(), &["services", if on { "start" } else { "stop" }, "redis"])?;
        }
        "mailpit" => {
            run(&brew_bin(), &["services", if on { "start" } else { "stop" }, "mailpit"])?;
        }
        _ => return Err(format!("unknown service: {key}")),
    }
    Ok(format!("{key} {}", if on { "started" } else { "stopped" }))
}

// ---------------------------------------------------------------------------
// Combined lifecycle
// ---------------------------------------------------------------------------

pub fn seed_dashboard() {
    let dir = apps_dir().join("dashboard");
    let _ = fs::create_dir_all(&dir);
    let index = dir.join("index.html");
    if !index.exists() {
        let _ = fs::write(
            index,
            "<!doctype html><meta charset=utf-8><title>Tusk</title>\
             <body style=\"font-family:-apple-system,sans-serif;background:#0f1115;color:#e6e6e6;display:grid;place-items:center;height:100vh;margin:0\">\
             <div style=text-align:center><h1> · Tusk</h1><p>Your local dev environment is running.</p></div>",
        );
    }
}

pub fn start_all() -> Result<String, String> {
    ensure_dirs();
    seed_dashboard();
    let state = load_state();
    start_mariadb().ok();
    restart_php_fpm(&state.active_php)?;
    start_web()?;
    if brew_installed("redis") { let _ = set_service("redis", true); }
    if brew_installed("mailpit") { let _ = set_service("mailpit", true); }
    Ok("All services started".into())
}

pub fn stop_all() -> Result<String, String> {
    let _ = stop_web();
    stop_php_fpm();
    let _ = stop_mariadb();
    if brew_installed("redis") { let _ = set_service("redis", false); }
    if brew_installed("mailpit") { let _ = set_service("mailpit", false); }
    Ok("All services stopped".into())
}

pub fn restart_all() -> Result<String, String> {
    let _ = stop_all();
    start_all()
}

/// Which base-stack formulae are still missing (php@active, mariadb, nginx).
/// Apache (httpd) is intentionally NOT part of the base — it's installed
/// on demand only if the user switches to it, so first-run stays small.
pub fn base_missing_formulae() -> Vec<String> {
    let state = load_state();
    let mut to_install: Vec<String> = Vec::new();
    if !php::is_installed(&state.active_php) {
        to_install.push(format!("php@{}", state.active_php));
    }
    for f in ["mariadb", "nginx"] {
        if !brew_installed(f) {
            to_install.push(f.to_string());
        }
    }
    to_install
}


// ---------------------------------------------------------------------------
// MariaDB dev-mode root (Laragon-style: root / blank password over TCP)
// ---------------------------------------------------------------------------

/// Reconfigure MariaDB's root to use a blank password over TCP so GUI tools
/// like phpMyAdmin can log in as root with no password. This is the standard
/// local-dev convention (MAMP/Laragon do the same) and runs WITHOUT sudo,
/// because MariaDB's data dir is user-owned. Local dev only — never expose.
pub fn dev_reset_root() -> Result<String, String> {
    use std::thread::sleep;
    use std::time::Duration;

    let brew = brew_prefix();
    let mysqld_safe = format!("{brew}/opt/mariadb/bin/mariadbd-safe");
    let mysql = format!("{brew}/opt/mariadb/bin/mariadb");
    let mysqladmin = format!("{brew}/opt/mariadb/bin/mariadb-admin");
    let datadir = format!("{brew}/var/mysql");

    // 1. Stop the normal service and wait for it to exit.
    let _ = stop_mariadb();
    for _ in 0..15 {
        if !run_ok("pgrep", &["-f", "mariadbd"]) {
            break;
        }
        sleep(Duration::from_secs(1));
    }

    // 2. Start a temporary server with grants disabled and no networking.
    std::process::Command::new(&mysqld_safe)
        .args([
            &format!("--datadir={datadir}"),
            "--skip-grant-tables",
            "--skip-networking",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to start temp mariadb: {e}"))?;

    // 3. Wait until the socket answers.
    let mut up = false;
    for _ in 0..20 {
        if run_ok(&mysql, &["-u", "root", "-e", "SELECT 1"]) {
            up = true;
            break;
        }
        sleep(Duration::from_secs(1));
    }
    if !up {
        return Err("temporary MariaDB did not come up".into());
    }

    // 4. Set root to blank-password native auth (localhost + 127.0.0.1).
    let sql = "FLUSH PRIVILEGES;\
        ALTER USER 'root'@'localhost' IDENTIFIED VIA mysql_native_password USING PASSWORD('');\
        CREATE USER IF NOT EXISTS 'root'@'127.0.0.1' IDENTIFIED VIA mysql_native_password USING PASSWORD('');\
        GRANT ALL PRIVILEGES ON *.* TO 'root'@'127.0.0.1' WITH GRANT OPTION;\
        FLUSH PRIVILEGES;";
    run(&mysql, &["-u", "root", "-e", sql])?;

    // 5. Shut the temp server down and restart normally.
    let _ = run(&mysqladmin, &["-u", "root", "shutdown"]);
    for _ in 0..15 {
        if !run_ok("pgrep", &["-f", "mariadbd"]) {
            break;
        }
        sleep(Duration::from_secs(1));
    }
    start_mariadb()?;
    Ok("MariaDB root set to blank password (dev mode)".into())
}
