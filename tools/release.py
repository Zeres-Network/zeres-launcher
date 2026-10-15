"""Publica una version del launcher en GitHub: compila firmado, prepara latest.json (autoactualizacion) y crea el release.

Uso:  python tools/release.py [--notes "texto"] [--no-publish]
La version sale de src-tauri/tauri.conf.json (subela antes, tambien en Cargo.toml).
La clave privada de firma NO esta en el repo: se lee de ZERES_SIGNING_KEY o de
C:/Users/Administrator/zeres-devtools/zeres-launcher-updater.key (sin contraseña).
"""
import datetime, json, os, shutil, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPO = "Zeres-Network/zeres-launcher"
KEY = os.environ.get("ZERES_SIGNING_KEY", "C:/Users/Administrator/zeres-devtools/zeres-launcher-updater.key")
PACKS_TOKEN = os.path.join(os.path.dirname(KEY), "zeres-packs-token.txt")


def main():
    args = sys.argv[1:]
    notes = args[args.index("--notes") + 1] if "--notes" in args else ""
    conf = json.load(open(os.path.join(ROOT, "src-tauri", "tauri.conf.json"), encoding="utf-8"))
    version = conf["version"]
    cargo = open(os.path.join(ROOT, "src-tauri", "Cargo.toml"), encoding="utf-8").read()
    if f'version = "{version}"' not in cargo:
        sys.exit(f"Cargo.toml no tiene version = \"{version}\": sube las dos a la vez")

    env = dict(os.environ)
    env["TAURI_SIGNING_PRIVATE_KEY"] = open(KEY, encoding="utf-8").read().strip()
    env["TAURI_SIGNING_PRIVATE_KEY_PASSWORD"] = ""
    # token de solo lectura del repo privado de packs de texturas (ver src-tauri/src/packs.rs); nunca va en git
    if os.path.exists(PACKS_TOKEN):
        env["ZERES_PACKS_TOKEN"] = open(PACKS_TOKEN, encoding="utf-8").read().strip()
    elif "--sin-packs" not in args:
        sys.exit(f"falta {PACKS_TOKEN} (token del repo de packs); usa --sin-packs para compilar sin packs")
    subprocess.run("npx tauri build", cwd=ROOT, env=env, shell=True, check=True)

    nsis = os.path.join(ROOT, "src-tauri", "target", "release", "bundle", "nsis")
    setup = os.path.join(nsis, f"{conf['productName']}_{version}_x64-setup.exe")
    sig = open(setup + ".sig", encoding="utf-8").read().strip()
    dist = os.path.join(ROOT, "dist", version)
    os.makedirs(dist, exist_ok=True)
    asset = f"ZeresLauncher_{version}_x64-setup.exe"
    shutil.copyfile(setup, os.path.join(dist, asset))
    latest = {
        "version": version,
        "notes": notes or f"Zeres Launcher {version}",
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": {"windows-x86_64": {"signature": sig, "url": f"https://github.com/{REPO}/releases/download/v{version}/{asset}"}},
    }
    json.dump(latest, open(os.path.join(dist, "latest.json"), "w", encoding="utf-8"), indent=2, ensure_ascii=False)
    print("Listo en", dist)
    if "--no-publish" in args:
        return
    subprocess.run(["gh", "release", "create", f"v{version}", os.path.join(dist, asset), os.path.join(dist, "latest.json"),
                    "--repo", REPO, "--title", f"Zeres Launcher {version} (beta)", "--notes", notes or f"Zeres Launcher {version}"],
                   check=True)


main()
