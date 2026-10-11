//! Descargas: cliente HTTP compartido, lectura de URLs (http y file://) y descarga en paralelo con comprobacion de sha1.

use anyhow::{anyhow, Context, Result};
use futures::stream::{self, StreamExt};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

pub fn client() -> &'static reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("ZeresLauncher/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("cliente http")
    })
}

/// Ruta local si `url` es `file://...` o una ruta de disco.
fn local_path(url: &str) -> Option<PathBuf> {
    if let Ok(u) = url::Url::parse(url) {
        if u.scheme() == "file" {
            return u.to_file_path().ok();
        }
        if u.scheme().len() == 1 {
            return Some(PathBuf::from(url)); // "C:\..."
        }
        return None;
    }
    Some(PathBuf::from(url))
}

pub async fn get_bytes(url: &str) -> Result<Vec<u8>> {
    if let Some(p) = local_path(url) {
        return tokio::fs::read(&p).await.with_context(|| format!("no se pudo leer {}", p.display()));
    }
    let mut last = anyhow!("sin intentos");
    for attempt in 0..3 {
        match client().get(url).send().await.and_then(|r| r.error_for_status()) {
            Ok(r) => match r.bytes().await {
                Ok(b) => return Ok(b.to_vec()),
                Err(e) => last = e.into(),
            },
            Err(e) => last = e.into(),
        }
        tokio::time::sleep(std::time::Duration::from_millis(400 * (attempt + 1))).await;
    }
    Err(last.context(format!("no se pudo descargar {url}")))
}

pub async fn get_json(url: &str) -> Result<serde_json::Value> {
    let b = get_bytes(url).await?;
    serde_json::from_slice(&b).with_context(|| format!("JSON invalido en {url}"))
}

pub fn sha1_hex(data: &[u8]) -> String {
    hex::encode(Sha1::digest(data))
}

pub fn sha1_file(p: &Path) -> Option<String> {
    std::fs::read(p).ok().map(|d| sha1_hex(&d))
}

#[derive(Clone, Debug)]
pub struct Job {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// ¿El archivo ya esta bien? Con `deep` se comprueba el sha1; si no, basta con que exista con el tamaño correcto.
pub fn is_ok(job: &Job, deep: bool) -> bool {
    let Ok(meta) = std::fs::metadata(&job.path) else { return false };
    if let Some(size) = job.size {
        if meta.len() != size {
            return false;
        }
    }
    if deep || job.size.is_none() {
        if let Some(h) = &job.sha1 {
            return sha1_file(&job.path).map(|x| x.eq_ignore_ascii_case(h)).unwrap_or(false);
        }
    }
    true
}

async fn fetch_one(job: &Job) -> Result<()> {
    let data = get_bytes(&job.url).await?;
    if let Some(h) = &job.sha1 {
        let got = sha1_hex(&data);
        if !got.eq_ignore_ascii_case(h) {
            return Err(anyhow!("hash incorrecto en {} (esperado {h}, recibido {got})", job.url));
        }
    }
    if let Some(dir) = job.path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = job.path.with_extension("part");
    tokio::fs::write(&tmp, &data).await?;
    tokio::fs::rename(&tmp, &job.path).await?;
    Ok(())
}

/// Descarga lo que falte (en paralelo). `progress(hechos, total)` se llama tras cada archivo.
pub async fn download_all<F>(jobs: Vec<Job>, parallel: usize, deep: bool, progress: F) -> Result<()>
where
    F: Fn(usize, usize) + Send + Sync + 'static,
{
    let pending: Vec<Job> = tokio::task::spawn_blocking(move || jobs.into_iter().filter(|j| !is_ok(j, deep)).collect())
        .await
        .unwrap_or_default();
    let total = pending.len();
    if total == 0 {
        return Ok(());
    }
    let done = Arc::new(AtomicUsize::new(0));
    let progress = Arc::new(progress);
    progress(0, total);
    let results: Vec<Result<()>> = stream::iter(pending.into_iter().map(|job| {
        let done = done.clone();
        let progress = progress.clone();
        async move {
            let r = fetch_one(&job).await;
            let n = done.fetch_add(1, Ordering::SeqCst) + 1;
            progress(n, total);
            r
        }
    }))
    .buffer_unordered(parallel.clamp(1, 32))
    .collect()
    .await;
    let errors: Vec<String> = results.into_iter().filter_map(|r| r.err().map(|e| e.to_string())).collect();
    if !errors.is_empty() {
        return Err(anyhow!("{} archivo(s) no se pudieron descargar. Primero: {}", errors.len(), errors[0]));
    }
    Ok(())
}
