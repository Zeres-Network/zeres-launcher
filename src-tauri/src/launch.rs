//! Preparar una instancia (Java, Minecraft, Fabric, modpack) y arrancar el juego entrando directo al servidor.

/// Argumentos de Java por defecto (G1 ajustado para Minecraft con mods).
const DEFAULT_JVM_ARGS: &str = "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=100 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15";

/// Los de las versiones 0.1.0-0.1.7 (si el jugador no los cambio, se usan los nuevos).
const OLD_DEFAULT_JVM_ARGS: &str = "-XX:+UseG1GC -XX:MaxGCPauseMillis=50 -XX:+UnlockExperimentalVMOptions -XX:G1NewSizePercent=20";

use crate::modpack::{self, InstanceState};
use crate::mojang::{self, Library};
use crate::net;
use crate::settings::{base_dir, Account, Settings};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

pub fn instance_dir(id: &str) -> PathBuf {
    base_dir().join("instances").join(id)
}

/// Lo que se manda a la interfaz mientras se prepara: (etapa, texto, hechos, total).
pub type Progress = Arc<dyn Fn(&str, &str, usize, usize) + Send + Sync>;

pub struct Prepared {
    pub java: PathBuf,
    pub version: Value,
    pub base_id: String,
    pub libs: Vec<Library>,
    pub natives: PathBuf,
    pub logging_arg: Option<String>,
    pub main_class: String,
    pub extra_jvm: Vec<Value>,
    pub extra_game: Vec<Value>,
}

/// Estado de una instancia frente al manifest: "install", "update" o "ready".
pub fn instance_status(id: &str, entry: &Value) -> (&'static str, String, String) {
    let dir = instance_dir(id);
    let st = InstanceState::load(&dir);
    let latest = entry["version"].as_str().unwrap_or_default().to_string();
    let installed = st.pack_version.clone();
    let state = if installed.is_empty() {
        "install"
    } else if installed != latest {
        "update"
    } else {
        "ready"
    };
    (state, installed, latest)
}

/// Descarga todo lo necesario para la instancia `id` segun su entrada del manifest.
pub async fn prepare(id: &str, entry: &Value, manifest_url: &str, settings: &Settings, deep: bool, progress: Progress) -> Result<Prepared> {
    let dir = instance_dir(id);
    std::fs::create_dir_all(&dir)?;
    let parallel = settings.parallel;

    // 1) modpack (si la instancia tiene): de el sale la version de Minecraft y de Fabric
    let mut minecraft = entry["minecraft"].as_str().unwrap_or("1.21.1").to_string();
    let mut fabric = entry["fabric"].as_str().map(String::from);
    let old = InstanceState::load(&dir);
    let mut state = InstanceState { pack_version: entry["version"].as_str().unwrap_or_default().to_string(), ..Default::default() };
    if let Some(rel) = entry["mrpack"].as_str() {
        progress("pack", "Leyendo el modpack", 0, 0);
        let url = modpack::resolve(manifest_url, rel);
        let bytes = net::get_bytes(&url).await.context("no se pudo descargar el modpack")?;
        if let Some(h) = entry["sha1"].as_str() {
            if !net::sha1_hex(&bytes).eq_ignore_ascii_case(h) {
                return Err(anyhow!("el modpack descargado está dañado (hash distinto)"));
            }
        }
        let pack = modpack::read_pack(bytes, &dir)?;
        minecraft = pack.minecraft.clone();
        fabric = pack.fabric.clone();
        let disabled: HashSet<String> = settings.disabled_mods.get(id).cloned().unwrap_or_default().into_iter().collect();
        let mut jobs = Vec::new();
        let mut keep = HashSet::new();
        for (rel, job, optional, name) in &pack.files {
            if *optional {
                state.optional.push((rel.clone(), name.clone()));
            }
            if *optional && disabled.contains(rel) {
                let _ = std::fs::remove_file(&job.path);
                continue;
            }
            keep.insert(rel.clone());
            jobs.push(job.clone());
        }
        let p = progress.clone();
        net::download_all(jobs, parallel, deep, move |d, t| p("pack", "Descargando mods", d, t)).await?;
        for rel in modpack::apply_overrides(&pack, &dir)? {
            keep.insert(rel);
        }
        modpack::remove_stale(&dir, &old.managed, &keep);
        state.managed = keep.into_iter().collect();
        state.managed.sort();
    }
    state.minecraft = minecraft.clone();
    state.loader = fabric.clone().map(|f| format!("fabric {f}")).unwrap_or_else(|| "vanilla".into());

    // 2) Minecraft base
    progress("game", "Preparando Minecraft", 0, 0);
    let version = mojang::version_json(&minecraft).await?;
    let mut libs = mojang::libraries(&version);
    let mut main_class = version["mainClass"].as_str().unwrap_or("net.minecraft.client.main.Main").to_string();
    let mut extra_jvm = Vec::new();
    let mut extra_game = Vec::new();
    if let Some(loader) = &fabric {
        let profile = mojang::fabric_profile(&minecraft, loader).await?;
        libs = mojang::merge_libraries(libs, mojang::libraries(&profile));
        if let Some(mc) = profile["mainClass"].as_str() {
            main_class = mc.to_string();
        }
        extra_jvm = profile["arguments"]["jvm"].as_array().cloned().unwrap_or_default();
        extra_game = profile["arguments"]["game"].as_array().cloned().unwrap_or_default();
    }
    let mut jobs: Vec<net::Job> = libs.iter().filter_map(|l| l.job.clone()).collect();
    jobs.push(mojang::client_job(&version, &minecraft));
    let logging = mojang::logging_job(&version);
    if let Some((j, _)) = &logging {
        jobs.push(j.clone());
    }
    let p = progress.clone();
    net::download_all(jobs, parallel, deep, move |d, t| p("game", "Descargando librerías", d, t)).await?;
    let (_, index) = mojang::asset_index(&version).await?;
    let p = progress.clone();
    net::download_all(mojang::asset_jobs(&index), parallel.max(16), deep, move |d, t| p("assets", "Descargando recursos del juego", d, t)).await?;

    // 3) Java
    let java = if settings.java_path.trim().is_empty() {
        let p = progress.clone();
        mojang::ensure_java(&version, parallel, move |d, t| p("java", "Descargando Java", d, t)).await?
    } else {
        PathBuf::from(settings.java_path.trim())
    };

    // 4) nativos
    let natives = mojang::versions_dir().join(&minecraft).join("natives");
    {
        let natives = natives.clone();
        let native_libs: Vec<Library> = libs.iter().filter(|l| l.native).map(|l| Library { key: l.key.clone(), path: l.path.clone(), job: None, native: true }).collect();
        tokio::task::spawn_blocking(move || mojang::extract_natives(&native_libs, &natives)).await??;
    }

    state.save(&dir)?;
    Ok(Prepared {
        java,
        version,
        base_id: minecraft,
        libs,
        natives,
        logging_arg: logging.map(|(_, a)| a),
        main_class,
        extra_jvm,
        extra_game,
    })
}

