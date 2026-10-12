// Sin consola al abrir el .exe en Windows (release).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod launch;
mod modpack;
mod mojang;
mod net;
mod ping;
mod settings;
mod theme;

use serde_json::{json, Value};
use settings::Settings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;

/// Las dos instancias del launcher (mismo servidor, distinto cliente).
const INSTANCES: &[&str] = &["zeres", "cobblemon"];
const DEFAULT_HOST: &str = "zerescobblemon.holy.gg";

struct App {
    settings: Mutex<Settings>,
    busy: AtomicBool,
    manifest: Mutex<Option<Value>>,
}

type St<'a> = State<'a, Arc<App>>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn snapshot(app: &App) -> Settings {
    app.settings.lock().unwrap().clone()
}

async fn load_manifest(app: &App, force: bool) -> Result<Value, String> {
    if !force {
        if let Some(m) = app.manifest.lock().unwrap().clone() {
            return Ok(m);
        }
    }
    let url = snapshot(app).manifest_url;
    let m = modpack::manifest(&url).await.map_err(err)?;
    *app.manifest.lock().unwrap() = Some(m.clone());
    Ok(m)
}

fn server_of(m: Option<&Value>) -> (String, u16) {
    let host = m.and_then(|m| m["server"]["host"].as_str()).unwrap_or(DEFAULT_HOST).to_string();
    let port = m.and_then(|m| m["server"]["port"].as_u64()).unwrap_or(25565) as u16;
    (host, port)
}

// ================================================================== comandos

#[tauri::command]
fn get_state(app: St) -> Value {
    let s = snapshot(&app);
    json!({
        "settings": s.public(),
        "version": env!("CARGO_PKG_VERSION"),
        "theme_ready": theme::ready(),
        "ms_available": !auth::MS_CLIENT_ID.is_empty(),
        "data_dir": settings::base_dir(),
    })
}

/// Prepara los botones/fuente/panoramica (la primera vez descarga el cliente de Minecraft).
#[tauri::command]
async fn prepare_theme(handle: AppHandle) -> Result<(), String> {
    let h = handle.clone();
    theme::ensure(move |text, d, t| {
        let _ = h.emit("theme-progress", json!({"text": text, "done": d, "total": t}));
    })
    .await
    .map_err(err)
}

#[tauri::command]
async fn server_status(app: St<'_>) -> Result<Value, String> {
    let m = load_manifest(&app, false).await.ok();
    let (host, port) = server_of(m.as_ref());
    match ping::status(&host, port).await {
        Ok((v, ms)) => Ok(json!({"online": true, "ping": ms, "host": host, "status": v})),
        Err(e) => {
            let _ = std::fs::write(settings::base_dir().join("ping-error.log"), format!("{host}:{port} -> {e:#}"));
            Ok(json!({"online": false, "host": host, "error": e.to_string()}))
        }
    }
}

#[tauri::command]
async fn instances(app: St<'_>, refresh: bool) -> Result<Value, String> {
    let m = load_manifest(&app, refresh).await;
    let mut out = Vec::new();
    for id in INSTANCES {
        let st = modpack::InstanceState::load(&launch::instance_dir(id));
        let (state, installed, latest, entry) = match &m {
            Ok(m) => {
                let entry = m["instances"][*id].clone();
                let (s, i, l) = launch::instance_status(id, &entry);
                (s.to_string(), i, l, entry)
            }
            Err(_) => (if st.pack_version.is_empty() { "offline" } else { "ready" }.to_string(), st.pack_version.clone(), String::new(), Value::Null),
        };
        out.push(json!({
            "id": id, "state": state, "installed": installed, "latest": latest,
            "minecraft": if st.minecraft.is_empty() { entry["minecraft"].as_str().unwrap_or("1.21.1").to_string() } else { st.minecraft.clone() },
            "loader": st.loader, "optional": st.optional, "mods": st.managed.iter().filter(|p| p.starts_with("mods/")).count(),
        }));
    }
    Ok(json!({ "instances": out, "news": m.as_ref().ok().map(|m| m["news"].clone()).unwrap_or(Value::Null),
               "error": m.err() }))
}

#[tauri::command]
fn save_settings(app: St, patch: Value) -> Value {
    let mut s = app.settings.lock().unwrap();
    let mut cur = serde_json::to_value(&*s).unwrap_or_default();
    if let (Some(c), Some(p)) = (cur.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            if k != "accounts" {
                c.insert(k.clone(), v.clone());
            }
        }
    }
    if let Ok(new) = serde_json::from_value::<Settings>(cur) {
        let manifest_changed = new.manifest_url != s.manifest_url;
        *s = new;
        s.save();
        if manifest_changed {
            *app.manifest.lock().unwrap() = None;
        }
    }
    s.public()
}

