//! PHP: list/install/switch versions and toggle extensions.
//!
//! Version model: exactly one php-fpm runs at a time (the active version),
//! listening on a fixed unix socket, so switching version = restart php-fpm
//! with a different binary. Web-server config never has to change.

use super::*;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Clone)]
pub struct PhpVersion {
    pub version: String,
    pub installed: bool,
    pub active: bool,
    /// Full "8.3.33" if installed, else empty.
    pub full: String,
}

#[derive(Serialize, Clone)]
pub struct Extension {
    pub name: String,
    pub enabled: bool,
    /// Compiled-in extensions can't be disabled via ini.
    pub builtin: bool,
}

pub fn php_bin(version: &str) -> String {
    format!("{}/opt/php@{}/bin/php", brew_prefix(), version)
}
pub fn php_fpm_bin(version: &str) -> String {
    format!("{}/opt/php@{}/sbin/php-fpm", brew_prefix(), version)
}
pub fn php_ini_dir(version: &str) -> String {
    format!("{}/etc/php/{}", brew_prefix(), version)
}
pub fn conf_d(version: &str) -> String {
    format!("{}/etc/php/{}/conf.d", brew_prefix(), version)
}

pub fn is_installed(version: &str) -> bool {
    PathBuf::from(php_bin(version)).exists()
}

