// Zeres Launcher - interfaz. Habla con el nucleo en Rust (src-tauri) por invoke/eventos.
"use strict";

const TAURI = window.__TAURI__;
const invoke = (cmd, args) => TAURI.core.invoke(cmd, args);
const listen = (ev, fn) => TAURI.event.listen(ev, e => fn(e.payload));
const appWindow = TAURI.window.getCurrentWindow();
const TH = window.ZRES_THEME || "http://zres.localhost/theme/"; // texturas sacadas del Minecraft del jugador (ver theme.rs)
const $ = id => document.getElementById(id);

const INST = {
  zeres: { name: "Zeres Network", ribbon: "§6§lSURVIVAL  §7·  §6§lSKYBLOCK", tile: "red_nether_bricks" },
  // ribbonImg: el mismo rotulo que el titulo del scoreboard del servidor (glifo zeres_logo_scoreboard de Oraxen)
  cobblemon: { name: "Zeres Cobblemon", ribbon: "§b§lC O B B L E M O N", ribbonImg: "img/cobblemon.png", tile: "lapis_block" }
};
const SPLASHES = ["¡Beta!", "¡100% casero!", "¡Hazte con todos!", "¡Código abierto!", "¡Hecho en Rust!", "¡Sin anuncios!", "¡Atrápalos ya!"];

let settings = null, appVersion = "0.1.0", msAvailable = false;
let instances = {}, news = null, server = null;
let current = "zeres", busy = false, splashI = 0, tab = 0, selAcc = null;
// running: "instancia:uuid" de cada Minecraft abierto (uno por cuenta); starting: instancias abriendose
const running = new Set(), starting = new Set();
/** Instancia con algun Minecraft abierto (con cualquier cuenta). */
function anyRunning(id) { return [...running].some(k => k.startsWith(id + ":")); }
/** Esta cuenta ya juega en esta instancia. */
function accountRunning(id) { const a = activeAccount(); return !!a && running.has(id + ":" + a.uuid); }

/* ================================================================== utilidades */
function T(el, text, opt) { MC.set(typeof el === "string" ? $(el) : el, text, opt); }
function img(src) { return new Promise((ok, ko) => { const i = new Image(); i.crossOrigin = "anonymous"; i.onload = () => ok(i); i.onerror = ko; i.src = src; }); }
function renderStatic(root = document) { root.querySelectorAll("[data-t]").forEach(el => T(el, el.dataset.t, { c: el.dataset.c })); }
async function save(patch) { settings = await invoke("save_settings", { patch }); }
function activeAccount() { return settings.accounts.find(a => a.uuid === settings.active) || settings.accounts[0] || null; }

function setThemeVars() {
  const v = {
    "--btn": "gui/button.png", "--btn-hi": "gui/button_highlighted.png", "--btn-off": "gui/button_disabled.png",
    "--slider": "gui/slider.png", "--slider-hi": "gui/slider_highlighted.png", "--handle": "gui/slider_handle.png", "--handle-hi": "gui/slider_handle_highlighted.png",
    "--tab": "gui/tab.png", "--tab-hi": "gui/tab_highlighted.png", "--tab-sel": "gui/tab_selected.png", "--tab-sel-hi": "gui/tab_selected_highlighted.png",
    "--chk": "gui/checkbox.png", "--chk-hi": "gui/checkbox_highlighted.png", "--chk-on": "gui/checkbox_selected.png", "--chk-on-hi": "gui/checkbox_selected_highlighted.png",
    "--field": "gui/text_field.png", "--field-hi": "gui/text_field_highlighted.png", "--list-bg": "gui/menu_list_background.png",
    "--head-sep": "gui/header_separator.png", "--foot-sep": "gui/footer_separator.png"
  };
  for (const [k, p] of Object.entries(v)) document.documentElement.style.setProperty(k, `url("${TH}${p}")`);
}

/* ================================================================== panoramica */
const FACES = [["rotateY(0deg)", 0], ["rotateY(-90deg)", 1], ["rotateY(180deg)", 2], ["rotateY(90deg)", 3], ["rotateX(-90deg)", 4], ["rotateX(90deg)", 5]];
let yaw = 0, lastT = 0;
async function panoBase(inst) {
  // panoramica propia de Zeres si existe (ui/img/panorama/<instancia>/), si no la del juego
  try { const r = await fetch(`img/panorama/${inst}/panorama_0.png`, { method: "HEAD" }); if (r.ok) return `img/panorama/${inst}/`; } catch (_) { }
  return TH + "panorama/";
}
async function setPano() {
  const base = await panoBase(current);
  if (!$("cube").children.length) $("cube").innerHTML = FACES.map(([r, n]) => `<div class="face" data-n="${n}" style="transform:${r} translateZ(-500px)"></div>`).join("");
  document.querySelectorAll(".face").forEach(f => f.style.backgroundImage = `url("${base}panorama_${f.dataset.n}.png")`);
}
function spin(t) {
  if (settings && settings.animated && document.visibilityState === "visible") yaw += (t - lastT) * 0.0022;
  lastT = t;
  $("cube").style.transform = `translateZ(500px) rotateX(-6deg) rotateY(${yaw}deg)`;
  requestAnimationFrame(spin);
}