#[tauri::command]
fn add_offline(app: St, name: String) -> Result<Value, String> {
    let name = name.trim().to_string();
    if !auth::valid_name(&name) {
        return Err("El nombre tiene que tener entre 3 y 16 letras, números o _".into());
    }
    let mut s = app.settings.lock().unwrap();
    let acc = auth::offline(&name);
    s.accounts.retain(|a| !(a.kind == "offline" && a.name.eq_ignore_ascii_case(&name)));
    s.active = acc.uuid.clone();
    s.accounts.push(acc);
    s.save();
    Ok(s.public())
}

#[tauri::command]
fn remove_account(app: St, uuid: String) -> Value {
    let mut s = app.settings.lock().unwrap();
    s.accounts.retain(|a| a.uuid != uuid);
    if s.active == uuid {
        s.active = s.accounts.first().map(|a| a.uuid.clone()).unwrap_or_default();
    }
    s.save();
    s.public()
}

#[tauri::command]
fn set_active(app: St, uuid: String) -> Value {
    let mut s = app.settings.lock().unwrap();
    if s.accounts.iter().any(|a| a.uuid == uuid) {
        s.active = uuid;
        s.save();
    }
    s.public()
}

/// Empieza el inicio de sesion de Microsoft: devuelve el codigo y en segundo plano espera a que se complete.
#[tauri::command]
async fn ms_login_start(handle: AppHandle, app: St<'_>) -> Result<Value, String> {
    let dc = auth::device_code().await.map_err(err)?;
    let out = json!({"code": dc.user_code, "url": dc.verification_uri});
    let app = app.inner().clone();
    tauri::async_runtime::spawn(async move {
        let result = async {
            let (ms, refresh) = auth::poll_token(&dc).await?;
            auth::minecraft_login(&ms, &refresh).await
        }
        .await;
        match result {
            Ok(acc) => {
                let public = {
                    let mut s = app.settings.lock().unwrap();
                    s.accounts.retain(|a| a.uuid != acc.uuid);
                    s.active = acc.uuid.clone();
                    s.accounts.push(acc);
                    s.save();
                    s.public()
                };
                let _ = handle.emit("ms-login", json!({"ok": true, "settings": public}));
            }
            Err(e) => {
                let _ = handle.emit("ms-login", json!({"ok": false, "error": e.to_string()}));
            }
        }
    });
    Ok(out)
}

/// Prepara la instancia (instala/actualiza) y abre el juego entrando al servidor.
#[tauri::command]
async fn play(handle: AppHandle, app: St<'_>, id: String, repair: bool, launch: bool) -> Result<(), String> {
    if app.busy.swap(true, Ordering::SeqCst) {
        return Err("Ya hay algo en marcha".into());
    }
    let app2 = app.inner().clone();
    let result = run_play(&handle, &app2, &id, repair, launch).await;
    app.busy.store(false, Ordering::SeqCst);
    result.map_err(err)
}

