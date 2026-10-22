//! Todo lo de Mojang y Fabric: JSON de version, librerias (y nativos), assets, Java oficial y perfil de Fabric.

use crate::net::{self, Job};
use crate::settings::base_dir;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

const VERSION_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const JAVA_MANIFEST: &str =
    "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const ASSETS_URL: &str = "https://resources.download.minecraft.net";
const FABRIC_META: &str = "https://meta.fabricmc.net/v2/versions/loader";

// ------------------------------------------------------------------ sistema

/// Nombre del sistema en las reglas de los JSON de Mojang.
const OS_NAME: &str = if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "osx" } else { "linux" };
/// Extensiones de las librerias nativas que se extraen de los jar `natives-*`.
const NATIVE_EXT: &[&str] = if cfg!(windows) {
    &[".dll"]
} else if cfg!(target_os = "macos") {
    &[".dylib", ".jnilib"]
} else {
    &[".so"]
};
/// Plataforma en el manifest de Java de Mojang.
const JAVA_PLATFORM: &str = if cfg!(windows) {
    if cfg!(target_arch = "aarch64") { "windows-arm64" } else { "windows-x64" }
} else if cfg!(target_os = "macos") {
    if cfg!(target_arch = "aarch64") { "mac-os-arm64" } else { "mac-os" }
} else {
    "linux"
};
/// Ejecutable de Java dentro del runtime (javaw en Windows para que no salga consola).
const JAVA_BIN: &str = if cfg!(windows) {
    "bin/javaw.exe"
} else if cfg!(target_os = "macos") {
    "jre.bundle/Contents/Home/bin/java"
} else {
    "bin/java"
};

pub fn versions_dir() -> PathBuf {
    base_dir().join("versions")
}
pub fn libraries_dir() -> PathBuf {
    base_dir().join("libraries")
}
pub fn assets_dir() -> PathBuf {
    base_dir().join("assets")
}
pub fn runtime_dir() -> PathBuf {
    base_dir().join("runtime")
}

// ------------------------------------------------------------------ reglas

/// Evalua `rules` (os + features) como el launcher oficial. Sin reglas = permitido.
pub fn rules_ok(rules: Option<&Value>, features: &HashMap<String, bool>) -> bool {
    let Some(rules) = rules.and_then(|r| r.as_array()) else { return true };
    let mut allowed = false;
    for rule in rules {
        let mut matches = true;
        if let Some(os) = rule.get("os") {
            if let Some(name) = os.get("name").and_then(|v| v.as_str()) {
                matches &= name == OS_NAME;
            }
            if let Some(arch) = os.get("arch").and_then(|v| v.as_str()) {
                matches &= arch == "x86" && cfg!(target_arch = "x86") || arch == "x64" || arch == "x86_64";
            }
        }
        if let Some(feats) = rule.get("features").and_then(|f| f.as_object()) {
            for (k, v) in feats {
                let want = v.as_bool().unwrap_or(false);
                matches &= features.get(k).copied().unwrap_or(false) == want;
            }
        }
        if matches {
            allowed = rule.get("action").and_then(|a| a.as_str()) == Some("allow");
        }
    }
    allowed
}

// ------------------------------------------------------------------ version