/* ================================================================== iconos */
const TEX = {};
async function tex(name) { if (!TEX[name]) TEX[name] = await img(TH + name); return TEX[name]; }
function pixels(g, rows, pal) { rows.forEach((r, y) => [...r].forEach((c, x) => { if (pal[c]) { g.fillStyle = pal[c]; g.fillRect(x, y, 1, 1); } })); }
const BALL = ["................", ".....oooooo.....", "...oo######oo...", "..o###hh#####o..", ".o###hh#######o.", ".o############o.", "o######oo######o", "ooooooowwooooooo", "o......oo......o", "o..............o", ".o............o.", ".o............o.", "..o..........o..", "...oo......oo...", ".....oooooo.....", "................"];
/** Engranaje pixelado de 16x16 con sombreado (se pinta con un poco de luz arriba a la izquierda). */
function drawGear(cv) {
  const g = cv.getContext("2d"); g.clearRect(0, 0, 16, 16);
  for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
    const dx = x - 7.5, dy = y - 7.5, r = Math.hypot(dx, dy), a = Math.atan2(dy, dx);
    const tooth = Math.cos(a * 8) > 0.25;
    const outer = tooth ? 7.6 : 5.9;
    if (r > outer || r < 2.3) continue;
    const edge = r > outer - 1.1 || r < 3.3;
    const light = (-dx - dy) / 16;
    const v = edge ? 0x3a : Math.round(0xb8 + light * 70);
    const c = Math.max(0, Math.min(255, v));
    g.fillStyle = edge ? "#262728" : `rgb(${c},${c},${c + 6})`;
    g.fillRect(x, y, 1, 1);
  }
}
/** Hoja de novedades (16x16): papel con lineas y una estrella amarilla. */
const NEWS = [
  "..oooooooooo....", "..o########oo...", "..o#llll###o#o..", "..o########oooo.", "..o#llllll#####o", "..o############o",
  "..o#lllllllll##o", "..o############o", "..o#llllllll###o", "..o#######s####o", "..o#llll#sss###o", "..o#####sssss##o",
  "..o#######sss##o", "..o#######s#s##o", "..o############o", "..oooooooooooooo"];
function drawNewsIcon(cv) {
  const g = cv.getContext("2d"); g.clearRect(0, 0, 16, 16);
  pixels(g, NEWS, { o: "#262728", "#": "#e8e8e8", l: "#8d8d90", s: "#ffcf3a" });
}
/** Triangulo de "play" pixelado (9x9). */
const PLAY = ["##.......", "####.....", "######...", "########.", "#########", "########.", "######...", "####.....", "##......."];
function drawPlayIcon(cv, color) {
  const g = cv.getContext("2d"); g.clearRect(0, 0, 9, 9);
  pixels(g, PLAY, { "#": color });
}
async function drawIcons() {
  const zicon = await img("img/zeres_icon.png");
  for (const el of document.querySelectorAll(".inst")) {
    const id = el.dataset.inst, cv = el.querySelector("canvas");
    cv.width = 48; cv.height = 48;
    const g = cv.getContext("2d");
    g.clearRect(0, 0, 48, 48);
    if (id === "zeres") {
      g.imageSmoothingEnabled = true;
      g.drawImage(zicon, 3, 3, 42, 42);
    } else {
      g.imageSmoothingEnabled = false;
      const c = document.createElement("canvas"); c.width = 16; c.height = 16;
      const cg = c.getContext("2d");
      pixels(cg, BALL, { "#": "#e3262f", h: "#ff9a9a", o: "#111", w: "#fff" });
      cg.fillStyle = "#f2f2f2";
      [[8, 1, 6], [8, 9, 14], [9, 1, 14], [10, 2, 13], [11, 2, 13], [12, 3, 12], [13, 5, 10]].forEach(([y, a, b]) => cg.fillRect(a, y, b - a + 1, 1));
      cg.fillStyle = "#111"; cg.fillRect(6, 7, 4, 1); cg.fillRect(6, 8, 1, 1); cg.fillRect(9, 8, 1, 1);
      g.drawImage(c, 0, 0, 16, 16, 4, 4, 40, 40);
    }
  }
  drawGear($("gearIco"));
  drawNewsIcon($("newsIco"));
}
async function skinOf(acc) {
  if (acc && acc.skin_url) { try { return await img(acc.skin_url); } catch (_) { } }
  const alex = acc && parseInt(acc.uuid.slice(-1), 16) % 2 === 1;
  return tex(alex ? "skin/alex.png" : "skin/steve.png");
}
async function drawHead(cv, acc) {
  const s = await skinOf(acc), g = cv.getContext("2d");
  g.imageSmoothingEnabled = false; g.clearRect(0, 0, cv.width, cv.height);
  g.drawImage(s, 8, 8, 8, 8, 0, 0, cv.width, cv.height); g.drawImage(s, 40, 8, 8, 8, 0, 0, cv.width, cv.height);
}
async function drawBody(cv, acc) {
  const s = await skinOf(acc), g = cv.getContext("2d"), slim = !acc?.skin_url && acc && parseInt(acc.uuid.slice(-1), 16) % 2 === 1, aw = slim ? 3 : 4;
  g.imageSmoothingEnabled = false; g.clearRect(0, 0, 16, 32);
  const p = (sx, sy, w, h, dx, dy) => g.drawImage(s, sx, sy, w, h, dx, dy, w, h);
  p(8, 8, 8, 8, 4, 0); p(40, 8, 8, 8, 4, 0);
  p(20, 20, 8, 12, 4, 8); p(20, 36, 8, 12, 4, 8);
  p(44, 20, aw, 12, 4 - aw, 8); p(44, 36, aw, 12, 4 - aw, 8);
  p(36, 52, aw, 12, 12, 8); p(52, 52, aw, 12, 12, 8);
  p(4, 20, 4, 12, 4, 20); p(4, 36, 4, 12, 4, 20);
  p(20, 52, 4, 12, 8, 20); p(4, 52, 4, 12, 8, 20);
}