async fn run_play(handle: &AppHandle, app: &Arc<App>, id: &str, repair: bool, launch_game: bool) -> anyhow::Result<()> {
    let emit = |stage: &str, text: &str, d: usize, t: usize| {
        let _ = handle.emit("progress", json!({"id": id, "stage": stage, "text": text, "done": d, "total": t}));
    };
    let mut s = snapshot(app);
    let acc = s.active_account().cloned().ok_or_else(|| anyhow::anyhow!("Añade una cuenta en Perfil antes de jugar"))?;
    emit("manifest", "Buscando actualizaciones", 0, 0);
    let manifest = load_manifest(app, true).await.map_err(|e| anyhow::anyhow!(e))?;
    let entry = manifest["instances"][id].clone();
    if entry.is_null() {
        return Err(anyhow::anyhow!("La network no tiene la instancia '{id}'"));
    }
    let h = handle.clone();
    let idc = id.to_string();
    let progress: launch::Progress = Arc::new(move |stage, text, d, t| {
        let _ = h.emit("progress", json!({"id": idc, "stage": stage, "text": text, "done": d, "total": t}));
    });
    let prep = launch::prepare(id, &entry, &s.manifest_url, &s, repair, progress).await?;
    if !launch_game {
        emit("done", "Instancia lista", 1, 1);
        return Ok(());
    }
    emit("auth", "Iniciando sesión", 0, 0);
    let acc = auth::ensure_fresh(&acc).await?;
    if acc.kind == "microsoft" {
        let mut st = app.settings.lock().unwrap();
        if let Some(a) = st.accounts.iter_mut().find(|a| a.uuid == acc.uuid) {
            *a = acc.clone();
        }
        st.last_instance = id.to_string();
        st.save();
        s = st.clone();
    }
    // el juego se abre en la portada (la de ZeresClient en Cobblemon); se entra al servidor desde alli con "Jugar"
    let mut cmd = launch::command(id, &prep, &acc, &s, None);
    let log_dir = launch::instance_dir(id).join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let log = std::fs::File::create(log_dir.join("zeres-launcher.log"))?;
    cmd.stdout(log.try_clone()?).stderr(log);
    emit("launch", "Abriendo Minecraft", 0, 0);
    let mut child = cmd.spawn().map_err(|e| anyhow::anyhow!("No se pudo abrir Java: {e}"))?;
    let window = handle.get_webview_window("main");
    match s.close_mode {
        1 => {
            handle.exit(0);
        }
        0 => {
            if let Some(w) = &window {
                let _ = w.hide();
            }
        }
        _ => {}
    }
    let h = handle.clone();
    let idc = id.to_string();
    std::thread::spawn(move || {
        let status = child.wait();
        if let Some(w) = h.get_webview_window("main") {
            let _ = w.show();
            let _ = w.set_focus();
        }
        let code = status.ok().and_then(|s| s.code()).unwrap_or(-1);
        let _ = h.emit("game-exit", json!({"id": idc, "code": code}));
    });
    emit("running", "Minecraft abierto", 1, 1);
    Ok(())
}

/// ¿Hay una version nueva del launcher en GitHub? (latest.json del ultimo release, firmado)
#[tauri::command]
async fn check_update(handle: AppHandle) -> Result<Value, String> {
    let updater = handle.updater().map_err(err)?;
    match updater.check().await {
        Ok(Some(u)) => Ok(json!({"available": true, "version": u.version, "notes": u.body})),
        Ok(None) => Ok(json!({"available": false})),
        Err(e) => Ok(json!({"available": false, "error": e.to_string()})),
    }
}

/// Descarga la version nueva (comprobando la firma), la instala y reabre el launcher.
#[tauri::command]
async fn install_update(handle: AppHandle) -> Result<(), String> {
    let update = handle.updater().map_err(err)?.check().await.map_err(err)?.ok_or("No hay ninguna versión nueva")?;
    let h = handle.clone();
    let mut done: usize = 0;
    update
        .download_and_install(
            move |chunk, total| {
                done += chunk;
                let _ = h.emit("update-progress", json!({"done": done, "total": total}));
            },
            || {},
        )
        .await
        .map_err(err)?;
    handle.restart();
}

#[tauri::command]
fn open_folder(id: Option<String>) -> Result<(), String> {
    let dir = match id {
        Some(i) => launch::instance_dir(&i),
        None => settings::base_dir(),
    };
    std::fs::create_dir_all(&dir).map_err(err)?;
    std::process::Command::new("explorer").arg(dir).spawn().map_err(err)?;
    Ok(())
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://")) {
        return Err("url no permitida".into());
    }
    std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).spawn().map_err(err)?;
    Ok(())
}

// ================================================================== arranque

