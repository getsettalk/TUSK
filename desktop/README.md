# Chowk Desktop (Tauri GUI)

A native macOS app for Chowk — manage PHP versions, MariaDB, Nginx/Apache,
sites, extensions and phpMyAdmin from one window. Built with **Tauri v2**
(Rust backend + a dependency-light static HTML/CSS/JS frontend).

## Architecture

```
desktop/
├── src-tauri/                 # Rust backend
│   ├── src/
│   │   ├── main.rs            # Tauri builder, registers commands
│   │   ├── commands.rs        # thin #[tauri::command] wrappers
│   │   └── engine/            # all real logic
│   │       ├── mod.rs         # paths, state, brew helpers, command runners
│   │       ├── php.rs         # list / install / switch / extensions
│   │       ├── services.rs    # php-fpm, nginx, apache, mariadb orchestration
│   │       └── sites.rs       # sites (JSON store) + per-server vhost gen
│   ├── capabilities/default.json
│   ├── tauri.conf.json
│   └── Cargo.toml
└── ui/                        # frontend (no build step)
    ├── index.html
    ├── styles.css
    └── app.js                 # talks to Rust via window.__TAURI__.core.invoke
```

The engine shells out to the same Homebrew components the CLI uses and shares
the `~/.chowk` home, so the GUI and `chowk` CLI interoperate.

## Prerequisites

- [Rust](https://rustup.rs) (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Homebrew
- macOS (WKWebView is system-provided; no extra webview install)

## Develop / run

```bash
cd desktop/src-tauri
cargo run
```

`tauri-build` embeds `../ui` into the binary, so `cargo run` launches the full
app — no separate dev server or JS bundler needed.

## Design choices

- **One active php-fpm** on a fixed unix socket; switching version just
  restarts it, so web-server config never changes.
- **Privileged ports** (`<1024`, e.g. 80) are started through a native macOS
  admin prompt (`osascript`), so the app never needs to run as root.
- **Nginx or Apache** — the user picks in Settings; site vhosts are generated
  for whichever server is active.
- **Bundled base, dynamic extras** — base stack is expected via Homebrew;
  extra PHP versions and phpMyAdmin download on demand.

## Roadmap

- [ ] Bundle to `.app` / `.dmg` (`cargo tauri build`; needs `.icns`)
- [ ] Per-site PHP version
- [ ] HTTPS via mkcert
- [ ] Self-update
- [ ] Windows support
