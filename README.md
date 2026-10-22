# Zeres Launcher (BETA 0.1.0)

Launcher de Zeres Network con dos instancias del mismo servidor:

- **Zeres Network**: Minecraft 1.21.1 sin mods (survival, skyblock...).
- **Zeres Cobblemon**: Fabric + el modpack de Cobblemon.

Hecho desde cero con Rust + Tauri 2. La interfaz es HTML/CSS/JS sin frameworks.

## Cómo funciona
- **Java:** se descarga el oficial de Mojang. El jugador no instala nada.
- **Minecraft:** versión, librerías, recursos y Fabric salen de los servidores oficiales. Todo se comprueba con sha1 y se descarga en paralelo.
- **Modpacks:** se leen de `manifest.json` y de un `.mrpack`.
  - Los mods públicos se bajan del CDN de Modrinth.
  - Solo los propios (ZeresClient) van dentro del pack.
  - Al actualizar se descarga únicamente lo que ha cambiado, y se borran los mods que el pack nuevo ya no trae.
- **Botones, fuente, skins y panorámica:** el estilo Minecraft sale del Minecraft que el launcher descarga para el jugador (`src-tauri/src/theme.rs`). El repo no lleva ningún archivo de Mojang.
- **Cuentas:**
  - Sin premium: el mismo UUID offline que da el servidor.
  - Microsoft: falta el client ID de Azure aprobado por Mojang (`auth.rs`, `MS_CLIENT_ID`).

## Datos del jugador
`%APPDATA%\ZeresLauncher` en Windows, `~/.local/share/ZeresLauncher` en Linux: `launcher.json` (ajustes y cuentas), `instances/`, `versions/`, `libraries/`, `assets/`, `runtime/` (Java) y `theme/`.

## Compilar
```
npm install
npx tauri build          # instalador en src-tauri/target/release/bundle/nsis/
```
### Linux
Mismo código; `src-tauri/tauri.linux.conf.json` cambia los paquetes a AppImage, `.rpm` y `.deb`. En Linux el launcher
usa el Java de Mojang para Linux, nativos `.so` y, en portátiles con NVIDIA (driver propietario), abre el juego en la
gráfica dedicada.

Fedora (sin tocar el sistema, dentro de un toolbox):
```
toolbox create zeres-build
toolbox run -c zeres-build sudo dnf install -y rust cargo nodejs npm webkit2gtk4.1-devel gtk3-devel libsoup3-devel \
    openssl-devel librsvg2-devel libappindicator-gtk3-devel patchelf file
toolbox run -c zeres-build sh -c "npm ci && npx tauri build"
sudo dnf install ./src-tauri/target/release/bundle/rpm/*.rpm     # fuera del toolbox
```
Ubuntu/Debian: `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libssl-dev patchelf`
y lo mismo (`npm ci && npx tauri build`). El AppImage que se reparte a todas las distros sale del workflow (Ubuntu 22.04).

Para ver la interfaz en un navegador sin compilar, con un simulador del núcleo: `python dev/serve.py 8770` (hace falta haber abierto el launcher una vez, para que exista `theme/`).

Pruebas sin interfaz: `zeres-launcher.exe --selftest cobblemon --launch` (resultado en `%APPDATA%\ZeresLauncher\selftest.log`) y `zeres-launcher.exe --check-update` (busca, descarga y comprueba la firma de la versión nueva sin instalarla; resultado en `update-check.log`).

## Antes de publicar
- [ ] `auth.rs` → `MS_CLIENT_ID`: client ID de Azure aprobado por Mojang.
- [ ] Panorámicas propias del spawn en `ui/img/panorama/<instancia>/panorama_0..5.png` (si existen, se usan en vez de las del juego).
- [ ] Firma del .exe con SignPath (el aviso de Windows).

## Publicar una versión (autoactualización)
1. Subir `version` en `src-tauri/tauri.conf.json` y en `src-tauri/Cargo.toml`.
2. `python tools/release.py --notes "qué cambia"`: compila firmado, crea `latest.json` y el release `v<versión>` en GitHub.
   Después, en Linux, `python3 tools/release.py` (mismas opciones) añade AppImage, `.rpm` y `.deb` a ese release.
3. Al abrirse, los launchers ya instalados ven la versión nueva, la descargan, comprueban la firma, se instalan y se reinician.

La clave privada de firma **no** está en el repo (`zeres-devtools/zeres-launcher-updater.key`). Si se pierde, los launchers instalados no podrán actualizarse solos: guárdala bien.

## Política de firma de código
- El instalador de Windows se firma gratis con [SignPath.io](https://about.signpath.io), con un certificado de
  [SignPath Foundation](https://signpath.org) (en trámite).
- Solo se firman versiones compiladas por GitHub Actions a partir de este repositorio público.
- Equipo:
  - **Autores y revisores:** [miembros de Zeres-Network](https://github.com/orgs/Zeres-Network/people).
  - **Aprobador de las firmas:** el propietario de la organización (Days16).
- **Privacidad:** el launcher no recoge datos. Solo se conecta a Mojang/Microsoft (juego y cuentas), a Fabric y
  Modrinth (modpack), a GitHub (actualizaciones y modpacks) y al servidor de Zeres (estado del servidor).
