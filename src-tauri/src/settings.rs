//! Ajustes y cuentas del launcher, guardados en `<datos>/launcher.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Carpeta de datos: `%APPDATA%\ZeresLauncher`.
pub fn base_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("ZeresLauncher")
}

/// Manifest de los modpacks. En la beta local apunta a la carpeta de pruebas; al publicar se cambia por la del repo
/// `zeres-network-modpack` de GitHub (raw.githubusercontent.com/.../main/manifest.json).
pub const DEFAULT_MANIFEST: &str = "file:///C:/Users/Administrator/Desktop/dev/zeres-network-modpack/manifest.json";

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
    /// 0 = beta, 1 = estable
    pub channel: u8,
    pub manifest_url: String,
    /// mods opcionales desactivados por instancia (rutas dentro de la instancia)
    pub disabled_mods: HashMap<String, Vec<String>>,
    pub last_instance: String,
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
            jvm_args: "-XX:+UseG1GC -XX:MaxGCPauseMillis=50 -XX:+UnlockExperimentalVMOptions -XX:G1NewSizePercent=20".to_string(),
            parallel: 8,
            animated: true,
            channel: 0,
            manifest_url: DEFAULT_MANIFEST.to_string(),
            disabled_mods: HashMap::new(),
            last_instance: "zeres".to_string(),
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
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(base_dir());
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = Self::path().with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(tmp, Self::path());
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
        v
    }
}
