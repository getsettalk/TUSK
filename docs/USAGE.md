# Tusk — User Guide

Tusk is a free, open-source local development environment for macOS. It manages
**PHP (multiple versions), MariaDB (MySQL), Nginx / Apache, and phpMyAdmin** from
one small app — a free alternative to MAMP PRO / Laragon / Laravel Herd.

Under the hood Tusk orchestrates components installed through
[Homebrew](https://brew.sh), so nothing is locked behind a paywall.

---

## 1. Requirements

- macOS (Apple Silicon or Intel)
- [Homebrew](https://brew.sh) installed

That's it — Tusk installs everything else for you.

---

## 2. Install & first run

1. Open **Tusk.app** (drag it to `/Applications` from the `.dmg`).
2. Accept the licenses on first launch.
3. Go to **Settings → Install everything**. This downloads and installs
   PHP + MariaDB + Nginx + Apache + phpMyAdmin via Homebrew. A live log shows
   progress (it can take a few minutes the first time).
4. Go to **Services → Start all**. On port 80 macOS will ask for your password
   once (needed to run the web server on the standard port).

Open **http://localhost** — you should see the Tusk dashboard.

---

## 3. The tabs

### Services
Each service (Nginx/Apache, PHP, MySQL, Redis, Mailpit) has its own **power
button**:

- **Green** = running, **grey** = stopped. Click to toggle that one service.
- **Start all / Stop all** (top right) control everything at once.
- A service that isn't installed shows an **Install** button instead.

The status bar at the bottom shows how many services are running.

### PHP
- Lists every PHP version Tusk knows about.
- **Install** downloads a version via Homebrew (live log).
- **Use** makes a version the default — this switches **both** the web server
  **and** the terminal `php` command (Tusk relinks Homebrew for you).
- **✕** removes a version.

> After switching, open a **new terminal** (or run `hash -r`) so `php -v`
> reflects the change.

### Sites
- **Add** a site: give it a name (e.g. `myapp`) → it becomes
  `http://myapp.test`, served from `~/Sites/myapp`.
- Leave *docroot* blank to auto-create `~/Sites/<name>`, or point it at any
  folder.
- Each site row has **📁 Folder**, **▸_ Terminal**, and **✕ Remove**.
- **localhost** and **pma** (phpMyAdmin) are **system sites** — always present
  and marked *system*; they can't be deleted.

> Pretty `.test` URLs need the DNS helper once: **Settings → .test DNS**
> (asks for your password to write `/etc/resolver/test`).

### Ext (extensions)
- Pick a PHP version with the pills at the top.
- **Chips** show installed extensions: green = enabled, grey = disabled, 🔒 =
  built-in (compiled in, can't be disabled). Click a non-locked chip to toggle.
- **Add extension via PECL**: type a name (or use the quick-add pills:
  `xdebug`, `redis`, `imagick`, `mongodb`, `swoole`) → **Download**. Tusk
  installs any needed system libraries, builds the extension, and enables it.
  A live log shows the build.

> If a build fails (e.g. `imagick` needs ImageMagick 6, which conflicts with
> Homebrew's ImageMagick 7), Tusk tells you and leaves PHP untouched.

### Settings
- **Web server**: switch between **Nginx** and **Apache**. Site configs are
  regenerated for whichever you pick.
- **HTTP port**: default `80` (pretty URLs, asks for admin). Set a high port
  (e.g. `8080`) to avoid the password prompt.
- **Components**: Install everything / Base stack / phpMyAdmin / .test DNS /
  DB dev root.
- **Files & folders**: quick buttons to open php.ini, the config folder, logs,
  and your `~/Sites` (htdocs).

---

## 4. Where everything lives

| What | Location |
|------|----------|
| Your web files (**htdocs**) | `~/Sites/<site>/` → `http://<site>.test` |
| **php.ini** (per version) | `/opt/homebrew/etc/php/<version>/php.ini` |
| Extension `.ini` files | `/opt/homebrew/etc/php/<version>/conf.d/` |
| **Logs** (nginx, php-fpm, per-site) | `~/.chowk/logs/` |
| Tusk config (nginx/apache/php-fpm) | `~/.chowk/etc/` |
| phpMyAdmin | `http://pma.test` (login `root`, blank password) |

All buttons for these are under **Settings → Files & folders**.

---

## 5. Databases / phpMyAdmin

- MySQL (MariaDB) runs on `127.0.0.1:3306`.
- Open **http://pma.test** for phpMyAdmin.
- Log in as **root** with an **empty password**.
- If login is refused, run **Settings → DB dev root** once — this sets root to
  use a blank password over TCP (the standard local-dev convention). *Local
  development only — never expose this database to the internet.*

---

## 6. Tips & troubleshooting

- **Changed the web server, port, or added a site?** Restart the web server
  (toggle it off/on, or Stop all → Start all) so the new config loads.
- **Port 80 asks for a password every start** — that's macOS requiring admin to
  bind a privileged port. Use a high port in Settings to avoid it.
- **`php -v` shows the old version** — open a new terminal or run `hash -r`.
- **A site shows 404** — make sure the web server is running and the docroot has
  an `index.php` / `index.html`.
- **Everything Tusk writes** lives in `~/.chowk/` — safe to inspect.

---

## 7. Uninstall

- Quit Tusk and delete `Tusk.app`.
- Remove Tusk's config: `rm -rf ~/.chowk`
- The Homebrew components (php, mariadb, nginx, etc.) remain installed; remove
  them with `brew uninstall` if you no longer need them.

---

*Tusk is MIT-licensed and community-maintained. Contributions welcome.*
