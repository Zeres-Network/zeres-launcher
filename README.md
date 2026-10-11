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
`%APPDATA%\ZeresLauncher`: `launcher.json` (ajustes y cuentas), `instances/`, `versions/`, `libraries/`, `assets/`, `runtime/` (Java) y `theme/`.

## Compilar
```
npm install
npx tauri build          # instalador en src-tauri/target/release/bundle/nsis/
```
Para ver la interfaz en un navegador sin compilar, con un simulador del núcleo: `python dev/serve.py 8770` (hace falta haber abierto el launcher una vez, para que exista `theme/`).

Prueba sin interfaz: `zeres-launcher.exe --selftest cobblemon --launch` (resultado en `%APPDATA%\ZeresLauncher\selftest.log`).

## Antes de publicar
- [ ] `settings.rs` → `DEFAULT_MANIFEST`: cambiar la carpeta local por la URL raw del repo `zeres-network-modpack`.
- [ ] `auth.rs` → `MS_CLIENT_ID`: client ID de Azure aprobado por Mojang.
- [ ] Panorámicas propias del spawn en `ui/img/panorama/<instancia>/panorama_0..5.png` (si existen, se usan en vez de las del juego).
- [ ] Firma con SignPath y actualizaciones automáticas.
