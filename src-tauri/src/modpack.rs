//! Manifest de la network y modpacks en formato `.mrpack` (el de Modrinth).
//!
//! manifest.json:
//! ```json
//! { "instances": { "cobblemon": { "version": "0.1.0", "mrpack": "cobblemon/zeres-cobblemon-0.1.0.mrpack", "sha1": "..." },
//!                  "zeres": { "version": "1.21.1", "minecraft": "1.21.1" } },
//!   "server": { "host": "mc.zeres.fun", "port": 25565 }, "news": [ ... ] }
//! ```
//! La ruta del `.mrpack` puede ser relativa al manifest (carpeta local o repo de GitHub).

use crate::net::{self, Job};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct InstanceState {
    pub pack_version: String,
    pub minecraft: String,
    pub loader: String,
    /// archivos que puso el modpack (rutas relativas): se borran si una version nueva ya no los trae
    pub managed: Vec<String>,
    /// mods opcionales del pack (ruta relativa, nombre)
    pub optional: Vec<(String, String)>,
}

impl InstanceState {
    pub fn path(dir: &Path) -> PathBuf {
        dir.join(".zeres-instance.json")
    }
    pub fn load(dir: &Path) -> InstanceState {
        std::fs::read_to_string(Self::path(dir)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(Self::path(dir), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

pub async fn manifest(url: &str) -> Result<Value> {
    net::get_json(url).await.context("no se pudo leer el manifest de la network")
}

/// Resuelve `rel` respecto a la URL del manifest.
pub fn resolve(manifest_url: &str, rel: &str) -> String {
    if rel.contains("://") || Path::new(rel).is_absolute() {
        return rel.to_string();
    }
    match url::Url::parse(manifest_url).and_then(|b| b.join(rel)) {
        Ok(u) => u.to_string(),
        Err(_) => Path::new(manifest_url).parent().map(|p| p.join(rel).to_string_lossy().to_string()).unwrap_or(rel.to_string()),
    }
}

pub struct Pack {
    pub minecraft: String,
    pub fabric: Option<String>,
    pub files: Vec<(String, Job, bool, String)>, // (ruta relativa, descarga, opcional, nombre)
    pub zip: Vec<u8>,
}

/// Lee un .mrpack (bytes) y devuelve lo que hay que descargar.
pub fn read_pack(bytes: Vec<u8>, inst_dir: &Path) -> Result<Pack> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).context(".mrpack no valido")?;
    let mut index = String::new();
    zip.by_name("modrinth.index.json").context("el .mrpack no tiene modrinth.index.json")?.read_to_string(&mut index)?;
    let index: Value = serde_json::from_str(&index)?;
    let deps = &index["dependencies"];
    let minecraft = deps["minecraft"].as_str().ok_or_else(|| anyhow!("el .mrpack no dice la version de Minecraft"))?.to_string();
    let fabric = deps["fabric-loader"].as_str().map(String::from);
    let mut files = Vec::new();
    for f in index["files"].as_array().into_iter().flatten() {
        let rel = f["path"].as_str().unwrap_or_default().replace('\\', "/");
        if rel.is_empty() || rel.contains("..") {
            continue;
        }
        let client = f["env"]["client"].as_str().unwrap_or("required");
        if client == "unsupported" {
            continue;
        }
        let url = f["downloads"].as_array().and_then(|d| d.first()).and_then(|u| u.as_str()).unwrap_or_default();
        let name = Path::new(&rel).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        files.push((
            rel.clone(),
            Job {
                url: url.to_string(),
                path: inst_dir.join(&rel),
                sha1: f["hashes"]["sha1"].as_str().map(String::from),
                size: f["fileSize"].as_u64(),
            },
            client == "optional",
            name,
        ));
    }
    Ok(Pack { minecraft, fabric, files, zip: bytes })
}

/// Copia `overrides/` y `client-overrides/`. Los mods se sobreescriben siempre; el resto (configs) solo si no existe,
/// para no pisar lo que el jugador haya cambiado.
pub fn apply_overrides(pack: &Pack, inst_dir: &Path) -> Result<Vec<String>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&pack.zip))?;
    let mut written = Vec::new();
    for i in 0..zip.len() {
        let mut e = zip.by_index(i)?;
        if e.is_dir() {
            continue;
        }
        let name = e.name().replace('\\', "/");
        let rel = name.strip_prefix("overrides/").or_else(|| name.strip_prefix("client-overrides/"));
        let Some(rel) = rel else { continue };
        if rel.contains("..") {
            continue;
        }
        let out = inst_dir.join(rel);
        let always = rel.starts_with("mods/");
        if always || !out.exists() {
            if let Some(d) = out.parent() {
                std::fs::create_dir_all(d)?;
            }
            let mut buf = Vec::new();
            e.read_to_end(&mut buf)?;
            std::fs::write(&out, buf)?;
        }
        if always {
            written.push(rel.to_string());
        }
    }
    Ok(written)
}

/// Borra los archivos que el pack anterior puso y el nuevo ya no trae.
pub fn remove_stale(inst_dir: &Path, old: &[String], keep: &HashSet<String>) {
    for rel in old {
        if !keep.contains(rel) {
            let _ = std::fs::remove_file(inst_dir.join(rel));
        }
    }
}