/* ================================================================== inicio */
function fmtTime(sec) {
  const h = Math.floor(sec / 3600), m = Math.floor(sec % 3600 / 60);
  return h > 0 ? `${h} h ${m} min` : `${m} min`;
}
function instInfo(id) {
  const i = instances[id] || {};
  const mc = "§7Minecraft " + (i.minecraft || "1.21.1");
  const loader = id === "cobblemon" ? " §8· §7Fabric" + (i.mods ? " §8· §7" + i.mods + " mods" : "") + (i.installed ? " §8· §7pack " + i.installed : "") : " §8· §7Vanilla";
  return mc + loader;
}
/** [texto, modo] del boton grande: modo "play", "upd", "off". */
function playState() {
  const i = instances[current];
  if (busy) return ["§lPREPARANDO", "busy"];
  if (starting.has(current)) return ["§lABRIENDO...", "off"];
  if (accountRunning(current)) return ["§lEN JUEGO", "off"];
  if (!i) return ["§lJUGAR", "play"];
  if (i.state === "install") return ["§lINSTALAR", "upd"];
  if (i.state === "update") return ["§lACTUALIZAR", "upd"];
  if (i.state === "offline" && !i.installed) return ["§lSIN CONEXIÓN", "off"];
  return ["§lJUGAR", "play"];
}
async function renderHome() {
  document.querySelectorAll(".inst").forEach(el => el.classList.toggle("on", el.dataset.inst === current));
  T("ribbon", INST[current].ribbon);
  const rImg = $("ribbonImg"), rTxt = document.querySelector(".logo-box .ribbon");
  if (INST[current].ribbonImg) { rImg.src = INST[current].ribbonImg; rImg.hidden = false; rTxt.hidden = true; }
  else { rImg.hidden = true; rTxt.hidden = false; }
  T("splash", SPLASHES[splashI], { c: "e" });
  T("instName", "§l" + INST[current].name);
  T("instInfo", "§7MC " + (instances[current]?.minecraft || "1.21.1") + (current === "cobblemon" ? " §8· §7Fabric" : " §8· §7Vanilla"));
  T("instTime", "§7Jugado: §f" + fmtTime(instances[current]?.playtime || 0));
  const [label, mode] = playState();
  T("playT", label, { s: 3 });
  const pb = $("play");
  pb.classList.toggle("upd", mode === "upd");
  pb.classList.toggle("busy", mode === "busy");
  pb.disabled = mode === "busy" || mode === "off";
  $("playIco").style.display = mode === "play" ? "" : "none";
  drawPlayIcon($("playIco"), "#ffffff");
  const n = Array.isArray(news) && news[0];
  T("news", n ? "§e" + n.tag + " §7· §f" + n.title + " §8- §7" + MC.fit(n.text || "", 280) : "");
  renderNewsList();
  for (const id of Object.keys(INST)) {
    T("lbl-" + id, "§l" + INST[id].name.replace("Zeres ", "").toUpperCase());
    const st = instances[id]?.state;
    T("sub-" + id, st === "update" ? "§6Actualización" : st === "install" ? "§eSin instalar" : anyRunning(id) ? "§aEn juego" : "§bListo");
  }
  renderServer();
  const acc = activeAccount();
  T("sideName", acc ? "§l" + acc.name : "§7Sin cuenta");
  T("sideKind", !acc ? "§ePulsa para añadir" : acc.kind === "microsoft" ? "§aPremium" : "§bSin premium");
  await drawHead($("sideHead"), acc);
  await drawHead($("headIco"), acc);
}
/** Las ultimas novedades en la tarjeta del inicio (hasta 3 lineas). */
function renderNewsList() {
  const box = $("newsList");
  box.innerHTML = "";
  const list = Array.isArray(news) ? news.slice(0, 3) : [];
  if (!list.length) { const s = document.createElement("span"); s.className = "t"; box.appendChild(s); T(s, "§7Sin novedades por ahora"); return; }
  for (const n of list) {
    const s = document.createElement("span"); s.className = "t"; box.appendChild(s);
    T(s, MC.fit("§e" + (n.tag || "•") + " §f" + (n.title || ""), 190));
  }
}
function renderServer() {
  const fav = $("favicon").getContext("2d");
  fav.imageSmoothingEnabled = false; fav.clearRect(0, 0, 64, 64);
  if (!server) { T("srvName", "Zeres Network"); T("srvMotd1", "§8Conectando..."); T("srvMotd2", ""); T("srvPlayers", ""); return; }
  if (!server.online) {
    tex("gui/unknown_server.png").then(i => fav.drawImage(i, 0, 0, 64, 64));
    T("srvName", "Zeres Network"); T("srvMotd1", "§4No se puede conectar con el servidor"); T("srvMotd2", "§8" + MC.fit(server.error ? server.host + " · " + server.error : server.host, 225)); T("srvPlayers", "");
    $("pingbar").style.backgroundImage = `url("${TH}gui/server_list/unreachable.png")`;
    return;
  }
  const st = server.status;
  if (st.favicon) img(st.favicon).then(i => fav.drawImage(i, 0, 0, 64, 64)); else tex("gui/unknown_server.png").then(i => fav.drawImage(i, 0, 0, 64, 64));
  const motd = MC.chat(st.description).split("\n");
  T("srvName", "Zeres Network");
  T("srvMotd1", MC.fit(motd[0] || "", 225), { c: "7" });
  T("srvMotd2", MC.fit(motd[1] || "", 225), { c: "7" });
  T("srvPlayers", "§7" + (st.players?.online ?? 0) + "§8/§7" + (st.players?.max ?? 0));
  const ms = server.ping, bars = ms < 150 ? 5 : ms < 300 ? 4 : ms < 600 ? 3 : ms < 1000 ? 2 : 1;
  $("pingbar").style.backgroundImage = `url("${TH}gui/server_list/ping_${bars}.png")`;
  $("pingbar").title = ms + " ms";
}
async function refreshInstances(force) {
  try {
    const r = await invoke("instances", { refresh: !!force });
    instances = Object.fromEntries(r.instances.map(i => [i.id, i]));
    news = r.news;
    if (r.error && !busy) T("status", "§c" + MC.fit("Sin conexión con la network: " + r.error, 200));
  } catch (e) { T("status", "§c" + MC.fit(String(e), 200)); }
  renderHome();
}
let serverRetry = null;
async function refreshServer() {
  try { server = await invoke("server_status"); } catch (e) { server = { online: false, host: "zerescobblemon.holy.gg", error: String(e) }; }
  renderServer();
  clearTimeout(serverRetry);
  if (!server.online) serverRetry = setTimeout(refreshServer, 8000); // mientras no conecte, reintenta cada 8 s
}
async function selectInstance(id) {
  if (id === current || busy) return;
  current = id;
  save({ last_instance: id });
  await setPano();
  renderHome();
}

