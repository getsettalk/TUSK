//! Tauri command layer — thin, serializable wrappers over the engine.
//! Every command returns Result<_, String> so the frontend gets a clean error.

use crate::engine::{self, php, services, sites};
use std::fs;
use std::process::Command;
use std::sync::Mutex;
use tauri::AppHandle;

/// Current install/download step. The frontend POLLS this via `install_status`.
/// We deliberately do NOT push Tauri events for progress: the JS-side
/// `event.listen` leaked memory unboundedly in this wry/WebKit build (grew to
/// ~2GB within a minute, and to 33GB during a long install → OOM). Polling a
/// plain command is leak-free (verified).
static INSTALL_STATUS: Mutex<String> = Mutex::new(String::new());

#[tauri::command]
pub fn install_status() -> String {
    INSTALL_STATUS.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Emit a SINGLE coarse status line to the UI (e.g. "Installing php@8.2 — 1/4…").
/// We deliberately DO NOT stream a command's stdout as events: brew/curl repaint
/// a `\r` progress bar thousands of times per second, and turning each repaint
/// into an IPC event ballooned memory to tens of GB (observed 33GB OOM). So the
/// UI shows step-based progress + an elapsed timer instead of a live log.
fn emit_status(_app: &AppHandle, msg: &str) {
    if let Ok(mut s) = INSTALL_STATUS.lock() {
        *s = msg.to_string();
    }
}

fn install_log_path() -> String {
    format!("{}/install.log", engine::logs_dir().display())
}

/// Run a shell command with ALL output redirected to a log FILE (on disk, not
/// events). Bounded memory. The user can open the log from Settings if needed.
fn run_logged(cmd: &str) -> Result<(), String> {
    let log = install_log_path();
    let full = format!("printf '\\n$ %s\\n' \"{cmd}\" >> '{log}'; {cmd} >> '{log}' 2>&1");
    let status = Command::new("sh")
        .arg("-c")
        .arg(full)
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .env("HOMEBREW_NO_INSTALL_CLEANUP", "1")
        .env("NONINTERACTIVE", "1")
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed — see log: {log}"))
    }
}

/// Run blocking work off the UI thread (Tauri sync commands run on the main
/// thread and would freeze the window). Await the result in an async command.
async fn run_blocking<F>(f: F) -> Result<String, String>
where
    F: FnOnce() -> Result<String, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
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
pub async fn list_services() -> Vec<services::Service> {
    // Off the main thread so its brew calls never freeze the UI.
    tauri::async_runtime::spawn_blocking(services::list_services)
        .await
        .unwrap_or_default()
}
#[tauri::command]
pub async fn set_service(key: String, on: bool) -> Result<String, String> {
    run_blocking(move || services::set_service(&key, on)).await
}
#[tauri::command]
pub async fn install_service(app: AppHandle, key: String) -> Result<String, String> {
    run_blocking(move || {
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
        emit_status(&app, &format!("Installing {formula}…"));
        run_logged(&format!("{} install {}", engine::brew_bin(), formula))?;
        Ok(format!("{formula} installed"))
    })
    .await
}
#[tauri::command]
pub async fn svc_start() -> Result<String, String> {
    run_blocking(services::start_all).await
}
#[tauri::command]
pub async fn svc_stop() -> Result<String, String> {
    run_blocking(services::stop_all).await
}
#[tauri::command]
pub async fn svc_restart() -> Result<String, String> {
    run_blocking(services::restart_all).await
}
#[tauri::command]
pub async fn install_base(app: AppHandle) -> Result<String, String> {
    run_blocking(move || {
        let missing = services::base_missing_formulae();
        if missing.is_empty() {
            return Ok("Base stack already installed".into());
        }
        let n = missing.len();
        for (i, f) in missing.iter().enumerate() {
            emit_status(&app, &format!("Installing {f} — {}/{n}…", i + 1));
            run_logged(&format!("{} install {}", engine::brew_bin(), f))?;
        }
        Ok("Base stack installed".into())
    })
    .await
}
#[tauri::command]
pub async fn install_apache(app: AppHandle) -> Result<String, String> {
    run_blocking(move || {
        if engine::brew_installed("httpd") {
            return Ok("Apache already installed".into());
        }
        emit_status(&app, "Installing Apache (httpd)…");
        run_logged(&format!("{} install httpd", engine::brew_bin()))?;
        Ok("Apache installed".into())
    })
    .await
}
#[tauri::command]
pub async fn mariadb_dev_reset_root() -> Result<String, String> {
    run_blocking(services::dev_reset_root).await
}

