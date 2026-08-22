// Tusk desktop frontend. Talks to the Rust backend via Tauri's invoke().
const TAURI = window.__TAURI__;
const invoke = TAURI ? TAURI.core.invoke : async (c) => { console.warn("no tauri:", c); throw "Not running inside Tusk"; };

const $ = (s) => document.querySelector(s);
const $$ = (s) => Array.from(document.querySelectorAll(s));

function toast(msg, kind = "ok") {
  const t = $("#toast");
  t.textContent = msg; t.className = `toast ${kind}`;
  clearTimeout(t._t); t._t = setTimeout(() => t.classList.add("hidden"), 3000);
}
async function call(cmd, args) {
  try { return await invoke(cmd, args); }
  catch (e) { toast(typeof e === "string" ? e : JSON.stringify(e), "err"); throw e; }
}

// Full-screen progress overlay for long operations (installs/downloads).
// Shows an animated bar + elapsed timer + the LATEST status line only — no
// growing log box (that + a flood of events previously blew up memory).
let _busyTimer = null, _busySecs = 0;
function busy(msg) {
  $("#busy-msg").textContent = msg || "Working…";
  $("#busy-status").textContent = "";
  _busySecs = 0; $("#busy-elapsed").textContent = "0s elapsed";
  clearInterval(_busyTimer);
  _busyTimer = setInterval(() => { _busySecs++; $("#busy-elapsed").textContent = _busySecs + "s elapsed"; }, 1000);
  $("#busy").classList.remove("hidden");
}
function idle() { clearInterval(_busyTimer); $("#busy").classList.add("hidden"); }
function busyLog(line) {
  // Only ever show the single most-recent line — constant memory.
  const s = String(line || "").slice(0, 200);
  if (s) $("#busy-status").textContent = s;
}
async function withBusy(msg, cmd, args) {
  busy(msg);
  try { const r = await call(cmd, args); toast(typeof r === "string" && r ? r : "Done"); return r; }
  finally { idle(); }
}
// Stream the latest install/download status line from the backend.
if (TAURI && TAURI.event && TAURI.event.listen) {
  TAURI.event.listen("install-log", (e) => busyLog(e.payload));
}

// ---- navigation -----------------------------------------------------------
let currentView = "services";
function showView(name) {
  currentView = name;
  $$(".view").forEach((v) => v.classList.add("hidden"));
  $(`#view-${name}`).classList.remove("hidden");
  $$(".tab").forEach((t) => t.classList.toggle("active", t.dataset.view === name));
  if (name === "services") renderServices();
  if (name === "php") renderPhp();
  if (name === "sites") renderSites();
  if (name === "extensions") renderExtensions();
  if (name === "settings") renderSettings();
}
$$(".tab").forEach((t) => t.addEventListener("click", () => showView(t.dataset.view)));

// ---- license gate ---------------------------------------------------------
const LICENSE = `Tusk is free and open-source software, released under the MIT License.

Tusk orchestrates third-party components you install via Homebrew, each under
its own license:
  • PHP           — PHP License v3.01
  • MariaDB       — GPL v2
  • Nginx         — 2-clause BSD
  • Apache httpd  — Apache License 2.0
  • phpMyAdmin    — GPL v2
  • Redis         — RSALv2 / SSPLv1
  • Mailpit       — MIT

These are provided by their projects under their own terms, with no warranty.
Tusk itself comes with no warranty of any kind.`;

async function maybeShowLicense() {
  const state = await call("get_state").catch(() => null);
  if (state && !state.license_accepted) {
    $("#license-text").textContent = LICENSE;
    $("#license-gate").classList.remove("hidden");
  }
}
$("#license-agree").addEventListener("change", (e) => { $("#license-continue").disabled = !e.target.checked; });
$("#license-continue").addEventListener("click", async () => {
  await call("accept_license");
  $("#license-gate").classList.add("hidden");
  toast("Welcome to Tusk");
});

