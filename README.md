<div align="center">

<img src="docs/assets/logo.png" alt="Tusk" width="128" height="128" />

# Tusk

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

- 🐘 **Multiple PHP versions** — every branch Homebrew offers (8.1 → 8.5 today; new
  releases appear automatically). Install/switch in one click; switches the
  terminal `php` **and** the web server together.
- 🔒 **Security updates** — when you open Tusk it checks for newer PHP patch
  releases and shows an **Update** banner; one click upgrades, restarts PHP and
  removes the old build. Installs always fetch the latest patch.
- ⚡ **One-click setup** — on first launch pick a PHP version and press
  **Setup & Install**; Tusk installs and starts PHP, MySQL, Nginx and phpMyAdmin.
- 🟢 **Per-service control** — start/stop Nginx/Apache, PHP-FPM, MySQL, Redis, Mailpit
  individually, or all at once.
- 🔀 **Nginx or Apache** — pick your web server; site configs are generated for you.
- 🗄️ **MySQL (MariaDB) + phpMyAdmin** — set up for you at `pma.test` (login `root`, blank password).
- 🧩 **Extensions** — enable/disable, and download new ones via PECL (xdebug, redis,
  imagick, mongodb…) with a live build log.
- 🌐 **Pretty `.test` domains** — `myapp.test` via a one-time DNS helper.
- ⚙️ **Handy tools** — change ports, open `php.ini`/logs/config, jump to a site's
  folder or terminal — all from the app.
- 🪟 **One window** — opening Tusk again just brings the running window to the front.

## Install

**Recommended — one line** (downloads the latest release into /Applications and
avoids the unsigned-app "damaged" Gatekeeper error):

```bash
curl -fsSL https://raw.githubusercontent.com/getsettalk/tusk/main/install.sh | bash
```

Then open **Tusk**, accept the terms, and click **⚡ Setup & Install** (pick PHP 8.2).

**Updating Tusk:** run the same command again — it always installs the latest release.

<details>
<summary>Manual download instead</summary>

1. Download **`Tusk_x.y.z_aarch64.dmg`** from [Releases](https://github.com/getsettalk/tusk/releases), open it, drag **Tusk** to `/Applications`.
2. Because the app isn't Apple-notarized, macOS shows *"Tusk is damaged"*. Clear the download quarantine once:
   ```bash
   xattr -dr com.apple.quarantine /Applications/Tusk.app
   ```
3. Open Tusk.
</details>

**Requirements:** macOS (Apple Silicon) + [Homebrew](https://brew.sh).

## Usage

See the **[User Guide](docs/USAGE.md)** for a full walkthrough of every tab, where
files live (`php.ini`, logs, `~/Sites`), and troubleshooting.

Quick start:
- **Services** — toggle each service or Start/Stop all.
- **PHP** — Install a version, then **Use** to make it active (CLI + web).
  **Update** appears when a newer patch is out; **Check for updates** checks now.
- **Sites** — Add `myapp` → `http://myapp.test` served from `~/Sites/myapp`.
- **Ext** — toggle or download extensions.
- **Settings** — web server, ports, components, files & folders.

## What's new

| Version | Changes |
|---|---|
| **v0.1.11** | Only one Tusk runs at a time — opening it again focuses the existing window. |
| **v0.1.10** | PHP update banner now shows reliably; old PHP builds are removed after updating. |
| **v0.1.9** | PHP security updates in the app (banner + Update button + Check for updates), installs always get the latest patch, PHP 8.5 support with versions discovered from Homebrew. Prompted by **PHP2Shell** (CVE-2026-109618 / CVE-2026-109619, fixed in 8.5.12 / 8.4.27 / 8.3.36 / 8.2.35). |

Full history: [Releases](https://github.com/getsettalk/tusk/releases).

## Build from source

```bash
git clone https://github.com/getsettalk/tusk.git
cd tusk/desktop
npm install                    # Tauri CLI
npx @tauri-apps/cli build      # produces Tusk.app in src-tauri/target/release/bundle
# or run in dev:
cd src-tauri && cargo run
```

Needs [Rust](https://rustup.rs) and Node. The frontend is dependency-light static
HTML/CSS/JS; the backend is Rust (Tauri v2) that orchestrates Homebrew.

Architecture notes for the app: [desktop/README.md](desktop/README.md). The repo
also keeps the original Bash CLI engine (`bin/chowk`, `lib/`, `templates/`).

## Project layout

```
desktop/            Tauri app (the GUI)
  src-tauri/        Rust backend + engine
  ui/               static frontend
docs/USAGE.md       user guide
install.sh          one-line installer (latest release → /Applications)
bin/ lib/ templates/  original Bash CLI engine
```

## Contributing

Issues and PRs welcome — it's built to be readable and hackable. See
[docs/USAGE.md](docs/USAGE.md) to understand the moving parts.

## License

[MIT](LICENSE) © [getsettalk](https://github.com/getsettalk)