/* ================================================================== jugar */
async function play(repair = false, launch = true) {
  if (busy) return;
  if (!activeAccount()) { go("profile"); T("status", "§eAñade una cuenta para jugar"); return; }
  const id = current, before = instances[id] || {};
  busy = true; $("progBar").style.width = "0"; renderHome();
  // si estaba desactualizado (o sin instalar), el boton solo instala/actualiza: el juego se abre al volver a darle
  const updating = before.state === "update" || before.state === "install";
  const doLaunch = launch && !updating;
  try {
    await invoke("play", { id, repair, launch: doLaunch });
    if (!doLaunch) T("status", updating ? "§a¡Listo! Ya está al día · pulsa §lJUGAR" : "§aArchivos comprobados");
    else starting.add(id);
    // si acaba de instalar/actualizar el modpack, se ensenan sus novedades
    if ((before.state === "update" || before.state === "install") && Array.isArray(before.notes) && before.notes.length) {
      showNews("§6§l¡NOVEDADES!", "§7" + INST[id].name + " §8· §f" + (before.latest || ""), before.notes, "§lVALE");
    }
  } catch (e) {
    T("status", "§c" + MC.fit(String(e), 205));
  }
  busy = false;
  await refreshInstances(false);
}
listen("progress", p => {
  if (p.id !== current) return;
  $("progBar").style.width = p.total > 0 ? Math.round(p.done / p.total * 100) + "%" : "100%";
  const txt = p.total > 1 ? `${p.text} (${p.done}/${p.total})` : p.text;
  T("status", MC.fit(p.stage === "running" ? "§a" + txt : "§7" + txt, 205));
});
listen("game-ready", p => {
  starting.delete(p.id); running.add(p.key || p.id);
  if (p.id === current) T("status", "§aMinecraft abierto · ¡a jugar!");
  renderHome();
});
listen("game-exit", p => {
  starting.delete(p.id); running.delete(p.key || p.id);
  if (instances[p.id]) instances[p.id].playtime = p.total;
  T("status", p.code === 0 ? "§7Has jugado " + fmtTime(p.played || 0) : "§cMinecraft se cerró con un error (código " + p.code + ")");
  renderHome();
  refreshServer();
});

/* ================================================================== novedades */
function wrapMC(text, max) {
  const out = []; let line = "";
  for (const w of String(text).split(/\s+/)) {
    const tryL = line ? line + " " + w : w;
    if (MC.width(tryL) > max && line) { out.push(line); line = w; } else line = tryL;
  }
  if (line) out.push(line);
  return out;
}
function showNews(title, sub, items, ok) {
  T("nwTitle", title, { s: 3 });
  T("nwSub", sub);
  const list = $("nwList"); list.innerHTML = "";
  let first = true;
  for (const it of items) {
    if (it && typeof it === "object" && it.h) {
      const sp = document.createElement("span"); sp.className = "t"; if (!first) sp.style.marginTop = "8px"; list.appendChild(sp);
      T(sp, "§e§l" + it.h);
      first = false;
      continue;
    }
    first = false;
    wrapMC(it, 214).forEach((l, i) => {
      const sp = document.createElement("span"); sp.className = "t"; list.appendChild(sp);
      T(sp, (i === 0 ? "§a» §f" : "   §f") + l);
    });
  }
  T("nwOkT", ok || "§lVALE");
  $("v-news").classList.add("on");
}
/** Boton "Novedades": las ultimas versiones del launcher y lo nuevo del modpack de la instancia elegida. */
async function openNews() {
  const items = [];
  const inst = instances[current];
  if (inst && Array.isArray(inst.notes) && inst.notes.length) {
    items.push({ h: INST[current].name + " " + (inst.latest || "") });
    inst.notes.forEach(n => items.push(n));
  }
  try {
    const r = await invoke("whats_new", { all: true });
    for (const rel of (r && r.releases) || []) {
      const notes = notesToItems(rel.notes);
      items.push({ h: "Zeres Launcher " + rel.version });
      (notes.length ? notes : ["Mejoras y correcciones."]).forEach(n => items.push(n));
    }
  } catch (_) { }
  if (!items.length) items.push("No se han podido cargar las novedades (¿sin internet?).");
  showNews("§6§lNOVEDADES", "§7Lo último de Zeres", items, "§lVALE");
}
/** Notas de un release de GitHub (markdown sencillo) -> lineas. */
function notesToItems(md) {
  return String(md || "").split(/\r?\n/).map(l => l.replace(/^\s*[-*•]\s*/, "").replace(/\*\*/g, "").trim()).filter(Boolean);
}

