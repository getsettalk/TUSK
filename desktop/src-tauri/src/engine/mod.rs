//! Tusk engine — all the real work lives here, split by concern.
//! The Tauri command layer (`commands.rs`) is a thin wrapper over these.
//!
//! Design notes:
//! * Everything user-configurable is persisted in `~/.chowk/desktop-state.json`.
//! * Components (PHP, MariaDB, Nginx, Apache) are orchestrated over Homebrew —
//!   the engine shells out to the same binaries the CLI uses, so the GUI and
//!   the `chowk` CLI share one `~/.chowk` home and interoperate.
//! * Anything that needs to bind a privileged port (<1024) is run through a
//!   native macOS admin prompt (osascript), so the app never needs to be
//!   launched as root.

pub mod php;
pub mod services;
pub mod sites;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// PHP versions Tusk knows how to install via Homebrew, newest first.
/// This list is the source for the "install a new version" dropdown; adding a
/// future release here is all that's needed to offer it.
pub const AVAILABLE_PHP: &[&str] = &["8.4", "8.3", "8.2", "8.1", "8.0", "7.4"];

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
}

pub fn chowk_home() -> PathBuf {
    home().join(".chowk")
}
pub fn etc_dir() -> PathBuf { chowk_home().join("etc") }
pub fn logs_dir() -> PathBuf { chowk_home().join("logs") }
pub fn run_dir() -> PathBuf { chowk_home().join("run") }
pub fn apps_dir() -> PathBuf { chowk_home().join("apps") }
pub fn sites_json() -> PathBuf { etc_dir().join("sites.json") }
pub fn state_file() -> PathBuf { chowk_home().join("desktop-state.json") }

/// Default parent folder for new site document roots (~/Sites).
pub fn www_dir() -> PathBuf { home().join("Sites") }

/// Where phpMyAdmin lives, if installed: our curl-downloaded copy under
/// ~/.chowk/apps/phpmyadmin takes precedence, else a Homebrew install.
pub fn phpmyadmin_docroot() -> Option<String> {
    let local = apps_dir().join("phpmyadmin");
    if local.join("index.php").exists() {
        return Some(local.display().to_string());
    }
    if brew_installed("phpmyadmin") {
        return Some(format!("{}/share/phpmyadmin", brew_prefix()));
    }
    None
}

pub fn ensure_dirs() {
    for d in [etc_dir(), logs_dir(), run_dir(), apps_dir(), www_dir()] {
        let _ = fs::create_dir_all(d);
    }
}

// ---------------------------------------------------------------------------
// Homebrew
// ---------------------------------------------------------------------------

/// Resolve the Homebrew prefix (/opt/homebrew on Apple Silicon, /usr/local on Intel).
pub fn brew_prefix() -> String {
    run("brew", &["--prefix"])
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "/opt/homebrew".to_string())
}

pub fn brew_bin() -> String {
    format!("{}/bin/brew", brew_prefix())
}

/// Is a Homebrew formula installed?
pub fn brew_installed(formula: &str) -> bool {
    Command::new(brew_bin())
        .args(["list", formula])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Installed version of a Homebrew formula, e.g. "1.27.3" (empty if absent).
pub fn brew_version(formula: &str) -> String {
    run(&brew_bin(), &["list", "--versions", formula])
        .ok()
        .and_then(|s| s.split_whitespace().nth(1).map(|v| v.to_string()))
        .unwrap_or_default()
}

/// Is a brew *service* currently started?
pub fn brew_service_running(name: &str) -> bool {
    run(&brew_bin(), &["services", "list"])
        .map(|s| {
            s.lines()
                .any(|l| l.starts_with(name) && l.contains("started"))
        })
        .unwrap_or(false)
}

/// ONE `brew list --versions` -> map of installed formula -> version.
/// Lets callers avoid spawning a separate brew process per formula (that
/// per-service spawning, polled every few seconds, caused a runaway).
pub fn brew_versions_map() -> std::collections::HashMap<String, String> {
    let mut m = std::collections::HashMap::new();
    if let Ok(out) = run(&brew_bin(), &["list", "--versions"]) {
        for line in out.lines() {
            let mut it = line.split_whitespace();
            if let Some(name) = it.next() {
                m.insert(name.to_string(), it.next().unwrap_or("").to_string());
            }
        }
    }
    m
}

/// ONE `brew services list` -> set of started service names.
pub fn brew_started_set() -> std::collections::HashSet<String> {
    let mut s = std::collections::HashSet::new();
    if let Ok(out) = run(&brew_bin(), &["services", "list"]) {
        for line in out.lines() {
            if line.contains("started") {
                if let Some(name) = line.split_whitespace().next() {
                    s.insert(name.to_string());
                }
            }
        }
    }
    s
}

// ---------------------------------------------------------------------------
// Command runners
// ---------------------------------------------------------------------------

/// Run a command, returning stdout on success or a stderr-based error string.
pub fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("failed to spawn {cmd}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        Err(format!("{cmd} failed: {}{}", err.trim(), stdout.trim()))
    }
}

/// Run and only care whether it succeeded.
pub fn run_ok(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run a shell command line with a native macOS administrator prompt.
/// Used only for privileged actions (binding port <1024, editing /etc/resolver).
pub fn run_privileged(script: &str) -> Result<String, String> {
    // Escape backslashes and double quotes for the AppleScript string literal.
    let escaped = script.replace('\\', "\\\\").replace('"', "\\\"");
    let apple = format!(
        "do shell script \"{}\" with administrator privileges",
        escaped
    );
    let out = Command::new("osascript")
        .args(["-e", &apple])
        .output()
        .map_err(|e| format!("osascript spawn failed: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

/// True if a pid file exists and the process is alive.
/// Uses `ps -p` (not `kill -0`) so it also detects root-owned processes we
/// started via the admin prompt (a normal user can't signal a root process,
/// but can still see it with ps).
pub fn pid_alive(pid_file: &PathBuf) -> bool {
    if let Ok(s) = fs::read_to_string(pid_file) {
        if let Ok(pid) = s.trim().parse::<i32>() {
            return run_ok("ps", &["-p", &pid.to_string(), "-o", "pid="]);
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Persistent state
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct State {
    /// Has the user accepted the licenses/terms on first run?
    pub license_accepted: bool,
    /// Which web server to run: "nginx" or "apache".
    pub web_server: String,
    /// HTTP port for the web server (80 = pretty URLs, needs admin once).
    pub http_port: u16,
    /// Currently active PHP version, e.g. "8.3".
    pub active_php: String,
    /// TLD used for local sites (e.g. "test" -> myapp.test).
    pub tld: String,
}

impl Default for State {
    fn default() -> Self {
        State {
            license_accepted: false,
            web_server: "nginx".into(),
            http_port: 80,
            active_php: "8.2".into(),
            tld: "test".into(),
        }
    }
}

pub fn load_state() -> State {
    ensure_dirs();
    match fs::read_to_string(state_file()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => State::default(),
    }
}

pub fn save_state(state: &State) -> Result<(), String> {
    ensure_dirs();
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    fs::write(state_file(), json).map_err(|e| e.to_string())
}