pub async fn version_json(id: &str) -> Result<Value> {
    let path = versions_dir().join(id).join(format!("{id}.json"));
    if let Ok(s) = tokio::fs::read_to_string(&path).await {
        if let Ok(v) = serde_json::from_str(&s) {
            return Ok(v);
        }
    }
    let manifest = net::get_json(VERSION_MANIFEST).await?;
    let entry = manifest["versions"]
        .as_array()
        .and_then(|vs| vs.iter().find(|v| v["id"] == id))
        .ok_or_else(|| anyhow!("Mojang no tiene la version {id}"))?;
    let url = entry["url"].as_str().unwrap_or_default();
    let bytes = net::get_bytes(url).await?;
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    tokio::fs::write(&path, &bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn client_jar(id: &str) -> PathBuf {
    versions_dir().join(id).join(format!("{id}.jar"))
}

pub fn client_job(v: &Value, id: &str) -> Job {
    let d = &v["downloads"]["client"];
    Job {
        url: d["url"].as_str().unwrap_or_default().to_string(),
        path: client_jar(id),
        sha1: d["sha1"].as_str().map(String::from),
        size: d["size"].as_u64(),
    }
}

/// "grupo:artefacto:version[:clasificador][@ext]" -> ruta relativa de Maven.
pub fn maven_path(name: &str) -> String {
    let (coords, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, "jar"),
    };
    let p: Vec<&str> = coords.split(':').collect();
    if p.len() < 3 {
        return coords.replace(':', "/");
    }
    let group = p[0].replace('.', "/");
    let file = if p.len() > 3 {
        format!("{}-{}-{}.{ext}", p[1], p[2], p[3])
    } else {
        format!("{}-{}.{ext}", p[1], p[2])
    };
    format!("{group}/{}/{}/{file}", p[1], p[2])
}

/// Clave para no repetir la misma libreria en dos versiones (grupo:artefacto[:clasificador]).
fn lib_key(name: &str) -> String {
    let p: Vec<&str> = name.split('@').next().unwrap_or(name).split(':').collect();
    match p.len() {
        0..=2 => name.to_string(),
        3 => format!("{}:{}", p[0], p[1]),
        _ => format!("{}:{}:{}", p[0], p[1], p[3]),
    }
}

pub struct Library {
    pub key: String,
    pub path: PathBuf,
    pub job: Option<Job>,
    pub native: bool,
}

/// Librerias de un JSON de version (vanilla o Fabric), ya filtradas por reglas.
pub fn libraries(v: &Value) -> Vec<Library> {
    let feats = HashMap::new();
    let mut out = Vec::new();
    for lib in v["libraries"].as_array().into_iter().flatten() {
        if !rules_ok(lib.get("rules"), &feats) {
            continue;
        }
        let name = lib["name"].as_str().unwrap_or_default();
        let native = name.contains(":natives-");
        if let Some(a) = lib.get("downloads").and_then(|d| d.get("artifact")) {
            let rel = a["path"].as_str().map(String::from).unwrap_or_else(|| maven_path(name));
            let path = libraries_dir().join(&rel);
            let url = a["url"].as_str().unwrap_or_default();
            out.push(Library {
                key: lib_key(name),
                job: (!url.is_empty()).then(|| Job {
                    url: url.to_string(),
                    path: path.clone(),
                    sha1: a["sha1"].as_str().map(String::from),
                    size: a["size"].as_u64(),
                }),
                path,
                native,
            });
        } else if let Some(base) = lib.get("url").and_then(|u| u.as_str()) {
            // estilo Fabric: nombre maven + repositorio
            let rel = maven_path(name);
            let path = libraries_dir().join(&rel);
            let base = if base.ends_with('/') { base.to_string() } else { format!("{base}/") };
            out.push(Library {
                key: lib_key(name),
                job: Some(Job { url: format!("{base}{rel}"), path: path.clone(), sha1: lib["sha1"].as_str().map(String::from), size: lib["size"].as_u64() }),
                path,
                native,
            });
        } else {
            let rel = maven_path(name);
            let path = libraries_dir().join(&rel);
            out.push(Library { key: lib_key(name), job: None, path, native });
        }
    }
    out
}

/// Extrae las librerias nativas del sistema (.dll, .so, .dylib) a `dir`.
pub fn extract_natives(libs: &[Library], dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    for lib in libs.iter().filter(|l| l.native) {
        let Ok(f) = std::fs::File::open(&lib.path) else { continue };
        let mut zip = zip::ZipArchive::new(f)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let name = entry.name().to_string();
            if entry.is_dir() || name.starts_with("META-INF") || !NATIVE_EXT.iter().any(|e| name.ends_with(e)) {
                continue;
            }
            let file = Path::new(&name).file_name().map(|n| dir.join(n));
            if let Some(out) = file {
                if out.exists() {
                    continue;
                }
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                std::fs::write(out, buf)?;
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ assets

pub async fn asset_index(v: &Value) -> Result<(String, Value)> {
    let ai = &v["assetIndex"];
    let id = ai["id"].as_str().unwrap_or("legacy").to_string();
    let path = assets_dir().join("indexes").join(format!("{id}.json"));
    let job = Job {
        url: ai["url"].as_str().unwrap_or_default().to_string(),
        path: path.clone(),
        sha1: ai["sha1"].as_str().map(String::from),
        size: ai["size"].as_u64(),
    };
    net::download_all(vec![job], 1, false, |_, _| {}).await?;
    let s = tokio::fs::read_to_string(&path).await?;
    Ok((id, serde_json::from_str(&s)?))
}

pub fn asset_job(hash: &str, size: Option<u64>) -> Job {
    let sub = &hash[..2];
    Job {
        url: format!("{ASSETS_URL}/{sub}/{hash}"),
        path: assets_dir().join("objects").join(sub).join(hash),
        sha1: Some(hash.to_string()),
        size,
    }
}

pub fn asset_jobs(index: &Value) -> Vec<Job> {
    index["objects"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, o)| o["hash"].as_str().map(|h| asset_job(h, o["size"].as_u64())))
        .collect()
}

pub fn logging_job(v: &Value) -> Option<(Job, String)> {
    let c = v.get("logging")?.get("client")?;
    let f = &c["file"];
    let id = f["id"].as_str()?;
    let job = Job {
        url: f["url"].as_str()?.to_string(),
        path: assets_dir().join("log_configs").join(id),
        sha1: f["sha1"].as_str().map(String::from),
        size: f["size"].as_u64(),
    };
    let arg = c["argument"].as_str()?.replace("${path}", &job.path.to_string_lossy());
    Some((job, arg))
}

// ------------------------------------------------------------------ Java oficial de Mojang

/// Descarga (si falta) el Java que pide la version y devuelve la ruta del ejecutable (javaw.exe en Windows).
pub async fn ensure_java<F>(v: &Value, parallel: usize, progress: F) -> Result<PathBuf>
where
    F: Fn(usize, usize) + Send + Sync + 'static,
{
    let component = v["javaVersion"]["component"].as_str().unwrap_or("java-runtime-delta").to_string();
    let dir = runtime_dir().join(&component);
    let javaw = dir.join(JAVA_BIN);
    let marker = dir.join(".zeres-ok");
    if javaw.exists() && marker.exists() {
        return Ok(javaw);
    }
    let all = net::get_json(JAVA_MANIFEST).await?;
    let entry = all[JAVA_PLATFORM][&component]
        .as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| anyhow!("Mojang no tiene Java '{component}' para {JAVA_PLATFORM}"))?;
    let manifest = net::get_json(entry["manifest"]["url"].as_str().unwrap_or_default()).await?;
    let mut jobs = Vec::new();
    // en Linux y macOS: ejecutables (bin/java...) y enlaces simbolicos del runtime
    let mut executables = Vec::new();
    let mut links = Vec::new();
    for (rel, f) in manifest["files"].as_object().into_iter().flatten() {
        let path = dir.join(rel);
        match f["type"].as_str() {
            Some("directory") => {
                let _ = std::fs::create_dir_all(&path);
            }
            Some("link") => links.push((path, f["target"].as_str().unwrap_or_default().to_string())),
            Some("file") => {
                if f["executable"].as_bool() == Some(true) {
                    executables.push(path.clone());
                }
                let raw = &f["downloads"]["raw"];
                jobs.push(Job {
                    url: raw["url"].as_str().unwrap_or_default().to_string(),
                    path,
                    sha1: raw["sha1"].as_str().map(String::from),
                    size: raw["size"].as_u64(),
                });
            }
            _ => {}
        }
    }
    net::download_all(jobs, parallel, false, progress).await.context("descargando Java")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for p in &executables {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755))?;
        }
        for (p, target) in &links {
            let _ = std::fs::remove_file(p);
            let _ = std::os::unix::fs::symlink(target, p);
        }
    }
    #[cfg(not(unix))]
    let _ = (executables, links);
    std::fs::write(&marker, entry["version"]["name"].as_str().unwrap_or("ok"))?;
    Ok(javaw)
}

// ------------------------------------------------------------------ Fabric

pub async fn fabric_profile(mc: &str, loader: &str) -> Result<Value> {
    let id = format!("fabric-loader-{loader}-{mc}");
    let path = versions_dir().join(&id).join(format!("{id}.json"));
    if let Ok(s) = tokio::fs::read_to_string(&path).await {
        if let Ok(v) = serde_json::from_str(&s) {
            return Ok(v);
        }
    }
    let bytes = net::get_bytes(&format!("{FABRIC_META}/{mc}/{loader}/profile/json")).await?;
    let v: Value = serde_json::from_slice(&bytes).context("perfil de Fabric invalido")?;
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    tokio::fs::write(&path, &bytes).await?;
    Ok(v)
}

/// Lista final de librerias: las de la version base y encima las del loader (sin duplicar artefactos).
pub fn merge_libraries(base: Vec<Library>, extra: Vec<Library>) -> Vec<Library> {
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, Library> = HashMap::new();
    for lib in base.into_iter().chain(extra) {
        if !map.contains_key(&lib.key) {
            order.push(lib.key.clone());
        }
        map.insert(lib.key.clone(), lib);
    }
    order.into_iter().filter_map(|k| map.remove(&k)).collect()
}
