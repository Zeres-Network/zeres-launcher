fn main() {
    // token de solo lectura del repo privado de packs (ver src/packs.rs): llega por variable de entorno al compilar y se
    // guarda ofuscado; sin variable, el launcher compila igual y no descarga packs
    println!("cargo:rerun-if-env-changed=ZERES_PACKS_TOKEN");
    let token = std::env::var("ZERES_PACKS_TOKEN").unwrap_or_default();
    let key = b"zeres-launcher";
    let enc: String = token.trim().bytes().enumerate().map(|(i, b)| format!("{:02x}", b ^ key[i % key.len()])).collect();
    println!("cargo:rustc-env=ZERES_PACKS_KEY={enc}");
    tauri_build::build()
}