// ---- services (default view) ---------------------------------------------
async function renderServices() {
  const svcs = await call("list_services");
  const el = $("#svc-list"); el.innerHTML = "";
  let running = 0;
  for (const s of svcs) {
    if (s.running) running++;
    const led = s.running ? "on" : (s.installed ? "off" : "");
    const sub = s.installed ? `${s.version || ""} · :${s.port}` : "not installed";
    const row = document.createElement("div");
    row.className = "svc";
    const control = s.installed
      ? `<div class="pw ${s.running ? "on" : ""}" data-key="${s.key}" data-on="${s.running}"></div>`
      : `<button class="btn sm" data-install="${s.key}">Install</button>`;
    row.innerHTML = `
      <span class="led ${led}"></span>
      <div class="svc-info"><div class="svc-name">${s.label}</div><div class="svc-sub">${sub}</div></div>
      ${control}`;
    el.appendChild(row);
  }
  $("#statusbar").textContent = `${running}/${svcs.length} services running`;

  el.querySelectorAll(".pw").forEach((pw) => pw.addEventListener("click", async () => {
    if (pw.classList.contains("busy")) return;
    const on = !(pw.dataset.on === "true");
    pw.classList.add("busy");
    toast(`${on ? "Starting" : "Stopping"} ${pw.dataset.key}…`);
    try { await call("set_service", { key: pw.dataset.key, on }); } finally { renderServices(); }
  }));
  el.querySelectorAll("[data-install]").forEach((b) => b.addEventListener("click", async () => {
    try { await withBusy(`Installing ${b.dataset.install}…`, "install_service", { key: b.dataset.install }); }
    finally { renderServices(); }
  }));
}

$("#btn-start").addEventListener("click", async () => { toast("Starting all…"); await call("svc_start"); renderServices(); });
$("#btn-stop").addEventListener("click", async () => { toast("Stopping all…"); await call("svc_stop"); renderServices(); });

$("#q-site").addEventListener("click", () => call("open_url", { url: "http://localhost" }));
$("#q-pma").addEventListener("click", async () => { const st = await call("get_state"); call("open_url", { url: `http://pma.${st.tld}` }); });
$("#q-folder").addEventListener("click", async () => { call("open_folder", { path: await call("chowk_home_path") }); });
$("#q-terminal").addEventListener("click", async () => { call("open_terminal", { path: await call("sites_root_path") }); });

// ---- PHP ------------------------------------------------------------------
async function renderPhp() {
  const versions = await call("php_list");
  const el = $("#php-list"); el.innerHTML = "";
  for (const v of versions) {
    const tag = v.active ? '<span class="tag active">active</span>'
      : v.installed ? '<span class="tag installed">installed</span>'
      : '<span class="tag missing">available</span>';
    let btns = "";
    if (v.installed && !v.active) btns += `<button class="btn sm" data-act="switch" data-v="${v.version}">Use</button>`;
    if (v.installed) btns += `<button class="btn sm danger" data-act="uninstall" data-v="${v.version}">✕</button>`;
    if (!v.installed) btns += `<button class="btn sm primary" data-act="install" data-v="${v.version}">Install</button>`;
    const item = document.createElement("div");
    item.className = "item";
    item.innerHTML = `<div class="grow"><div class="main">php@${v.version}</div><div class="sub">${v.full || "&nbsp;"}</div></div>${tag}<div class="item-btns">${btns}</div>`;
    el.appendChild(item);
  }
  el.querySelectorAll("button").forEach((b) => b.addEventListener("click", async () => {
    const v = b.dataset.v, act = b.dataset.act;
    try {
      if (act === "switch") { await call("php_switch", { version: v }); toast(`php@${v} is now the default`); }
      if (act === "install") await withBusy(`Installing php@${v}…`, "php_install", { version: v });
      if (act === "uninstall") await withBusy(`Removing php@${v}…`, "php_uninstall", { version: v });
    } finally { renderPhp(); }
  }));
}