/// Sustituye ${...} y aplica reglas a una lista de argumentos del JSON de version.
fn expand(list: &[Value], vars: &HashMap<&str, String>, feats: &HashMap<String, bool>, out: &mut Vec<String>) {
    let sub = |s: &str| {
        let mut r = s.to_string();
        for (k, v) in vars {
            r = r.replace(&format!("${{{k}}}"), v);
        }
        r
    };
    for a in list {
        if let Some(s) = a.as_str() {
            out.push(sub(s));
        } else if mojang::rules_ok(a.get("rules"), feats) {
            match &a["value"] {
                Value::String(s) => out.push(sub(s)),
                Value::Array(arr) => arr.iter().filter_map(|x| x.as_str()).for_each(|s| out.push(sub(s))),
                _ => {}
            }
        }
    }
}

pub fn command(id: &str, prep: &Prepared, acc: &Account, settings: &Settings, server: Option<&str>) -> std::process::Command {
    let dir = instance_dir(id);
    let sep = ";";
    let mut cp: Vec<String> = prep.libs.iter().filter(|l| l.path.exists()).map(|l| l.path.to_string_lossy().to_string()).collect();
    cp.push(mojang::client_jar(&prep.base_id).to_string_lossy().to_string());
    let custom_res = !settings.fullscreen;
    let mut feats: HashMap<String, bool> = HashMap::new();
    feats.insert("has_custom_resolution".into(), custom_res);
    feats.insert("is_quick_play_multiplayer".into(), server.is_some());
    let user_type = if acc.kind == "microsoft" { "msa" } else { "legacy" };
    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", acc.name.clone()),
        ("version_name", prep.base_id.clone()),
        ("game_directory", dir.to_string_lossy().to_string()),
        ("assets_root", mojang::assets_dir().to_string_lossy().to_string()),
        ("assets_index_name", prep.version["assetIndex"]["id"].as_str().unwrap_or_default().to_string()),
        ("auth_uuid", acc.uuid.clone()),
        ("auth_access_token", if acc.access_token.is_empty() { "0".into() } else { acc.access_token.clone() }),
        ("clientid", String::new()),
        ("auth_xuid", String::new()),
        ("user_type", user_type.into()),
        ("version_type", "release".into()),
        ("natives_directory", prep.natives.to_string_lossy().to_string()),
        ("launcher_name", "zeres-launcher".into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("classpath", cp.join(sep)),
        ("classpath_separator", sep.into()),
        ("library_directory", mojang::libraries_dir().to_string_lossy().to_string()),
        ("resolution_width", settings.width.to_string()),
        ("resolution_height", settings.height.to_string()),
        ("quickPlayMultiplayer", server.unwrap_or_default().to_string()),
        ("user_properties", "{}".into()),
    ]);
    let ram = settings.ram.get(id).copied().unwrap_or(4).clamp(1, 32);
    let mut args: Vec<String> = vec![format!("-Xmx{ram}G"), "-Xms1G".into()];
    if settings.jvm_args.trim().is_empty() || settings.jvm_args.trim() == OLD_DEFAULT_JVM_ARGS {
        // recolector G1 ajustado para el juego (menos tirones); si el jugador pone los suyos en Ajustes, se usan esos
        args.extend(DEFAULT_JVM_ARGS.split_whitespace().map(String::from));
    } else {
        args.extend(settings.jvm_args.split_whitespace().map(String::from));
    }
    if let Some(l) = &prep.logging_arg {
        args.push(l.clone());
    }
    let jvm = prep.version["arguments"]["jvm"].as_array().cloned().unwrap_or_default();
    expand(&jvm, &vars, &feats, &mut args);
    expand(&prep.extra_jvm, &vars, &feats, &mut args);
    args.push(prep.main_class.clone());
    let game = prep.version["arguments"]["game"].as_array().cloned().unwrap_or_default();
    expand(&game, &vars, &feats, &mut args);
    expand(&prep.extra_game, &vars, &feats, &mut args);
    if settings.fullscreen {
        args.push("--fullscreen".into());
    }
    let mut cmd = std::process::Command::new(&prep.java);
    cmd.args(args).current_dir(&dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}
