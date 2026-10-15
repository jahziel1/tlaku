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
   | `packaging/flatpak/org.tlaku.app.metainfo.xml` | Un `<release version="X.Y.Z" date="AAAA-MM-DD">` nuevo arriba y la etiqueta `vX.Y.Z` en las URL de las capturas. No bloquea la publicación (`flatpak.yml` solo avisa si falta), pero Flathub lo exige: ver [Flatpak y Flathub](#flatpak-y-flathub). |

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
| `packaging/flatpak/org.tlaku.app.yml` | Manifiesto (estrategia A, ver abajo). Id `org.tlaku.app`, runtime `org.gnome.Platform//50`. |
| `packaging/flatpak/org.tlaku.app.metainfo.xml` | AppStream: nombre y resumen es/en, descripción, 5 capturas (de `apps/desktop/docs/screenshots/`, por la URL de la etiqueta), licencia MIT, donación Ko-fi, release 0.1.0, OARS sin contenido. |
| `packaging/flatpak/org.tlaku.app.desktop` | Entrada de menú: categorías `Graphics;2DGraphics;VectorGraphics;Engineering;` y palabras clave es/en. **Sin `MimeType`**: la app aún no abre archivos `.tlaku` pasados por argumento (no hay asociación de archivos en `tauri.conf.json`); declararlo sería prometer algo que no hace. |
| `packaging/flatpak/org.tlaku.app.svg` | Ícono (copia de `apps/desktop/src-tauri/icons/source/tlaku.svg`; Flathub prefiere SVG). `flatpak.yml` avisa si difieren. |
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
`~/.var/app/org.tlaku.app/` y **no se comparten** con una instalación `.deb` de la misma persona; para
pasar los ajustes, un `.tlakubundle`.

### Probar el paquete en Linux (lo que falta comprobar)

```sh
# Linux con flatpak y flatpak-builder; los .deb de la Release en packaging/flatpak/debs/
mkdir -p packaging/flatpak/debs
cp Tlaku_0.1.0_amd64.deb packaging/flatpak/debs/tlaku_x86_64.deb      # (o _arm64.deb -> tlaku_aarch64.deb)
flatpak-builder --force-clean --user --install-deps-from=flathub --repo=repo build-dir packaging/flatpak/org.tlaku.app.yml
flatpak build-bundle repo tlaku.flatpak org.tlaku.app && flatpak install --user tlaku.flatpak
flatpak run org.tlaku.app
# El linter de Flathub (lo que revisará el PR):
flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest packaging/flatpak/org.tlaku.app.yml
flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo repo
```

Lista de comprobación: abre la ventana (Wayland y X11; con GPU NVIDIA prueba
`--env=WEBKIT_DISABLE_DMABUF_RENDERER=1`); importar un SVG por el diálogo; guardar y abrir un proyecto;
la vista previa de una imagen; **la lista de puertos con un láser conectado**; conectar y mover; el
control remoto desde el celular; Ayuda > Buscar actualizaciones (debe decir que lo da Flathub).

### Enviar a Flathub (pasos)

1. **Decide el identificador.** `org.tlaku.app` coincide con el `identifier` de Tauri (de él salen el
   `app_id` de GTK/Wayland, el ícono de la barra de tareas y la carpeta de datos). **Pero Flathub exige
   que quien envía controle el dominio** (`tlaku.org`, accesible por HTTPS) para los identificadores
   `org.`; el proyecto hoy solo tiene `jahziel1.github.io/tlaku`. Opciones: (a) registrar `tlaku.org`
   y publicar el sitio ahí; (b) usar `io.github.jahziel1.tlaku` (Flathub lo acepta para proyectos en
   GitHub) y cambiar a la vez el `identifier` de `tauri.conf.json` (para que el `app_id` coincida con
   el `.desktop`), los nombres y `<id>` de los tres archivos de `packaging/flatpak/`, `Icon=`,
   `<launchable>` y el texto `flatpak update ...` de `help.update.managedHint`. Cambiar el `identifier`
   mueve la carpeta de datos de la app en todos los sistemas: hazlo antes de la primera versión pública.
2. **Publica la Release** (`vX.Y.Z`) con sus `.deb` y confirma que las URL de las capturas del metainfo
   responden (`appstreamcli validate packaging/flatpak/org.tlaku.app.metainfo.xml`, sin `--no-net`).
3. **Prepara el manifiesto de Flathub**: copia `packaging/flatpak/org.tlaku.app.yml` y cambia las dos
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
- Decidir el identificador y el dominio (punto 1).
- Que exista la Release `v0.1.0` con sus `.deb` y la etiqueta que usan las capturas.
- Decidir A o B según la respuesta de los revisores y mantener el runtime al día.
- Una regla udev distribuida para K40 y galvo (hoy no la hay) y confirmar que `serialport` ve los puertos dentro del sandbox.
- Opcional: ampliar el alcance de `asset://` a la ruta del portal para poder quitar los `--filesystem`.
- Opcional: asociación de archivos `.tlaku` (hay que implementarla en la app antes de poner `MimeType`).

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

- **Sitio** (`site/`): HTML y CSS estáticos, sin framework ni recursos externos, en español
  (`site/index.html`) y en inglés (`site/en/index.html`). Muestra qué es Tlaku, capturas, funciones,
  máquinas y su estado, la sección «Ayúdanos a probar» (con enlaces a los formularios de GitHub),
  descargas, manual, donaciones y licencia. Los textos dicen lo que hay: lo que
  no se ha probado con hardware se dice claramente.
- **Marcadores:** `{{REPO_URL}}`, `{{SPONSORS_URL}}` y `{{KOFI_URL}}`. Se llenan desde
  `site/config.json`:

  ```sh
  node scripts/build-site.mjs                  # construye site-dist/ (gitignorado)
  node scripts/build-site.mjs --readme         # además regenera el bloque de apoyo de README.md y README.es.md
  python3 -m http.server -d site-dist 8000     # para verlo en local
  ```

  Un valor vacío oculta su botón o enlace y muestra «aún no hay…». El script copia las capturas de
  `apps/desktop/docs/screenshots/` y falla si un marcador o una captura no existe.
- **GitHub Pages:** `.github/workflows/pages.yml` publica el sitio al empujar a `main`, pero está
  inactivo hasta crear la variable de repositorio `PAGES_ENABLED=true` (paso 6 de la lista de abajo).
- **Donaciones:** `.github/FUNDING.yml` (botón Sponsor del repositorio; líneas comentadas), sección
  «Apoyar el proyecto» de los README (el bloque entre `<!-- support:start -->` y `<!-- support:end -->` lo
  regenera `--readme`; hoy dice que aún no hay cuentas) y Ayuda > «Donar al proyecto…» en la app. Los enlaces de la app
  viven en `apps/desktop/src/features/help/links.ts` (`PROJECT_LINKS`, vacíos por omisión): sin
  ellos, el diálogo dice que el proyecto aún no tiene cuentas de donación.

## CI

`.github/workflows/ci.yml` corre en cada push a `main` y en cada pull request:

| Trabajo | Qué comprueba |
|---|---|
| Rust (Linux, macOS, Windows) | `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features` |
| cargo-deny | `cargo deny check licenses sources` |
| Interfaz | `tsc --noEmit`, `eslint`, `vitest run` y la tabla de atajos del README |
| Flatpak (`flatpak.yml`; solo con etiqueta o a mano) | `appstreamcli validate --no-net` y `desktop-file-validate` de los metadatos, `flatpak-builder` en x86_64 y aarch64 y adjunta los `.flatpak` a la Release |
| Prueba de humo (Linux) | arranca la app Tauri real bajo Xvfb (`apps/desktop/scripts/smoke-tauri.sh`) |
| Rust y humo (Linux ARM64) | en `ubuntu-22.04-arm`: `cargo nextest run --workspace --all-features` y la prueba de humo. Sin fmt ni clippy (no dependen de la arquitectura). Aún no es obligatorio en el ruleset |
| Sitio estático | `node scripts/build-site.mjs` (marcadores y capturas válidos) |

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
10. **Avisar:** con la Release publicada, el enlace «Última versión en GitHub» del sitio ya apunta a
    algo real. Antes de anunciarlo, repite el recordatorio del README: ninguna función de máquina se ha
    probado con un láser real, y pide a la comunidad que pruebe con los formularios de
    [Ayúdanos a probar](../README.es.md#ayúdanos-a-probar-con-máquinas-reales).

### Comandos de `gh` para el primer arranque

```sh
scripts/sync-labels.sh --dry-run                 # muestra lo que haría
scripts/sync-labels.sh                           # crea o actualiza las etiquetas
gh repo edit jahziel1/tlaku --enable-discussions --enable-issues \
  --description "Free and open source software for laser cutters and engravers / Software libre para láseres" \
  --homepage "https://jahziel1.github.io/tlaku/" \
  --add-topic laser --add-topic lasercutter --add-topic grbl --add-topic tauri --add-topic rust
```

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