// ---- Sites ----------------------------------------------------------------
async function renderSites() {
  const sites = await call("sites_list");
  const sys = await call("system_sites");
  const st = await call("get_state");
  const el = $("#sites-list"); el.innerHTML = "";

  // System sites first (localhost, phpMyAdmin) — locked, no delete.
  for (const s of sys) {
    const label = s.name === "localhost" ? "localhost" : `${s.name}.${st.tld}`;
    const url = s.name === "localhost" ? "http://localhost" : `http://${s.name}.${st.tld}`;
    const item = document.createElement("div");
    item.className = "item";
    item.innerHTML = `<div class="grow"><div class="main" data-open="${url}" style="cursor:pointer;color:var(--accent)">${label} <span class="tag installed">system</span></div><div class="sub">${s.docroot}</div></div>
      <div class="item-btns">
        <button class="btn sm" data-folder="${s.docroot}">📁</button>
      </div>`;
    el.appendChild(item);
  }

  for (const s of sites) {
    const url = `http://${s.name}.${st.tld}`;
    const item = document.createElement("div");
    item.className = "item";
    item.innerHTML = `<div class="grow"><div class="main" data-open="${url}" style="cursor:pointer;color:var(--accent)">${s.name}.${st.tld}</div><div class="sub">${s.docroot}</div></div>
      <div class="item-btns">
        <button class="btn sm" data-folder="${s.docroot}">📁</button>
        <button class="btn sm" data-term="${s.docroot}">▸_</button>
        <button class="btn sm danger" data-rm="${s.name}">✕</button>
      </div>`;
    el.appendChild(item);
  }
  el.querySelectorAll("[data-open]").forEach((a) => a.addEventListener("click", () => call("open_url", { url: a.dataset.open })));
  el.querySelectorAll("[data-folder]").forEach((b) => b.addEventListener("click", () => call("open_folder", { path: b.dataset.folder })));
  el.querySelectorAll("[data-term]").forEach((b) => b.addEventListener("click", () => call("open_terminal", { path: b.dataset.term })));
  el.querySelectorAll("[data-rm]").forEach((b) => b.addEventListener("click", async () => { await call("site_remove", { name: b.dataset.rm }); renderSites(); }));
}
$("#site-add-btn").addEventListener("click", async () => {
  const name = $("#site-name").value.trim(); const root = $("#site-root").value.trim();
  if (!name) { toast("Enter a site name", "err"); return; }
  await call("site_add", { name, docroot: root });
  $("#site-name").value = ""; $("#site-root").value = ""; renderSites();
});

// ---- Extensions -----------------------------------------------------------
let extVersion = null;
let extAll = [];
async function renderExtensions() {
  const versions = (await call("php_list")).filter((v) => v.installed);
  const pills = $("#ext-versions"); pills.innerHTML = "";
  if (!versions.length) { $("#ext-list").innerHTML = '<p class="muted tiny">Install a PHP version first.</p>'; return; }
  if (!extVersion || !versions.some((v) => v.version === extVersion)) extVersion = versions.find((v) => v.active)?.version || versions[0].version;
  for (const v of versions) {
    const b = document.createElement("button");
    b.className = "pill-btn" + (v.version === extVersion ? " active" : "");
    b.textContent = `php@${v.version}`;
    b.addEventListener("click", () => { extVersion = v.version; renderExtensions(); });
    pills.appendChild(b);
  }
  // Popular extensions to download in one click.
  const popular = ["xdebug", "redis", "imagick", "mongodb", "swoole"];
  const pop = $("#ext-popular"); pop.innerHTML = "";
  for (const name of popular) {
    const b = document.createElement("button");
    b.className = "pill-btn"; b.textContent = "+ " + name;
    b.addEventListener("click", () => addExtension(name));
    pop.appendChild(b);
  }
  extAll = await call("php_extensions", { version: extVersion });
  drawChips();
}

