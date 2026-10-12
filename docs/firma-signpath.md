# Firma gratis del instalador (SignPath Foundation)

Sin firma, Windows enseña "Windows protegió tu PC" al instalar el launcher. **SignPath Foundation** firma gratis
proyectos de código abierto con un certificado de verdad. Lo que piden y cómo está cada cosa:

| Requisito | Estado |
|---|---|
| Repositorio público con licencia libre | Hecho: `Zeres-Network/zeres-launcher`, MIT |
| Versiones publicadas | Hecho: releases v0.1.x en GitHub |
| Política de firma de código en el repo | Hecho: sección "Política de firma de código" del README |
| Compilar en un CI público (GitHub Actions), no en un PC | Preparado: `.github/workflows/release.yml` |
| Solicitud enviada | **Falta (tú)** |

## 1. Enviar la solicitud
1. Entra en <https://signpath.org> → **Apply** (formulario para proyectos open source).
2. Datos:
   - **Proyecto:** Zeres Launcher.
   - **Repositorio:** <https://github.com/Zeres-Network/zeres-launcher>.
   - **Licencia:** MIT.
   - **Qué se firma:** el instalador de Windows (NSIS) que genera GitHub Actions.
   - **Correo de contacto:** el tuyo.
3. Revisan el proyecto y responden por correo. Si lo aprueban, te dan acceso a su panel con un **organization
   id**, un **project slug** y una **signing policy**.

## 2. Cuando lo aprueben (lo hago yo)
- **Secretos del repo de GitHub** (Settings → Secrets → Actions):
  - `TAURI_SIGNING_PRIVATE_KEY`: la clave del actualizador (`zeres-devtools/zeres-launcher-updater.key`);
  - `SIGNPATH_API_TOKEN`: el token que da SignPath.
- **Workflow:** activar el paso de SignPath en `.github/workflows/release.yml` (está comentado) con el
  organization id y el project slug.
- **Publicar:** a partir de ahí, las versiones se publican subiendo una etiqueta (`git tag v0.2.0 && git push --tags`).
  GitHub las compila, SignPath firma el instalador y se sube el release con su `latest.json`.

Hasta entonces se sigue publicando como ahora, con `python tools/release.py`.
