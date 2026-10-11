//! Estilo Minecraft sin redistribuir nada de Mojang: los botones, la fuente, las skins por defecto y la panoramica
//! se sacan del Minecraft que el launcher ya descarga para el jugador (jar del cliente + assets) y se guardan en
//! `<datos>/theme`, que la interfaz lee por el protocolo `zres://`.

use crate::mojang;
use crate::net;
use anyhow::Result;
use std::io::Read;
use std::path::PathBuf;

pub const THEME_VERSION: &str = "1.21.1";
const MARKER: &str = ".theme-v1";

pub fn dir() -> PathBuf {
    crate::settings::base_dir().join("theme")
}

/// (ruta dentro del jar, nombre en theme/)
const FROM_JAR: &[(&str, &str)] = &[
    ("assets/minecraft/font/include/default.json", "font/default.json"),
    ("assets/minecraft/textures/font/ascii.png", "font/ascii.png"),
    ("assets/minecraft/textures/font/accented.png", "font/accented.png"),
    ("assets/minecraft/textures/font/nonlatin_european.png", "font/nonlatin_european.png"),
    ("assets/minecraft/textures/gui/menu_list_background.png", "gui/menu_list_background.png"),
    ("assets/minecraft/textures/gui/menu_background.png", "gui/menu_background.png"),
    ("assets/minecraft/textures/gui/header_separator.png", "gui/header_separator.png"),
    ("assets/minecraft/textures/gui/footer_separator.png", "gui/footer_separator.png"),
    ("assets/minecraft/textures/misc/unknown_server.png", "gui/unknown_server.png"),
    ("assets/minecraft/textures/entity/player/wide/steve.png", "skin/steve.png"),
    ("assets/minecraft/textures/entity/player/slim/alex.png", "skin/alex.png"),
    ("assets/minecraft/textures/block/red_nether_bricks.png", "block/red_nether_bricks.png"),
    ("assets/minecraft/textures/block/lapis_block.png", "block/lapis_block.png"),
    ("assets/minecraft/textures/block/stone.png", "block/stone.png"),
];
/// carpetas de sprites que se copian enteras
const SPRITE_DIRS: &[(&str, &str)] = &[
    ("assets/minecraft/textures/gui/sprites/widget/", "gui/"),
    ("assets/minecraft/textures/gui/sprites/server_list/", "gui/server_list/"),
    ("assets/minecraft/textures/gui/sprites/icon/", "gui/icon/"),
];

pub fn ready() -> bool {
    dir().join(MARKER).exists()
}

pub async fn ensure<F>(progress: F) -> Result<()>
where
    F: Fn(&str, usize, usize) + Send + Sync + 'static,
{
    if ready() {
        return Ok(());
    }
    let progress = std::sync::Arc::new(progress);
    progress("Preparando el launcher", 0, 0);
    let v = mojang::version_json(THEME_VERSION).await?;
    let p = progress.clone();
    net::download_all(vec![mojang::client_job(&v, THEME_VERSION)], 1, false, move |d, t| p("Descargando Minecraft", d, t)).await?;
    let (_, index) = mojang::asset_index(&v).await?;
    let mut pano = Vec::new();
    for n in 0..6 {
        let key = format!("minecraft/textures/gui/title/background/panorama_{n}.png");
        if let Some(h) = index["objects"][&key]["hash"].as_str() {
            pano.push((n, mojang::asset_job(h, index["objects"][&key]["size"].as_u64())));
        }
    }
    let p = progress.clone();
    net::download_all(pano.iter().map(|(_, j)| j.clone()).collect(), 6, false, move |d, t| p("Descargando la panorámica", d, t)).await?;

    let out = dir();
    let jar = mojang::client_jar(THEME_VERSION);
    let pano2 = pano.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&jar)?)?;
        for i in 0..zip.len() {
            let mut e = zip.by_index(i)?;
            let name = e.name().to_string();
            let target = FROM_JAR
                .iter()
                .find(|(src, _)| *src == name)
                .map(|(_, dst)| dst.to_string())
                .or_else(|| {
                    SPRITE_DIRS.iter().find_map(|(src, dst)| {
                        name.strip_prefix(src).filter(|r| r.ends_with(".png") && !r.contains('/')).map(|r| format!("{dst}{r}"))
                    })
                });
            if let Some(t) = target {
                let path = out.join(t);
                std::fs::create_dir_all(path.parent().unwrap())?;
                let mut buf = Vec::new();
                e.read_to_end(&mut buf)?;
                std::fs::write(path, buf)?;
            }
        }
        std::fs::create_dir_all(out.join("panorama"))?;
        for (n, job) in pano2 {
            std::fs::copy(&job.path, out.join("panorama").join(format!("panorama_{n}.png")))?;
        }
        std::fs::write(out.join(MARKER), THEME_VERSION)?;
        Ok(())
    })
    .await??;
    Ok(())
}
