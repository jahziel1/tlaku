# Cómo publicar una versión de Tlaku

Una versión se publica empujando una etiqueta `v*` al repositorio. El flujo
[`.github/workflows/release.yml`](../.github/workflows/release.yml) compila los instaladores en
tres sistemas (Linux en x86_64 y en ARM64) y los deja en un **borrador** de GitHub Release; después `flatpak.yml` añade los paquetes Flatpak. Tú lo revisas y lo publicas.

| Sistema | Instaladores | Runner |
|---|---|---|
| macOS | `.dmg` (binario universal: Apple Silicon e Intel) y `.app.tar.gz` | `macos-latest` |
| Windows | `.exe` (NSIS) y `.msi` | `windows-latest` |
| Linux x86_64 | `.AppImage` (`_amd64`) y `.deb` (`_amd64`) | `ubuntu-22.04` |
| Linux ARM64 (experimental) | `.AppImage` (`_aarch64`) y `.deb` (`_arm64`) | `ubuntu-22.04-arm` |
| Linux Flatpak (sin probar) | `Tlaku_<versión>_x86_64.flatpak` y `_aarch64.flatpak`, reempaquetando los `.deb` | `flatpak.yml` (lo llama `release.yml`) |

**Primera vez:** el repositorio es <https://github.com/jahziel1/tlaku>. Las llaves del actualizador
ya están (2026-10-14: llave pública en `tauri.conf.json`, privada y contraseña como secretos de
GitHub); faltan los certificados. La
[lista de pasos para publicar](#pasos-para-publicar-lo-que-tienes-que-hacer-tú) al final de este archivo
marca lo que ya está hecho y dice qué falta y en qué orden.

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
   | `packaging/flatpak/io.github.jahziel1.tlaku.metainfo.xml` | Un `<release version="X.Y.Z" date="AAAA-MM-DD">` nuevo arriba y la etiqueta `vX.Y.Z` en las URL de las capturas. No bloquea la publicación (`flatpak.yml` solo avisa si falta), pero Flathub lo exige: ver [Flatpak y Flathub](#flatpak-y-flathub). |

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
   - Están los instaladores de los tres sistemas (`.dmg`, `.exe`, `.msi`, `.AppImage`, `.deb`; los de
     Linux, en `_amd64` y en `_arm64`/`_aarch64`).
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
[README](../README.es.md) lo explica al usuario. Resumen:

- **macOS (Gatekeeper).** Al abrir aparece "no se puede abrir porque Apple no puede comprobar que
  no contiene software malicioso" (o "está dañado"). Se abre con clic derecho > Abrir, o desde
  Ajustes del Sistema > Privacidad y seguridad > "Abrir de todos modos". Si dice "dañado":
  `xattr -dr com.apple.quarantine /Applications/Tlaku.app`.
- **Windows (SmartScreen).** "Windows protegió su PC": pulsa "Más información" y luego "Ejecutar
  de todas formas".
- **Linux.** Sin avisos. El `.AppImage` necesita permiso de ejecución
  (`chmod +x Tlaku_*.AppImage`) y, en distribuciones recientes, `libfuse2`.

## Linux ARM64 y Raspberry Pi

La entrada «Linux ARM64» de `release.yml` compila **de forma nativa** en el runner `ubuntu-22.04-arm`
(alojado por GitHub; gratis en repositorios públicos, 4 núcleos y 16 GB; en repositorios privados
consume minutos). No se cruza desde x86 porque `linuxdeploy` no sabe hacer AppImage de ARM por
compilación cruzada. Se instalan las mismas dependencias de apt que en x86 y se firman con las mismas
variables `TAURI_SIGNING_*`.

**`latest.json`.** `tauri-action` (v0.6.2) toma la arquitectura del runner y escribe en `latest.json` las
claves `linux-aarch64` y `linux-aarch64-appimage`; antes lee el `latest.json` del borrador y fusiona, así
que no pisa `linux-x86_64`. Las dos entradas de Linux dejan archivos con nombres distintos
(`Tlaku_X.Y.Z_amd64.AppImage` / `_aarch64.AppImage`, `_amd64.deb` / `_arm64.deb`). Si dos entradas terminan
a la vez, una podría pisar a la otra (lectura-modificación-escritura sin bloqueo): el trabajo
«Comprobar latest.json» falla si falta alguna de `darwin-aarch64`, `darwin-x86_64`, `windows-x86_64`,
`linux-x86_64` o `linux-aarch64`; en ese caso, «Re-run failed jobs» sobre la entrada que falte vuelve a
fusionarla. Solo el **AppImage** se actualiza desde la app; el `.deb` se reinstala a mano (igual que en x86).

**Requisitos para el usuario (Raspberry Pi y otras ARM64).**

- **Sistema de 64 bits obligatorio** (`uname -m` debe decir `aarch64`). Raspberry Pi 3B+, 4, 400 y 5 con
  Raspberry Pi OS de 64 bits (Debian 12 *bookworm* o posterior) o Ubuntu ARM 22.04+. **No** sirve el
  sistema de 32 bits (`armv7l`), aunque la placa sea de 64 bits, ni la Pi 2 o anteriores.
- **Memoria:** se recomiendan **4 GB de RAM o más** (Pi 4/5). Con 2 GB conviene proyectos pequeños y
  cerrar el navegador; imágenes grandes y vista previa consumen memoria. Con 1 GB no es razonable.
- **Gráficos:** la ventana es WebKitGTK (`libwebkit2gtk-4.1-0`, que Tauri añade como dependencia del `.deb` y Raspberry Pi OS
  bookworm trae). Usa el escritorio con aceleración 3D (Wayland/labwc o X11 con *KMS*, no el modo de
  *framebuffer*). Si la ventana sale en blanco, parpadea o se ve rota, prueba antes de abrir la app:
  `WEBKIT_DISABLE_COMPOSITING_MODE=1 tlaku`, y si no basta, `WEBKIT_DISABLE_DMABUF_RENDERER=1`
  (variables de WebKitGTK; no las hemos podido comprobar en una Pi real). El lienzo con miles de
  objetos puede ir lento en una Pi.
- **Puerto serie:** el usuario debe estar en `dialout` (ver la sección siguiente; en Raspberry Pi OS el
  usuario inicial ya suele estarlo).
- **AppImage:** necesita `libfuse2` (`sudo apt install libfuse2`; en bookworm el paquete se llama
  `libfuse2`, en Ubuntu 24.04 `libfuse2t64`) o ejecutarlo con `--appimage-extract-and-run`.
- **Cámara local:** se captura con `getUserMedia` dentro de WebKitGTK; en Linux depende de que esa
  versión de WebKitGTK tenga activo el flujo de medios, algo que no está comprobado ni en x86. Una cámara
  de red (URL) no tiene esa dependencia.
- **Qué no cubre:** Raspberry Pi 32 bits, `armv7`, RISC-V.

**Estado: experimental.** Compila y se prueba en el runner ARM de la CI (`ci.yml`, trabajo «Rust y humo
(Linux ARM64)»: pruebas de todo el espacio de trabajo y prueba de humo bajo Xvfb), pero **no se ha
ejecutado en una Raspberry Pi real**: no se ha medido el rendimiento ni probado con un láser. Lo que
hay que comprobar en una Pi real está en el informe de la mejora 11a de
[plan-mejoras-usuarios.md](plan-mejoras-usuarios.md) y se recoge con la plantilla `machine_report.yml`.

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

## Flatpak y Flathub

**Estado: preparado, sin probar en un Linux real.** La máquina de desarrollo es macOS: el manifiesto no
se ha construido con `flatpak-builder` ni la aplicación se ha ejecutado dentro del sandbox. Lo que sí
se validó: los metadatos con `appstreamcli validate --pedantic` y el `.desktop` con
`desktop-file-validate`, los flujos con `actionlint`, el YAML del manifiesto con un analizador, y el
código del actualizador con las pruebas de la interfaz (las de Rust están escritas, pero no se
compilaron aquí; las corre la CI).

### Qué hay

| Archivo | Para qué |
|---|---|
| `packaging/flatpak/io.github.jahziel1.tlaku.yml` | Manifiesto (estrategia A, ver abajo). Id `io.github.jahziel1.tlaku`, runtime `org.gnome.Platform//50`. |
| `packaging/flatpak/io.github.jahziel1.tlaku.metainfo.xml` | AppStream: nombre y resumen es/en, descripción, 5 capturas (de `apps/desktop/docs/screenshots/`, por la URL de la etiqueta), licencia MIT, donación Ko-fi, release 0.1.0, OARS sin contenido. |
| `packaging/flatpak/io.github.jahziel1.tlaku.desktop` | Entrada de menú: categorías `Graphics;2DGraphics;VectorGraphics;Engineering;` y palabras clave es/en. **Sin `MimeType`**: la app aún no abre archivos `.tlaku` pasados por argumento (no hay asociación de archivos en `tauri.conf.json`); declararlo sería prometer algo que no hace. |
| `packaging/flatpak/io.github.jahziel1.tlaku.svg` | Ícono (copia de `apps/desktop/src-tauri/icons/source/tlaku.svg`; Flathub prefiere SVG). `flatpak.yml` avisa si difieren. |
| `.github/workflows/flatpak.yml` | Baja los `.deb` de la Release, construye con `flatpak/flatpak-github-actions/flatpak-builder@v6` en `x86_64` y `aarch64` (este con QEMU, como documenta esa acción) y adjunta los `.flatpak` a la misma Release. Se lanza a mano con la etiqueta de una Release que ya tenga los `.deb`. |
| `release.yml` (trabajo `flatpak`) | Llama a `flatpak.yml` tras «instaladores», solo con etiqueta. No cambia los otros trabajos. |

### Estrategia: reempaquetar el `.deb` (A) y no compilar desde fuentes (B)

Se evaluaron dos caminos (guía de Tauri: <https://v2.tauri.app/distribute/flatpak/>):

- **A. Reempaquetar el `.deb`** (implementada). El manifiesto extrae el `.deb` que ya produce `release.yml`
  y lo instala en `/app`. Ventajas: es **exactamente el binario** que se probó y publicó; no hacen falta
  Rust, Node ni fuentes sin red; la CI tarda poco (incluso emulando ARM). Es uno de los dos caminos
  que describe la documentación de Tauri. Desventaja: **a Flathub le interesa lo contrario**: sus
  requisitos dicen que el código abierto debe compilarse íntegramente desde fuentes (con excepciones
  caso por caso). Un PR que reempaqueta un binario puede ser rechazado o pedir justificación; no hay
  garantía.
- **B. Compilar desde fuentes** (lo que Flathub prefiere; no implementada). Hay que generar
  `cargo-sources.json` (`flatpak-cargo-generator.py Cargo.lock`) y `node-sources.json`
  (`flatpak-node-generator npm apps/desktop/package-lock.json`) con
  <https://github.com/flatpak/flatpak-builder-tools>, usar las extensiones
  `org.freedesktop.Sdk.Extension.rust-stable` y `node` y compilar sin red: `CARGO_HOME` apuntando a las
  fuentes generadas, `npm ci --offline`, `npm run build:tauri` y `npx tauri build -b deb` (o
  `cargo build --release -p tlaku-desktop`; Tauri exige el `dist` ya construido) y extraer el `.deb` igual
  que en A. Cuidado con tres cosas: (1) el toolchain está fijado a Rust 1.99.0 en `rust-toolchain.toml`
  y el workspace usa edition 2024: la extensión `rust-stable` del runtime elegido debe ser igual o más
  nueva; (2) `build.rs` y `tauri-build` no deben intentar salir a la red; (3) el árbol de dependencias
  de npm y Cargo es grande: el manifiesto de fuentes se regenera en cada versión. Es mantenimiento
  real; no se hizo ahora porque no se puede probar sin Linux.

**Recomendación:** usar A para el `.flatpak` de cada Release (instalable con `flatpak install --user`
sin esperar a nadie) y, **si Flathub rechaza A**, pasar a B solo para el repositorio de Flathub. El
resto (permisos, metainfo, `.desktop`, ícono, código del actualizador) es común a las dos.

### Runtime: `org.gnome.Platform//50`

La Plataforma GNOME trae GTK 3, WebKitGTK 4.1 (`libwebkit2gtk-4.1`, la que enlaza Tauri 2) y libsoup 3,
así que la app no compila ningún módulo de más. La alternativa `org.freedesktop.Platform` obligaría a
compilar WebKitGTK. Flathub rechaza runtimes sin soporte: antes de enviar, comprueba la versión vigente
en <https://docs.flathub.org/docs/for-app-authors/runtimes> y sube `runtime-version` (y la imagen
`gnome-NN` de `flatpak.yml`; las que existen se ven en
<https://github.com/flathub-infra/flatpak-github-actions/pkgs/container/flatpak-github-actions>). El
manifiesto incluye una comprobación con `ldd` que **hace fallar la compilación** si el ejecutable
necesita una biblioteca que el runtime no trae (por ejemplo `libxdo`); si ocurre, hay que añadir un
módulo que la compile.

### Permisos (`finish-args`) y por qué

| Permiso | Por qué |
|---|---|
| `--share=ipc`, `--socket=wayland`, `--socket=fallback-x11` | Mostrar la ventana (Wayland; X11 si no hay). |
| `--device=dri` | Aceleración gráfica de WebKitGTK para el lienzo. |
| `--share=network` | Controladoras por red (GRBL por TCP, Ruida por UDP), cámaras IP (`fetch_camera_frame`, solo red local) y el **control remoto desde el celular** (servidor HTTP en la red local). |
| `--device=all` | **El permiso más amplio y el que Flathub revisará.** Cubre `/dev/ttyUSB*` y `/dev/ttyACM*` (láseres GRBL), `/dev/bus/usb` (K40 y galvo con `nusb`) y `/dev/video*` (cámaras USB). No existe un permiso más acotado para puertos serie: `--device=usb` (Flatpak 1.16) cubre USB pero no `tty`. |
| `--filesystem=xdg-documents`, `xdg-download`, `xdg-pictures:ro` | Los diálogos de archivo ya pasan por el **portal de documentos** (no hace falta permiso para lo que el usuario elige). Estas carpetas existen porque la vista previa de imágenes usa el protocolo `asset://` con el alcance de `tauri.conf.json` (`$DOCUMENT`, `$DOWNLOAD`, `$PICTURE`, `$DESKTOP`): un archivo abierto por el portal tiene la ruta `/run/user/<uid>/doc/...`, fuera de ese alcance, y la imagen no se vería. Con acceso directo la ruta coincide. Mejora pendiente: añadir `/run/user/*/doc/**` al alcance y quitar estos permisos (es un cambio de seguridad en `tauri.conf.json`; no se hizo aquí). `$DESKTOP` no se concede: los archivos del escritorio abren por el portal, solo falla la vista previa de imágenes. |

No se piden: `--socket=pulseaudio`, `--talk-name=*`, `--filesystem=home`, `--socket=session-bus`, `--socket=ssh-auth`.

**Grupo `dialout` en el sandbox.** Flatpak no crea ni cambia grupos: los permisos de `/dev/ttyUSB0` los
decide el sistema anfitrión. El usuario debe estar en `dialout` (`uucp` en Arch) **en el sistema**,
exactamente como con el `.deb` (ver [Linux: permiso del puerto serie](#linux-permiso-del-puerto-serie));
no hay nada que configurar dentro del sandbox. Para K40 y galvo por USB, el sistema puede necesitar una
regla udev que dé acceso al usuario (el proyecto aún no distribuye una). **Riesgo sin comprobar:**
`serialport` enumera los puertos con libudev; dentro del sandbox la base de datos de udev puede no
estar visible y la lista de puertos podría salir vacía aunque el dispositivo exista. Es lo primero que
hay que probar en un Linux real.

### Actualizador desactivado en Flatpak

Las actualizaciones las da Flathub; el programa no puede reemplazar sus propios archivos. La detección
está en `src-tauri/src/updater.rs` (`in_flatpak()`): variable `FLATPAK_ID` o archivo `/.flatpak-info`.
Dentro del sandbox el plugin `tauri-plugin-updater` **no se registra** y Rust inyecta
`window.__TLAKU_SANDBOX__ = 'flatpak'` en las ventanas (`lib.rs`, `build_app`). La interfaz
(`features/help/updater.ts`, `updatesManagedByStore`) no programa el chequeo de arranque y Ayuda >
«Buscar actualizaciones…» dice que las actualizaciones las da Flathub y cómo actualizar. Pruebas:
`updater.rs` (detección y script) y `updater.test.tsx`. Los datos del Flatpak viven en
`~/.var/app/io.github.jahziel1.tlaku/` y **no se comparten** con una instalación `.deb` de la misma persona; para
pasar los ajustes, un `.tlakubundle`.

### Cambio de identificador y migración de datos

El `identifier` pasó de `org.tlaku.app` a `io.github.jahziel1.tlaku` antes de publicar la beta 0.1.0. Tauri
cuelga de él las carpetas de la app (`app_config_dir`, `app_data_dir`, `app_local_data_dir`, y `app_cache_dir` para la copia del catálogo comunitario, que no se migra porque se
vuelve a bajar), el bundle de macOS (`CFBundleIdentifier`), el `app_id` de GTK y el
código de actualización del MSI (`upgradeCode`, que Tauri deriva del id; no hay otro fijado en `tauri.conf.json`): como no hay instalaciones públicas, no hay
nada que actualizar «en sitio». `src-tauri/src/id_migration.rs` copia al arrancar lo que haya en las
carpetas del id viejo (ver `docs/mapa.md`) y está probado en `id_migration/tests.rs`.

Lo que **no** se migra: el `localStorage` del WebView, que WKWebView (macOS) y WebView2 (Windows) guardan por
id y no se puede leer desde Rust. Vive ahí solo lo que la interfaz recuerda de pasada: perfiles de costos
(`tlaku.costs`), ajustes de imagen propios (`tlaku.imagePresets`), pestañas abiertas (`tlaku.tabs.session`),
cola de trabajos, archivos recientes, sesión de tiling, historial de la consola, pasos de movimiento y
estados de paneles. Lo importante (dispositivos, materiales, preferencias del motor, historial de trabajos,
versiones, retazos, cámara, plantillas, `backup.json` y `respaldos-locales/` del respaldo a carpeta, curvas de tono, recuperación, punto de control de un trabajo interrumpido) está en
archivos y sí se migra; las preferencias se reconstruyen desde `preferences.json`. Solo afectó a quien
corrió compilaciones de desarrollo con el id viejo.

### Probar el paquete en Linux (lo que falta comprobar)

```sh
# Linux con flatpak y flatpak-builder; los .deb de la Release en packaging/flatpak/debs/
mkdir -p packaging/flatpak/debs
cp Tlaku_0.1.0_amd64.deb packaging/flatpak/debs/tlaku_x86_64.deb      # (o _arm64.deb -> tlaku_aarch64.deb)
flatpak-builder --force-clean --user --install-deps-from=flathub --repo=repo build-dir packaging/flatpak/io.github.jahziel1.tlaku.yml
flatpak build-bundle repo tlaku.flatpak io.github.jahziel1.tlaku && flatpak install --user tlaku.flatpak
flatpak run io.github.jahziel1.tlaku
# El linter de Flathub (lo que revisará el PR):
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest packaging/flatpak/io.github.jahziel1.tlaku.yml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo repo
```

Lista de comprobación: abre la ventana (Wayland y X11; con GPU NVIDIA prueba
`--env=WEBKIT_DISABLE_DMABUF_RENDERER=1`); importar un SVG por el diálogo; guardar y abrir un proyecto;
la vista previa de una imagen; **la lista de puertos con un láser conectado**; conectar y mover; el
control remoto desde el celular; Ayuda > Buscar actualizaciones (debe decir que lo da Flathub).

### Enviar a Flathub (pasos)

> La carpeta lista, los hashes de la Release 0.1.0 y los obstáculos conocidos (compilar desde fuentes y
> política de IA de Flathub) están en [Dar de alta en tiendas > Flathub (Linux)](#flathub-linux).

1. **El identificador ya está decidido: `io.github.jahziel1.tlaku`** (Flathub exige que quien envía controle
   el dominio del id; para proyectos en GitHub acepta `io.github.<usuario>.<app>`). Es también el
   `identifier` de `tauri.conf.json`, de donde salen el `app_id` de GTK/Wayland, el ícono de la barra de
   tareas, el nombre de los archivos de `packaging/flatpak/`, `Icon=`, `<launchable>` y la carpeta de datos.
   Antes de la beta el id era `io.github.jahziel1.tlaku`; al arrancar, la app copia sola su configuración y sus datos
   de la carpeta antigua a la nueva (`src-tauri/src/id_migration.rs`; ver la nota de la sección siguiente).
2. **Publica la Release** (`vX.Y.Z`) con sus `.deb` y confirma que las URL de las capturas del metainfo
   responden (`appstreamcli validate packaging/flatpak/io.github.jahziel1.tlaku.metainfo.xml`, sin `--no-net`).
3. **Prepara el manifiesto de Flathub**: copia `packaging/flatpak/io.github.jahziel1.tlaku.yml` y cambia las dos
   fuentes `.deb` locales por las URL de la Release con su suma de verificación (`sha256sum`):

   ```yaml
   - type: file
     url: https://github.com/jahziel1/tlaku/releases/download/v0.1.0/Tlaku_0.1.0_amd64.deb
     sha256: <sha256 del .deb>
     dest-filename: tlaku.deb
     only-arches: [x86_64]
   - type: file
     url: https://github.com/jahziel1/tlaku/releases/download/v0.1.0/Tlaku_0.1.0_arm64.deb
     sha256: <sha256 del .deb>
     dest-filename: tlaku.deb
     only-arches: [aarch64]
   ```

   Los otros tres archivos (`.desktop`, metainfo, SVG) se suben junto al manifiesto (Flathub no admite
   binarios en el PR; el SVG es texto). Con la estrategia B, ahí también van `cargo-sources.json` y
   `node-sources.json`.
4. **Abre el PR**: bifurca <https://github.com/flathub/flathub>, clona la rama `new-pr`
   (`git clone --branch=new-pr git@github.com:<tu usuario>/flathub.git`), crea una rama con el nombre
   de la app, añade los archivos y abre el PR **contra `new-pr`** (no contra `master`); sigue la lista de
   <https://docs.flathub.org/docs/for-app-authors/submission>.
5. **Qué revisarán**: que pase `flatpak-builder-lint` (manifiesto, metainfo y repositorio; los avisos y
   errores de AppStream son bloqueantes); que el id coincida con un dominio que controlas o use el
   prefijo `io.github.`; la justificación de cada permiso (sobre todo `--device=all`: explica serie, USB y
   cámara, como arriba); que el runtime sea el vigente; que el código abierto se compile desde fuentes (el
   riesgo de la estrategia A: prepara la respuesta a «por qué reempaquetar» o pasa a B); que el nombre y
   el ícono no suplanten a otro programa; que el `.desktop` y el ícono estén bien nombrados; capturas
   accesibles y de la versión; y que quien envía sea quien mantiene el proyecto.
6. **Después de aceptado**: llega una invitación al repositorio de la app en Flathub. Cada versión
   nueva: añade el `<release>` al metainfo, actualiza `url` y `sha256` (o el commit, con B) y abre un PR.

### Qué falta para Flathub

- Probar el paquete en un Linux real (x86_64 y, si se puede, ARM64): ventana, serie, USB, cámara, actualizador.
- ~~Decidir el identificador y el dominio~~ (hecho: `io.github.jahziel1.tlaku`).
- ~~Que exista la Release `v0.1.0`~~ (publicada el 2026-10-15; la carpeta de envío está en `packaging/flathub/`, ver [Dar de alta en tiendas](#flathub-linux)).
- Decidir A o B según la respuesta de los revisores y mantener el runtime al día.
- Una regla udev distribuida para K40 y galvo (hoy no la hay) y confirmar que `serialport` ve los puertos dentro del sandbox.
- Opcional: ampliar el alcance de `asset://` a la ruta del portal para poder quitar los `--filesystem`.
- Opcional: asociación de archivos `.tlaku` (hay que implementarla en la app antes de poner `MimeType`).

## Dar de alta en tiendas

Tres vías para que la gente instale Tlaku con su gestor de paquetes. **Los archivos están preparados
para la versión 0.1.0; no se ha enviado nada** (ni a `microsoft/winget-pkgs`, ni a Homebrew, ni a
`flathub/flathub`). Los tres envíos los haces tú, con tu cuenta: esta sección dice cómo.

### Lista de comprobación

| | winget (Windows) | Homebrew (macOS) | Flathub (Linux) |
|---|---|---|---|
| **Lo que el usuario escribe** | `winget install jahziel1.Tlaku` | `brew install --cask jahziel1/tlaku/tlaku` | `flatpak install flathub io.github.jahziel1.tlaku` |
| **Archivos listos** | `packaging/winget/manifests/j/jahziel1/Tlaku/0.1.0/` (4 YAML) | `packaging/homebrew/Casks/tlaku.rb` | `packaging/flathub/io.github.jahziel1.tlaku/` (4 archivos) |
| **Dónde va** | PR a `microsoft/winget-pkgs` | Repositorio **tuyo** `jahziel1/homebrew-tlaku` (tap) | PR a `flathub/flathub` (rama `new-pr`) |
| **Qué necesitas** | Cuenta de GitHub y, si usas `wingetcreate`, un token con permiso `public_repo`. La primera vez el bot pide aceptar el CLA de Microsoft. | Cuenta de GitHub y crear un repositorio público llamado exactamente `homebrew-tlaku`. | Cuenta de GitHub con autenticación en dos pasos, **un Linux real** (o una VM) para probar el paquete antes, y leer la política de IA (ver abajo). |
| **Decisiones tuyas** | ¿`jahziel1.Tlaku` o `Tlaku.Tlaku` como identificador? (no se puede cambiar después sin duplicar el paquete). Solo el instalador `.exe`, no el `.msi`. | ¿Tap propio ya, o esperar a firmar y notarizar para pedir el cask oficial? | ¿Estrategia A (reempaquetar `.deb`, probablemente rechazada) o B (compilar desde fuentes)? ¿Runtime 50 o 51? Cómo declarar el uso de IA. |
| **Tiempo tuyo** | 30–45 min | 15–20 min | 2–4 h de pruebas en Linux + 30 min del PR (+ rehacer a B: 1–3 días) |
| **Espera hasta estar disponible** | Validación automática en horas; revisión humana de 1 a 7 días (más si el antivirus marca el instalador sin firmar) | Inmediata | Revisión de 1 a 4 semanas con varias rondas de comentarios |
| **Riesgo principal** | Antivirus/SmartScreen con el instalador sin firmar; `ProductCode` sin comprobar en Windows | Gatekeeper: la app no está notarizada (el usuario debe quitar la cuarentena) | Flathub exige compilar desde fuentes y prohíbe contenido hecho con IA en el manifiesto |

Los tiempos son estimaciones (ni Microsoft ni Flathub los garantizan). Orden recomendado: **Homebrew
(tap)** hoy; **winget** esta semana; **Flathub** cuando haya probado el paquete en Linux y decidido A/B.

### winget (Windows)

**Qué hay** en `packaging/winget/manifests/j/jahziel1/Tlaku/0.1.0/` (la ruta es la que exige
`winget-pkgs`: `manifests/<inicial>/<Publicador>/<Paquete>/<versión>/`):

| Archivo | Contenido |
|---|---|
| `jahziel1.Tlaku.yaml` | Versión (esquema **1.28.0**, el vigente según `doc/manifest/schema/` de winget-pkgs al 2026-10-15), idioma por omisión `en-US`. |
| `jahziel1.Tlaku.installer.yaml` | Instalador NSIS (`InstallerType: nullsoft`), `Scope: user` (Tauri lo instala por usuario, sin administrador), x64, URL de la Release y SHA256 del `.exe`, `ProductCode: Tlaku`, fecha de la Release. |
| `jahziel1.Tlaku.locale.en-US.yaml` | Descripción en inglés: licencia MIT, enlaces (sitio, soporte, Release, manual), etiquetas, notas de versión. |
| `jahziel1.Tlaku.locale.es-MX.yaml` | Lo mismo en español de México. |

SHA256 usado (calculado bajando el asset real de la Release v0.1.0; coincide con el que publica GitHub):
`Tlaku_0.1.0_x64-setup.exe` = `97c86f8b25a2c3514de793ab7f0748b011093ae6c987120ce6a4d969f92e3880`.

**Por qué solo el `.exe` y no el `.msi`.** winget admite varios instaladores por manifiesto, pero el
`.msi` existe en dos idiomas (`en-US`, `es-ES`) y aportaría un segundo `ProductCode`/`UpgradeCode` que
no se puede leer desde macOS. Con un solo instalador por arquitectura no hay ambigüedad y se evita
una instalación doble. Si quieres ofrecer el MSI (por ejemplo para empresas), añade un segundo
elemento en `Installers` con `InstallerType: wix`, `Scope: machine`, `InstallerLocale` y su
`ProductCode` (se lee con Orca o en la clave de desinstalación de Windows tras instalarlo).

**Validación hecha (en macOS, sin `winget`):** los cuatro YAML contra los esquemas JSON oficiales
1.28.0 (`microsoft/winget-cli`, `schemas/JSON/manifests/v1.28.0/`) con `ajv`:
`node scripts/winget-manifests.mjs validate <carpeta> <esquemas>` (el script y sus instrucciones de uso
están en su cabecera; también se probó que un valor inválido falla). **Sin comprobar en Windows:**
`winget validate`, la instalación en Windows Sandbox y el `ProductCode`. El valor `Tlaku` sale de la
plantilla NSIS de Tauri (la clave de desinstalación es
`HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\<productName>`); compruébalo en un Windows con
el instalador puesto:

```powershell
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Tlaku" /v DisplayVersion
winget settings --enable LocalManifestFiles
winget validate --manifest .\manifests\j\jahziel1\Tlaku\0.1.0
winget install --manifest .\manifests\j\jahziel1\Tlaku\0.1.0      # instala y abre la app
winget uninstall jahziel1.Tlaku
# Opcional, en un clon de winget-pkgs (usa Windows Sandbox):
.\Tools\SandboxTest.ps1 manifests\j\jahziel1\Tlaku\0.1.0
```

**Cómo enviarlo** (elige una):

1. **Con `wingetcreate`** (<https://github.com/microsoft/winget-create>; `winget install wingetcreate`).
   Para el primer envío usa el manifiesto ya hecho: bifurca y haz el PR a mano (opción 2), o deja que la
   herramienta lo cree: `wingetcreate new https://github.com/jahziel1/tlaku/releases/download/v0.1.0/Tlaku_0.1.0_x64-setup.exe`
   (te hará las mismas preguntas; copia los textos de nuestros YAML). Para **versiones siguientes** es
   lo más cómodo: `wingetcreate update jahziel1.Tlaku -u <URL del .exe nuevo> -v <versión> -t <token> --submit`
   (baja el instalador, recalcula el SHA256 y abre el PR). El token es un *personal access token* de
   GitHub con permiso `public_repo` (`wingetcreate token --store` lo guarda).
2. **PR manual:** bifurca <https://github.com/microsoft/winget-pkgs>, clona con
   `--filter=blob:none --no-checkout`, `sparse-checkout set manifests/j/jahziel1`, crea una rama,
   copia la carpeta `packaging/winget/manifests/j/jahziel1` dentro de `manifests/j/` (queda
   `manifests/j/jahziel1/Tlaku/0.1.0/`), `git commit -m "New package: jahziel1.Tlaku version 0.1.0"`,
   sube y abre el PR contra `master`. Llena la lista de la plantilla del PR. **Un PR por versión y por
   paquete**.

**Qué revisan** (<https://learn.microsoft.com/windows/package-manager/package/repository>): una tubería
automática valida el esquema y la ruta; comprueba que la URL responda y que el SHA256 coincida
(`Error-Hash-Mismatch`); que la URL venga directo del lugar de publicación del autor, sin redirecciones
raras (GitHub Releases es lo normal); instala **sin interacción** en máquinas virtuales (`/S`, lo
estándar de NSIS) y desinstala; busca el binario con Microsoft Defender y varios antivirus
(`Binary-Validation-Error` / `Validation-Defender-Error`); y aplica las políticas de contenido (nombre,
descripciones no engañosas). Luego un moderador aprueba. Si falla, el bot etiqueta el PR
(`Needs-Author-Feedback`): tienes 10 días para responder. **Riesgo propio:** el instalador no está
firmado; los falsos positivos de antivirus en instaladores sin firma son lo más probable que retrase la
aprobación (se resuelve enviando el archivo a <https://www.microsoft.com/wdsi/filesubmission>).
Firmar el `.exe` (ver [Firma de código](#firma-de-código)) lo evita en buena parte.

**Cada versión nueva:** el flujo opcional `.github/workflows/winget.yml` se dispara al **publicar** la
Release (o a mano con la etiqueta) y deja los manifiestos de esa versión como **artefacto** del flujo
(`winget-vX.Y.Z`): versión, URL, fecha y SHA256 reales, validados contra el esquema; omite las versiones
de prueba (`-beta`). No envía nada. Descarga el artefacto y haz el PR, o ejecuta
`wingetcreate update`. En local: `node scripts/winget-manifests.mjs generate vX.Y.Z salida`. El script
parte de los archivos de la 0.1.0 y quita sus notas de versión (quedan `ReleaseNotesUrl` y la descripción).
La aplicación también se actualiza sola (Ayuda > Buscar actualizaciones); `UpgradeBehavior: install`
hace que `winget upgrade` instale encima sin desinstalar.

### Homebrew (macOS)

**Recomendación: tap propio ahora.** El cask oficial (`Homebrew/homebrew-cask`) hoy no es viable:

- **Notoriedad.** Las reglas de `brew audit --new` exigen que el repositorio llegue a alguno de estos mínimos: 30 forks,
  30 seguidores (*watchers*) o 75 estrellas (`GITHUB_NOTABILITY_THRESHOLDS` en el código de Homebrew; la
  página <https://docs.brew.sh/Acceptable-Casks> permite revisiones caso por caso, sin garantía).
  El repositorio tiene 0 de cada una (creado el 2026-10-14).
- **Firma y notarización.** El código de Homebrew 6 dice textualmente «The homebrew/cask tap requires
  all casks to be signed and notarized by Apple», y desde el **2026-09-01** los casks que no pasan
  Gatekeeper se deshabilitan (anuncio de Homebrew 5.0; lo vimos citado por terceros, y el código local
  de `brew audit` lo confirma). La app de Tlaku no está notarizada.
  La prueba: `brew audit --cask --new --online` sobre nuestro cask da exactamente esos dos errores
  («Signature verification failed… not signed by a distributor that meets the system Gatekeeper
  requirements» y «GitHub repository not notable enough»).

**Ventajas del tap propio** (`jahziel1/homebrew-tlaku`): sin revisión ni espera, sin requisitos de
notoriedad ni de notarización, tú decides cuándo subir de versión, y se puede migrar al cask oficial
después (basta cambiar el comando de instalación). Desventajas: el usuario ejecuta un paso más
(`brew tap` implícito en `brew install --cask jahziel1/tlaku/tlaku`) y Homebrew no avisa de
actualizaciones automáticamente hasta que subas el cask.

**Qué hay:** `packaging/homebrew/Casks/tlaku.rb` (raíz del tap = esa carpeta): `url` del `.dmg` universal
de la Release, `sha256` real `7dbabd44861389c01b3534ea377f69a5403e37e1a9d114dcdb77b6d097f885e6`,
`app "Tlaku.app"` (comprobado montando el `.dmg`: contiene `Tlaku.app`, binario universal
x86_64+arm64, `CFBundleIdentifier` `io.github.jahziel1.tlaku`, `LSMinimumSystemVersion` 11.0),
`livecheck` con `:github_latest`, `auto_updates true`, `depends_on macos: :big_sur`, `zap` de las
carpetas de datos que Tauri crea a partir del identificador (`Application Support`, `Caches`,
`HTTPStorages`, `Logs`, `Preferences`, `Saved Application State`, `WebKit`) y un `caveats` sobre
Gatekeeper.

**Validación hecha** con Homebrew 6.0.21 en un tap local temporal (ya borrado): `brew style --cask`
sin ofensas, `brew audit --cask --strict --online` sin errores (descarga el `.dmg` y comprueba el
`sha256`), `brew livecheck --cask` devuelve `0.1.0 ==> 0.1.0` y `brew info` muestra los avisos. No se
probó `brew install` (instalaría en `/Applications`). Para repetirla:

```sh
brew tap-new --no-git local/tlaku-prueba
cp packaging/homebrew/Casks/tlaku.rb "$(brew --repository local/tlaku-prueba)/Casks/"
brew style --cask local/tlaku-prueba/tlaku
brew audit --cask --strict --online local/tlaku-prueba/tlaku
brew livecheck --cask local/tlaku-prueba/tlaku
brew untap local/tlaku-prueba
```

(`brew audit` puede clonar `homebrew/core` la primera vez: ~1.4 GB; se quita con `brew untap homebrew/core`.)

**Cómo publicarlo:**

1. Crea el repositorio público **`jahziel1/homebrew-tlaku`** (el prefijo `homebrew-` es obligatorio).
2. Copia el contenido de `packaging/homebrew/` (la carpeta `Casks/`) a la raíz, añade un `README.md`
   con la instalación y haz commit y push a la rama principal.
3. Prueba en tu Mac: `brew install --cask jahziel1/tlaku/tlaku` y, si Gatekeeper lo bloquea,
   `xattr -dr com.apple.quarantine /Applications/Tlaku.app`.
4. Anúnciala en el README y en la Release.

**Gatekeeper (importante).** Homebrew 6 ya no tiene `--no-quarantine`: la app instalada con `brew`
conserva la cuarentena y macOS la bloquea la primera vez. El `caveats` del cask da la solución
(`xattr -dr com.apple.quarantine /Applications/Tlaku.app` o «Abrir de todos modos» en Ajustes del
Sistema > Privacidad y seguridad). **Hallazgo:** al revisar el `.dmg`, `codesign -dv` muestra la app con
firma *ad hoc* de enlazador (`adhoc,linker-signed`, `Info.plist=not bound`, `Sealed Resources=none`) y
`spctl` la rechaza («no usable signature»). En Apple Silicon una app con firma así y en cuarentena suele
mostrar «Tlaku está dañado y no se puede abrir» en vez del aviso normal (no comprobado en un Mac real); mientras no se notarice, vale
la pena firmarla al menos *ad hoc* completa en la compilación (`codesign --force --deep -s -`) para que
el sello cubra el paquete. Con la membresía de Apple Developer (la sección [macOS: firmar y
notarizar](#macos-firmar-y-notarizar) ya está lista) desaparece el problema y el cask oficial pasa a
ser posible cuando el proyecto tenga notoriedad.

**Cada versión nueva:** cambia `version` y `sha256` (`shasum -a 256 Tlaku_X.Y.Z_universal.dmg`), repite
`brew audit` y sube. `brew livecheck --cask jahziel1/tlaku/tlaku` avisa cuando hay versión nueva.

### Flathub (Linux)

**Carpeta lista para el PR:** `packaging/flathub/io.github.jahziel1.tlaku/` (los cuatro archivos van
en la **raíz** de la rama del PR, no en una subcarpeta):

| Archivo | Diferencia con `packaging/flatpak/` |
|---|---|
| `io.github.jahziel1.tlaku.yml` | Las fuentes `.deb` locales se cambiaron por URL + sha256 de la Release v0.1.0: `Tlaku_0.1.0_amd64.deb` = `444b1d7facbf6b998d0dad534e2a8db9fe68ae300eae60c1aaf73e581af9efbe` (x86_64) y `Tlaku_0.1.0_arm64.deb` = `6feb2e63e6b7c8af964359c61bb68da91dd3ecaeb9028f4235b12dcfd9f5f36d` (aarch64). Hashes calculados con los assets reales y iguales a los que publica GitHub. |
| `io.github.jahziel1.tlaku.metainfo.xml` | Idéntico al de `packaging/flatpak/`: `<release version="0.1.0" date="2026-10-15">`, capturas por la etiqueta `v0.1.0`. Se acortó el `<summary>` a 34 caracteres (la guía de calidad de Flathub pide ≤35; antes tenía 70). |
| `io.github.jahziel1.tlaku.desktop`, `.svg` | Idénticos. |
| `flathub.json` | **No hace falta**: Flathub compila x86_64 y aarch64 por omisión, y solo se necesita para limitar arquitecturas o desactivar compilaciones. |

Mantén las copias sincronizadas: `diff -r packaging/flatpak packaging/flathub/io.github.jahziel1.tlaku`
solo debe mostrar el manifiesto `.yml` (y que en `flatpak` no estén los `.deb`).

**Validación hecha (macOS):** `appstreamcli validate --pedantic --no-net` sin avisos (con red solo
avisa que `ko-fi.com` devuelve 403 a los robots; el linter de Flathub usa `--no-net`);
`desktop-file-validate` sin errores; el manifiesto cargado con `ruamel.yaml`; y las comprobaciones de
manifiesto de `flatpak-builder-lint` (`finish_args`, `toplevel`, `modules`, `jsonschema`, `flathub_json`)
ejecutadas directamente en Python: **0 errores**. **No se pudo ejecutar** el linter completo
(`flatpak-builder-lint manifest/repo/builddir`: necesita `flatpak-builder`, OSTree y el repositorio de
Flathub, solo en Linux) ni se construyó el paquete. Hazlo tú en Linux:

```sh
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest io.github.jahziel1.tlaku.yml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder appstream io.github.jahziel1.tlaku.metainfo.xml
flatpak-builder --force-clean --user --install-deps-from=flathub --repo=repo build-dir io.github.jahziel1.tlaku.yml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo repo
```

**Tres obstáculos que debes conocer antes de enviar** (leídos de
<https://docs.flathub.org/docs/for-app-authors/requirements> el 2026-10-15):

1. **Compilar desde fuentes.** «All source available submissions must be built entirely from source
   code.» Las excepciones son «case-by-case» para proveedores conocidos. La estrategia A (reempaquetar
   los `.deb`) **probablemente será rechazada**; esta carpeta la deja lista porque es lo que se
   construyó y publicó como `.flatpak` en la Release, pero el camino realista es la **estrategia B**
   (descrita arriba, en [Estrategia](#estrategia-reempaquetar-el-deb-a-y-no-compilar-desde-fuentes-b)):
   `cargo-sources.json` + `node-sources.json` con `flatpak-builder-tools`, módulo `simple` que compila
   offline. No está hecha ni probada.
2. **Política de IA de Flathub.** Exige declarar cualquier código, documentación o empaquetado generado
   con IA, y dice: «Flathub manifests must not contain AI-generated or AI-assisted content» (declararlo
   no exime) y «AI tools or agents must not open or automate Flathub submission pull requests» ni
   redactar mensajes de commit, descripciones o respuestas de revisión. Tlaku se desarrolló con
   asistencia de IA y **estos archivos de empaquetado también**. Antes de enviar tienes que decidir tú:
   escribir a mano el manifiesto (es corto; esta carpeta sirve de referencia, no de entrega), abrir y
   redactar tú el PR y todas las respuestas, y declarar en el PR el uso de IA en el código de la
   aplicación. Una declaración falsa o incompleta puede acabar en rechazo o veto. Esto no lo decide
   ningún agente: es tuyo.
3. **Runtime.** El manifiesto usa `org.gnome.Platform//50`. GNOME 51 salió el 2026-09-16 y ya está en
   Flathub (`org.gnome.Platform//51`). El linter solo marca un runtime como **error** cuando lleva 3
   versiones de atraso; con 50 daría un aviso `runtime-update-available-to-org.gnome.Platform-51`, pero
   un revisor puede pedir subirlo. Para subir: `runtime-version: '51'` y, en `flatpak.yml`, la imagen
   `gnome-51` (existe `gnome-50` y `gnome-49`; confirma `gnome-51` en
   <https://github.com/flathub-infra/flatpak-github-actions/pkgs/container/flatpak-github-actions>), y
   comprueba que el `ldd` del manifiesto sigue pasando (que 51 aún trae `libwebkit2gtk-4.1`).

**Pasos del envío** (ver también «Enviar a Flathub (pasos)», más arriba):

1. Prueba el paquete en un Linux real (lista de comprobación en «Probar el paquete en Linux»).
2. Decide A/B, el runtime y la declaración de IA (obstáculos 1–3). Si vas con B, regenera los
   `*-sources.json` y reemplaza las fuentes `.deb`.
3. Bifurca <https://github.com/flathub/flathub> **con todas las ramas** (no solo `master`), clona la
   rama `new-pr` y crea una rama nueva: `git checkout -b tlaku new-pr`.
4. Copia los archivos de `packaging/flathub/io.github.jahziel1.tlaku/` a la raíz, `git add`,
   `git commit -m "Add io.github.jahziel1.tlaku"` y sube.
5. Abre el PR **contra `new-pr`** (no `master`) con el título `Add io.github.jahziel1.tlaku`, llena la
   lista de la plantilla y escribe tú la descripción, incluida la justificación de `--device=all`
   (puertos serie `/dev/ttyUSB*`/`ttyACM*`, placas USB K40/galvo y cámaras `/dev/video*`; no existe
   permiso más acotado para serie) y de `--share=network` (controladoras por red y control remoto).
6. Comenta `bot, build` en el PR para lanzar la compilación de prueba de Flathub; corrige lo que salga.
7. Revisores: responden en el PR (suelen tardar semanas; contesta rápido). Al aceptar, se crea
   `github.com/flathub/io.github.jahziel1.tlaku` y te invitan (necesitas 2FA); el ID `io.github.jahziel1.*`
   se verifica con tu inicio de sesión de GitHub.
8. Cada versión nueva: PR (o commit) en ese repositorio con el `<release>` nuevo en el metainfo y
   `url` + `sha256` nuevos (o el commit y la etiqueta con B). Opcional: `x-checker-data`
   (flatpak-external-data-checker) para que un bot proponga las subidas.

**Qué revisan:** `flatpak-builder-lint` (errores bloqueantes), metainfo (capturas accesibles y de la
versión, descripción, licencia, ID), permisos justificados, fuente construida desde código, runtime
vigente, que el nombre e ícono no suplanten a otro programa y que quien envía mantenga el proyecto.

## Actualizaciones automáticas

**Qué hay en el código (ya integrado):**

- `tauri-plugin-updater` y `tauri-plugin-process` (ambos MIT o Apache-2.0, comprobado con
  `cargo deny check licenses sources`) y sus equivalentes de npm.
- Menú **Ayuda > Buscar actualizaciones…** y la preferencia **Buscar actualizaciones al iniciar**
  (Preferencias > General, **apagada de fábrica**; ni siquiera se importa desde un `.tlakuprefs`).
  Si hay una versión nueva, un diálogo muestra la versión y sus notas con los botones **Descargar e
  instalar** y **Ahora no**; tras instalar ofrece **Reiniciar ahora**. No instala con un trabajo en
  curso en la máquina. La descarga se verifica con la firma del proyecto.
- **En Flatpak el actualizador se desactiva** (las actualizaciones las da Flathub): no se registra el
  plugin y el menú lo explica; ver [Flatpak y Flathub](#flatpak-y-flathub).
- **Estado actual: activado.** `tauri.conf.json` trae la llave pública del proyecto, el endpoint
  `https://github.com/jahziel1/tlaku/releases/latest/download/latest.json` y
  `bundle.createUpdaterArtifacts: true`; solo `release.yml` empaqueta, con los secretos
  `TAURI_SIGNING_PRIVATE_KEY` y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. `releases/latest` **no incluye
  las prereleases**: publica las betas como versión normal con «beta» en el título, o el actualizador
  no las verá. La llave privada vive fuera del repositorio (`~/.tauri/tlaku.key` y su respaldo);
  si se pierde, las versiones instaladas ya no aceptarán actualizaciones.
- **Sin llave configurada no pasa nada malo** (un fork, o tras `--desactivar`): `plugins.updater` con
  `pubkey` y `endpoints` vacíos y `bundle.createUpdaterArtifacts: false`. En esa compilación el actualizador
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

- **Sitio** (`site/`): HTML y CSS estáticos, sin framework ni recursos externos, en español y en
  inglés. Portada (`site/index.html`, `site/en/index.html`) con qué es Tlaku, capturas, funciones,
  máquinas y su estado, la sección «Ayúdanos a probar» (con enlaces a los formularios de GitHub),
  descargas, manual, donaciones y licencia, y tres páginas por idioma pensadas para las búsquedas:
  «Alternativa libre a LightBurn» (`alternativa-a-lightburn/`, `en/lightburn-alternative/`), «Tlaku para
  la Creality Falcon2» (`falcon2/`, `en/falcon2/`) y las preguntas frecuentes (`preguntas-frecuentes/`,
  `en/faq/`). Los textos dicen lo que hay: lo que no se ha probado con hardware se dice claramente (hoy:
  solo una Falcon2 Pro 22 W real, sin corte con potencia alta); al cambiar el estado de las pruebas,
  actualiza la portada, esas tres páginas y los README.
- **Cómo se arma una página:** cada `index.html` lleva su `<title>`, su `<meta name="description">` y
  los marcadores `<!--@include head-->`, `sprite`, `header` y `footer` (parciales en `site/_partials/`,
  uno por idioma donde cambia el texto) y `<!--@seo-->`, que `scripts/build-site.mjs` sustituye por
  canonical, hreflang (es, en y x-default), Open Graph, Twitter Card y JSON-LD (`scripts/site-seo.mjs`).
  Además de los marcadores de abajo hay `{{ROOT}}`, `{{HOME}}` y `{{ALT_URL}}` (rutas relativas de cada
  página) y `{{VERSION}}` (la de `tauri.conf.json`). **Para sumar una página:** crea sus dos archivos
  (es y en), regístrala en `site/pages.json` (el build falla si un `.html` no está ahí) y enlázala desde
  `site/_partials/{es,en}/header.html` y `footer.html`.
- **Imágenes para redes** (`site/og-es.png`, `site/og-en.png`, 1200 × 630): se generan una vez con
  `python3 scripts/site-og.py` a partir de una captura de `apps/desktop/docs/screenshots/` y se guardan
  en el repositorio; no se regeneran en la CI.
- **Marcadores:** `{{REPO_URL}}`, `{{SPONSORS_URL}}` y `{{KOFI_URL}}`. Se llenan desde
  `site/config.json`:

  ```sh
  node scripts/build-site.mjs                  # construye site-dist/ (gitignorado), con sitemap.xml y robots.txt
  node scripts/check-site.mjs site-dist        # enlaces internos, metadatos, JSON-LD, sitemap y CSP
  node scripts/build-site.mjs --readme         # además regenera el bloque de apoyo de README.md y README.es.md
  python3 -m http.server -d site-dist 8000     # para verlo en local
  ```

  Un valor vacío oculta su botón o enlace y muestra «aún no hay…». El script copia las capturas de
  `apps/desktop/docs/screenshots/` y falla si un marcador o una captura no existe. `check-site.mjs`
  falla si hay un enlace o una ancla rota, un recurso externo (la CSP es `default-src 'none'`), un
  estilo o script en línea, un JSON-LD que no parsea o no coincide con la página, o un sitemap sin
  alguna página. El JSON-LD (`<script type="application/ld+json">`) es un bloque de datos, no se
  ejecuta, y la CSP (`script-src 'self'`) no lo bloquea: se comprobó en Chrome sin avisos en la consola.
- **Botones de descarga de los README:** apuntan a los archivos de la versión publicada
  (`releases/download/v0.1.0/Tlaku_0.1.0_…`). Al sacar una versión nueva, actualiza esos enlaces y el
  número en «Descarga la beta» / «Download the beta» de `README.md` y `README.es.md`. El sitio usa
  `releases/latest` y la versión de `tauri.conf.json`, así que no necesita tocarse.
- **GitHub Pages:** `.github/workflows/pages.yml` publica el sitio al empujar a `main`, pero está
  inactivo hasta crear la variable de repositorio `PAGES_ENABLED=true` (paso 6 de la lista de abajo).
- **Donaciones:** `.github/FUNDING.yml` (botón Sponsor del repositorio; líneas comentadas), sección
  «Apoyar el proyecto» de los README (el bloque entre `<!-- support:start -->` y `<!-- support:end -->` lo
  regenera `--readme`; hoy dice que aún no hay cuentas) y Ayuda > «Donar al proyecto…» en la app. Los enlaces de la app
  viven en `apps/desktop/src/features/help/links.ts` (`PROJECT_LINKS`, vacíos por omisión): sin
  ellos, el diálogo dice que el proyecto aún no tiene cuentas de donación.

### Dar de alta el sitio en Google Search Console

Aún no está hecho; son pasos para cuando el autor quiera que Google conozca el sitio (sin esto Google
también lo encuentra por los enlaces, pero más despacio). La indexación tarda de días a semanas y
no se puede forzar.

1. Entra en <https://search.google.com/search-console> con la cuenta de Google del autor.
2. **Añadir propiedad** > tipo **Prefijo de la URL** con `https://jahziel1.github.io/tlaku/`. El tipo
   «Dominio» no sirve: `github.io` no es del autor.
3. **Verificar** con la **etiqueta HTML**: Search Console da una línea
   `<meta name="google-site-verification" content="…">`. Pégala dentro del `<head>` de `site/index.html`
   (debe estar en la portada), construye, comprueba con `check-site.mjs`, súbela a `main` y, cuando el
   sitio ya la sirva, pulsa **Verificar**. No uses el método del archivo `googleXXXX.html`: el build
   falla con cualquier `.html` que no esté en `site/pages.json`. Deja la etiqueta puesta, o Google
   revocará la verificación.
4. **Sitemaps** > escribe `sitemap.xml` (queda en `https://jahziel1.github.io/tlaku/sitemap.xml`) >
   Enviar. Debe decir «Correcto» y detectar 8 páginas (4 por idioma).
5. **Inspección de URL**: pega la portada y las tres páginas nuevas, en español y en inglés, y pulsa
   **Solicitar indexación** (hay una cuota diaria pequeña).
6. Más adelante revisa **Indexación > Páginas** (que las 8 estén indexadas y qué razón dan las que no),
   **Rendimiento** (con qué consultas aparece el sitio) y **Mejoras** (datos estructurados). No esperes
   el desplegable de preguntas en los resultados: Google limita ese formato a sitios de ciertos tipos
   (gobierno, salud), y los resultados enriquecidos de `SoftwareApplication` piden valoraciones, que el
   sitio no inventa. El marcado es válido y no estorba.
7. Opcional: **Bing Webmaster Tools** permite importar la propiedad desde Search Console.

Notas: en un sitio de proyecto de GitHub Pages el `robots.txt` queda en `/tlaku/robots.txt`, no en la
raíz del dominio, así que los rastreadores no lo consultan (leen `https://jahziel1.github.io/robots.txt`,
que el autor no controla); es inofensivo y por eso el sitemap se envía a mano en el paso 4. Si algún día
hay un dominio propio (Settings > Pages > Custom domain), cambia `site_url` en `site/pages.json`,
vuelve a construir y da de alta una propiedad de tipo **Dominio**.

## CI

`.github/workflows/ci.yml` corre en cada push a `main` y en cada pull request:

| Trabajo | Qué comprueba |
|---|---|
| Rust (Linux, macOS, Windows) | `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features` |
| cargo-deny | `cargo deny check licenses sources` |
| Interfaz | `tsc --noEmit`, `eslint`, `vitest run` y la tabla de atajos del README |
| Flatpak (`flatpak.yml`; solo con etiqueta o a mano) | `appstreamcli validate --no-net` y `desktop-file-validate` de los metadatos, `flatpak-builder` en x86_64 y aarch64 y adjunta los `.flatpak` a la Release |
| Manifiestos de tiendas (`winget.yml`; opcional, al publicar una Release o a mano) | genera los manifiestos de winget de esa versión, los valida contra el esquema JSON y los deja como artefacto; no envía nada |
| Prueba de humo (Linux) | arranca la app Tauri real bajo Xvfb (`apps/desktop/scripts/smoke-tauri.sh`) |
| Rust y humo (Linux ARM64) | en `ubuntu-22.04-arm`: `cargo nextest run --workspace --all-features` y la prueba de humo. Sin fmt ni clippy (no dependen de la arquitectura). Aún no es obligatorio en el ruleset |
| Sitio estático | `node scripts/build-site.mjs` (marcadores y capturas válidos), el catálogo de la comunidad y `node scripts/check-site.mjs` (enlaces internos, metadatos SEO, JSON-LD, sitemap) |

Las pruebas de Rust incluyen la validación de `community/materials/*.json` (`cargo test -p tlaku-doc
community`) y las de la interfaz comprueban que los formularios de `.github/ISSUE_TEMPLATE/` tengan los
ids que la app prellena. El flujo `.github/workflows/community.yml` comenta en los issues de ajustes de
material (permiso mínimo `issues: write`; no escribe en el repositorio).

Tauri exige que exista `apps/desktop/dist`; en los trabajos de Rust se crea una página vacía de
relleno y en la prueba de humo se compila la interfaz real. Los flujos no se pueden ejecutar fuera de
GitHub: se validaron con `actionlint` y ejecutando cada comando en local; la primera ejecución real
puede revelar ajustes (por ejemplo, paquetes de apt).

## Pasos para publicar: lo que tienes que hacer tú

Todo lo siguiente necesita algo que solo tú tienes (cuentas, llaves, dinero). Hazlo en este orden;
cada paso indica el archivo donde va el valor.

1. ~~**Crear el repositorio** en GitHub (público) y subir el código.~~ **Hecho:**
   <https://github.com/jahziel1/tlaku>.
2. ~~**Poner la URL en el proyecto.**~~ **Hecho:** `homepage` y `repository` en `[workspace.package]` de
   `Cargo.toml` (el crate de la app los hereda; Tauri usa `homepage` en el `.deb`), `repo` e `issues` en
   `apps/desktop/src/features/help/links.ts`, `REPO_URL` en `site/config.json` y el `git clone` y los
   enlaces de los README.
3. **Configurar el repositorio en la web de GitHub** (no se puede por API desde aquí; ver también los
   comandos de abajo):
   - Settings > General > Features: activar **Discussions** y crear categorías (por ejemplo «Preguntas»,
     «Ideas», «Mostrar y contar», «Máquinas y ajustes»). Los formularios y SUPPORT.md apuntan a
     `/discussions`.
   - Settings > Code security: activar **Private vulnerability reporting** (lo usan SECURITY.md y el
     Código de conducta), Dependabot alerts y Dependabot security updates.
   - Crear las etiquetas: `scripts/sync-labels.sh` (usa `.github/labels.yml`; necesita `gh auth login`).
   - Settings > Branches: proteger `main` (CI obligatoria y revisión).
   - Settings > Actions > General: permitir las acciones; los permisos por omisión del `GITHUB_TOKEN`
     pueden ser de solo lectura (cada flujo pide los suyos).
   - Opcional: temas del repositorio (`laser`, `lasercutter`, `grbl`, `tauri`, `rust`, `open-source`).
4. **Generar las llaves del actualizador** y cargar los secretos (sección
   [Actualizaciones automáticas](#actualizaciones-automáticas)): `npx tauri signer generate -w
   ~/.tauri/tlaku.key`; secretos `TAURI_SIGNING_PRIVATE_KEY` y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`;
   luego `scripts/configure-updater.sh https://github.com/jahziel1/tlaku ~/.tauri/tlaku.key.pub` y sube
   el cambio de `tauri.conf.json`. Guarda la llave privada fuera del repositorio. **Pendiente.**
5. ✓ **Donaciones (Ko-fi, hecho 2026-10-14):** `https://ko-fi.com/danielmartinez55371` en `.github/FUNDING.yml`,
   `links.ts` (`KOFI_URL`), `site/config.json`, el permiso `opener:allow-open-url` (solo esa página) y los dos
   README (`node scripts/build-site.mjs --readme`). GitHub Sponsors queda opcional: si se abre, llena
   `SPONSORS_URL`/`sponsors`, agrega su URL al permiso del opener y descomenta `github:` en `FUNDING.yml`.
6. **Certificados de firma** (opcional; sin ellos los instaladores salen sin firmar y funcionan con
   advertencias): Apple Developer Program para macOS y un certificado de firma de código (o SignPath)
   para Windows; los secretos y su formato están en [Firma de código](#firma-de-código). **Pendiente.**
7. **Activar el sitio:** Settings > Pages > Source: "GitHub Actions" y, en Settings > Secrets and
   variables > Actions > Variables, crear `PAGES_ENABLED` = `true`. Empuja a `main` (o lanza el flujo
   "Sitio" a mano). La dirección será <https://jahziel1.github.io/tlaku/>.
8. **Comprobar la CI:** empuja a `main` y revisa que los cinco trabajos de CI pasen; corrige lo que
   salga en la primera ejecución real.
9. **Primera beta:** sube la versión (`0.x.0-beta.1`, en los tres lugares de
   [Sacar una versión](#sacar-una-versión); recuerda que el `.msi` solo admite sufijos numéricos, así
   que para probar Windows usa `0.x.0-1`), etiqueta `v0.x.0-beta.1`, empuja la etiqueta, revisa el
   borrador en Releases, prueba un instalador y publícalo (se marca como pre-lanzamiento).
10. **Avisar:** con la Release publicada, el enlace «Descargar la beta en GitHub» del sitio ya apunta a
    algo real. Antes de anunciarlo, repite el recordatorio del README: solo se probó una Falcon2 Pro (sin
    corte con potencia alta) y lo demás va con emuladores, y pide a la comunidad que pruebe con los formularios de
    [Ayúdanos a probar](../README.es.md#ayúdanos-a-probar-con-máquinas-reales).

### Comandos de `gh` para el primer arranque

```sh
scripts/sync-labels.sh --dry-run                 # muestra lo que haría
scripts/sync-labels.sh                           # crea o actualiza las etiquetas
gh repo edit jahziel1/tlaku --enable-discussions --enable-issues \
  --description "Free open source LightBurn alternative for laser engravers and cutters / Alternativa libre a LightBurn para láseres" \
  --homepage "https://jahziel1.github.io/tlaku/" \
  --add-topic laser --add-topic lasercutter --add-topic grbl --add-topic tauri --add-topic rust \
  --add-topic lightburn-alternative --add-topic laser-engraver --add-topic gcode --add-topic ruida --add-topic k40
```

La descripción y los temas del repositorio también cuentan para las búsquedas (GitHub muestra la
descripción como título del resultado); revisa que digan lo mismo que el sitio.

Las categorías de Discussions y el Private vulnerability reporting se activan en la web (Settings).

### Revisar un ajuste de material de la comunidad

Cuando llegue un issue con la etiqueta `material-settings` (el flujo `community.yml` comenta un resumen):
revisa los valores y la foto; `node scripts/community/issue-to-preset.mjs <número>`; revisa el archivo
generado en `community/materials/`; `cargo test -p tlaku-doc community`; abre el PR (con `git commit -s`).
Detalle en [community/materials/README.es.md](../community/materials/README.es.md).

## Pendiente de decidir

- **Certificados.** Hasta que se compren, los instaladores de macOS y Windows se publican sin
  firmar. El programa de [SignPath Foundation](https://signpath.org/) (firma gratuita para
  proyectos libres) es una alternativa a evaluar para Windows.