/// Prueba sin interfaz: `zeres-launcher.exe --selftest <instancia> [--launch]`. Prepara la instancia igual que el
/// boton Jugar y, con --launch, abre el juego con una cuenta sin premium "ZeresTest" sin entrar a ningun servidor.
/// Escribe el resultado en `<datos>/selftest.log`.
fn selftest(id: &str, launch_game: bool) {
    use std::io::Write;
    let log_path = settings::base_dir().join("selftest.log");
    let _ = std::fs::create_dir_all(settings::base_dir());
    let log = Arc::new(Mutex::new(std::fs::File::create(&log_path).expect("log")));
    let line = {
        let log = log.clone();
        move |s: String| {
            let mut f = log.lock().unwrap();
            let _ = writeln!(f, "{s}");
            let _ = f.flush();
        }
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result: anyhow::Result<()> = rt.block_on(async {
        let s = Settings::load();
        let m = modpack::manifest(&s.manifest_url).await?;
        let entry = m["instances"][id].clone();
        let (host, port) = server_of(Some(&m));
        let (rh, rp) = ping::resolve(&host, port).await;
        match ping::status(&host, port).await {
            Ok((v, ms)) => line(format!("PING {host} -> {rh}:{rp} OK {ms} ms, jugadores {}/{}", v["players"]["online"], v["players"]["max"])),
            Err(e) => line(format!("PING {host} -> {rh}:{rp} FALLO: {e}")),
        }
        let l2 = line.clone();
        let progress: launch::Progress = Arc::new(move |stage, text, d, t| {
            if t == 0 || d == t || d % 200 == 0 {
                l2(format!("[{stage}] {text} {d}/{t}"));
            }
        });
        let start = std::time::Instant::now();
        let prep = launch::prepare(id, &entry, &s.manifest_url, &s, false, progress).await?;
        line(format!("PREPARADO en {:.1}s · java={} · main={} · libs={}", start.elapsed().as_secs_f32(), prep.java.display(), prep.main_class, prep.libs.len()));
        if launch_game {
            let acc = auth::offline("ZeresTest");
            let mut cmd = launch::command(id, &prep, &acc, &s, None);
            let dir = launch::instance_dir(id).join("logs");
            std::fs::create_dir_all(&dir)?;
            let out = std::fs::File::create(dir.join("zeres-launcher.log"))?;
            cmd.stdout(out.try_clone()?).stderr(out);
            let child = cmd.spawn()?;
            line(format!("LANZADO pid={}", child.id()));
        }
        Ok(())
    });
    if let Err(e) = result {
        line(format!("ERROR: {e:#}"));
    } else {
        line("OK".into());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--selftest") {
        selftest(args.get(i + 1).map(String::as_str).unwrap_or("zeres"), args.iter().any(|a| a == "--launch"));
        return;
    }
    // prueba del actualizador sin interfaz: busca version nueva, la descarga y comprueba la firma (no instala)
    let check_only = args.iter().any(|a| a == "--check-update");
    let state = Arc::new(App { settings: Mutex::new(Settings::load()), busy: AtomicBool::new(false), manifest: Mutex::new(None) });
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        // zres://theme/... -> <datos>/theme/... (texturas sacadas del Minecraft del jugador)
        .register_uri_scheme_protocol("zres", |_ctx, req| {
            let path = req.uri().path().trim_start_matches('/');
            let rel = path.strip_prefix("theme/").unwrap_or(path);
            let file = theme::dir().join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
            let ok = !rel.contains("..") && file.starts_with(theme::dir());
            let (status, body) = match ok.then(|| std::fs::read(&file).ok()).flatten() {
                Some(b) => (200, b),
                None => (404, Vec::new()),
            };
            let mime = if rel.ends_with(".png") { "image/png" } else if rel.ends_with(".json") { "application/json" } else { "application/octet-stream" };
            tauri::http::Response::builder()
                .status(status)
                .header("Content-Type", mime)
                .header("Access-Control-Allow-Origin", "*")
                .header("Cache-Control", "max-age=31536000")
                .body(body)
                .unwrap()
        })
        .setup(move |app| {
            if check_only {
                let h = app.handle().clone();
                if let Some(w) = h.get_webview_window("main") {
                    let _ = w.hide();
                }
                tauri::async_runtime::spawn(async move {
                    let msg = match h.updater() {
                        Ok(u) => match u.check().await {
                            Ok(Some(up)) => match up.download(|_, _| {}, || {}).await {
                                Ok(bytes) => format!("NUEVA {} -> {} descargada y firma OK ({} bytes)", up.current_version, up.version, bytes.len()),
                                Err(e) => format!("NUEVA {} pero fallo la descarga/firma: {e}", up.version),
                            },
                            Ok(None) => format!("SIN NOVEDADES (actual {})", env!("CARGO_PKG_VERSION")),
                            Err(e) => format!("ERROR al comprobar: {e}"),
                        },
                        Err(e) => format!("ERROR updater: {e}"),
                    };
                    let _ = std::fs::write(settings::base_dir().join("update-check.log"), msg);
                    h.exit(0);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            prepare_theme,
            server_status,
            instances,
            save_settings,
            add_offline,
            remove_account,
            set_active,
            ms_login_start,
            play,
            open_folder,
            open_url,
            check_update,
            install_update
        ])
        .run(tauri::generate_context!())
        .expect("error al arrancar el launcher");
}