/* ================================================================== navegacion */
function go(v) {
  document.querySelectorAll(".view").forEach(x => x.classList.toggle("on", x.id === "v-" + v));
  $("bSettings").classList.toggle("on", v === "settings");
  $("bProfile").classList.toggle("on", v === "profile" || v === "add" || v === "ms");
  $("pano").classList.toggle("blur", v !== "home");
  if (v === "home") renderHome();
  if (v === "settings") renderSettings();
  if (v === "profile") renderProfile();
  if (v === "add") { $("nameInput").value = ""; T("nameErr", ""); nameRender(); setTimeout(() => $("nameInput").focus(), 0); }
}

/* ================================================================== ajustes */
const TABS = ["Juego", "Launcher", "Instancia"];
const RES = [[854, 480], [1280, 720], [1600, 900], [1920, 1080]];
function btn(label, act, cls = "") { return `<button class="mcbtn ${cls}" data-act="${act}"><span class="t" data-t="${label.replace(/"/g, "&quot;")}"></span></button>`; }
function note(text, c = "8") { return `<div class="note"><span class="t" data-t="${MC.fit(text, 305).replace(/"/g, "&quot;")}" data-c="${c}"></span></div>`; }
function renderSettings() {
  $("tabs").innerHTML = TABS.map((t, i) => `<div class="tab ${i === tab ? "on" : ""}" data-tab="${i}"><span class="t" data-t="${t}"></span></div>`).join("");
  const s = settings, yes = b => b ? "Sí" : "No", name = INST[current].name;
  let h = `<div class="opt-grid">`;
  if (tab === 0) {
    const ram = s.ram[current] || 4, rec = current === "cobblemon" ? 6 : 4;
    h += note("§7" + name);
    h += `<div class="slider wide" id="ram"><div class="handle" style="left:${(ram - 1) / 15 * 600}px"></div><div class="lbl"><span class="t" data-t="Memoria: ${ram} GB${ram === rec ? " §7(recomendado)" : ""}"></span></div></div>`;
    h += btn("Pantalla completa: " + yes(s.fullscreen), "full");
    h += btn("Resolución: " + s.width + "x" + s.height, "res", s.fullscreen ? "off" : "");
    h += btn("Java: " + (s.java_path ? "Personalizado" : "Automático"), "java");
    h += btn("Argumentos por defecto", "jvmreset");
    h += `<label class="field wide" id="jvmField" style="width:100%"><span class="t" id="jvmTxt"></span><input id="jvmInput" spellcheck="false"></label>`;
    h += note("En automático se usa el Java oficial de Mojang.");
  } else if (tab === 1) {
    h += btn("Al jugar: " + ["Ocultar launcher", "Cerrar launcher", "No hacer nada"][s.close_mode], "close");
    h += btn("Canal: Beta", "channel", "off");
    h += btn("Descargas a la vez: " + s.parallel, "dl");
    h += btn("Panorámica animada: " + yes(s.animated), "anim");
    h += btn("Abrir carpeta del launcher", "folder");
    h += btn("Buscar actualizaciones", "refresh");
    h += note("Origen de los modpacks", "7");
    h += `<label class="field wide" id="manField" style="width:100%"><span class="t" id="manTxt"></span><input id="manInput" spellcheck="false"></label>`;
    h += note("Zeres Launcher " + appVersion + " (BETA) · código abierto");
  } else {
    const i = instances[current] || {}, opt = i.optional || [], off = (s.disabled_mods || {})[current] || [];
    h += note("§7" + name + " §8· " + instInfo(current));
    if (opt.length) {
      opt.forEach(([path, file]) => {
        const on = !off.includes(path);
        h += `<div class="chk ${on ? "on" : ""}" data-mod="${path}"><i></i><span class="t" data-t="${MC.fit(modName(file), 120)}"></span></div>`;
      });
      h += note("Los mods opcionales se aplican la próxima vez que juegues.");
    } else {
      h += note(current === "cobblemon" ? "Juega una vez para ver los mods opcionales." : "Esta instancia no tiene mods opcionales.");
    }
    h += btn("Reparar archivos", "repair");
    h += btn("Abrir carpeta", "ifolder");
  }
  h += `</div>`;
  $("setList").innerHTML = h;
  renderStatic($("v-settings"));
  document.querySelectorAll("[data-tab]").forEach(t => t.onclick = () => { tab = +t.dataset.tab; renderSettings(); });
  document.querySelectorAll("#setList [data-act]").forEach(b => b.onclick = () => settingAction(b.dataset.act));
  document.querySelectorAll("#setList [data-mod]").forEach(m => m.onclick = async () => {
    const off = new Set((settings.disabled_mods || {})[current] || []);
    off.has(m.dataset.mod) ? off.delete(m.dataset.mod) : off.add(m.dataset.mod);
    await save({ disabled_mods: { ...(settings.disabled_mods || {}), [current]: [...off] } });
    renderSettings();
  });
  const ram = $("ram");
  if (ram) {
    // durante el arrastre solo se mueve la barra (sin redibujar la pantalla); se guarda al soltar
    const rec = current === "cobblemon" ? 6 : 4;
    const show = gb => {
      ram.querySelector(".handle").style.left = (gb - 1) / 15 * (ram.clientWidth - 16) + "px";
      T(ram.querySelector(".lbl .t"), "Memoria: " + gb + " GB" + (gb === rec ? " §7(recomendado)" : ""));
    };
    const gbAt = e => { const r = ram.getBoundingClientRect(); return Math.round(1 + Math.max(0, Math.min(1, (e.clientX - r.left - 8) / (r.width - 16))) * 15); };
    show(settings.ram[current] || 4);
    ram.onmousedown = e => {
      e.preventDefault();
      let gb = gbAt(e); show(gb);
      const mv = ev => { const g = gbAt(ev); if (g !== gb) { gb = g; show(gb); } };
      const up = async () => {
        removeEventListener("mousemove", mv); removeEventListener("mouseup", up);
        settings.ram[current] = gb; await save({ ram: settings.ram });
      };
      addEventListener("mousemove", mv); addEventListener("mouseup", up);
    };
    ram.onwheel = async e => {
      e.preventDefault();
      const gb = Math.max(1, Math.min(16, (settings.ram[current] || 4) + (e.deltaY < 0 ? 1 : -1)));
      settings.ram[current] = gb; show(gb); await save({ ram: settings.ram });
    };
  }
  bindField("jvmField", "jvmInput", "jvmTxt", settings.jvm_args, 290, v => save({ jvm_args: v }));
  bindField("manField", "manInput", "manTxt", settings.manifest_url, 290, v => save({ manifest_url: v }));
}
async function settingAction(a) {
  const s = settings;
  if (a === "full") await save({ fullscreen: !s.fullscreen });
  if (a === "res" && !s.fullscreen) { const i = RES.findIndex(r => r[0] === s.width && r[1] === s.height); const n = RES[(i + 1) % RES.length]; await save({ width: n[0], height: n[1] }); }
  if (a === "java" && s.java_path) await save({ java_path: "" });
  if (a === "jvmreset") await save({ jvm_args: "" });
  if (a === "close") await save({ close_mode: (s.close_mode + 1) % 3 });
  if (a === "dl") await save({ parallel: s.parallel >= 16 ? 2 : s.parallel * 2 });
  if (a === "anim") await save({ animated: !s.animated });
  if (a === "folder") invoke("open_folder", { id: null });
  if (a === "ifolder") invoke("open_folder", { id: current });
  if (a === "refresh") { await refreshInstances(true); }
  if (a === "repair") { go("home"); play(true, false); return; }
  renderSettings();
}

