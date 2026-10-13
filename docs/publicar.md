# Cómo publicar una versión de Tlaku

Una versión se publica empujando una etiqueta `v*` al repositorio. El flujo
[`.github/workflows/release.yml`](../.github/workflows/release.yml) compila los instaladores en
tres sistemas y los deja en un **borrador** de GitHub Release; tú lo revisas y lo publicas.

| Sistema | Instaladores | Runner |
|---|---|---|
| macOS | `.dmg` (binario universal: Apple Silicon e Intel) y `.app.tar.gz` | `macos-latest` |
| Windows | `.exe` (NSIS) y `.msi` | `windows-latest` |
| Linux | `.AppImage` y `.deb` | `ubuntu-22.04` |

La firma de código es **opcional**: sin secretos, el flujo publica instaladores sin firmar y no
falla (ver [Firma](#firma-de-código) y [Qué ve el usuario sin firma](#qué-ve-el-usuario-si-no-hay-firma)).

## Sacar una versión

1. **Sube la versión en los tres lugares** (deben coincidir; el flujo lo comprueba contra la
   etiqueta y se detiene si algo no cuadra):

   | Archivo | Qué cambiar |
   |---|---|
   | `Cargo.toml` (raíz) | `version` en `[workspace.package]`. Todos los crates, incluido `apps/desktop/src-tauri`, la heredan. |
   | `apps/desktop/src-tauri/tauri.conf.json` | `"version"`. Es la versión que muestran los instaladores. |
   | `apps/desktop/package.json` | `"version"`. |

   ```sh
   # desde la raíz del repositorio, por ejemplo para 0.2.0
   sed -i '' 's/^version = ".*"/version = "0.2.0"/' Cargo.toml   # en Linux: sed -i sin ''
   # revisa que solo cambió la línea de [workspace.package]
   (cd apps/desktop && npm version 0.2.0 --no-git-tag-version)   # package.json y package-lock.json
   # edita "version" en apps/desktop/src-tauri/tauri.conf.json
   cargo check --workspace          # actualiza Cargo.lock
   ```

2. **Comprueba que todo pasa** (los mismos comandos que la CI):

   ```sh
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

3. **Confirma y etiqueta.** La etiqueta es la versión precedida de `v`:

   ```sh
   git commit -am "Versión 0.2.0"
   git tag -a v0.2.0 -m "Tlaku 0.2.0"
   git push origin main v0.2.0
   ```

4. **Espera al flujo** (pestaña Actions, "Publicar"). Compilar sin caché tarda varios minutos por
   sistema. Si un sistema falla, corrige y vuelve a lanzar solo ese trabajo con "Re-run": la
   acción actualiza el mismo borrador.

5. **Revisa el borrador** en Releases:
   - Están los instaladores de los tres sistemas (`.dmg`, `.exe`, `.msi`, `.AppImage`, `.deb`).
   - Las notas generadas automáticamente (lista de cambios y colaboradores) son legibles; edítalas
     si hace falta y añade lo importante para quien instala.
   - Descarga al menos un instalador y pruébalo.
   - Pulsa **Publish release**. Una etiqueta con guion (`v0.2.0-beta.1`) se marca como
     pre-lanzamiento.

Notas:

- Para probar el flujo sin crear Release, ejecútalo a mano desde Actions ("Run workflow"): compila y
  deja los instaladores como artefactos del flujo.
- El instalador `.msi` (WiX) solo admite pre-lanzamientos numéricos (`0.2.0-1`, no `0.2.0-beta.1`).
  Con una etiqueta tipo `-beta.1` el trabajo de Windows fallará; usa un sufijo numérico.
- Si te equivocas de etiqueta: `git push --delete origin v0.2.0 && git tag -d v0.2.0`, borra el
  borrador en GitHub y empieza otra vez.
- En Settings > Actions > General, "Workflow permissions" no debe bloquear al flujo (el flujo pide
  `contents: write` por sí mismo).

## Firma de código

Todo es opcional y se configura con secretos del repositorio (Settings > Secrets and variables >
Actions > New repository secret). El flujo detecta qué hay y firma solo lo que puede.

### Resumen de secretos

| Secreto | Plataforma | Obligatorio para | Contenido |
|---|---|---|---|
| `APPLE_CERTIFICATE` | macOS | firmar | Certificado "Developer ID Application" exportado como `.p12`, en base64 |
| `APPLE_CERTIFICATE_PASSWORD` | macOS | firmar | Contraseña con la que exportaste el `.p12` |
| `APPLE_SIGNING_IDENTITY` | macOS | firmar | Por ejemplo `Developer ID Application: Tu Nombre (ABCDE12345)` |
| `APPLE_ID` | macOS | notarizar | Correo de tu Apple ID |
| `APPLE_PASSWORD` | macOS | notarizar | Contraseña específica de app (no la del Apple ID) |
| `APPLE_TEAM_ID` | macOS | notarizar | Identificador de equipo de 10 caracteres |
| `WINDOWS_CERTIFICATE` | Windows | firmar | Certificado de firma de código `.pfx`, en base64 |
| `WINDOWS_CERTIFICATE_PASSWORD` | Windows | firmar | Contraseña del `.pfx` |
| `TAURI_SIGNING_PRIVATE_KEY` | todas | actualizaciones (desactivadas) | Llave privada del actualizador |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | todas | actualizaciones (desactivadas) | Su contraseña |

`GITHUB_TOKEN` lo pone GitHub; no hay que crearlo.

### macOS: firmar y notarizar

Requiere una cuenta del Apple Developer Program (de pago).

1. En developer.apple.com crea un certificado **Developer ID Application** y descárgalo a tu
   llavero.
2. Averigua la identidad exacta: `security find-identity -v -p codesigning`. Ese texto completo es
   `APPLE_SIGNING_IDENTITY`.
3. En Acceso a Llaveros exporta el certificado con su llave privada como `.p12` (con contraseña) y
   codifícalo en una sola línea: `base64 -i certificado.p12 | pbcopy`. Pégalo en
   `APPLE_CERTIFICATE`.
4. Para notarizar: en appleid.apple.com crea una **contraseña específica de app** (`APPLE_PASSWORD`)
   y anota tu `APPLE_ID` y `APPLE_TEAM_ID` (visible en developer.apple.com > Membership).
5. Con los tres primeros secretos la app se firma; con los seis también se notariza y el
   instalador abre sin advertencias.

La app usa la cámara, así que con el runtime reforzado necesita el permiso declarado en
[`Entitlements.plist`](../apps/desktop/src-tauri/Entitlements.plist) (ya configurado en
`bundle.macOS`).

### Windows: firmar

Hace falta un certificado de firma de código (de una autoridad certificadora, tipo OV o EV).
Exporta el certificado con su llave privada a `.pfx`, codifícalo (`base64 -i cert.pfx` en macOS o
Linux; en PowerShell `[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx"))`) y guárdalo
en `WINDOWS_CERTIFICATE`, con su contraseña en `WINDOWS_CERTIFICATE_PASSWORD`. El flujo lo importa
al almacén del runner y firma el `.exe` y el `.msi` con sello de tiempo.

Los certificados OV no eliminan por sí solos la pantalla de SmartScreen: esta se apacigua cuando
el archivo acumula reputación. Los EV la evitan desde el principio, pero suelen venir en un
dispositivo físico o en la nube, y para eso Tauri ofrece `bundle.windows.signCommand` (por
ejemplo con Azure Trusted Signing o un servicio de firma remoto); habría que adaptar el flujo.

### Linux

Los `.deb` y `.AppImage` no se firman con ningún secreto. Se publican las sumas de verificación que
muestra GitHub para cada archivo adjunto.

## Qué ve el usuario si no hay firma

Es normal en un proyecto sin certificados de pago, y la sección "Instalación" del
[README](../README.md) lo explica al usuario. Resumen:

- **macOS (Gatekeeper).** Al abrir aparece "no se puede abrir porque Apple no puede comprobar que
  no contiene software malicioso" (o "está dañado"). Se abre con clic derecho > Abrir, o desde
  Ajustes del Sistema > Privacidad y seguridad > "Abrir de todos modos". Si dice "dañado":
  `xattr -dr com.apple.quarantine /Applications/Tlaku.app`.
- **Windows (SmartScreen).** "Windows protegió su PC": pulsa "Más información" y luego "Ejecutar
  de todas formas".
- **Linux.** Sin avisos. El `.AppImage` necesita permiso de ejecución
  (`chmod +x Tlaku_*.AppImage`) y, en distribuciones recientes, `libfuse2`.

## Linux: permiso del puerto serie

Los láseres GRBL se ven como `/dev/ttyUSB0` o `/dev/ttyACM0`. En la mayoría de las distribuciones
esos dispositivos pertenecen al grupo `dialout` (en Arch y derivadas, `uucp`), así que el usuario
debe pertenecer a él:

```sh
sudo usermod -aG dialout "$USER"     # Arch: sudo usermod -aG uucp "$USER"
# cierra la sesión y vuelve a entrar para que surta efecto; comprueba con:
id -nG | tr ' ' '\n' | grep -x dialout
```

Si la máquina aparece y desaparece al conectarla, suele ser `ModemManager` o `brltty` quedándose
con el puerto: `sudo systemctl disable --now ModemManager` (o desinstala `brltty`).

El paquete `.deb` declara dependencia de `libudev1` (que necesita la biblioteca de puertos serie);
las bibliotecas de WebKitGTK las detecta Tauri solo. El `.deb` no modifica grupos del usuario a
propósito: añadir a alguien a `dialout` es una decisión suya, por eso se documenta.

## Actualizaciones automáticas (preparadas, no activadas)

Hoy la app **no** se actualiza sola. Estos son los pasos para activarlo cuando se decida; ninguno
está aplicado en el código ni en la configuración. Ojo: esta firma es **distinta** de la firma del
sistema operativo; el actualizador exige la suya aunque la app no esté firmada por Apple o
Microsoft.

1. **Generar las llaves** (una sola vez, desde `apps/desktop`):

   ```sh
   npx tauri signer generate -w ~/.tauri/tlaku.key
   ```

   Crea `tlaku.key` (privada) y `tlaku.key.pub` (pública), y pide una contraseña. **Guarda la
   privada y la contraseña fuera del repositorio** (gestor de contraseñas). Si se pierden, las
   instalaciones existentes ya no podrán actualizarse, solo reinstalarse a mano.

2. **Secretos de GitHub**: `TAURI_SIGNING_PRIVATE_KEY` (el contenido del archivo `tlaku.key`) y
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. En `release.yml`, descomenta las dos líneas que los pasan
   al paso "Compilar y empaquetar".

3. **`apps/desktop/src-tauri/tauri.conf.json`**: activar los artefactos de actualización y la
   llave pública (contenido de `tlaku.key.pub`):

   ```json
   {
     "bundle": { "createUpdaterArtifacts": true },
     "plugins": {
       "updater": {
         "pubkey": "CONTENIDO DE tlaku.key.pub",
         "endpoints": [
           "https://github.com/<organización>/<repositorio>/releases/latest/download/latest.json"
         ],
         "windows": { "installMode": "passive" }
       }
     }
   }
   ```

   Con esto, `tauri-action` genera y sube `latest.json` con cada Release. El enlace `latest`
   solo funciona con la Release **publicada** (los borradores no cuentan). Mientras
   `createUpdaterArtifacts` esté activo, la compilación exige `TAURI_SIGNING_PRIVATE_KEY`.

4. **`apps/desktop/src-tauri/Cargo.toml`**:

   ```toml
   [target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]
   tauri-plugin-updater = "2"
   tauri-plugin-process = "2"   # opcional, para reiniciar tras instalar
   ```

   Ambos son MIT o Apache-2.0; confirma con `cargo deny check licenses sources`.

5. **`apps/desktop/src-tauri/src/lib.rs`**, al construir la aplicación:

   ```rust
   tauri::Builder::default()
       // ...plugins existentes...
       .setup(|app| {
           #[cfg(desktop)]
           app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
           Ok(())
       })
   ```

   (Si ya hay un `.setup`, añade ahí la línea con `#[cfg(desktop)]`.)

6. **Permisos**: en `apps/desktop/src-tauri/capabilities/default.json` añade `"updater:default"`
   (y `"process:allow-restart"` si usas el plugin `process`).

7. **Interfaz**: `npm install @tauri-apps/plugin-updater` en `apps/desktop` (y
   `@tauri-apps/plugin-process` si hace falta) y llamar a `check()` / `downloadAndInstall()` desde
   un comando de menú o un aviso; ver la documentación oficial del actualizador de Tauri.

8. Probar con dos versiones consecutivas antes de anunciarlo.

## Pendiente de decidir

- **Página del proyecto.** `bundle` no declara `homepage` porque el repositorio no tiene URL
  pública definida. Cuando exista, añade `homepage` en `[workspace.package]` de `Cargo.toml`
  (Tauri la usa en el `.deb`) y en el README.
- **Certificados.** Hasta que se compren, los instaladores de macOS y Windows se publican sin
  firmar. El programa de [SignPath Foundation](https://signpath.org/) (firma gratuita para
  proyectos libres) es una alternativa a evaluar para Windows.
