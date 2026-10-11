// Simulador del nucleo de Tauri para probar la interfaz en un navegador normal (solo desarrollo).
window.ZRES_THEME = "/theme/";
(() => {
  const listeners = {};
  const emit = (ev, payload) => (listeners[ev] || []).forEach(f => f({ payload }));
  let settings = { ram: { zeres: 4, cobblemon: 6 }, fullscreen: false, width: 1280, height: 720, close_mode: 0, java_path: "",
    jvm_args: "-XX:+UseG1GC -XX:MaxGCPauseMillis=50", parallel: 8, animated: true, channel: 0,
    manifest_url: "file:///C:/Users/Administrator/Desktop/dev/zeres-network-modpack/manifest.json", disabled_mods: {}, last_instance: "zeres",
    accounts: [{ kind: "offline", name: "Days16", uuid: "a1b2c3d4e5f60718293a4b5c6d7e8f90" }], active: "a1b2c3d4e5f60718293a4b5c6d7e8f90" };
  const inst = { zeres: { id: "zeres", state: "install", installed: "", latest: "1.21.1", minecraft: "1.21.1", loader: "", optional: [], mods: 0 },
    cobblemon: { id: "cobblemon", state: "update", installed: "0.0.9", latest: "0.1.0", minecraft: "1.21.1", loader: "fabric 0.19.3",
      optional: [["mods/xaerominimap-fabric-1.21.1-26.4.2.jar", "xaerominimap-fabric-1.21.1-26.4.2.jar"], ["mods/xaeroworldmap-fabric-1.21.1-1.44.2.jar", "xaeroworldmap-fabric-1.21.1-1.44.2.jar"]], mods: 45 } };
  const handlers = {
    get_state: () => ({ settings, version: "0.1.0", theme_ready: true, ms_available: false }),
    instances: () => ({ instances: Object.values(inst), news: [{ tag: "LAUNCHER", title: "Beta 0.1.0", text: "Primera versión del launcher de Zeres Network." }] }),
    server_status: () => ({ online: true, ping: 48, host: "mc.zeres.fun", status: { description: { text: "", extra: [{ text: "Zeres Network ", color: "gold", bold: true }, { text: "- Survival, SkyBlock y Cobblemon\n", color: "gray" }, { text: "¡Temporada 1 de ranked abierta!", color: "yellow" }] }, players: { online: 57, max: 200 } } }),
    save_settings: ({ patch }) => (settings = { ...settings, ...patch }),
    add_offline: ({ name }) => { if (!/^[A-Za-z0-9_]{3,16}$/.test(name)) throw "El nombre tiene que tener entre 3 y 16 letras, números o _"; const uuid = (name + "00000000000000000000000000000000").slice(0, 32); settings.accounts.push({ kind: "offline", name, uuid }); settings.active = uuid; return settings; },
    remove_account: ({ uuid }) => { settings.accounts = settings.accounts.filter(a => a.uuid !== uuid); return settings; },
    set_active: ({ uuid }) => { settings.active = uuid; return settings; },
    ms_login_start: () => { throw "El inicio de sesión con Microsoft aún no está disponible en esta beta (falta la aprobación de Mojang)."; },
    play: async ({ id }) => {
      const steps = [["pack", "Descargando mods", 45], ["game", "Descargando librerías", 70], ["assets", "Descargando recursos del juego", 120], ["java", "Descargando Java", 30]];
      for (const [stage, text, total] of steps) for (let d = 0; d <= total; d += Math.ceil(total / 12)) { emit("progress", { id, stage, text, done: Math.min(d, total), total }); await new Promise(r => setTimeout(r, 60)); }
      inst[id].state = "ready"; inst[id].installed = inst[id].latest;
      emit("progress", { id, stage: "running", text: "Minecraft abierto", done: 1, total: 1 });
    },
    open_folder: () => null, open_url: () => null, prepare_theme: () => null
  };
  window.__TAURI__ = {
    core: { invoke: async (cmd, args) => { const h = handlers[cmd]; if (!h) throw "sin simulador para " + cmd; return JSON.parse(JSON.stringify(await h(args || {}))); } },
    event: { listen: async (ev, fn) => { (listeners[ev] = listeners[ev] || []).push(fn); return () => { }; } },
    window: { getCurrentWindow: () => ({ minimize() { }, close() { } }) }
  };
})();
