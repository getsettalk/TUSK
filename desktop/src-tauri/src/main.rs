// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod engine;

fn main() {
    engine::ensure_dirs();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // state / settings
            commands::get_state,
            commands::accept_license,
            commands::set_web_server,
            commands::set_port,
            // services
            commands::svc_status,
            commands::list_services,
            commands::set_service,
            commands::install_service,
            commands::install_status,
            commands::svc_start,
            commands::svc_stop,
            commands::svc_restart,
            commands::install_base,
            commands::install_apache,
            commands::install_everything,
            commands::mariadb_dev_reset_root,
            // php
            commands::php_list,
            commands::php_install,
            commands::php_uninstall,
            commands::php_switch,
            commands::php_extensions,
            commands::php_set_extension,
            commands::php_install_extension,
            commands::php_ini_path,
            // sites
            commands::sites_list,
            commands::system_sites,
            commands::site_add,
            commands::site_remove,
            // dns / pma
            commands::dns_setup,
            commands::phpmyadmin_install,
            // os integration
            commands::open_folder,
            commands::open_terminal,
            commands::open_url,
            commands::open_editor,
            commands::chowk_home_path,
            commands::sites_root_path,
            commands::logs_path,
            commands::config_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tusk");
}