/* nombre bonito de un mod opcional a partir del archivo */
const MOD_NAMES = { xaerominimap: "Minimapa (Xaero)", xaeroworldmap: "Mapa del mundo (Xaero)", iris: "Shaders (Iris)", sodium: "Sodium" };
function modName(file) {
  const base = file.replace(/\.jar$/i, "").split(/[-_+]/)[0].toLowerCase();
  return MOD_NAMES[base] || base.charAt(0).toUpperCase() + base.slice(1);
}

/* campo de texto con la fuente del juego: el <input> real va invisible encima */
let caretOn = true;
const fields = [];
function bindField(fieldId, inputId, txtId, value, max, onChange) {
  const f = $(fieldId), i = $(inputId);
  if (!f || !i) return;
  i.value = value || "";
  const draw = () => {
    const v = i.value, focused = document.activeElement === i;
    const shown = MC.width(v) > max ? "..." + v.slice(-Math.floor(max / 6)) : v;
    T(txtId, shown + (focused && caretOn ? "_" : ""), { c: "f" });
  };
  i.oninput = draw;
  i.onfocus = () => { f.classList.add("focus"); draw(); };
  i.onblur = () => { f.classList.remove("focus"); draw(); if (onChange && i.value !== value) { value = i.value; onChange(i.value); } };
  i.onkeydown = e => { if (e.key === "Enter") i.blur(); };
  fields.push(draw);
  draw();
}
setInterval(() => { caretOn = !caretOn; fields.forEach(d => { try { d(); } catch (_) { } }); }, 320);

