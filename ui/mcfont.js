// Fuente de Minecraft dibujada desde las texturas del propio juego (theme/font): mismos glifos, anchos, sombra y codigos §.
"use strict";

const MC = (() => {
  const COLORS = { 0: "#000000", 1: "#0000AA", 2: "#00AA00", 3: "#00AAAA", 4: "#AA0000", 5: "#AA00AA", 6: "#FFAA00", 7: "#AAAAAA",
    8: "#555555", 9: "#5555FF", a: "#55FF55", b: "#55FFFF", c: "#FF5555", d: "#FF55FF", e: "#FFFF55", f: "#FFFFFF" };
  const NAMES = { black: "0", dark_blue: "1", dark_green: "2", dark_aqua: "3", dark_red: "4", dark_purple: "5", gold: "6", gray: "7",
    dark_gray: "8", blue: "9", green: "a", aqua: "b", red: "c", light_purple: "d", yellow: "e", white: "f" };
  const glyphs = new Map();
  let ready = false;

  function img(src) {
    return new Promise((ok, ko) => { const i = new Image(); i.crossOrigin = "anonymous"; i.onload = () => ok(i); i.onerror = ko; i.src = src; });
  }

  async function load(base) {
    const def = await (await fetch(base + "font/default.json")).json();
    for (const p of def.providers) {
      if (p.type !== "bitmap") continue;
      const im = await img(base + "font/" + p.file.split("/").pop());
      const c = document.createElement("canvas"); c.width = im.width; c.height = im.height;
      const g = c.getContext("2d"); g.drawImage(im, 0, 0);
      const data = g.getImageData(0, 0, im.width, im.height).data;
      const cols = [...p.chars[0]].length, cw = im.width / cols, ch = im.height / p.chars.length;
      const scale = (p.height || 8) / ch;
      p.chars.forEach((row, ry) => [...row].forEach((chr, rx) => {
        if (chr === "\u0000" || glyphs.has(chr)) return;
        const mask = new Uint8Array(cw * ch); let maxX = -1;
        for (let y = 0; y < ch; y++) for (let x = 0; x < cw; x++) {
          if (data[((ry * ch + y) * im.width + rx * cw + x) * 4 + 3] > 0) { mask[y * cw + x] = 1; if (x > maxX) maxX = x; }
        }
        glyphs.set(chr, { mask, cw, ch, w: maxX + 1, scale, ascent: p.ascent });
      }));
    }
    ready = true;
  }

  // letras "versalitas" de los MOTD (ʜᴀᴄɪᴇɴᴅᴏ): se dibujan como mayusculas pequeñas
  const SMALL = { "ᴀ": "A", "ʙ": "B", "ᴄ": "C", "ᴅ": "D", "ᴇ": "E", "ꜰ": "F", "ɢ": "G", "ʜ": "H", "ɪ": "I", "ᴊ": "J", "ᴋ": "K", "ʟ": "L",
    "ᴍ": "M", "ɴ": "N", "ᴏ": "O", "ᴘ": "P", "ǫ": "Q", "ʀ": "R", "ꜱ": "S", "ᴛ": "T", "ᴜ": "U", "ᴠ": "V", "ᴡ": "W", "ʏ": "Y", "ᴢ": "Z" };
  /** Glifo de un caracter; si la fuente no lo tiene, se rasteriza con una fuente del sistema (como el "unifont" del juego). */
  function glyph(c) {
    let g = glyphs.get(c);
    if (g) return g;
    if (SMALL[c]) {
      const base = glyphs.get(SMALL[c]);
      if (base) { g = { ...base, small: true }; glyphs.set(c, g); return g; }
    }
    const cv = document.createElement("canvas"); cv.width = 16; cv.height = 16;
    const x = cv.getContext("2d"); x.font = "bold 9px Arial, sans-serif"; x.textBaseline = "alphabetic"; x.fillStyle = "#fff";
    x.fillText(c, 0, 8);
    const d = x.getImageData(0, 0, 16, 16).data, mask = new Uint8Array(16 * 16); let maxX = -1;
    for (let y = 0; y < 16; y++) for (let xx = 0; xx < 16; xx++) if (d[(y * 16 + xx) * 4 + 3] > 110) { mask[y * 16 + xx] = 1; if (xx > maxX) maxX = xx; }
    g = maxX < 0 ? glyphs.get("?") : { mask, cw: 16, ch: 16, w: maxX + 1, scale: 1, ascent: 8 };
    glyphs.set(c, g);
    return g;
  }

  function adv(c, bold) {
    if (c === " ") return 4;
    const g = glyph(c);
    const w = g.small ? Math.ceil(g.w * 0.8) : Math.floor(g.w * g.scale + 0.5);
    return w + 1 + (bold ? 1 : 0);
  }

  function parse(text, base) {
    const out = []; let col = base || "f", bold = false;
    for (let i = 0; i < text.length; i++) {
      if (text[i] === "§" && text[i + 1] === "#" && /^[0-9a-fA-F]{6}$/.test(text.substr(i + 2, 6))) {
        col = "#" + text.substr(i + 2, 6); bold = false; i += 7;
        continue;
      }
      if (text[i] === "§" && i + 1 < text.length) {
        const k = text[++i].toLowerCase();
        if (COLORS[k]) { col = k; bold = false; } else if (k === "l") bold = true; else if (k === "r") { col = base || "f"; bold = false; }
        continue;
      }
      out.push({ c: text[i], col, bold });
    }
    return out;
  }

  function width(text) { return parse(String(text)).reduce((w, p) => w + adv(p.c, p.bold), 0); }

  /** Canvas con el texto; s = pixeles de pantalla por pixel de la fuente. */
  function render(text, opt = {}) {
    const s = opt.s || 2, parts = parse(String(text), opt.c), shadow = opt.shadow !== false;
    const w = Math.max(1, parts.reduce((a, p) => a + adv(p.c, p.bold), 0) + 1), top = 3, h = 12;
    const cv = document.createElement("canvas"); cv.width = w * s; cv.height = h * s;
    cv.style.width = w * s + "px"; cv.style.height = h * s + "px"; cv.style.marginTop = -top * s + "px";
    if (!ready) return cv;
    const g = cv.getContext("2d");
    const draw = (pass) => {
      let x = 0;
      for (const p of parts) {
        if (p.c !== " ") {
          const gl = glyph(p.c);
          let col = p.col.startsWith("#") ? p.col : COLORS[p.col];
          if (pass === 0) { const n = parseInt(col.slice(1), 16); col = `rgb(${(n >> 16 & 255) >> 2},${(n >> 8 & 255) >> 2},${(n & 255) >> 2})`; }
          g.fillStyle = col;
          const off = pass === 0 ? 1 : 0;
          if (gl.small) {
            // mayuscula reducida (5 px de alto en vez de 7), apoyada en la linea base
            const k = 0.8, oy = top + 7 - gl.ascent * k, px = Math.ceil(k * s);
            for (let b = 0; b < (p.bold ? 2 : 1); b++)
              for (let y = 0; y < gl.ch; y++) for (let xx = 0; xx < gl.cw; xx++)
                if (gl.mask[y * gl.cw + xx]) g.fillRect(Math.round((x + xx * k + off + b) * s), Math.round((oy + y * k + off) * s), px, px);
          } else {
            const oy = top + 7 - gl.ascent, px = Math.ceil(gl.scale * s);
            for (let b = 0; b < (p.bold ? 2 : 1); b++)
              for (let y = 0; y < gl.ch; y++) for (let xx = 0; xx < gl.cw; xx++)
                if (gl.mask[y * gl.cw + xx]) g.fillRect((x + xx * gl.scale + off + b) * s, (oy + y * gl.scale + off) * s, px, px);
          }
        }
        x += adv(p.c, p.bold);
      }
    };
    if (shadow) draw(0);
    draw(1);
    return cv;
  }

  /** Pone el texto en el elemento (lo sustituye). */
  function set(el, text, opt) {
    if (!el) return;
    el.innerHTML = "";
    el.appendChild(render(text, opt));
  }

  /** Recorta el texto con "..." para que quepa en `max` pixeles de fuente. */
  function fit(text, max) {
    if (width(text) <= max) return text;
    let t = text;
    while (t.length > 1 && width(t + "...") > max) t = t.slice(0, -1);
    return t + "...";
  }

  /** Componente de chat JSON (MOTD de un servidor) -> texto con codigos §. */
  function chat(c) {
    if (c == null) return "";
    if (typeof c === "string") return c;
    let out = "";
    const walk = (n, inh) => {
      if (typeof n === "string") { out += n; return; }
      const col = n.color ? (NAMES[n.color] || (n.color.startsWith("#") ? n.color.toUpperCase() : inh.col)) : inh.col;
      const bold = n.bold ?? inh.bold;
      if (n.text) out += (col.startsWith("#") ? "§" + col : "§" + col) + (bold ? "§l" : "") + n.text;
      (n.extra || []).forEach(e => walk(e, { col, bold }));
    };
    walk(c, { col: "7", bold: false });
    return out;
  }
  function nearest(hex) {
    const n = parseInt(hex.slice(1), 16), r = n >> 16 & 255, g = n >> 8 & 255, b = n & 255;
    let best = "f", d = 1e9;
    for (const [k, h] of Object.entries(COLORS)) {
      const m = parseInt(h.slice(1), 16), dd = (r - (m >> 16 & 255)) ** 2 + (g - (m >> 8 & 255)) ** 2 + (b - (m & 255)) ** 2;
      if (dd < d) { d = dd; best = k; }
    }
    return best;
  }

  return { load, render, set, width, fit, chat, get ready() { return ready; } };
})();
