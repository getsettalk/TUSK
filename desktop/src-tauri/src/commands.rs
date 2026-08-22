//! Tauri command layer — thin, serializable wrappers over the engine.
//! Every command returns Result<_, String> so the frontend gets a clean error.

use crate::engine::{self, php, services, sites};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter};

/// Run a shell command and stream its combined output to the frontend as
/// "install-log" events (so the UI can show a live terminal-style progress).
fn stream_shell(app: &AppHandle, script: &str) -> Result<(), String> {
    let _ = app.emit("install-log", format!("$ {script}"));
    // Merge stderr into stdout so brew/pecl progress lines all come through.
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(format!("{script} 2>&1"))
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let _ = app.emit("install-log", line);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        let _ = app.emit("install-log", "✓ done".to_string());
        Ok(())
    } else {
        let _ = app.emit("install-log", "✗ failed (see output above)".to_string());
        Err("command failed — see log".into())
    }
}

// ---------------------------------------------------------------------------
// State / settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_state() -> engine::State {
    engine::load_state()
}

#[tauri::command]
pub fn accept_license() -> Result<(), String> {
    let mut s = engine::load_state();
    s.license_accepted = true;
    engine::save_state(&s)
}

#[tauri::command]
pub fn set_web_server(server: String) -> Result<(), String> {
    if server != "nginx" && server != "apache" {
        return Err("server must be 'nginx' or 'apache'".into());
    }
    let mut s = engine::load_state();
    // Don't allow switching the web server while one is running — stop it first.
    if services::web_running(&s.web_server) {
        return Err("Stop the web server before switching (Services → power off).".into());
    }
    s.web_server = server;
    engine::save_state(&s)
}

#[tauri::command]
pub fn set_port(port: u16) -> Result<(), String> {
    let mut s = engine::load_state();
    s.http_port = port;
    engine::save_state(&s)
}

// ---------------------------------------------------------------------------
// Services
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn svc_status() -> services::Status {
    services::status()
}
#[tauri::command]
pub fn list_services() -> Vec<services::Service> {
    services::list_services()
}
#[tauri::command]
pub fn set_service(key: String, on: bool) -> Result<String, String> {
    services::set_service(&key, on)
}
#[tauri::command]
pub fn install_service(app: AppHandle, key: String) -> Result<String, String> {
    // Resolve which brew formula this service needs.
    let state = engine::load_state();
    let formula = match key.as_str() {
        "web" => if state.web_server == "apache" { "httpd".to_string() } else { "nginx".to_string() },
        "mariadb" => "mariadb".into(),
        "redis" => "redis".into(),
        "mailpit" => "mailpit".into(),
        "php" => format!("php@{}", state.active_php),
        _ => return Err(format!("no installer for {key}")),
    };
    if engine::brew_installed(&formula) {
        return Ok(format!("{formula} already installed"));
    }
    stream_shell(&app, &format!("{} install {}", engine::brew_bin(), formula))?;
    Ok(format!("{formula} installed"))
}
#[tauri::command]
pub fn svc_start() -> Result<String, String> {
    services::start_all()
}
#[tauri::command]
pub fn svc_stop() -> Result<String, String> {
    services::stop_all()
}
#[tauri::command]
pub fn svc_restart() -> Result<String, String> {
    services::restart_all()
}
#[tauri::command]
pub fn install_base(app: AppHandle) -> Result<String, String> {
    let missing = services::base_missing_formulae();
    if missing.is_empty() {
        return Ok("Base stack already installed".into());
    }
    stream_shell(&app, &format!("{} install {}", engine::brew_bin(), missing.join(" ")))?;
    Ok("Base stack installed".into())
}
#[tauri::command]
pub fn install_apache(app: AppHandle) -> Result<String, String> {
    if engine::brew_installed("httpd") {
        return Ok("Apache already installed".into());
    }
    stream_shell(&app, &format!("{} install httpd", engine::brew_bin()))?;
    Ok("Apache installed".into())
}
#[tauri::command]
pub fn mariadb_dev_reset_root() -> Result<String, String> {
    services::dev_reset_root()
}

// ---------------------------------------------------------------------------
// PHP
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn php_list() -> Vec<php::PhpVersion> {
    php::list()
}
#[tauri::command]
pub fn php_install(app: AppHandle, version: String) -> Result<String, String> {
    if php::is_installed(&version) {
        return Ok(format!("php@{version} already installed"));
    }
    stream_shell(&app, &format!("{} install php@{}", engine::brew_bin(), version))?;
    Ok(format!("php@{version} installed"))
}
#[tauri::command]
pub fn php_uninstall(version: String) -> Result<String, String> {
    php::uninstall(&version)
}
#[tauri::command]
pub fn php_switch(version: String) -> Result<String, String> {
    php::switch(&version)
}
#[tauri::command]
pub fn php_extensions(version: String) -> Result<Vec<php::Extension>, String> {
    php::extensions(&version)
}
#[tauri::command]
pub fn php_set_extension(version: String, name: String, enable: bool) -> Result<String, String> {
    php::set_extension(&version, &name, enable)
}
#[tauri::command]
pub fn php_install_extension(app: AppHandle, version: String, name: String) -> Result<String, String> {
    // Some PECL extensions need system libraries; install those first.
    let deps: &[&str] = match name.to_lowercase().as_str() {
        "imagick" => &["pkg-config", "imagemagick"],
        "mongodb" => &["pkg-config", "openssl@3"],
        _ => &[],
    };
    for d in deps {
        if !engine::brew_installed(d) {
            stream_shell(&app, &format!("{} install {}", engine::brew_bin(), d))?;
        }
    }
    // Stream the (slow) PECL build.
    let pecl = php::pecl_bin(&version);
    let _ = stream_shell(&app, &format!("yes '' | '{}' install '{}'", pecl, name));
    // enable_installed_extension verifies the .so built before enabling.
    php::enable_installed_extension(&version, &name)
}
#[tauri::command]
pub fn php_ini_path(version: String) -> String {
    format!("{}/php.ini", php::php_ini_dir(&version))
}

