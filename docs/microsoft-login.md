# Cuentas premium (Microsoft): cómo activarlas

El código del launcher ya está hecho (`src-tauri/src/auth.rs`): inicio de sesión con código de dispositivo →
Xbox Live → Minecraft. Solo falta el **ID de la aplicación de Azure** y que **Mojang la apruebe**. Es gratis.

## 1. Registrar la aplicación en Azure (5 minutos)
0. **Si tu cuenta es personal (Outlook, Hotmail, Xbox) y es la primera vez que usas Azure**, al entrar en el portal
   sale el error *"Selected user account does not exist in tenant 'Microsoft Services'..."*. Antes hay que crear tu
   directorio gratis en <https://azure.microsoft.com/free> → **Comienza gratis**. Pide teléfono y tarjeta solo para
   verificar; no cobra nada, y registrar aplicaciones es gratis. Cuando termine, ya puedes seguir con el paso 1.
1. Entra en <https://portal.azure.com> con una cuenta de Microsoft. Vale cualquiera; mejor una del equipo de Zeres.
2. Busca **"Microsoft Entra ID"** → **Registros de aplicaciones** → **Nuevo registro**:
   - **Nombre:** `Zeres Launcher`.
   - **Tipos de cuenta admitidos:** *Solo cuentas personales de Microsoft*.
   - **URI de redirección:** déjalo vacío.
   - Pulsa **Registrar**.
3. En la aplicación creada, ve a **Autenticación** → **Configuración avanzada** → *Permitir flujos de clientes
   públicos* = **Sí** → **Guardar**.
4. Copia el **Id. de aplicación (cliente)** de la página **Información general**. Es un código del tipo
   `1a2b3c4d-....`.

## 2. Pedir a Mojang acceso a la API de Minecraft
Las aplicaciones nuevas tienen que estar aprobadas para poder iniciar sesión en Minecraft. Hay que rellenar el
formulario de revisión de Minecraft:
- <https://aka.ms/mce-reviewappid>. Si el enlace cambia, busca "Minecraft Services API app review form".
- **Datos que piden:**
  - el Id. de aplicación del paso 1;
  - el nombre (`Zeres Launcher`);
  - una descripción: *"Launcher de código abierto del servidor Zeres Network: descarga el juego y el modpack y
    permite iniciar sesión con la cuenta de Minecraft del jugador"*;
  - el enlace al repo: <https://github.com/Zeres-Network/zeres-launcher>.
- La respuesta llega por correo y puede tardar unos días o semanas.

## 3. Cuando esté aprobada
Pásame el Id. de aplicación. Lo pongo en `MS_CLIENT_ID` (`src-tauri/src/auth.rs`) y publico una versión nueva. El
botón "+ Microsoft" del perfil empezará a funcionar. Mientras no esté aprobada, ese botón avisa de que aún no
está disponible.

El Id. de aplicación **no es secreto**: puede ir en el código público. Nunca compartas contraseñas ni "secretos de
cliente". Para este tipo de aplicación no se usan.