/* ================================================================== perfil */
/* Muñeco 3D con la skin: cada parte es una caja con sus 6 caras recortadas de la textura (formato de skin de Minecraft). */
const K = 9; // pixeles de pantalla por pixel de skin
function skinUrl(acc) {
  if (acc && acc.skin_url) return acc.skin_url;
  const alex = acc && parseInt(acc.uuid.slice(-1), 16) % 2 === 1;
  return TH + (alex ? "skin/alex.png" : "skin/steve.png");
}
function isSlim(acc) { return !(acc && acc.skin_url) && !!acc && parseInt(acc.uuid.slice(-1), 16) % 2 === 1; }
/** Caja de w*h*d con la textura en (u, v), centrada en (x, y, z) (en pixeles de skin). */
function cuboid(url, u, v, w, h, d, x, y, z, grow = 0) {
  const W = (w + grow) * K, H = (h + grow) * K, D = (d + grow) * K;
  const face = (fu, fv, fw, fh, pw, ph, tf) => `<i style="width:${pw}px;height:${ph}px;left:${(W - pw) / 2}px;top:${(H - ph) / 2}px;
      background-image:url('${url}');background-size:${64 * K * pw / (fw * K)}px ${64 * K * ph / (fh * K)}px;
      background-position:-${fu * pw / fw}px -${fv * ph / fh}px;transform:${tf}"></i>`;
  return `<div class="part" style="width:${W}px;height:${H}px;transform:translate3d(${x * K - W / 2}px,${y * K - H / 2}px,${z * K}px)">
    ${face(u + d, v + d, w, h, W, H, `translateZ(${D / 2}px)`)}
    ${face(u + 2 * d + w, v + d, w, h, W, H, `rotateY(180deg) translateZ(${D / 2}px)`)}
    ${face(u, v + d, d, h, D, H, `rotateY(-90deg) translateZ(${W / 2}px)`)}
    ${face(u + d + w, v + d, d, h, D, H, `rotateY(90deg) translateZ(${W / 2}px)`)}
    ${face(u + d, v, w, d, W, D, `rotateX(90deg) translateZ(${H / 2}px)`)}
    ${face(u + d + w, v, w, d, W, D, `rotateX(-90deg) translateZ(${H / 2}px)`)}
  </div>`;
}
function buildPlayer(acc) {
  const url = skinUrl(acc), aw = isSlim(acc) ? 3 : 4, ax = 4 + aw / 2;
  $("player").innerHTML =
    cuboid(url, 0, 0, 8, 8, 8, 0, -10, 0) + cuboid(url, 32, 0, 8, 8, 8, 0, -10, 0, 1) +            // cabeza + gorro
    cuboid(url, 16, 16, 8, 12, 4, 0, 0, 0) + cuboid(url, 16, 32, 8, 12, 4, 0, 0, 0, 0.5) +        // cuerpo + chaqueta
    cuboid(url, 40, 16, aw, 12, 4, -ax, 0, 0) + cuboid(url, 40, 32, aw, 12, 4, -ax, 0, 0, 0.5) +   // brazo derecho
    cuboid(url, 32, 48, aw, 12, 4, ax, 0, 0) + cuboid(url, 48, 48, aw, 12, 4, ax, 0, 0, 0.5) +     // brazo izquierdo
    cuboid(url, 0, 16, 4, 12, 4, -2, 12, 0) + cuboid(url, 0, 32, 4, 12, 4, -2, 12, 0, 0.5) +       // pierna derecha
    cuboid(url, 16, 48, 4, 12, 4, 2, 12, 0) + cuboid(url, 0, 48, 4, 12, 4, 2, 12, 0, 0.5);         // pierna izquierda
}
let rotY = -25, rotX = -8, dragging = null, lastSpin = 0;
function spinPlayer(t) {
  if (!dragging && $("v-profile").classList.contains("on") && t - lastSpin < 100) rotY += (t - lastSpin) * 0.012;
  lastSpin = t;
  $("player").style.transform = `translateY(-14px) rotateX(${rotX}deg) rotateY(${rotY}deg)`;
  requestAnimationFrame(spinPlayer);
}
requestAnimationFrame(spinPlayer);
$("stage").addEventListener("mousedown", e => { dragging = { x: e.clientX, y: e.clientY, ry: rotY, rx: rotX }; });
addEventListener("mousemove", e => { if (dragging) { rotY = dragging.ry + (e.clientX - dragging.x) * 0.6; rotX = Math.max(-30, Math.min(30, dragging.rx - (e.clientY - dragging.y) * 0.3)); } });
addEventListener("mouseup", () => { dragging = null; });

async function renderProfile() {
  const accs = settings.accounts, act = activeAccount();
  buildPlayer(act);
  T("pName", act ? "§l" + act.name : "§7Sin cuenta", { s: 3 });
  T("pKind", act ? (act.kind === "microsoft" ? "§aCuenta Microsoft · premium" : "§7Cuenta sin premium") : "§8Añade una cuenta para jugar");
  $("accList").innerHTML = accs.map(a => `<div class="acc ${a.uuid === settings.active ? "on" : ""}" data-acc="${a.uuid}">
      <canvas width="8" height="8"></canvas><div class="txt"><span class="t"></span><span class="t"></span></div>
      <button class="del" data-del="${a.uuid}" title="Quitar cuenta"><span class="t"></span></button></div>`).join("");
  if (!accs.length) $("accList").innerHTML = `<div class="acc"><div class="txt"><span class="t" id="noAcc"></span></div></div>`;
  if (!accs.length) T("noAcc", "§7Todavía no hay cuentas");
  for (const el of document.querySelectorAll("[data-acc]")) {
    const a = accs.find(x => x.uuid === el.dataset.acc), ts = el.querySelectorAll(".t");
    await drawHead(el.querySelector("canvas"), a);
    T(ts[0], (a.uuid === settings.active ? "§a" : "§f") + a.name + (a.uuid === settings.active ? " §2(en uso)" : ""));
    T(ts[1], a.kind === "microsoft" ? "§aMicrosoft" : "§7Sin premium");
    T(ts[2], "§fx");
    el.onclick = async e => {
      if (e.target.closest("[data-del]")) { settings = await invoke("remove_account", { uuid: a.uuid }); renderProfile(); renderHome(); return; }
      settings = await invoke("set_active", { uuid: a.uuid }); renderProfile(); renderHome();
    };
  }
}

/* ================================================================== cuentas */
let msUrl = "";
async function msLogin() {
  go("ms");
  T("msCode", ""); T("msLine1", "§7Pidiendo un código a Microsoft..."); T("msLine2", "");
  try {
    const r = await invoke("ms_login_start");
    msUrl = r.url;
    T("msLine1", "§7Entra en §f" + r.url + " §7e introduce este código:");
    T("msCode", "§l" + r.code, { s: 4 });
    T("msLine2", "§7Esta ventana se cerrará sola al terminar.");
    invoke("open_url", { url: r.url });
  } catch (e) {
    T("msLine1", "§c" + MC.fit(String(e), 300));
    T("msLine2", "§7Mientras tanto puedes usar una cuenta sin premium.");
  }
}
listen("ms-login", r => {
  if (r.ok) { settings = r.settings; go("profile"); }
  else { T("msLine1", "§c" + MC.fit(r.error, 300)); T("msCode", ""); }
});
function nameRender() { T("nameTxt", $("nameInput").value + (document.activeElement === $("nameInput") && caretOn ? "_" : "")); }
fields.push(() => { if ($("v-add").classList.contains("on")) nameRender(); });