// ---------------------------------------------------------------------------
// Sites
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn sites_list() -> Vec<sites::Site> {
    sites::list()
}
#[tauri::command]
pub fn site_add(name: String, docroot: String) -> Result<String, String> {
    sites::add(&name, &docroot)
}
#[tauri::command]
pub fn site_remove(name: String) -> Result<String, String> {
    sites::remove(&name)
}

// ---------------------------------------------------------------------------
// DNS (*.test) — privileged, one time
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn dns_setup() -> Result<String, String> {
    let brew = engine::brew_prefix();
    let state = engine::load_state();
    let tld = state.tld;
    if !engine::brew_installed("dnsmasq") {
        return Err("dnsmasq not installed. Install it first.".into());
    }
    // One privileged script: add dnsmasq mapping, (re)start it, write resolver.
    let script = format!(
        "grep -q 'address=/.{tld}/127.0.0.1' {brew}/etc/dnsmasq.conf || echo 'address=/.{tld}/127.0.0.1' >> {brew}/etc/dnsmasq.conf; \
         {brew}/bin/brew services restart dnsmasq; \
         mkdir -p /etc/resolver; \
         echo 'nameserver 127.0.0.1' > /etc/resolver/{tld}",
        tld = tld,
        brew = brew
    );
    engine::run_privileged(&script)?;
    Ok(format!("*.{tld} now resolves to 127.0.0.1"))
}

// ---------------------------------------------------------------------------
// phpMyAdmin
// ---------------------------------------------------------------------------

fn random_hex(bytes: usize) -> String {
    // 16 bytes -> 32 hex chars, enough for phpMyAdmin's blowfish_secret.
    let raw = fs::read(PathBuf::from("/dev/urandom")).unwrap_or_default();
    let slice: Vec<u8> = raw.into_iter().take(bytes).collect();
    slice.iter().map(|b| format!("{:02x}", b)).collect()
}

#[tauri::command]
pub fn phpmyadmin_install(app: AppHandle) -> Result<String, String> {
    let brew = engine::brew_prefix();
    if !engine::brew_installed("phpmyadmin") {
        stream_shell(&app, &format!("{} install phpmyadmin", engine::brew_bin()))?;
    }
    let cfg = format!("{brew}/etc/phpmyadmin.config.inc.php");

    // Always (re)write a working dev config: filled blowfish secret, TCP host,
    // AllowNoPassword so the blank-password dev root works.
    let secret = random_hex(16);
    let body = format!(
        "<?php\n\
         $cfg['blowfish_secret'] = '{secret}';\n\
         $i = 0;\n$i++;\n\
         $cfg['Servers'][$i]['auth_type'] = 'cookie';\n\
         $cfg['Servers'][$i]['host'] = '127.0.0.1';\n\
         $cfg['Servers'][$i]['port'] = '3306';\n\
         $cfg['Servers'][$i]['compress'] = false;\n\
         $cfg['Servers'][$i]['AllowNoPassword'] = true;\n\
         $cfg['UploadDir'] = '';\n\
         $cfg['SaveDir'] = '';\n",
        secret = secret
    );
    fs::write(&cfg, body).map_err(|e| e.to_string())?;

    // pma is a system site: regenerate vhosts so it appears on next web start.
    let st = engine::load_state();
    let _ = sites::generate_vhosts(&st.web_server, st.http_port, &st.tld);
    Ok("phpMyAdmin ready at pma.test (login root, blank password). Restart the web server if it was running.".into())
}

// ---------------------------------------------------------------------------
// OS integration: open folder / terminal / url
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    Command::new("open")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_terminal(path: String) -> Result<(), String> {
    let script = format!(
        "tell application \"Terminal\"\nactivate\ndo script \"cd '{}'\"\nend tell",
        path.replace('\'', "")
    );
    Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    Command::new("open")
        .arg(&url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn chowk_home_path() -> String {
    engine::chowk_home().display().to_string()
}
#[tauri::command]
pub fn sites_root_path() -> String {
    engine::www_dir().display().to_string()
}
#[tauri::command]
pub fn logs_path() -> String {
    engine::logs_dir().display().to_string()
}
#[tauri::command]
pub fn config_path() -> String {
    engine::etc_dir().display().to_string()
}

/// Open a file in the default text editor (macOS `open -t`).
#[tauri::command]
pub fn open_editor(path: String) -> Result<(), String> {
    Command::new("open")
        .args(["-t", &path])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// First-run convenience: install PHP + MariaDB + Nginx + Apache + phpMyAdmin.
#[tauri::command]
pub fn install_everything(app: AppHandle) -> Result<String, String> {
    let missing = services::base_missing_formulae();
    if !missing.is_empty() {
        stream_shell(&app, &format!("{} install {}", engine::brew_bin(), missing.join(" ")))?;
    }
    phpmyadmin_install(app)?;
    Ok("Base stack + phpMyAdmin installed".into())
}

/// System sites (localhost + phpMyAdmin) — always present, non-deletable.
#[tauri::command]
pub fn system_sites() -> Vec<sites::Site> {
    sites::system_sites()
}