async function addExtension(name) {
  name = (name || "").trim(); if (!name) return;
  await withBusy(`Downloading & building ${name} (PECL)…`, "php_install_extension", { version: extVersion, name });
  $("#ext-add").value = "";
  renderExtensions();
}
$("#ext-add-btn").addEventListener("click", () => addExtension($("#ext-add").value));
$("#ext-add").addEventListener("keydown", (e) => { if (e.key === "Enter") addExtension($("#ext-add").value); });
function drawChips() {
  const q = ($("#ext-search").value || "").toLowerCase();
  const grid = $("#ext-list"); grid.innerHTML = "";
  const shown = extAll.filter((e) => e.name.toLowerCase().includes(q));
  for (const e of shown) {
    const chip = document.createElement("div");
    chip.className = "chip" + (e.enabled ? " on" : "") + (e.builtin ? " locked" : "");
    chip.textContent = e.name;
    chip.title = e.builtin ? "Built-in — always on" : (e.enabled ? "Click to disable" : "Click to enable");
    if (!e.builtin) chip.addEventListener("click", async () => {
      await call("php_set_extension", { version: extVersion, name: e.name, enable: !e.enabled });
      e.enabled = !e.enabled; drawChips();
    });
    grid.appendChild(chip);
  }
}
$("#ext-search").addEventListener("input", drawChips);

// ---- Settings -------------------------------------------------------------
async function renderSettings() {
  const st = await call("get_state");
  const status = await call("svc_status").catch(() => null);
  const webRunning = status && status.web_running;
  $$(".seg-btn").forEach((b) => {
    b.classList.toggle("active", b.dataset.server === st.web_server);
    b.disabled = webRunning;                    // can't switch while running
    b.title = webRunning ? "Stop the web server to switch" : "";
    b.style.opacity = webRunning ? "0.5" : "";
    b.style.cursor = webRunning ? "not-allowed" : "";
  });
  const hint = $("#web-lock-hint");
  if (hint) hint.textContent = webRunning ? "Stop the web server (Services) to change it." : "";
  $("#port-input").value = st.http_port;
}
$$(".seg-btn").forEach((b) => b.addEventListener("click", async () => {
  await call("set_web_server", { server: b.dataset.server });
  renderSettings(); toast(`Web server → ${b.dataset.server}`);
}));
$("#port-save").addEventListener("click", async () => {
  const port = parseInt($("#port-input").value, 10); if (!port) return;
  await call("set_port", { port }); toast(`Port set to ${port}`);
});
$("#btn-install-all").addEventListener("click", () => withBusy("Installing PHP + MariaDB + Nginx + phpMyAdmin…", "install_everything"));
$("#btn-install-base").addEventListener("click", () => withBusy("Installing base stack…", "install_base"));
$("#btn-install-pma").addEventListener("click", () => withBusy("Installing phpMyAdmin…", "phpmyadmin_install"));
$("#btn-dns").addEventListener("click", () => withBusy("Setting up .test (admin prompt)…", "dns_setup"));
$("#btn-db-reset").addEventListener("click", () => withBusy("Reconfiguring MariaDB root…", "mariadb_dev_reset_root"));

// Files & folders
$("#btn-open-ini").addEventListener("click", async () => {
  const st = await call("get_state");
  const p = await call("php_ini_path", { version: st.active_php });
  call("open_editor", { path: p });
});
$("#btn-open-config").addEventListener("click", async () => call("open_folder", { path: await call("config_path") }));
$("#btn-open-logs").addEventListener("click", async () => call("open_folder", { path: await call("logs_path") }));
$("#btn-open-htdocs").addEventListener("click", async () => call("open_folder", { path: await call("sites_root_path") }));

// ---- boot -----------------------------------------------------------------
(async function init() {
  await maybeShowLicense();
  showView("services");
  setInterval(() => { if (currentView === "services") renderServices(); }, 5000);
})();
