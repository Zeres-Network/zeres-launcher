//! Packs de texturas del repo privado de la organizacion (p. ej. Zeres-Network/zeres-resourcepacks): mientras el servidor
//! no los mande al entrar, el launcher los descarga en `resourcepacks/` de la instancia y los activa en `options.txt`.
//! Se leen con un token de solo lectura que se mete al compilar (variable ZERES_PACKS_TOKEN, ver build.rs); sin token o
//! sin conexion no se hace nada y se juega igual.

use crate::net;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::Path;

/// Archivo con los packs que puso el launcher (para quitar los que dejen de estar en el repo).
const STATE: &str = ".zeres-packs.json";
const KEY: &[u8] = b"zeres-launcher";

fn token() -> Option<String> {
    let enc = env!("ZERES_PACKS_KEY");
    if enc.is_empty() {
        return None;
    }
    let bytes = hex::decode(enc).ok()?;
    String::from_utf8(bytes.iter().enumerate().map(|(i, b)| b ^ KEY[i % KEY.len()]).collect()).ok()
}

async fn raw(repo: &str, path: &str, token: &str) -> Result<Vec<u8>> {
    let url = format!("https://api.github.com/repos/{repo}/contents/{path}");
    let r = net::client()
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github.raw")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?
        .error_for_status()?;
    Ok(r.bytes().await?.to_vec())
}

/// Pone al dia los packs del repo en la instancia y los activa. Devuelve cuantos hay.
pub async fn sync(repo: &str, inst_dir: &Path) -> Result<usize> {
    let Some(token) = token() else { return Ok(0) };
    let manifest: Value = serde_json::from_slice(&raw(repo, "manifest.json", &token).await?)?;
    let dir = inst_dir.join("resourcepacks");
    std::fs::create_dir_all(&dir)?;
    let mut names = Vec::new();
    for p in manifest["packs"].as_array().cloned().unwrap_or_default() {
        let (Some(file), Some(sha)) = (p["file"].as_str(), p["sha1"].as_str()) else { continue };
        if file.contains(['/', '\\']) || file.contains("..") {
            continue;
        }
        let out = dir.join(file);
        if net::sha1_file(&out).map_or(true, |h| !h.eq_ignore_ascii_case(sha)) {
            let bytes = raw(repo, &format!("packs/{file}"), &token).await?;
            if !net::sha1_hex(&bytes).eq_ignore_ascii_case(sha) {
                return Err(anyhow!("el pack {file} se ha descargado dañado"));
            }
            std::fs::write(&out, bytes)?;
        }
        names.push(file.to_string());
    }
    let state_path = dir.join(STATE);
    let old: Vec<String> = std::fs::read(&state_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    for f in &old {
        if !names.contains(f) {
            let _ = std::fs::remove_file(dir.join(f));
        }
    }
    std::fs::write(&state_path, serde_json::to_vec(&names)?)?;
    enable(inst_dir, &old, &names)?;
    Ok(names.len())
}

/// Activa los packs en `options.txt` (resourcePacks): por encima de los demas y en el orden del repo (el ultimo arriba).
fn enable(inst_dir: &Path, old: &[String], names: &[String]) -> Result<()> {
    let path = inst_dir.join("options.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let mine: Vec<String> = names.iter().map(|n| format!("file/{n}")).collect();
    let gone: Vec<String> = old.iter().map(|n| format!("file/{n}")).collect();
    let idx = lines.iter().position(|l| l.starts_with("resourcePacks:"));
    let mut list: Vec<String> = idx
        .and_then(|i| serde_json::from_str(&lines[i]["resourcePacks:".len()..]).ok())
        .unwrap_or_else(|| vec!["vanilla".into(), "fabric".into()]);
    list.retain(|e| !mine.contains(e) && !gone.contains(e));
    list.extend(mine.iter().cloned());
    let line = format!("resourcePacks:{}", serde_json::to_string(&list)?);
    match idx {
        Some(i) => lines[i] = line,
        None => lines.push(line),
    }
    // si Minecraft los marco como incompatibles, se quitan de esa lista para que no pregunte
    if let Some(i) = lines.iter().position(|l| l.starts_with("incompatibleResourcePacks:")) {
        let mut inc: Vec<String> = serde_json::from_str(&lines[i]["incompatibleResourcePacks:".len()..]).unwrap_or_default();
        inc.retain(|e| !mine.contains(e));
        lines[i] = format!("incompatibleResourcePacks:{}", serde_json::to_string(&inc)?);
    }
    std::fs::write(&path, lines.join("\n") + "\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::enable;

    #[test]
    fn activa_los_packs_encima_y_quita_los_viejos() {
        let dir = std::env::temp_dir().join(format!("zeres-packs-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("options.txt"), "fov:0.0\nresourcePacks:[\"vanilla\",\"fabric\",\"file/zerescore-pack.zip\",\"file/viejo.zip\",\"file/mio.zip\"]\nincompatibleResourcePacks:[\"file/zerescore-pack.zip\"]\n").unwrap();
        let names = vec!["ZeresPack-servidor.zip".to_string(), "zerescore-pack.zip".to_string()];
        enable(&dir, &["viejo.zip".to_string()], &names).unwrap();
        let out = std::fs::read_to_string(dir.join("options.txt")).unwrap();
        assert!(out.contains("resourcePacks:[\"vanilla\",\"fabric\",\"file/mio.zip\",\"file/ZeresPack-servidor.zip\",\"file/zerescore-pack.zip\"]"), "{out}");
        assert!(out.contains("incompatibleResourcePacks:[]"), "{out}");
        assert!(out.starts_with("fov:0.0\n"));
        // sin options.txt (primera vez)
        std::fs::remove_file(dir.join("options.txt")).unwrap();
        enable(&dir, &[], &names).unwrap();
        let out = std::fs::read_to_string(dir.join("options.txt")).unwrap();
        assert_eq!(out, "resourcePacks:[\"vanilla\",\"fabric\",\"file/ZeresPack-servidor.zip\",\"file/zerescore-pack.zip\"]\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod red {
    /// Descarga real del repo privado (necesita ZERES_PACKS_TOKEN al compilar): cargo test -- --ignored packs_reales
    #[tokio::test]
    #[ignore]
    async fn packs_reales() {
        let dir = std::env::temp_dir().join(format!("zeres-packs-red-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let n = super::sync("Zeres-Network/zeres-resourcepacks", &dir).await.unwrap();
        assert_eq!(n, 2);
        for f in ["ZeresPack-servidor.zip", "zerescore-pack.zip"] {
            assert!(dir.join("resourcepacks").join(f).exists(), "{f}");
        }
        let opts = std::fs::read_to_string(dir.join("options.txt")).unwrap();
        println!("{opts}");
        // segunda vez: ya estan, no descarga nada y deja lo mismo
        assert_eq!(super::sync("Zeres-Network/zeres-resourcepacks", &dir).await.unwrap(), 2);
        assert_eq!(std::fs::read_to_string(dir.join("options.txt")).unwrap(), opts);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
