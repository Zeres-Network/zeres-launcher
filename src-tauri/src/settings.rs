//! Ajustes y cuentas del launcher, guardados en `<datos>/launcher.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Carpeta de datos: `%APPDATA%\ZeresLauncher`.
pub fn base_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("ZeresLauncher")
}

/// Manifest de los modpacks: repo `zeres-network-modpack` de GitHub.
pub const DEFAULT_MANIFEST: &str = "https://raw.githubusercontent.com/Zeres-Network/zeres-network-modpack/main/manifest.json";
/// Origen de la beta local (antes de publicar): se cambia solo por el de GitHub.
const OLD_LOCAL_MANIFEST: &str = "file:///C:/Users/Administrator/Desktop/dev/zeres-network-modpack/manifest.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Account {
    /// "offline" o "microsoft"
    pub kind: String,
    pub name: String,
    /// UUID sin guiones
    pub uuid: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: u64,
    #[serde(default)]
    pub skin_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// GB de RAM por instancia
    pub ram: HashMap<String, u32>,
    pub fullscreen: bool,
    pub width: u32,
    pub height: u32,
    /// 0 = ocultar el launcher al jugar, 1 = cerrarlo, 2 = no hacer nada
    pub close_mode: u8,
    /// vacio = Java automatico (el de Mojang)
    pub java_path: String,
    pub jvm_args: String,
    pub parallel: usize,
    pub animated: bool,
    /// 0 = beta, 2 = dev (solo staff, ver `staff_code`)
    pub channel: u8,
    /// codigo personal de staff (se comprueba contra la lista "staff" del manifest; cada uno tiene el suyo)
    pub staff_code: String,
    pub manifest_url: String,
    /// mods opcionales desactivados por instancia (rutas dentro de la instancia)
    pub disabled_mods: HashMap<String, Vec<String>>,
    pub last_instance: String,
    /// segundos jugados por instancia
    pub playtime: HashMap<String, u64>,
    /// ultima version del launcher cuyas novedades ya vio el jugador
    pub last_seen_version: String,
    pub accounts: Vec<Account>,
    pub active: String,
}

impl Default for Settings {
    fn default() -> Self {
        let mut ram = HashMap::new();
        ram.insert("zeres".to_string(), 4);
        ram.insert("cobblemon".to_string(), 6);
        Settings {
            ram,
            fullscreen: false,
            width: 1280,
            height: 720,
            close_mode: 0,
            java_path: String::new(),
            jvm_args: String::new(),
            parallel: 8,
            animated: true,
            channel: 0,
            staff_code: String::new(),
            manifest_url: DEFAULT_MANIFEST.to_string(),
            disabled_mods: HashMap::new(),
            last_instance: "zeres".to_string(),
            playtime: HashMap::new(),
            last_seen_version: String::new(),
            accounts: Vec::new(),
            active: String::new(),
        }
    }
}

impl Settings {
    fn path() -> PathBuf {
        base_dir().join("launcher.json")
    }

    pub fn load() -> Settings {
        let mut s: Settings = std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if s.manifest_url.is_empty() || s.manifest_url == OLD_LOCAL_MANIFEST || s.manifest_url.contains("/Days16/") {
            s.manifest_url = DEFAULT_MANIFEST.to_string();
        }
        s
    }

    /// Guarda los ajustes. Primero en un temporal y renombrando (para no dejar el archivo a medias); si eso falla,
    /// escribe directamente. Si tampoco puede, deja el motivo en `save-error.log` y devuelve false.
    pub fn save(&self) -> bool {
        let _ = std::fs::create_dir_all(base_dir());
        let Ok(json) = serde_json::to_string_pretty(self) else { return false };
        let path = Self::path();
        let tmp = path.with_extension("json.tmp");
        let first = std::fs::write(&tmp, &json).and_then(|_| std::fs::rename(&tmp, &path));
        if first.is_ok() {
            return true;
        }
        let _ = std::fs::remove_file(&tmp);
        match std::fs::write(&path, &json) {
            Ok(_) => true,
            Err(e2) => {
                let _ = std::fs::write(
                    base_dir().join("save-error.log"),
                    format!("{}: no se pudo guardar {} ({:?} / {e2})", crate::auth::now(), path.display(), first.err()),
                );
                false
            }
        }
    }

    pub fn active_account(&self) -> Option<&Account> {
        self.accounts.iter().find(|a| a.uuid == self.active).or_else(|| self.accounts.first())
    }

    /// Copia para la interfaz, sin tokens.
    pub fn public(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self).unwrap_or_default();
        if let Some(accs) = v.get_mut("accounts").and_then(|a| a.as_array_mut()) {
            for a in accs {
                if let Some(o) = a.as_object_mut() {
                    o.remove("access_token");
                    o.remove("refresh_token");
                }
            }
        }
        if let Some(o) = v.as_object_mut() {
            o.insert("staff_code".into(), serde_json::Value::Bool(!self.staff_code.is_empty())); // el codigo no sale
        }
        v
    }
}
