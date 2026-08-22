<div align="center">

<img src="docs/assets/logo.png" alt="Tusk" width="128" height="128" />

#  · Tusk

### The free, open-source local dev environment for macOS — PHP · MySQL · Nginx/Apache · phpMyAdmin, in one small app

A no-cost alternative to **MAMP PRO**, **Laragon**, and **Laravel Herd** — run and
switch **multiple PHP versions**, manage **MariaDB/MySQL**, **Nginx or Apache**, and
**phpMyAdmin** from a compact native app. No paywalls, no version locks.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Platform: macOS](https://img.shields.io/badge/platform-macOS-black)
![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%20v2-24c8db)

**by [getsettalk](https://github.com/getsettalk) · [github.com/getsettalk/tusk](https://github.com/getsettalk/tusk)**

</div>

---

## Why Tusk?

MAMP's free tier locks most PHP versions behind PRO. Laravel Herd is free but ships
no MySQL or phpMyAdmin. Laragon is Windows-only. **Tusk gives you the whole local
stack on macOS for free** by orchestrating components you install through
[Homebrew](https://brew.sh) — nothing bundled behind a paywall.

> Keywords: free MAMP alternative macOS · Laragon for Mac · Laravel Herd alternative
> · local PHP development environment · switch PHP versions macOS · MariaDB + phpMyAdmin
> · nginx / apache local server · WAMP/XAMPP for macOS.

## Features

- 🐘 **Multiple PHP versions** — install/switch 7.4 → 8.4 in one click; switches the
  terminal `php` **and** the web server together.
- 🟢 **Per-service control** — start/stop Nginx/Apache, PHP-FPM, MySQL, Redis, Mailpit
  individually, or all at once.
- 🔀 **Nginx or Apache** — pick your web server; site configs are generated for you.
- 🗄️ **MySQL (MariaDB) + phpMyAdmin** — bundled, at `pma.test`, ready to use.
- 🧩 **Extensions** — enable/disable, and download new ones via PECL (xdebug, redis,
  imagick, mongodb…) with a live build log.
- 🌐 **Pretty `.test` domains** — `myapp.test` via a one-time DNS helper.
- ⚙️ **Handy tools** — change ports, open `php.ini`/logs/config, jump to a site's
  folder or terminal — all from the app.

## Install

1. Download the latest **`Tusk_x.y.z_aarch64.dmg`** from
   [Releases](https://github.com/getsettalk/tusk/releases).
2. Open the `.dmg` and drag **Tusk** to `/Applications`.
3. Launch it, then **Settings → Install everything** and **Services → Start all**.

> First launch on an unsigned build: right-click the app → **Open** (macOS
> Gatekeeper), or *System Settings → Privacy & Security → Open Anyway*.

**Requirements:** macOS + [Homebrew](https://brew.sh).

## Usage

See the **[User Guide](docs/USAGE.md)** for a full walkthrough of every tab, where
files live (`php.ini`, logs, `~/Sites`), and troubleshooting.

Quick start:
- **Services** — toggle each service or Start/Stop all.
- **PHP** — Install a version, then **Use** to make it active (CLI + web).
- **Sites** — Add `myapp` → `http://myapp.test` served from `~/Sites/myapp`.
- **Ext** — toggle or download extensions.
- **Settings** — web server, ports, components, files & folders.

## Build from source

```bash
git clone https://github.com/getsettalk/tusk.git
cd tusk/desktop
npx @tauri-apps/cli build      # produces .app + .dmg in src-tauri/target/release/bundle
# or run in dev:
cd src-tauri && cargo run
```

Needs [Rust](https://rustup.rs) and Node. The frontend is dependency-light static
HTML/CSS/JS; the backend is Rust (Tauri v2) that orchestrates Homebrew.

There's also a **[CLI](desktop/README.md)** version of the engine for terminal users.

## Project layout

```
desktop/            Tauri app (the GUI)
  src-tauri/        Rust backend + engine
  ui/               static frontend
docs/USAGE.md       user guide
```

## Contributing

Issues and PRs welcome — it's built to be readable and hackable. See
[docs/USAGE.md](docs/USAGE.md) to understand the moving parts.

## License

[MIT](LICENSE) © [getsettalk](https://github.com/getsettalk)
