# Tlaku

English · [Español](README.es.md)

[![CI](https://github.com/jahziel1/tlaku/actions/workflows/ci.yml/badge.svg)](https://github.com/jahziel1/tlaku/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Support on Ko-fi](https://img.shields.io/badge/Ko--fi-Support%20Tlaku-FF5E5B?logo=ko-fi&logoColor=white)](https://ko-fi.com/danielmartinez55371)

> **Tlaku is free and always will be.** If it saves you time or money, you can
> [buy the project a coffee on Ko-fi](https://ko-fi.com/danielmartinez55371) ☕ — it pays for test machines and development time.
> Testing on your machine and [sharing the results](https://github.com/jahziel1/tlaku/issues/new?template=machine_report.yml) helps just as much.

**Free software for laser cutters and engravers.** Tlaku is an open source alternative (MIT license)
to commercial laser control programs: you design or import a job, adjust the layers, see how it will
be cut and send it to the machine with safety checks before anything moves.

The name comes from the Nahuatl *tlacuilo*: the painter-scribe who drew and wrote the codices. A
tlacuilo turned ideas into strokes on a surface; Tlaku turns a design into the movements of a laser.

**Who it is for:** people with a diode or CO2 laser cutter or engraver who want an open program that
explains what it does, especially people coming from LightBurn and people just starting out. There is
a specific guide for switching: [coming from LightBurn](docs/manual/en/14-desde-lightburn.md).

> **Honest status:** Tlaku already has a complete desktop application, but **no machine feature has
> been tested with a real laser yet**. Everything has been validated with a controller emulator and
> with virtual machines inside the app. Until there are hardware tests, treat every job as a test:
> start with low power, scrap material and your hand near the emergency stop. Details in
> [supported controllers](#supported-controllers).

![Tlaku main window with a sample design and its layers](apps/desktop/docs/screenshots/01-principal-oscuro.png)

## Help us test on real machines

Tlaku has been tested **only with emulators, virtual machines and automated tests**. No machine
feature has been used with a real laser yet: the first test will be the author's Creality Falcon2,
and everything else depends on people who own other machines. That is why the most valuable thing you
can do today is to try it on yours and tell us what happened, **even if everything went well**.

- **Tell us how it works on your machine:** open the
  [Real machine test report](https://github.com/jahziel1/tlaku/issues/new?template=machine_report.yml)
  form. Follow the [validation list](docs/validacion-hardware.md) (in Spanish) in order, from the
  safest level (connect and read) to the riskiest (a small job), and stop at the first level that
  fails or leaves you in doubt.
- **Share your material settings:** the
  [Share material settings](https://github.com/jahziel1/tlaku/issues/new?template=material_settings.yml)
  form collects speed, power and result. After a review they join the library as **unverified
  community data**, under the CC0 license ([how it works](community/materials/README.md)).
- **Is your controller missing?** Ask for support with
  [New controller support](https://github.com/jahziel1/tlaku/issues/new?template=new_controller.yml).
- **Something broken?** [Report a bug](https://github.com/jahziel1/tlaku/issues/new?template=bug_report.yml).

From inside the app, the **Help** menu has "Report a Problem…", "Share My Machine Results…" and
"Share Material Settings…": they open your browser with the form already filled in (version,
system, controller, and the `$I` response if the console already has it). **Tlaku sends nothing on
its own:** you review the form and decide whether to post it (a GitHub account is needed).

**Safety:** use eye protection for your laser's wavelength, scrap material, keep your hand near the
emergency stop and never leave the machine alone. Tlaku is provided without warranty. If you find
something that could fire the laser where it should not, fail to stop it, or move the machine
unexpectedly, do not open a public issue: follow [SECURITY.md](SECURITY.md).

## Main features

- **Simple mode:** a guided version for beginners, in five big steps (open, material, place, review,
  start). You pick the material and, for each color of the design, **Cut**, **Engrave** or **Ignore**;
  Tlaku sets speed, power and passes from the factory material library and asks how many watts your
  laser has. It is adjusted with plain words and saved as your material. The first time it asks if
  this is your first time with a laser, and a button in the top bar switches mode without touching the
  design. The settings are starting points, not a guarantee. See [Simple mode](docs/manual/en/01b-modo-sencillo.md).

- **Design and editing:** canvas with rulers and the machine bed, selection (with special
  selections), transform with handles or exact values, flip, mirror over a line, quick and
  two-point rotations, draw order, lock, nested groups, match sizes, rectangles, ellipses, polygons,
  stars and Bézier curves, measure, node editor, snapping to nodes, midpoints, centers and
  intersections, context menu, align and distribute, grid and circular arrays, copy and paste SVG
  between programs and undo with history. Parametric shapes (rectangle, ellipse, polygon and star)
  keep their parameters in the project and stay editable when it is reopened.
- **Advanced design:** trim shapes with one click, split subpaths, optimize nodes, round corners
  (rounded, chamfer or bite), cut shapes with a knife, resize interlocking slots to the real material
  thickness, warp (arc, bulge, perspective and more), rubber-band outline, tangent circle, boolean
  wizard with thumbnails, copy along a path, virtual array (copies that are not objects), dock and
  move together, group by containment and **nesting** to arrange pieces on the bed or on an irregular
  material. Guides from the rulers, manual tabs, start point and cut direction, notes, templates, new
  window, reorderable tabs and an art library with 41 drawings.
- **Text and codes:** text with system fonts (on a path, curved or with welded letters), 15
  single-line fonts (Hershey) and an SHX font reader, and QR, Data Matrix and barcodes (Code 128, Code
  39, EAN-13, EAN-8, UPC-A, ITF and Codabar). **Variable text:** serial numbers, data from a CSV, date
  and time, advancing after each job and one copy per row.
- **Geometry operations:** union, difference, intersection, weld, offset, join paths, close paths and
  remove duplicates.
- **Import:** LightBurn projects (`.lbrn`, `.lbrn2`) with their layers and settings, SVG (with real
  `clip-path` clipping), DXF, PDF and AI (the paths), HPGL, G-code, images (PNG, JPG, BMP, WebP, TIFF,
  GIF and TGA) and image tracing (one or several at once) with preview; with an options dialog (scale,
  DPI, DXF unit, PDF page, target layer). LightBurn projects also bring perforation, dot mode, flood
  fill, overcut, ramp, pauses, through-cut and sub-layers.
- **Export and print:** SVG, DXF, PNG and vector PDF at 1:1 scale, of the whole design or of the
  selection, and print at real size.
- **Preferences and backup:** preferences window, configurable keyboard shortcuts, `.tlakuprefs`
  files to take your settings to another computer, autosave and recovery after an unexpected close.
- **Cut layers:** Line, Fill and Image modes; speed, power, passes, interval, angle, angle increment,
  bidirectional, overscan computed with the machine's physics, kerf, tabs, air, Z axis per layer and
  per pass, and fill by sweep, by contours or by regions, grouped by shape or by group.
- **Advanced cutting (line layers):** perforation (cut and skip, for folds), lead-in and lead-out of
  the contour (straight or arc), overcut, pauses at start and end, through-cut, dot mode and power
  ramp.
- **Layer organization:** additional operations (sub-layers, for example fill and then outline), copy
  and paste settings, delete a layer with its objects, your own defaults for new layers and beginner
  mode. Each object can carry its own cut priority and power scale.
- **Image:** dithering (Floyd-Steinberg, Jarvis, Stucki, Atkinson, Bayer), halftone, newsprint,
  sketch, threshold, grayscale, brightness, contrast and gamma adjustments, send without dithering,
  edge enhancement and speck removal, crop with a shape (mask), convert vectors to bitmap and save the
  processed image.
- **Preview:** playback of the toolpath with estimated time and a per-layer breakdown, and a window
  of optimization settings (respect layers, groups and priority, inner first, best start point, remove
  segments that would be cut twice, hide mechanical backlash).
- **Machine:** "Add machine" wizard with read-only detection, connection by serial port or **by
  network** (TCP/Telnet and WebSocket, with "Test connection"), move the head (configurable steps, Z
  and U axes, focus, saved positions, position the laser with a click, final position), test fire with
  double confirmation, origin and "Start from", 9-point job origin, send only the selection, framing of
  a rectangle or of the outline (also with the laser at low power and in a loop), checklist before
  sending, live speed and power adjustments (GRBL), pause and stop, macros, console with explained
  responses, the controller's `$$` settings with backup, second laser, `.tlakubundle` configuration
  packages and G-code export. Bridge power on line layers.
- **Production:** **Print & Cut** (printed registration marks, measured with the laser or with the
  camera, fit without scale, uniform scale or affine with the error of each mark), **tiling**
  (splitting a job larger than the bed into overlapping sections, alignment marks, section-by-section
  sending and resuming if interrupted) and an **automatic feeder** in the machine profile, which tiling
  uses between sections. Images do not rotate with the Print & Cut registration.
- **Control from your phone:** watch a job's progress and **pause or stop** from the phone's browser,
  by scanning a QR code in Preferences (nothing to install). **Pause and stop always work**;
  **starting** requires the safety checklist on the phone and that the job has been framed on the
  computer after the last change, and a galvo laser is never started from the phone. Off by default,
  only on your local network and with a key that changes every time the app opens. **There is no
  HTTPS: use it only on a trusted network.** See
  [Control from your phone](docs/manual/en/09e-control-desde-el-celular.md).
- **Automation:** optional local UDP server (off by default, `127.0.0.1` only) so other programs can
  send `LOAD`, `START`, `STATUS` and `STOP`; `START` asks for confirmation in the app.
- **Ruida and K40 (experimental):** Ruida controllers (CO2) over the network from the app, and K40 with
  an M2 Nano board over USB, from the app and from the command line (`tlaku k40`); K40 power is set
  with the knob.
- **Galvo (experimental):** fiber, UV and CO2 galvo lasers with the EZCAD2 LMCV4 board, over USB.
  Today it is driven from the command line (`tlaku galvo list/status/light/mark`); the interface in the
  app is on its way. Tested only against a simulated board. A fiber laser is **class 4 and its beam is
  invisible**: read [Galvo safety](docs/manual/en/13d-galvo-seguridad.md) before connecting anything.
  The engine also has sliced 3D engraving, cylinder and cone correction, wobble and the split,
  repeated and rotary marking sequence.
- **Workshop:** material library with **134 factory settings** (each with its source and confidence
  level, by machine class: diode, CO2 and fiber; they are starting points), community settings (CC0,
  unverified), material test, rotary (roller or chuck), scanning offset correction, camera with lens
  calibration (including the fisheye model), alignment and engraved marks, bed capture and tracing,
  head camera with mosaic and center finder. Inches in the interface.
- **Languages:** Spanish and English, switchable live.
- **Help:** a 34-page user manual inside the application (Help menu, with search) and in
  [docs/manual/en](docs/manual/en/00-indice.md) (also in Spanish in [docs/manual](docs/manual/00-indice.md)),
  shortcuts with `F1`, what's new and "About".

![Simple mode on the Material step: laser power, material and starting-point notices](apps/desktop/docs/screenshots/59-simple-material.png)

![Job preview with playback and per-layer results](apps/desktop/docs/screenshots/04-vista-previa.png)

## What sets it apart

- **Explainable cut planner.** Each step of the cut order says which rule decided it (layer, inside
  before container, overlap, proximity) and conflicts are flagged instead of being resolved silently.
  It shows in the preview and with `tlaku plan`; it respects the start point and direction you set by
  hand.
- **Built-in nesting.** Arranges pieces on the bed or inside an irregular material with no-fit
  polygons and time-limited search, with rotations, mirroring and pieces inside the holes of others;
  it runs in the background with progress and can be stopped (`tlaku-nest`).
- **Tiling (splitting large jobs).** LightBurn does not have it: a job larger than the bed is split
  into overlapping sections with alignment marks at the joins, sent section by section (you move the
  material, or an automatic feeder does) and can be resumed after a restart. Experimental: tested with
  the virtual machine, not with a real laser ([manual](docs/manual/en/09d-tiling-y-alimentador.md)).
- **Virtual machine.** You can connect the app to a simulated machine (Falcon2 Pro, GRBL, grblHAL,
  FluidNC, Marlin, Ruida and others), also behind a local TCP socket to test the network connection,
  and trigger faults on purpose: flame, tilt, alarm, disconnection. It is good for learning, testing a
  complete flow and reporting bugs without touching a laser.
- **Safety check before sending.** Before moving the machine its state is queried and nothing is
  transmitted if it is in alarm, paused, with the door open or running. You are warned before
  starting, not halfway through the job, if the overscan leaves the bed. The app never unlocks or
  homes on its own, and the console asks for confirmation and explains each `ALARM:N` and `error:N`.
- **Importers that warn.** Missing units in a DXF, SVG at 96 or 72 DPI, text without outline,
  clips and masks, what a PDF carries that is not a path, the scale of an HPGL: nothing is lost
  silently, every omission comes out as a warning.
- **Open source and open formats.** MIT license; `.tlaku` project as versioned JSON, `.tlakumat`
  materials, and everything the app does can be run from the command line (`tlaku`) and checked with
  tests.
- **An interface that explains.** An unavailable command says why, instead of staying gray for no
  reason.

![Virtual machine connected, with the simulated faults panel](apps/desktop/docs/screenshots/25-maquina-virtual.png)

| | | |
|---|---|---|
| ![Node editor](apps/desktop/docs/screenshots/11-editor-de-nodos.png) | ![Camera panel](apps/desktop/docs/screenshots/17-panel-camara.png) | ![Material library](apps/desktop/docs/screenshots/19-materiales.png) |
| Node editor | Camera | Materials |

### Coming next

These features are still pending; none blocks the beta. The full criterion is in section 2 of
[docs/spec/cobertura/auditoria.md](docs/spec/cobertura/auditoria.md) and the plan in
[docs/plan-completo.md](docs/plan-completo.md) (both in Spanish).

- **Before the beta:** automatic updates, the site with donations and publishing the installers (in
  progress), and testing "New window" in a real window.
- **Design and layers:** the full Beginner mode (hiding tools and menus; simple mode covers a lot of
  it), image and text properties in the same window, snapping of the warp handles, reverting a
  chamfer (today Undo is used), per-color and per-machine layer defaults, inches in layers, machines
  and camera, and autofocus.
- **Ruida:** choosing the document name and overwriting, deleting and running documents from the
  controller's memory, writing its settings (with automatic backup and `.lbset`) and the rotary. All
  of this waits for a test machine.
- **Galvo:** a dedicated rotary editor in the profile, splitting whole shapes and marking on the fly,
  the nine-point wizard and applying an EZCAD `.cor`, and the preview of the cylinder and cone
  warp. They wait for a real board.
- **Other machines:** the per-layer U offset and laser 2 with Ruida, the native feeder of a DSP
  controller with multi-column tiling, and finding the center by touching three points with the head.
  Camera preferences and the `.sc` and `.mage` formats (no public
  specification) are left for later.
- **Real machine tests:** advanced cutting, live adjustments, the network connection, Ruida, the
  K40, the galvo and the rest of the machine features have only been validated with emulators. The
  list to do it is in [docs/validacion-hardware.md](docs/validacion-hardware.md), and you can help:
  see [Help us test on real machines](#help-us-test-on-real-machines).

The real status, feature by feature (213 of the 217 features in the inventory are done; the other 4 need hardware or documentation that is not public), is in
[docs/spec/cobertura/README.md](docs/spec/cobertura/README.md) and the plan in
[docs/plan-completo.md](docs/plan-completo.md).

## Supported controllers

None has been tested with a real machine; "stable" means it works end to end against the emulator
and the app's virtual machines, with automated tests. The first test with real hardware will be a
Creality Falcon2. Full guide: [docs/manual/en/13-controladoras.md](docs/manual/en/13-controladoras.md).

| Controller or firmware | Status | Notes |
|---|---|---|
| GRBL 1.1 (M4) and GRBL-M3, over USB serial | Stable in the emulator; **to be validated on a real machine** | Most diode lasers (Creality Falcon2 included). Catalog of 21 models with sources. |
| grblHAL and FluidNC | Stable in the emulator; **to be validated on a real machine** | Own G-code dialect and virtual machine. Over USB, and over the network (TCP/Telnet and WebSocket, tested against the emulator over real sockets). |
| Marlin and Smoothie | Stable in the emulator (basic); **to be validated on a real machine** | Line-by-line sending, lost-position warning and virtual machine. |
| Old GRBL (no M4), GRBL-LPC and custom G-code | **To be validated on a real machine** | Own dialect; no dedicated virtual machine. The custom type comes with a dialect editor. |
| Ruida (CO2) | **Experimental**, in the app and on the command line | Over the network (UDP): connect, send (by default it stores the job in the controller and does not start it), pause, stop, jumps, framing, information and document list. Tested only against an emulator; it does not write settings. |
| K40 with M2 Nano board | **Experimental**, in the app and on the command line | LHYMICRO-GL protocol over direct USB (not serial), tested only against a simulated board. LightBurn does not support it. **Power is set with the machine's knob**, not by software; grays are binarized and there is no Z, air, console or overrides. Position is counted from home. |
| Fiber, UV or CO2 galvo (JCZ/BJJCZ LMCV4 board, EZCAD2) | **Experimental**, on the command line (`tlaku galvo`); the interface in the app is on its way | Direct USB, only the `9588:9899` board; tested only against a simulated board. No pause (stop only). **Invisible class 4 beam**: [read the safety page](docs/manual/en/13d-galvo-seguridad.md) before connecting. |
| Galvo EZCAD3/DLC and BSL, Trocen, TopWisdom | Not supported | A K40 whose board was swapped for GRBL-LPC or Smoothie may work. |

## Installation

### Download the application

The installers will be on the repository's **Releases** page (the latest version, "Assets" section).
No version has been published yet; until the first one, build from source (next section).

| System | File |
|---|---|
| macOS 11 or later (Apple Silicon and Intel) | `.dmg`: open it and drag Tlaku to Applications |
| Windows 10 or later | `.exe` (NSIS installer) or `.msi` |
| Linux x86_64 | `.deb` (`sudo apt install ./Tlaku_*_amd64.deb`) or `.AppImage` (`chmod +x` and run it) |
| Linux ARM64 / Raspberry Pi 4 and 5, 64-bit system (experimental) | `.deb` (`sudo apt install ./Tlaku_*_arm64.deb`) or `*_aarch64.AppImage` |
| Linux, any distribution (Flatpak, **untested**) | `Tlaku_*_x86_64.flatpak` or `Tlaku_*_aarch64.flatpak`: `flatpak install --user ./Tlaku_*_x86_64.flatpak` |

**Unsigned application.** While the project has no signing certificates, the system will warn you the
first time you open the installer:

- **macOS:** right-click Tlaku > Open, or System Settings > Privacy & Security > "Open Anyway". If it
  says the app is damaged: `xattr -dr com.apple.quarantine /Applications/Tlaku.app`.
- **Windows (SmartScreen):** "More info" and then "Run anyway".
- **Linux:** no warnings. To use the laser's serial port, add your user to the `dialout` group
  (`sudo usermod -aG dialout "$USER"`) and log in again.

**Flatpak (untested).** CI builds the package from the `.deb`, but **nobody has run it on a real Linux
machine yet** (and it is not on Flathub). Run it with `flatpak run io.github.jahziel1.tlaku`. The serial port still
needs your user in the **system's** `dialout` group, and updates come from Flatpak (the app does not
update itself). It has device access (`--device=all`) for the serial port and USB. Details and
permissions in [docs/publicar.md](docs/publicar.md#flatpak-y-flathub) (Spanish).

**Raspberry Pi (experimental).** You need a **64-bit** system (`uname -m` must print `aarch64`): Raspberry
Pi OS 64-bit (Debian 12 "bookworm" or later) or Ubuntu ARM on a Pi 4 or 5 (4 GB of RAM or more
recommended). The 32-bit system and the Pi 3 or earlier are not supported. The window uses WebKitGTK
with graphics acceleration; if it comes up blank or garbled, try `WEBKIT_DISABLE_COMPOSITING_MODE=1 tlaku`.
It has been built and tested in CI on an ARM64 machine, but **not yet on a real Raspberry Pi**: if you
try it, tell us how it went with the
[machine report](https://github.com/jahziel1/tlaku/issues/new?template=machine_report.yml). Details in
[docs/publicar.md](docs/publicar.md#linux-arm64-y-raspberry-pi) (Spanish).

More detail, and how each version is published, in [docs/publicar.md](docs/publicar.md) (Spanish).

### Build from source

Requires recent stable Rust (edition 2024). On Linux, `serialport` needs `libudev-dev` and
`pkg-config`.

```sh
git clone https://github.com/jahziel1/tlaku.git && cd tlaku
cargo build --release
# the CLI binary ends up in target/release/tlaku
```

To build the desktop application you also need Node 22.13 or later and the Tauri dependencies for
your system (on Linux, `libwebkit2gtk-4.1-dev` and others; see the
[Tauri guide](https://v2.tauri.app/start/prerequisites/)):

```sh
cd apps/desktop
npm ci
npm run tauri build     # installers in target/release/bundle/
```

For development, `npm run tauri dev`. Details in [apps/desktop/README.md](apps/desktop/README.md).

## Contributing

Tlaku belongs to everyone and is made possible by whoever tests it, improves it and spreads it.

- **Testing on a real machine** and telling us what happened is the most valuable help today (see
  [Help us test on real machines](#help-us-test-on-real-machines)).
- **Sharing material settings** from your machine: [community/materials](community/materials/README.md).
- **Reporting a problem:** [open an issue](https://github.com/jahziel1/tlaku/issues/new/choose) with the
  right form. Questions and ideas: [Discussions](https://github.com/jahziel1/tlaku/discussions). More
  in [SUPPORT.md](SUPPORT.md).
- **Code, documentation and translations:** read [CONTRIBUTING.md](CONTRIBUTING.md). The project is
  clean-room: no code, text or screenshots are copied from commercial programs.
- Every project space follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## Support the project

There is no company behind the project. Donations, if there are any, pay for tests with real
machines, installer signing and development time. Donating is voluntary: the program is the same with
or without a donation. Inside the app: Help menu > "Donate to the Project…".

<!-- This block is regenerated by `node scripts/build-site.mjs --readme` from site/config.json
     (see docs/publicar.md). Do not edit it by hand. -->
<!-- support:start -->
- Ko-fi: https://ko-fi.com/danielmartinez55371
<!-- support:end -->

## Documentation

The user manual is available in English and Spanish. Technical documents (architecture, API contract,
specification, hardware research) are written in Spanish for now.

- [User manual](docs/manual/en/00-indice.md): from first steps to troubleshooting (English; the Spanish original is [here](docs/manual/00-indice.md)).
- [docs/arquitectura.md](docs/arquitectura.md): pipeline, responsibility of each crate and conventions.
- [docs/app-api/README.md](docs/app-api/README.md): contract between the interface and the engine.
- [docs/spec/00-resumen.md](docs/spec/00-resumen.md) and [coverage](docs/spec/cobertura/README.md): functional inventory and real status.
- [docs/plan-completo.md](docs/plan-completo.md): plan by waves.
- [docs/hardware/](docs/hardware/): research on controllers and machines, with sources.
- [docs/validacion-hardware.md](docs/validacion-hardware.md): checklist for real machine tests.
- [docs/publicar.md](docs/publicar.md): how a version is published.
- [docs/ideas-mejoras.md](docs/ideas-mejoras.md): ideas to investigate.
- [apps/desktop/README.md](apps/desktop/README.md): the desktop application and its shortcuts.
- [SECURITY.md](SECURITY.md), [SUPPORT.md](SUPPORT.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
- [THIRD_PARTY.md](THIRD_PARTY.md): third-party code.

## The engine

The engine is a Rust library organized in small crates that know nothing about the interface; the
application (`apps/desktop`, Tauri 2 and React) and the CLI use the same ones.

| Crate | What it does |
|---|---|
| `tlaku-core` | Job model (points, polylines, layers, cut settings), basic cut order and `Toolpath`, the common representation drivers consume. |
| `tlaku-svg`, `tlaku-dxf` | Import SVG and DXF into polylines in mm, one layer per color, and warn about what they cannot import. |
| `tlaku-lbrn` | Reads LightBurn projects (`.lbrn`, `.lbrn2`) into a neutral model: shapes, groups, text, images and layers. |
| `tlaku-geom` | Booleans, weld, offset, kerf compensation, auto-join, path closing, duplicate removal and tabs; trim, split, node optimization, corner rounding, knife cutting, slots, warp, hull and tangent circle. |
| `tlaku-nest` | Nesting: arranges pieces on the bed or on an irregular material (no-fit polygons, greedy placement and time-limited simulated annealing). |
| `tlaku-plan` | Explainable cut order planner. |
| `tlaku-fill`, `tlaku-raster` | Fill by sweep, by contours or by regions; images with dithering, shape mask and bitmap sweep. |
| `tlaku-trace` | Image tracing. |
| `tlaku-text` | Text to outlines with system fonts, kerning, text on a path, curved and welded; single-line fonts (Hershey included and an SHX reader). |
| `tlaku-codes` | QR, Data Matrix and barcodes as vector shapes. |
| `tlaku-export` | Exports the design to SVG, DXF, PNG and PDF at 1:1 scale. |
| `tlaku-sim` | Time estimation with per-axis acceleration and physical overscan computation. |
| `tlaku-grbl` | G-code by dialect, serial ports, character-counting streaming and pre-flight check. |
| `tlaku-ruida` | Experimental: `.rd` files and network session for Ruida controllers. |
| `tlaku-lhymicro` | Experimental: K40 with a Lihuiyu M2 Nano board (LHYMICRO-GL over USB) with a simulated board. |
| `tlaku-galvo` | Experimental: JCZ/BJJCZ LMC galvo boards over USB: 12-byte codec, compiler with field correction, command allow-list, guaranteed shutdown on errors, framing with the red pointer and simulated board. |
| `tlaku-emu` | Controller emulator (GRBL, Falcon2 Pro, grblHAL, FluidNC, GRBL-LPC, Marlin, Smoothie). |
| `tlaku-camera` | Lens calibration, alignment and height correction. |
| `tlaku-doc` | Types of the contract with the app, exported to TypeScript; the material library (factory and community). |
| `tlaku-pipeline` | Document and machine to `Toolpath`, estimates, bed limits and G-code; PDF and AI, HPGL, G-code and LightBurn importers. |
| `tlaku-cli` | The `tlaku` binary. |

## Command line

`tlaku <command> --help` is the complete reference for each command.

| Command | What it does |
|---|---|
| `gcode` | Converts an SVG, DXF or image (PNG, JPEG, BMP) into G-code for GRBL and prints the job summary (layers, estimated time, occupied area). The engine also recognizes PDF, AI, HPGL, G-code and LightBurn as input; the CLI help does not list them yet. |
| `plan` | Prints the order of layers and cuts with the reason for each step, without generating anything. |
| `send` | Sends a G-code file to the machine with character counting. Ctrl+C cancels with feed hold and soft reset. |
| `frame` | Traces, with the laser off, the rectangle the design occupies (overscan included). |
| `cmd` | Sends individual GRBL commands (`$X`, `$H`, `$I`, G-code...) and shows the full response, with errors and alarms translated. |
| `ports` | Lists the serial ports. |
| `info` | Reads the machine's version (`$I`) and configuration (`$$`), without moving it. |
| `ruida` | Experimental. `export-rd` writes a `.rd` file; `send` sends it over UDP to a Ruida controller (it stores the job, `--run` runs it); `status` reads its state. |
| `k40` | Experimental. `list`, `status` (read-only), `send` (with `--dry-run` and `--emu`) and `export-egv` for a K40 with an M2 Nano board. Power is set on the machine. |
| `galvo` | Experimental. `list`, `status` (read-only), `light` (red pointer, never fires the laser) and `mark` (with `--red-only`, `--dry-run`, `--dump` and `--emu`) for LMCV4 galvo boards. With real hardware it requires `--i-understand-laser-safety`. |

Examples, run from the repository root (they use the test design
`crates/tlaku-cli/tests/fixtures/design.svg`: a red outline and a blue rectangle):

```sh
# SVG -> G-code. Each color of the design is a layer: the red outline is cut in two
# passes at 300 mm/min and the blue rectangle is filled at 3000 mm/min.
target/release/tlaku gcode crates/tlaku-cli/tests/fixtures/design.svg -o job.gcode \
  --layer "#ff0000:speed=300,power=100,passes=2,air" \
  --layer "#0000ff:mode=fill,interval=0.2,speed=3000,power=30"

# Order of layers and shapes, with the reason for each step (writes no files)
target/release/tlaku plan crates/tlaku-cli/tests/fixtures/design.svg \
  --layer "#ff0000:kerf=0.1" --layer "#0000ff:mode=fill,interval=2"

# With the machine connected: find the port, frame it and send the job
target/release/tlaku ports
target/release/tlaku frame crates/tlaku-cli/tests/fixtures/design.svg --port /dev/cu.usbserial-XXXX
target/release/tlaku send job.gcode --port /dev/cu.usbserial-XXXX

# Machine in alarm: read its version (no prompt) and home it (asks for confirmation)
target/release/tlaku cmd --port /dev/cu.usbserial-XXXX '$I' '$H'
```

**Layer order.** So the piece does not come loose before engraving, by default (`--layer-order
auto`) fills and images go first, then lines, and finally cuts: lines with kerf, or with 80 % power or
more and several passes. Within each group the file order is kept. `--layer-order file` respects only
the file order, and `order=N` inside `--layer` fixes the position of a layer (from 1) above the
rest. The summary shows the final order and why.

**Before sending.** `send` and `frame` open the port, query the state (`?`) and only transmit if the
machine is idle (`Idle`). In alarm, hold (`Hold`), door open or running (`Run`) they transmit nothing
and explain what to do; `$X` (unlock) and `$H` (homing) are suggested with the exact `tlaku cmd`
command but never executed. Both ask for confirmation before moving the machine (`--yes` skips it).

**`cmd`.** Each argument is a command, they are sent in order and it stops at the first error or
alarm. Write the `$` in single quotes. Read-only ones (`$I`, `$$`, `$#`, `$G`, `$N`, `?`) are sent
without asking. `$X` (warns that unlocking without homing can leave the position unknown), `$H` (the
machine will move; it waits up to 60 s for it to finish) and everything else (G0/G1/G2/G3, `$J=`,
`M3`/`M4`, `$N=value` settings) ask for confirmation showing exactly what will be sent; with no
answer (stdin closed) they are rejected. `--yes` skips the questions. Real-time by name: `status`,
`hold`, `resume` and `reset`.

Other `gcode` options: `--speed`, `--power`, `--passes`, `--air`, `--mode line|fill`, `--s-max`
(`$30`), `--bed` (work area, `400x415` by default), `--tolerance`, `--no-flip`, `--power-mode
dynamic|constant` (M4 or M3), `--start-gcode` and `--end-gcode`. Images need `--image-size` and
`--image-pos`. `plan` and `frame` accept the same design options.

The optimization settings of the app window are also flags: `--no-inner-first`, `--no-reduce-travel`,
`--no-best-start`, `--no-choose-direction`, `--mix-layers`, `--remove-overlaps[=MM]` and
`--hide-backlash`. The summary and `tlaku plan` explain which ones are changed. Advanced cutting
settings (perforation, lead-ins, sub-layers...) are used from the app; the CLI does not have `--layer`
options for them yet.

Without installing, you can use `cargo run --bin tlaku -- <command>`.

## Tests

```sh
cargo test --workspace
```

Several tests compare their output with golden files (`tests/fixtures` of each crate: `tlaku-grbl`,
`tlaku-fill`, `tlaku-raster`). If a change is intentional, regenerate them with
`UPDATE_GOLDEN=1 cargo test --workspace` and review the diff before committing. More details in
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE). Dependencies must have permissive licenses; `cargo deny check` verifies it (see
[deny.toml](deny.toml)). Adapted third-party code is listed in [THIRD_PARTY.md](THIRD_PARTY.md).
Community material settings (`community/materials/`) are released by their authors under
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).