fn full_version(version: &str) -> String {
    run(&php_bin(version), &["-r", "echo PHP_VERSION;"])
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// List every version Tusk knows about, marking installed/active.
pub fn list() -> Vec<PhpVersion> {
    let active = load_state().active_php;
    AVAILABLE_PHP
        .iter()
        .map(|v| {
            let installed = is_installed(v);
            PhpVersion {
                version: v.to_string(),
                installed,
                active: *v == active,
                full: if installed { full_version(v) } else { String::new() },
            }
        })
        .collect()
}

/// Uninstall a PHP version.
pub fn uninstall(version: &str) -> Result<String, String> {
    if !is_installed(version) {
        return Ok(format!("php@{version} not installed"));
    }
    run(&brew_bin(), &["uninstall", "--ignore-dependencies", &format!("php@{version}")])?;
    Ok(format!("php@{version} removed"))
}

/// Switch the active PHP version: persist choice, relink the CLI `php`, and
/// restart php-fpm. After this, both the terminal `php` and the web server use
/// the chosen version.
pub fn switch(version: &str) -> Result<String, String> {
    if !is_installed(version) {
        return Err(format!("php@{version} is not installed"));
    }
    let mut state = load_state();
    state.active_php = version.to_string();
    save_state(&state)?;

    relink_cli(version); // best-effort: makes `php` in the terminal match
    // Restart php-fpm on the shared socket with the new binary.
    services::restart_php_fpm(version)?;
    Ok(format!("Active PHP -> {version} (CLI + web)"))
}

/// Point Homebrew's `php` (and phpize/php-config/pecl/pear) at the given
/// version by unlinking every other php and force-linking this one.
/// Best-effort: switching still succeeds for the web server even if linking
/// fails (e.g. Homebrew permission quirks).
fn relink_cli(active: &str) {
    // Unlink the unversioned formula and every versioned one first.
    let _ = run(&brew_bin(), &["unlink", "php"]);
    for v in AVAILABLE_PHP {
        let _ = run(&brew_bin(), &["unlink", &format!("php@{v}")]);
    }
    // php@X.Y are keg-only, so force + overwrite are required to link them.
    let _ = run(
        &brew_bin(),
        &["link", "--overwrite", "--force", &format!("php@{active}")],
    );
}

// ---------------------------------------------------------------------------
// Extensions
// ---------------------------------------------------------------------------

/// List extensions for a version: those reported by `php -m` (enabled), plus
/// any Tusk-managed disabled ones parked in conf.d as *.ini.disabled.
pub fn extensions(version: &str) -> Result<Vec<Extension>, String> {
    if !is_installed(version) {
        return Err(format!("php@{version} is not installed"));
    }
    // Suppress startup/errors so a broken extension can't leak warning text
    // into the module list.
    let modules = run(
        &php_bin(version),
        &["-d", "display_errors=0", "-d", "display_startup_errors=0", "-d", "error_reporting=0", "-m"],
    )?;
    let mut list: Vec<Extension> = Vec::new();
    for line in modules.lines() {
        let name = line.trim();
        // Keep only real module names; drop section headers and any stray
        // warning/error text (which contains paths, parens, quotes, etc.).
        if name.is_empty()
            || name.starts_with('[')
            || name.len() > 40
            || name.contains(['/', '(', ')', '\'', '"', ':', '<', '>'])
            || name.starts_with("Warning")
            || name.starts_with("Deprecated")
            || name.starts_with("Fatal")
            || name.starts_with("PHP ")
        {
            continue;
        }
        // An extension is "builtin" (not ini-toggleable) unless we own a
        // conf.d file enabling it.
        let managed = PathBuf::from(format!("{}/ext-{}.ini", conf_d(version), name.to_lowercase())).exists();
        list.push(Extension {
            name: name.to_string(),
            enabled: true,
            builtin: !managed,
        });
    }
    // Add our disabled (parked) extensions so they can be re-enabled.
    if let Ok(entries) = fs::read_dir(conf_d(version)) {
        for e in entries.flatten() {
            let fname = e.file_name().to_string_lossy().to_string();
            if let Some(ext) = fname.strip_prefix("ext-").and_then(|s| s.strip_suffix(".ini.disabled")) {
                list.push(Extension {
                    name: ext.to_string(),
                    enabled: false,
                    builtin: false,
                });
            }
        }
    }
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(list)
}

/// PECL binary path for a version (used by the streaming command layer).
pub fn pecl_bin(version: &str) -> String {
    format!("{}/opt/php@{}/bin/pecl", brew_prefix(), version)
}

/// After a PECL build, write the conf.d ini and reload php-fpm. Split out so
/// the command layer can stream the (slow) build separately.
/// Refuses to enable if the compiled `.so` isn't present (build failed), so we
/// never leave PHP emitting "Unable to load dynamic library" on every run.
pub fn enable_installed_extension(version: &str, name: &str) -> Result<String, String> {
    let lower = name.to_lowercase();
    // Resolve the extension_dir and verify the .so actually built.
    let ext_dir = run(
        &php_bin(version),
        &["-d", "error_reporting=0", "-r", "echo ini_get('extension_dir');"],
    )
    .unwrap_or_default();
    let so = format!("{}/{}.so", ext_dir.trim(), lower);
    if !PathBuf::from(&so).exists() {
        return Err(format!(
            "Build failed: {lower}.so was not created. It likely needs a system library \
             (e.g. imagick needs ImageMagick). Nothing was enabled."
        ));
    }

    // PECL sometimes appends `extension="<name>.so"` to the main php.ini itself.
    // Strip any such line so our conf.d entry isn't a duplicate ("already loaded").
    let ini_path = format!("{}/php.ini", php_ini_dir(version));
    if let Ok(ini) = fs::read_to_string(&ini_path) {
        let cleaned: String = ini
            .lines()
            .filter(|l| {
                let t = l.trim().to_lowercase();
                !(t.contains(&lower)
                    && (t.starts_with("extension=") || t.starts_with("zend_extension=")))
            })
            .collect::<Vec<_>>()
            .join("\n");
        if cleaned != ini {
            let _ = fs::write(&ini_path, cleaned + "\n");
        }
    }

    // xdebug/opcache load as zend_extension; everything else as extension.
    let dir = conf_d(version);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let directive = if lower == "xdebug" || lower == "opcache" { "zend_extension" } else { "extension" };
    fs::write(
        format!("{}/ext-{}.ini", dir, lower),
        format!("{}={}\n", directive, lower),
    )
    .map_err(|e| e.to_string())?;

    if load_state().active_php == version {
        let _ = services::restart_php_fpm(version);
    }
    Ok(format!("{name} installed & enabled for php@{version}"))
}

/// Enable or disable a loadable extension by managing a conf.d ini file.
/// Note: extensions compiled into the PHP binary can't be disabled this way.
pub fn set_extension(version: &str, name: &str, enable: bool) -> Result<String, String> {
    let dir = conf_d(version);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let lower = name.to_lowercase();
    let active_ini = PathBuf::from(format!("{}/ext-{}.ini", dir, lower));
    let parked_ini = PathBuf::from(format!("{}/ext-{}.ini.disabled", dir, lower));

    if enable {
        let body = format!("extension={}\n", name);
        fs::write(&active_ini, body).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&parked_ini);
    } else {
        // Park our ini if we own one; built-ins can't be disabled.
        if active_ini.exists() {
            fs::rename(&active_ini, &parked_ini).map_err(|e| e.to_string())?;
        } else {
            return Err(format!(
                "'{name}' is compiled into php@{version} and can't be disabled via ini."
            ));
        }
    }
    // Apply live if this version is active.
    let state = load_state();
    if state.active_php == version {
        let _ = services::restart_php_fpm(version);
    }
    Ok(format!("{} {}", name, if enable { "enabled" } else { "disabled" }))
}
