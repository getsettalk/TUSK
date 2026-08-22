//! Sites: stored as a simple JSON list of {name, docroot}. Server-specific
//! vhost files are generated on demand, so the same sites work whether the
//! user runs nginx or apache.

use super::*;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct Site {
    pub name: String,
    pub docroot: String,
}

pub fn list() -> Vec<Site> {
    match fs::read_to_string(sites_json()) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save(sites: &[Site]) -> Result<(), String> {
    ensure_dirs();
    let json = serde_json::to_string_pretty(sites).map_err(|e| e.to_string())?;
    fs::write(sites_json(), json).map_err(|e| e.to_string())
}

/// Reserved names for system sites the user can't create or delete.
pub const RESERVED: &[&str] = &["localhost", "pma"];

/// System sites (always present, non-deletable): the dashboard + phpMyAdmin.
pub fn system_sites() -> Vec<Site> {
    let mut v = vec![Site {
        name: "localhost".into(),
        docroot: apps_dir().join("dashboard").display().to_string(),
    }];
    if let Some(pma) = phpmyadmin_docroot() {
        v.push(Site { name: "pma".into(), docroot: pma });
    }
    v
}

/// Add a site. If docroot is empty, create ~/Sites/<name> with a placeholder.
pub fn add(name: &str, docroot: &str) -> Result<String, String> {
    let name = name.trim().trim_end_matches(".test").to_string();
    if name.is_empty() {
        return Err("Site name required".into());
    }
    if RESERVED.contains(&name.as_str()) {
        return Err(format!("'{name}' is a reserved system site"));
    }
    let root = if docroot.trim().is_empty() {
        let d = www_dir().join(&name);
        fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        let index = d.join("index.php");
        if !index.exists() {
            let _ = fs::write(index, "<?php phpinfo();\n");
        }
        d.display().to_string()
    } else {
        docroot.trim().to_string()
    };

    let mut sites = list();
    if sites.iter().any(|s| s.name == name) {
        return Err(format!("Site '{name}' already exists"));
    }
    sites.push(Site { name: name.clone(), docroot: root });
    save(&sites)?;

    // Regenerate vhosts + reload the running server.
    let state = load_state();
    generate_vhosts(&state.web_server, state.http_port, &state.tld)?;
    reload_web(&state.web_server, state.http_port);
    Ok(format!("Site added: {name}.{}", state.tld))
}

pub fn remove(name: &str) -> Result<String, String> {
    let name = name.trim().trim_end_matches(".test").to_string();
    let mut sites = list();
    let before = sites.len();
    sites.retain(|s| s.name != name);
    if sites.len() == before {
        return Err(format!("Site '{name}' not found"));
    }
    save(&sites)?;
    let state = load_state();
    generate_vhosts(&state.web_server, state.http_port, &state.tld)?;
    reload_web(&state.web_server, state.http_port);
    Ok(format!("Site removed: {name}"))
}

/// Write per-site vhost files for the chosen server into etc/<server>-sites/.
pub fn generate_vhosts(server: &str, port: u16, tld: &str) -> Result<(), String> {
    ensure_dirs();
    let brew = brew_prefix();
    let sock = services::php_sock();
    let dir = etc_dir().join(format!("{server}-sites"));
    // Start clean so removed sites disappear.
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // User sites + system sites (phpMyAdmin) — the latter are always present
    // when installed and can't be deleted by the user.
    let mut all = list();
    if let Some(pma) = phpmyadmin_docroot() {
        all.push(Site { name: "pma".into(), docroot: pma });
    }

    for site in all {
        let server_name = format!("{}.{}", site.name, tld);
        let body = match server {
            "apache" => format!(
                "<VirtualHost *:{port}>\n\
                 \x20 ServerName {server_name}\n\
                 \x20 DocumentRoot \"{root}\"\n\
                 \x20 <Directory \"{root}\">\n\
                 \x20\x20\x20 AllowOverride All\n\
                 \x20\x20\x20 Require all granted\n\
                 \x20\x20\x20 DirectoryIndex index.php index.html\n\
                 \x20 </Directory>\n\
                 \x20 <FilesMatch \\.php$>\n\
                 \x20\x20\x20 SetHandler \"proxy:unix:{sock}|fcgi://localhost/\"\n\
                 \x20 </FilesMatch>\n\
                 </VirtualHost>\n",
                port = port,
                server_name = server_name,
                root = site.docroot,
                sock = sock,
            ),
            _ => format!(
                "server {{\n\
                 \x20 listen {port};\n\
                 \x20 server_name {server_name};\n\
                 \x20 root {root};\n\
                 \x20 index index.php index.html index.htm;\n\
                 \x20 location / {{ try_files $uri $uri/ /index.php?$query_string; }}\n\
                 \x20 location ~ \\.php$ {{\n\
                 \x20\x20\x20 try_files $uri =404;\n\
                 \x20\x20\x20 fastcgi_split_path_info ^(.+\\.php)(/.+)$;\n\
                 \x20\x20\x20 fastcgi_pass unix:{sock};\n\
                 \x20\x20\x20 fastcgi_index index.php;\n\
                 \x20\x20\x20 include {brew}/etc/nginx/fastcgi_params;\n\
                 \x20\x20\x20 fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\n\
                 \x20 }}\n\
                 }}\n",
                port = port,
                server_name = server_name,
                root = site.docroot,
                sock = sock,
                brew = brew,
            ),
        };
        let file = dir.join(format!("{}.conf", site.name));
        fs::write(file, body).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn reload_web(server: &str, port: u16) {
    if !services::web_running(server) {
        return;
    }
    let brew = brew_prefix();
    match server {
        "apache" => {
            let conf = etc_dir().join("httpd.conf");
            let cmd = format!("{}/bin/httpd -f {} -k graceful", brew, conf.display());
            if port < 1024 { let _ = run_privileged(&cmd); }
            else { let _ = run(&format!("{brew}/bin/httpd"), &["-f", &conf.display().to_string(), "-k", "graceful"]); }
        }
        _ => {
            let conf = etc_dir().join("nginx.conf");
            let cmd = format!("{}/bin/nginx -c {} -s reload", brew, conf.display());
            if port < 1024 { let _ = run_privileged(&cmd); }
            else { let _ = run(&format!("{brew}/bin/nginx"), &["-c", &conf.display().to_string(), "-s", "reload"]); }
        }
    }
}
