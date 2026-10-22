"""Publica una version del launcher en GitHub: compila firmado, prepara latest.json (autoactualizacion) y crea el release.

Uso:  python tools/release.py [--notes "texto"] [--no-publish] [--canal dev]
Canal beta (por defecto): release v<version> normal; el launcher lo busca en releases/latest/download/latest.json.
Canal dev (solo staff): release dev-v<version> marcado como pre-release (no pasa a "latest") y su latest.json se sube
al release fijo "canal-dev"; usa una version propia (p. ej. 0.2.0-dev.1).
La version sale de src-tauri/tauri.conf.json (subela antes, tambien en Cargo.toml).
La clave privada de firma NO esta en el repo: se lee de ZERES_SIGNING_KEY o de
C:/Users/Administrator/zeres-devtools/zeres-launcher-updater.key (en Linux, de ../zeres-devtools/ junto al repo).

Linux: primero se publica la version desde Windows y despues se ejecuta lo mismo en Linux (python3 tools/release.py
[--canal dev]): compila AppImage, .rpm y .deb, los sube a ese mismo release y añade sus entradas al latest.json
(sin tocar la de Windows). El AppImage compilado en una distro muy nueva no arranca en distros con glibc antigua: para
repartirlo a todos, mejor el del workflow (Ubuntu 22.04).
"""
import datetime, json, os, shutil, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPO = "Zeres-Network/zeres-launcher"
LINUX = sys.platform.startswith("linux")
DEFAULT_KEY = (os.path.join(os.path.dirname(ROOT), "zeres-devtools", "zeres-launcher-updater.key") if LINUX
               else "C:/Users/Administrator/zeres-devtools/zeres-launcher-updater.key")
KEY = os.environ.get("ZERES_SIGNING_KEY", DEFAULT_KEY)
PACKS_TOKEN = os.path.join(os.path.dirname(KEY), "zeres-packs-token.txt")


def main():
    args = sys.argv[1:]
    notes = args[args.index("--notes") + 1] if "--notes" in args else ""
    dev = "--canal" in args and args[args.index("--canal") + 1] == "dev"
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
    tag = f"dev-v{version}" if dev else f"v{version}"
    if LINUX:
        return release_linux(version, tag, notes, dev, "--no-publish" in args)

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
        "platforms": {"windows-x86_64": {"signature": sig, "url": f"https://github.com/{REPO}/releases/download/{tag}/{asset}"}},
    }
    json.dump(latest, open(os.path.join(dist, "latest.json"), "w", encoding="utf-8"), indent=2, ensure_ascii=False)
    print("Listo en", dist)
    if "--no-publish" in args:
        return
    subprocess.run(["gh", "release", "create", tag, os.path.join(dist, asset), os.path.join(dist, "latest.json"),
                    "--repo", REPO, "--title", f"Zeres Launcher {version} ({'dev' if dev else 'beta'})",
                    "--notes", notes or f"Zeres Launcher {version}"] + (["--prerelease"] if dev else []),
                   check=True)
    if dev:
        # el canal dev lee siempre el latest.json del release fijo "canal-dev"
        if subprocess.run(["gh", "release", "view", "canal-dev", "--repo", REPO], capture_output=True).returncode != 0:
            subprocess.run(["gh", "release", "create", "canal-dev", "--repo", REPO, "--prerelease", "--title", "Canal dev (staff)",
                            "--notes", "Puntero del canal dev del launcher: solo contiene latest.json."], check=True)
        subprocess.run(["gh", "release", "upload", "canal-dev", os.path.join(dist, "latest.json"), "--repo", REPO, "--clobber"], check=True)


def release_linux(version, tag, notes, dev, no_publish):
    """Sube AppImage, .rpm y .deb al release `tag` (ya creado desde Windows) y los añade a su latest.json."""
    bundle = os.path.join(ROOT, "src-tauri", "target", "release", "bundle")
    # (clave del actualizador, carpeta del bundle, extension, nombre del asset)
    kinds = [("linux-x86_64-appimage", "appimage", ".AppImage", f"ZeresLauncher_{version}_amd64.AppImage"),
             ("linux-x86_64-rpm", "rpm", ".rpm", f"ZeresLauncher_{version}_x86_64.rpm"),
             ("linux-x86_64-deb", "deb", ".deb", f"ZeresLauncher_{version}_amd64.deb")]
    dist = os.path.join(ROOT, "dist", version)
    os.makedirs(dist, exist_ok=True)
    latest_path = os.path.join(dist, "latest.json")
    if not no_publish:
        # el latest.json que subio Windows (si no hay release todavia, se empieza uno solo con Linux)
        subprocess.run(["gh", "release", "download", tag, "--repo", REPO, "--pattern", "latest.json", "--dir", dist, "--clobber"])
    latest = json.load(open(latest_path, encoding="utf-8")) if os.path.exists(latest_path) else {}
    if latest.get("version") != version:
        latest = {"version": version, "notes": notes or f"Zeres Launcher {version}",
                  "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), "platforms": {}}
    assets = []
    for key, folder, ext, asset in kinds:
        files = [f for f in os.listdir(os.path.join(bundle, folder)) if f.endswith(ext) and version in f]
        if not files:
            sys.exit(f"no se ha generado el {ext} en {bundle}/{folder}")
        src = os.path.join(bundle, folder, files[0])
        shutil.copyfile(src, os.path.join(dist, asset))
        sig = open(src + ".sig", encoding="utf-8").read().strip()
        latest["platforms"][key] = {"signature": sig, "url": f"https://github.com/{REPO}/releases/download/{tag}/{asset}"}
        assets.append(os.path.join(dist, asset))
    # sin sufijo: el AppImage (lo que usan los actualizadores que no saben como se instalo el launcher)
    latest["platforms"]["linux-x86_64"] = latest["platforms"]["linux-x86_64-appimage"]
    json.dump(latest, open(latest_path, "w", encoding="utf-8"), indent=2, ensure_ascii=False)
    print("Listo en", dist)
    if no_publish:
        return
    if subprocess.run(["gh", "release", "view", tag, "--repo", REPO], capture_output=True).returncode != 0:
        subprocess.run(["gh", "release", "create", tag, "--repo", REPO, "--title", f"Zeres Launcher {version} ({'dev' if dev else 'beta'})",
                        "--notes", notes or f"Zeres Launcher {version}"] + (["--prerelease"] if dev else []), check=True)
    subprocess.run(["gh", "release", "upload", tag, *assets, latest_path, "--repo", REPO, "--clobber"], check=True)
    if dev:
        subprocess.run(["gh", "release", "upload", "canal-dev", latest_path, "--repo", REPO, "--clobber"], check=True)


main()
