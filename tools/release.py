"""Publica una version de Launcher Minecraft en GitHub: compila firmado, prepara latest.json y crea el release.

Uso:  python tools/release.py [--notes "texto"] [--no-publish]
Release minecraft-v<version> marcado como pre-release (no pasa a "latest", asi no afecta a Zeres Launcher) y su latest.json
se sube al release fijo "launcher-minecraft", de donde lee el actualizador. La version sale de src-tauri/tauri.conf.json
(subela antes, tambien en Cargo.toml). La clave de firma NO esta en el repo y este launcher no lleva el token de los packs.
"""
import datetime, json, os, shutil, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPO = "Zeres-Network/zeres-launcher"
KEY = os.environ.get("ZERES_SIGNING_KEY", "C:/Users/Administrator/zeres-devtools/zeres-launcher-updater.key")
POINTER = "launcher-minecraft"


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
    env.pop("ZERES_PACKS_TOKEN", None)
    subprocess.run("npx tauri build", cwd=ROOT, env=env, shell=True, check=True)
    tag = f"minecraft-v{version}"
    nsis = os.path.join(ROOT, "src-tauri", "target", "release", "bundle", "nsis")
    setup = os.path.join(nsis, f"{conf['productName']}_{version}_x64-setup.exe")
    sig = open(setup + ".sig", encoding="utf-8").read().strip()
    dist = os.path.join(ROOT, "dist", version)
    os.makedirs(dist, exist_ok=True)
    asset = f"LauncherMinecraft_{version}_x64-setup.exe"
    shutil.copyfile(setup, os.path.join(dist, asset))
    latest = {
        "version": version,
        "notes": notes or f"Launcher Minecraft {version}",
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": {"windows-x86_64": {"signature": sig, "url": f"https://github.com/{REPO}/releases/download/{tag}/{asset}"}},
    }
    json.dump(latest, open(os.path.join(dist, "latest.json"), "w", encoding="utf-8"), indent=2, ensure_ascii=False)
    print("Listo en", dist)
    if "--no-publish" in args:
        return
    subprocess.run(["gh", "release", "create", tag, os.path.join(dist, asset), os.path.join(dist, "latest.json"), "--repo", REPO,
                    "--prerelease", "--title", f"Launcher Minecraft {version}", "--notes", notes or f"Launcher Minecraft {version}"], check=True)
    if subprocess.run(["gh", "release", "view", POINTER, "--repo", REPO], capture_output=True).returncode != 0:
        subprocess.run(["gh", "release", "create", POINTER, "--repo", REPO, "--prerelease", "--title", "Launcher Minecraft (actualizador)",
                        "--notes", "Puntero del actualizador de Launcher Minecraft: solo contiene latest.json."], check=True)
    subprocess.run(["gh", "release", "upload", POINTER, os.path.join(dist, "latest.json"), "--repo", REPO, "--clobber"], check=True)


main()
