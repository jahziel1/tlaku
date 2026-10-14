# Cómo publicar una versión de Tlaku

Una versión se publica empujando una etiqueta `v*` al repositorio. El flujo
[`.github/workflows/release.yml`](../.github/workflows/release.yml) compila los instaladores en
tres sistemas y los deja en un **borrador** de GitHub Release; tú lo revisas y lo publicas.

| Sistema | Instaladores | Runner |
|---|---|---|
| macOS | `.dmg` (binario universal: Apple Silicon e Intel) y `.app.tar.gz` | `macos-latest` |
| Windows | `.exe` (NSIS) y `.msi` | `windows-latest` |
| Linux | `.AppImage` y `.deb` | `ubuntu-22.04` |

**Primera vez:** el proyecto todavía no tiene repositorio público, cuentas de donación, llaves ni
certificados. La [lista de pasos para publicar](#pasos-para-publicar-lo-que-tienes-que-hacer-tú) al
final de este archivo dice qué hacer y en qué orden; todo lo demás ya está preparado.

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
| `TAURI_SIGNING_PRIVATE_KEY` | todas | actualizaciones | Contenido de la llave privada del actualizador (`tlaku.key`) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | todas | actualizaciones | Su contraseña |

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

## Actualizaciones automáticas

**Qué hay en el código (ya integrado):**

- `tauri-plugin-updater` y `tauri-plugin-process` (ambos MIT o Apache-2.0, comprobado con
  `cargo deny check licenses sources`) y sus equivalentes de npm.
- Menú **Ayuda > Buscar actualizaciones…** y la preferencia **Buscar actualizaciones al iniciar**
  (Preferencias > General, **apagada de fábrica**; ni siquiera se importa desde un `.tlakuprefs`).
  Si hay una versión nueva, un diálogo muestra la versión y sus notas con los botones **Descargar e
  instalar** y **Ahora no**; tras instalar ofrece **Reiniciar ahora**. No instala con un trabajo en
  curso en la máquina. La descarga se verifica con la firma del proyecto.
- **Sin llave configurada no pasa nada malo:** `tauri.conf.json` trae `plugins.updater` con `pubkey`
  y `endpoints` vacíos y `bundle.createUpdaterArtifacts: false`. En esa compilación el actualizador
  ni siquiera se registra (`src-tauri/src/updater.rs`: el plugin exige `pubkey` y fallaría al
  arrancar) y el menú responde «Las actualizaciones automáticas no están configuradas en esta
  compilación». Interfaz: `features/help/updater.ts` y `UpdateDialog` en `HelpDialogs.tsx`. Permisos
  en `capabilities/default.json` (`updater:default`, `process:allow-restart`).
- `release.yml` ya pasa `TAURI_SIGNING_PRIVATE_KEY` y su contraseña al empaquetador; mientras
  `createUpdaterArtifacts` esté en `false` los ignora, así que la compilación local y la CI no exigen
  la llave.

**Para activarlo** (la firma es **distinta** de la del sistema operativo: el actualizador exige la
suya aunque la app no esté firmada por Apple o Microsoft):

1. **Generar las llaves** (una sola vez, desde `apps/desktop`):

   ```sh
   npx tauri signer generate -w ~/.tauri/tlaku.key
   ```

   Crea `tlaku.key` (privada) y `tlaku.key.pub` (pública), y pide una contraseña. **Guarda la
   privada y la contraseña fuera del repositorio** (gestor de contraseñas); nunca las subas. Si se
   pierden, las instalaciones existentes ya no podrán actualizarse, solo reinstalarse a mano.

2. **Secretos de GitHub**: `TAURI_SIGNING_PRIVATE_KEY` (el contenido de `tlaku.key`) y
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

3. **Llenar la configuración** con el script (pública, no la privada; el script se niega a usar una
   llave privada):

   ```sh
   scripts/configure-updater.sh https://github.com/<organización>/<repositorio> ~/.tauri/tlaku.key.pub
   ```

   Escribe `plugins.updater.pubkey`, el endpoint `<repo>/releases/latest/download/latest.json` y
   activa `bundle.createUpdaterArtifacts`. Confirma el cambio con `git diff` y súbelo. Desde aquí la
   compilación **exige** `TAURI_SIGNING_PRIVATE_KEY` (por eso viene apagado). Para volver a la
   plantilla: `scripts/configure-updater.sh --desactivar`.

   El enlace `latest` solo funciona con la Release **publicada** (los borradores no cuentan) y
   **no incluye pre-lanzamientos**: una beta (`-beta.1`) no se ofrece como actualización. Al publicar
   la primera versión estable se activa de verdad el canal.

4. **Probar con dos versiones consecutivas** antes de anunciarlo: instala la anterior, publica la
   siguiente y comprueba Ayuda > Buscar actualizaciones….

## Sitio web y donaciones

- **Sitio** (`site/`): HTML y CSS estáticos, sin framework ni recursos externos, en español
  (`site/index.html`) y en inglés (`site/en/index.html`). Muestra qué es Tlaku, capturas, funciones,
  máquinas y su estado, descargas, manual, donaciones y licencia. Los textos dicen lo que hay: lo que
  no se ha probado con hardware se dice claramente.
- **Marcadores:** `{{REPO_URL}}`, `{{SPONSORS_URL}}` y `{{KOFI_URL}}`. Se llenan desde
  `site/config.json`:

  ```sh
  node scripts/build-site.mjs                  # construye site-dist/ (gitignorado)
  node scripts/build-site.mjs --readme         # además llena los marcadores del README
  python3 -m http.server -d site-dist 8000     # para verlo en local
  ```

  Un valor vacío oculta su botón o enlace y muestra «aún no hay…». El script copia las capturas de
  `apps/desktop/docs/screenshots/` y falla si un marcador o una captura no existe.
- **GitHub Pages:** `.github/workflows/pages.yml` publica el sitio al empujar a `main`, pero está
  inactivo hasta crear la variable de repositorio `PAGES_ENABLED=true` (paso 6 de la lista de abajo).
- **Donaciones:** `.github/FUNDING.yml` (botón Sponsor del repositorio; líneas comentadas), sección
  «Apoyar el proyecto» del README y Ayuda > «Donar al proyecto…» en la app. Los enlaces de la app
  viven en `apps/desktop/src/features/help/links.ts` (`PROJECT_LINKS`, vacíos por omisión): sin
  ellos, el diálogo dice que el proyecto aún no tiene cuentas de donación.

## CI

`.github/workflows/ci.yml` corre en cada push a `main` y en cada pull request:

| Trabajo | Qué comprueba |
|---|---|
| Rust (Linux, macOS, Windows) | `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features` |
| cargo-deny | `cargo deny check licenses sources` |
| Interfaz | `tsc --noEmit`, `eslint`, `vitest run` y la tabla de atajos del README |
| Prueba de humo (Linux) | arranca la app Tauri real bajo Xvfb (`apps/desktop/scripts/smoke-tauri.sh`) |
| Sitio estático | `node scripts/build-site.mjs` (marcadores y capturas válidos) |

Tauri exige que exista `apps/desktop/dist`; en los trabajos de Rust se crea una página vacía de
relleno y en la prueba de humo se compila la interfaz real. Los flujos no se pueden ejecutar fuera de
GitHub: se validaron con `actionlint` y ejecutando cada comando en local; la primera ejecución real
puede revelar ajustes (por ejemplo, paquetes de apt).

## Pasos para publicar: lo que tienes que hacer tú

Todo lo siguiente necesita algo que solo tú tienes (cuentas, llaves, dinero). Hazlo en este orden;
cada paso indica el archivo donde va el valor.

1. **Crear el repositorio** en GitHub (público) y subir el código. Elige organización y nombre; con
   eso tienes `<url>` = `https://github.com/<organización>/<repositorio>`.
2. **Poner la URL en el proyecto:**
   - `homepage = "<url>"` en `[workspace.package]` de `Cargo.toml` (Tauri la usa en el `.deb`).
   - `repo` e `issues` (`<url>/issues`) en `apps/desktop/src/features/help/links.ts`.
   - `REPO_URL` en `site/config.json`.
   - En el README, la dirección del reporte de problemas (busca «PENDIENTE») y el `git clone`.
3. **Generar las llaves del actualizador** y cargar los secretos (sección
   [Actualizaciones automáticas](#actualizaciones-automáticas)): `npx tauri signer generate -w
   ~/.tauri/tlaku.key`; secretos `TAURI_SIGNING_PRIVATE_KEY` y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`;
   luego `scripts/configure-updater.sh <url> ~/.tauri/tlaku.key.pub` y sube el cambio de
   `tauri.conf.json`. Guarda la llave privada fuera del repositorio.
4. **Configurar las donaciones** (opcional, cuando las tengas): abre GitHub Sponsors y/o Ko-fi;
   llena `SPONSORS_URL` y `KOFI_URL` en `site/config.json`, `sponsors` y `kofi` en `links.ts` y
   descomenta las líneas de `.github/FUNDING.yml`. Después `node scripts/build-site.mjs --readme`
   llena la sección «Apoyar el proyecto» del README.
5. **Certificados de firma** (opcional; sin ellos los instaladores salen sin firmar y funcionan con
   advertencias): Apple Developer Program para macOS y un certificado de firma de código (o SignPath)
   para Windows; los secretos y su formato están en [Firma de código](#firma-de-código).
6. **Activar el sitio:** Settings > Pages > Source: "GitHub Actions" y, en Settings > Secrets and
   variables > Actions > Variables, crear `PAGES_ENABLED` = `true`. Empuja a `main` (o lanza el flujo
   "Sitio" a mano).
7. **Comprobar la CI:** empuja a `main` y revisa que los cinco trabajos de CI pasen; corrige lo que
   salga en la primera ejecución real.
8. **Primera beta:** sube la versión (`0.x.0-beta.1`, en los tres lugares de
   [Sacar una versión](#sacar-una-versión); recuerda que el `.msi` solo admite sufijos numéricos, así
   que para probar Windows usa `0.x.0-1`), etiqueta `v0.x.0-beta.1`, empuja la etiqueta, revisa el
   borrador en Releases, prueba un instalador y publícalo (se marca como pre-lanzamiento).
9. **Avisar:** con la Release publicada, el enlace «Última versión en GitHub» del sitio ya apunta a
   algo real. Antes de anunciarlo, repite el recordatorio del README: ninguna función de máquina se ha
   probado con un láser real.

## Pendiente de decidir

- **Certificados.** Hasta que se compren, los instaladores de macOS y Windows se publican sin
  firmar. El programa de [SignPath Foundation](https://signpath.org/) (firma gratuita para
  proyectos libres) es una alternativa a evaluar para Windows.