/* ================================================================== arranque */
/* autoactualizacion: nada mas abrir, mira si hay version nueva en GitHub y se instala sola */
async function selfUpdate() {
  let info;
  try { info = await invoke("check_update"); } catch (_) { return; }
  if (!info || !info.available) return;
  $("boot").classList.add("on");
  $("bootText").textContent = "Actualizando el launcher a la versión " + info.version + "...";
  $("bootBar").style.width = "5%";
  await listen("update-progress", p => {
    const pct = p.total ? Math.round(p.done / p.total * 100) : 50;
    $("bootBar").style.width = pct + "%";
    $("bootText").textContent = "Actualizando el launcher a la versión " + info.version + " (" + pct + "%)";
  });
  try {
    await invoke("install_update"); // al terminar el launcher se reinicia solo
  } catch (e) {
    $("bootText").textContent = "No se pudo actualizar: " + e;
    await new Promise(r => setTimeout(r, 2500));
    $("boot").classList.remove("on");
  }
}

async function boot() {
  await selfUpdate();
  const st = await invoke("get_state");
  settings = st.settings; appVersion = st.version; msAvailable = st.ms_available;
  (st.running || []).forEach(id => running.add(id));
  const cantSave = st.can_save === false;
  current = settings.last_instance in INST ? settings.last_instance : "zeres";
  if (!st.theme_ready) {
    $("boot").classList.add("on");
    const un = await listen("theme-progress", p => {
      $("bootText").textContent = p.total > 0 ? `${p.text} (${p.done}/${p.total})` : p.text + "...";
      $("bootBar").style.width = p.total > 0 ? Math.round(p.done / p.total * 100) + "%" : "15%";
    });
    for (;;) {
      try { await invoke("prepare_theme"); break; } catch (e) {
        $("bootText").textContent = "No se pudo descargar: " + e + " · reintentando en 5 s";
        await new Promise(r => setTimeout(r, 5000));
      }
    }
    un();
  }
  setThemeVars();
  await MC.load(TH);
  $("boot").classList.remove("on");
  renderStatic();
  T("barTitle", "Zeres Launcher"); T("barBeta", "§eBETA " + appVersion); T("wMinT", "_"); T("wCloseT", "x");
  await drawIcons();
  await setPano();
  requestAnimationFrame(spin);
  renderHome();
  refreshInstances(false).then(() => { if (cantSave) T("status", "§cNo se pueden guardar los ajustes (save-error.log)"); });
  refreshServer();
  setInterval(refreshServer, 30000);

  // novedades: si el launcher se acaba de actualizar, se ensena lo nuevo de cada version
  $("nwOk").onclick = () => $("v-news").classList.remove("on");
  invoke("whats_new", { all: false }).then(r => {
    if (!r || !r.show) return;
    const items = [];
    (r.releases || []).forEach(rel => notesToItems(rel.notes).forEach(n => items.push(n)));
    if (!items.length) items.push("Mejoras y correcciones.");
    showNews("§6§l¡NOVEDADES!", r.from ? "§7Zeres Launcher §f" + r.from + " §7» §a" + r.to : "§7Zeres Launcher §a" + r.to, items, "§l¡VAMOS!");
  }).catch(() => { });
  $("bNews").onclick = () => openNews();

  // eventos
  $("wMin").onclick = () => appWindow.minimize();
  $("wClose").onclick = () => appWindow.close();
  document.querySelectorAll(".inst").forEach(el => {
    el.onclick = () => selectInstance(el.dataset.inst);
  });
  $("play").onclick = () => play(false, true);
  $("acct").onclick = () => go("profile");
  $("splash").onclick = () => { splashI = (splashI + 1) % SPLASHES.length; T("splash", SPLASHES[splashI], { c: "e" }); };
  $("bSettings").onclick = () => go($("v-settings").classList.contains("on") ? "home" : "settings");
  $("bProfile").onclick = () => go($("v-profile").classList.contains("on") ? "home" : "profile");
  document.querySelectorAll("[data-home]").forEach(b => b.onclick = () => go("home"));
  document.querySelectorAll("[data-goto]").forEach(b => b.onclick = () => go(b.dataset.goto));
  $("pOff").onclick = () => go("add");
  $("pMs").onclick = () => msLogin();
  $("msOpen").onclick = () => msUrl && invoke("open_url", { url: msUrl });
  const ni = $("nameInput");
  ni.oninput = () => { ni.value = ni.value.replace(/[^A-Za-z0-9_]/g, ""); nameRender(); };
  ni.onfocus = () => $("nameField").classList.add("focus");
  ni.onblur = () => { $("nameField").classList.remove("focus"); nameRender(); };
  const addOk = async () => {
    try { settings = await invoke("add_offline", { name: ni.value }); go("profile"); }
    catch (e) { T("nameErr", String(e), { c: "c" }); }
  };
  $("addOk").onclick = addOk;
  ni.onkeydown = e => { if (e.key === "Enter") addOk(); };
  document.addEventListener("keydown", e => { if (e.key === "Escape" && !$("v-home").classList.contains("on")) go("home"); });
  document.addEventListener("contextmenu", e => e.preventDefault());
}
boot().catch(e => { document.body.innerHTML = `<pre style="color:#f66;padding:20px">${e && e.stack || e}</pre>`; });