// ---------------------------------------------------------------------------
// PHP
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn php_list() -> Vec<php::PhpVersion> {
    php::list()
}
#[tauri::command]
pub async fn php_install(app: AppHandle, version: String) -> Result<String, String> {
    run_blocking(move || {
        if php::is_installed(&version) {
            return Ok(format!("php@{version} already installed"));
        }
        emit_status(&app, &format!("Downloading php@{version}…"));
        run_logged(&format!("{} install php@{}", engine::brew_bin(), version))?;
        Ok(format!("php@{version} installed"))
    })
    .await
}
#[tauri::command]
pub async fn php_uninstall(version: String) -> Result<String, String> {
    run_blocking(move || php::uninstall(&version)).await
}
#[tauri::command]
pub async fn php_switch(version: String) -> Result<String, String> {
    run_blocking(move || php::switch(&version)).await
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
pub async fn php_install_extension(app: AppHandle, version: String, name: String) -> Result<String, String> {
    run_blocking(move || {
        // Some PECL extensions need system libraries; install those first.
        let deps: &[&str] = match name.to_lowercase().as_str() {
            "imagick" => &["pkg-config", "imagemagick"],
            "mongodb" => &["pkg-config", "openssl@3"],
            _ => &[],
        };
        for d in deps {
            if !engine::brew_installed(d) {
                emit_status(&app, &format!("Installing dependency {d}…"));
                run_logged(&format!("{} install {}", engine::brew_bin(), d))?;
            }
        }
        // Build the (slow) PECL extension — output to the log file, not events.
        emit_status(&app, &format!("Building {name} (PECL)…"));
        let pecl = php::pecl_bin(&version);
        let _ = run_logged(&format!("yes '' | '{}' install '{}'", pecl, name));
        // enable_installed_extension verifies the .so built before enabling.
        php::enable_installed_extension(&version, &name)
    })
    .await
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
pub async fn dns_setup() -> Result<String, String> {
    run_blocking(move || {
        let brew = engine::brew_prefix();
        let state = engine::load_state();
        let tld = state.tld;
        if !engine::brew_installed("dnsmasq") {
            // dnsmasq isn't part of the base install; fetch it on demand.
            engine::run(&engine::brew_bin(), &["install", "dnsmasq"])?;
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
    })
    .await
}

// ---------------------------------------------------------------------------
// phpMyAdmin
// ---------------------------------------------------------------------------

fn random_hex(bytes: usize) -> String {
    // Read EXACTLY `bytes` bytes from /dev/urandom. NEVER use fs::read() here —
    // /dev/urandom is an endless stream, so fs::read() reads until it exhausts
    // memory (this was the real cause of the multi-GB / 41GB OOM crash).
    use std::io::Read;
    let mut buf = vec![0u8; bytes];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    }
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

#[tauri::command]
pub async fn phpmyadmin_install(app: AppHandle) -> Result<String, String> {
    run_blocking(move || phpmyadmin_install_blocking(&app)).await
}

fn phpmyadmin_install_blocking(app: &AppHandle) -> Result<String, String> {
    engine::ensure_dirs();
    let dir = engine::apps_dir().join("phpmyadmin");

    // Download the official package directly with curl (light, ~5MB, and — unlike
    // `brew install phpmyadmin` — pulls NO extra PHP dependency).
    if !dir.join("index.php").exists() {
        emit_status(app, "Downloading phpMyAdmin…");
        let tmp = engine::apps_dir().join("pma-download.tar.xz");
        let url = "https://www.phpmyadmin.net/downloads/phpMyAdmin-latest-english.tar.xz";
        run_logged(&format!(
            "curl -fL --retry 3 --retry-delay 2 --connect-timeout 20 -o '{}' '{}'",
            tmp.display(), url
        ))?;
        emit_status(app, "Extracting phpMyAdmin…");
        let stage = engine::apps_dir().join("pma-stage");
        let _ = fs::remove_dir_all(&stage);
        fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
        run_logged(&format!("tar -xf '{}' -C '{}'", tmp.display(), stage.display()))?;
        // The archive has a single top-level phpMyAdmin-* directory.
        let extracted = fs::read_dir(&stage)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.is_dir()
                && p.file_name().map(|n| n.to_string_lossy().starts_with("phpMyAdmin")).unwrap_or(false))
            .ok_or_else(|| "phpMyAdmin folder not found in archive".to_string())?;
        let _ = fs::remove_dir_all(&dir);
        fs::rename(&extracted, &dir).map_err(|e| e.to_string())?;
        let _ = fs::remove_dir_all(&stage);
        let _ = fs::remove_file(&tmp);
    }

    // Dev config: filled blowfish secret, TCP host, AllowNoPassword so the
    // blank-password dev root works.
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
    fs::write(dir.join("config.inc.php"), body).map_err(|e| e.to_string())?;

    // pma is a system site: regenerate vhosts so it appears on next web start.
    let st = engine::load_state();
    let _ = sites::generate_vhosts(&st.web_server, st.http_port, &st.tld);
    Ok("phpMyAdmin ready at pma.test (login root, blank password). Start/restart the web server.".into())
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

/// First-run convenience: install ONLY the essentials —
/// PHP (active version) + MariaDB + Nginx + phpMyAdmin. Apache and everything
/// else stay optional (installed later on demand), so first-run stays small.
#[tauri::command]
pub async fn install_everything(app: AppHandle) -> Result<String, String> {
    run_blocking(move || {
        let missing = services::base_missing_formulae(); // php@active, mariadb, nginx
        let total = missing.len() + 1; // + phpMyAdmin
        for (i, f) in missing.iter().enumerate() {
            emit_status(&app, &format!("Installing {f} — {}/{total}…", i + 1));
            run_logged(&format!("{} install {}", engine::brew_bin(), f))?;
        }
        emit_status(&app, &format!("Setting up phpMyAdmin — {total}/{total}…"));
        phpmyadmin_install_blocking(&app)?;
        Ok("Installed PHP + MariaDB + Nginx + phpMyAdmin".into())
    })
    .await
}

/// One-click first-run: pick a PHP version, then download + install + configure
/// + start EVERYTHING, in the right order so pma.test works immediately.
/// The user never has to open Settings.
#[tauri::command]
pub async fn first_run_setup(app: AppHandle, version: String) -> Result<String, String> {
    run_blocking(move || {
        if !engine::brew_present() {
            return Err(
                "Homebrew is required. Install it first from https://brew.sh, then reopen Tusk."
                    .into(),
            );
        }
        // 1. Record the chosen PHP version so the base install fetches it.
        let mut st = engine::load_state();
        st.active_php = version.clone();
        engine::save_state(&st)?;

        // 2. Install base stack (php@version, mariadb, nginx) one-by-one.
        let missing = services::base_missing_formulae();
        let total = missing.len() + 3; // + phpMyAdmin, DNS, start
        for (i, f) in missing.iter().enumerate() {
            emit_status(&app, &format!("Installing {f} — step {}/{total}…", i + 1));
            run_logged(&format!("{} install {}", engine::brew_bin(), f))?;
        }

        // 3. phpMyAdmin (curl download).
        emit_status(&app, "Setting up phpMyAdmin…");
        phpmyadmin_install_blocking(&app)?;

        // 4. Pretty .test domains (needs one admin prompt).
        emit_status(&app, "Setting up .test domains (enter your Mac password)…");
        let brew = engine::brew_prefix();
        let tld = engine::load_state().tld;
        if !engine::brew_installed("dnsmasq") {
            run_logged(&format!("{} install dnsmasq", engine::brew_bin()))?;
        }
        let dns_script = format!(
            "grep -q 'address=/.{tld}/127.0.0.1' {brew}/etc/dnsmasq.conf || echo 'address=/.{tld}/127.0.0.1' >> {brew}/etc/dnsmasq.conf; \
             {brew}/bin/brew services restart dnsmasq; mkdir -p /etc/resolver; echo 'nameserver 127.0.0.1' > /etc/resolver/{tld}",
            tld = tld, brew = brew
        );
        let _ = engine::run_privileged(&dns_script);

        // 5. Start services (this regenerates vhosts incl. pma, then starts
        //    nginx on :80 — one more admin prompt).
        emit_status(&app, "Starting services (enter your Mac password)…");
        services::start_all()?;

        // 6. Make MariaDB root usable by phpMyAdmin (blank password dev root).
        emit_status(&app, "Configuring database…");
        let _ = services::dev_reset_root();

        // 7. Mark setup complete so the app opens straight to the dashboard next time.
        let mut st = engine::load_state();
        st.setup_done = true;
        engine::save_state(&st)?;
        Ok("Setup complete — http://localhost and http://pma.test are ready.".into())
    })
    .await
}


/// System sites (localhost + phpMyAdmin) — always present, non-deletable.
#[tauri::command]
pub fn system_sites() -> Vec<sites::Site> {
    sites::system_sites()
}
