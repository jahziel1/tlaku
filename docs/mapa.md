# Mapa del código

Léelo antes de tocar nada. Responde "¿dónde está X?" y "¿dónde agrego Y?". Qué hace cada crate y
las decisiones de diseño están en [arquitectura.md](arquitectura.md) (no se repiten aquí); el
contrato de comandos y eventos, en [app-api/](app-api/README.md) (su tabla dice en qué archivo de
área está cada comando; abre solo ese). Las líneas son aproximadas (2026-10, tras partir archivos en la ola 6); si el código cambió, `grep -n` el nombre.

## 1. Estructura de arriba abajo

Workspace Cargo: `crates/*` y `apps/desktop/src-tauri` (crate `tlaku-desktop`). Otros: `scripts/`
(`run-stress.sh`, `fetch-corpus.sh`), `tests/corpus/` (archivos reales; no leer), `.cargo/config.toml`
(`TS_RS_EXPORT_DIR` de los bindings), `.github/workflows/{ci,release,flatpak}.yml`, `packaging/flatpak/`.

### 1.1 `crates/*` (entrada `src/lib.rs` salvo que se indique)

| Crate | Qué es / módulos clave |
|---|---|
| `tlaku-core` | `Point`, `Polyline`, `Rgb`, `Job`, `Toolpath`. `bezpath.rs`, `optimize.rs`, `toolpath.rs` |
| `tlaku-doc` | Contrato con la app (serde + ts-rs). `document.rs` (`Document`, `DesignObject`), `layer.rs` (`LayerSettings`), `device.rs` (`DeviceProfile`, `DeviceKind`, `GcodeDialect`), `lbdev.rs` (+`lbdev/tests.rs`: `parse_lbdev`, lector de perfiles `.lbdev` de LightBurn, `LbdevImport`/`LbdevNotice`), `job.rs` (`JobOptions`, `JobPreview`), `machine.rs` (`StatusReport`, eventos), `cut.rs`, `galvo.rs`, `materials.rs`, `factory.rs` + `data/materials-default.json`, `community.rs` + `build.rs` (aportes de `community/materials/*.json`, validados por `cargo test -p tlaku-doc community`), `community_catalog.rs` (+`community_catalog/tests.rs`: índice publicado `catalog/v1/index.json`, `build_index`, `validate_index`; ejemplo `examples/community_catalog.rs`), `text.rs`, `virtual_array.rs`, `tests.rs` |
| `tlaku-svg` | `lib.rs` (`import_svg`), `clip.rs` (clip-path/mask) |
| `tlaku-dxf` | `lib.rs`, `aci.rs` (colores), `geom.rs` |
| `tlaku-lbrn` | Lector neutro de proyectos `.lbrn/.lbrn2`: `parse.rs`, `pathdata.rs` |
| `tlaku-geom` | `boolean`, `offset`, `kerf`, `join`, `dedupe`, `tabs`, `fillet`, `trim`, `split`, `slots`, `knife`, `hull`, `tangent`, `warp`, `simplify`, `nesting` (par-impar) |
| `tlaku-plan` | Orden de corte: `planner.rs`, `sched.rs`, `improve.rs`, `contain.rs`, `overlap.rs`, `explain.rs`, `describe.rs`; los textos de las razones y avisos salen de `crates/tlaku-i18n/locales/{es,en}/plan.txt` |
| `tlaku-fill` | Relleno: `scan.rs` (barrido), `offset.rs` (contornos), `flood.rs`, `cluster.rs` |
| `tlaku-raster` | Imagen a potencia: `adjust`, `dither`, `screen` (semitono), `sketch`, `mask`, `resample`, `warp` (giro afín de una imagen), `preview` (vista previa entera o por trozo), `scan.rs` (barrido a bloque); carga sin materializar la imagen: `load.rs` (cabecera, PNG por filas, TIFF por franjas/teselas con YCbCr, `convert_to_png`) y `downsample.rs`; `load_for_process` reduce a la rejilla antes de ajustar |
| `tlaku-sim` | Tiempo/overscan: `profile.rs`, `plan.rs`, `sample.rs` |
| `tlaku-text` | Texto a contornos: `library.rs`, `layout.rs`, `opentype.rs` (funciones OpenType y ejes variables: `font_capabilities`, `selected_features`; `TextSpec.features/variations/uppercase/valign/word_spacing_mm` en `spec.rs`), `linefont.rs` (Hershey en `data/`), `shx.rs`, `effects.rs`, `path_sampler.rs`. Pruebas: `tests/outline.rs` (Bebas Neue) y `tests/opentype.rs` (fuente sintética `tests/fixtures/TlakuTest-Var.ttf`, que genera `make_test_font.py`) |
| `tlaku-trace` | Vectorizar imagen: `binarize`, `polygon`, `spline` (usa `visioncortex` mediante `vtracer/`, la parte de biblioteca de vtracer 0.6.5 adaptada, solo modo binario; ver `THIRD_PARTY.md`); `golden.rs` compara byte a byte con `tests/fixtures/golden_*` |
| `tlaku-codes` | QR y barras: `encode.rs`, `geometry.rs` |
| `tlaku-nest` | Nesting: `engine.rs`, `search.rs`, `prep.rs`, `region.rs`, `convex.rs`, `ops.rs`; varias hojas y retazos: `solve.rs` (preparar piezas y una hoja), `sheets.rs` (`nest_sheets`), `leftover.rs` (sobrante de cada hoja); giros por pieza (`PieceRotation`: veta, sin giro) en `lib.rs`; banco de calidad `examples/nest_bench.rs` |
| `tlaku-camera` | Lente/homografía/marcas sin OpenCV: `calibration.rs`, `marks.rs`, `chessboard`, `circle`, `homography`, `height`, `mosaic`, `undistort` |
| `tlaku-export` | Salida SVG/DXF/PNG/PDF/AI: `svg.rs`, `dxf_out.rs`, `png_out.rs`, `pdf.rs` (`export_pdf`, `export_ai`: PDF 1.5 con una capa por color), `scene.rs` |
| `tlaku-pipeline` | `Document`+`DeviceProfile`+`JobOptions` a `ComputedJob`. `build.rs` (núcleo), `layers.rs`, `place.rs`, `stroke.rs`, `tabs.rs`, `zaxis.rs`, `rotary.rs`, `scan_offset.rs`, `tile.rs` (`plan_grid`), `split.rs` (`galvo_rotary`, `mark_sequence`), `split_shapes.rs` (formas enteras, `doc_mark_sequence`), `slice3d.rs`, `cylinder.rs`, `gcode.rs`, `ruida.rs`, `galvo.rs`, `preview.rs`, `material_test.rs`, `registration.rs` (`fit_with_choice`, imágenes que giran), `processed.rs` (`processed_png`, `processed_preview`); importadores: `import.rs` (despacha por extensión), `import_pdf.rs`, `import_hpgl.rs`, `import_gcode.rs`, `import_rd.rs` (`.rd` de Ruida), `import_egv.rs` (`.egv` del K40), `import_lbrn.rs` + `import_lbrn/{layers,bitmap,text}.rs` |
| `tlaku-parts` | Generadores de piezas (mejora 2 del plan de usuarios; 7 generadores: `box`, `tray`, `hinge`, `fit_test`, `gears`, `joints`, `rounded_box`): `boxes/` (caja con dientes, `panels.rs` con el marco 3D y las aristas, `dividers.rs` divisores con pestañas y media madera, `feet.rs` zócalo y patas), `tray.rs` (bandeja, cajón y caja exterior; asas), `hinge.rs` (bisagras flexibles: rectas, onda, zigzag, ladrillo, rombo; `zone_cuts` sirve también a la caja redondeada), `gears/` (engranajes de evolvente: `involute.rs` perfil y cremallera, `mod.rs` parámetros, agujero, aligeramiento y pares), `joints.rs` (cola de milano plana y pestaña-ranura sueltas), `rounded_box.rs` (caja de esquinas redondeadas: base, tapa y lateral con bisagra en los arcos; `Contour` recorre la línea media), `fit_test.rs` (pieza de prueba de encaje), `fingers.rs` (reparto de dientes), `rectilinear.rs` (rectángulo menos muescas), `font.rs` (rótulos de trazo), `layout.rs` (filas), `registry.rs` (`GENERATORS`, formularios, `generate`) |
| `tlaku-i18n` | Mensajes del motor es/en: `tr!`, idioma activo (`locale.rs`), catálogos `locales/{es,en}/<crate>.txt` (`catalog.rs`), códigos de error (`coded`/`split_code`); guía en su `README.md` (receta 3.12) |
| `tlaku-grbl` | G-code y enlace: `gcode.rs`, `streamer.rs`, `preflight.rs`, `frame.rs`, `serial.rs`, `net.rs` (TCP/Telnet/WebSocket), `link.rs` (`LinkProfile`) |
| `tlaku-ruida` | Experimental: `rd.rs` (archivo), `cmd.rs` (comandos), `packet.rs`, `swizzle.rs`, `link.rs` (UDP), `session.rs`, `safety.rs`, `settings/` (lista cerrada de ajustes escribibles, respaldo `.tlakuruida`, calibración), `emu.rs` |
| `tlaku-lhymicro` | Experimental (K40): `encode.rs`, `packet.rs`, `crc.rs`, `speed.rs`, `status.rs`, `transport.rs`, `usb.rs`, `session.rs`, `emu.rs`, `decode.rs` |
| `tlaku-galvo` | Experimental (JCZ/BJJCZ): `compile.rs`, `cmd.rs`, `codes.rs`, `params.rs`, `calib.rs` (`FieldMap`, ahora con el `.cor`), `cor.rs` (`parse_cor`, `CorGrid`), `safety.rs`, `session.rs`, `transport.rs`, `usb.rs`, `wire.rs`, `emu.rs` (`forbidden_commands`) |
| `tlaku-emu` | Emuladores GRBL/RepRap sin hardware: `machine.rs` (núcleo), `profile.rs` (perfiles), `vm.rs` (`VirtualMachine`), `net.rs`, `fault.rs`, `clock.rs`, `trace.rs`, `parse.rs`, `model.rs` |
| `tlaku-cli` | Binario `tlaku` (`main.rs`): `cli.rs` (args), `options.rs`, `job.rs`, `link.rs` (GRBL), `ruida.rs`, `k40.rs`, `galvo.rs`, `report.rs` |
| `tlaku-bench` | Estrés con archivos reales: `main.rs` (`run`/`gen`/`worker`), `driver.rs`, `synth.rs`, `report.rs`, `summary.rs` |

### 1.2 `apps/desktop/src-tauri/src/` (crate `tlaku-desktop`)

| Archivo / carpeta | Contenido |
|---|---|
| `lib.rs` (160) | `mod` + `generate_handler![...]` (líneas ~38-139): **registro de todos los comandos** |
| `commands.rs` (671) | Comandos base: importar, guardar, `compute_job`, `connect`, jog, `start_job`, `frame_*`; `AppState`; `TauriSink` (línea ~71, convierte `Emit` en evento Tauri) |
| `engine.rs` (+`engine/tests.rs`) | Lógica sin Tauri: motor, archivos, perfiles |
| `machine.rs` (1789) | Conexión GRBL en hilo dedicado (`Handle`, `Connection`, `Worker`, `Emit`, `Sink`) |
| `machine/active.rs` | `ActiveMachine` y `Machine` (`Grbl`/`Ruida`/`Lhy`/`Galvo`): despacho común (jog, pausa, cancelar) |
| `machine/{ruida,lhymicro,galvo}.rs` | Un hilo y un `Worker` por tipo de máquina; cada uno con `tests.rs` en su carpeta |
| `machine/common.rs` | Lo que repiten esos tres: `log` (consola), `virtual_speed` (`virtual:<perfil>@vel`) y `Paced` (transporte con ritmo) |
| `machine/virtual_port.rs` | Puertos `virtual:<perfil>[@vel]` (6 perfiles GRBL/Marlin/Smoothie en `KINDS`) |
| `machine/{extras,frame}.rs` | Ejes extra, encuadre |
| `edit/` | Un archivo por área: `text`, `variables`, `codes`, `geometry`, `shape_ops`, `design`, `nest`, `remnants` (inventario de retazos), `trace`, `art`, `camera`, `materials`, `devices`, `files`, `imaging` (`preview_image`), `printcut` (`fit_registration`, `plan_tiles`) (registro en `edit/mod.rs`) |
| `remote/` | Control remoto por celular (axum): `server.rs`, `hub.rs` (también `QueueInfo` y `Checklist.material`: la cola guiada se ve en el celular y confirmar el siguiente exige la casilla del material), `auth.rs`, `net.rs`, `page.html` (tarjeta «Cola de trabajos»), `tests.rs` |
| `versions/` | Historial de versiones del proyecto (mejora 10): `store.rs` (una carpeta de versiones: gzip, escritura atómica, sin repetir), `retention.rs` (`plan`, política pura), `config.rs` (`versions.json`), `project.rs` (ruta a carpeta, `capture_file`, `snapshot`, `list`, `read_doc`, miniatura SVG), `commands.rs` (`versions_*`), `tests.rs` |
| `id_migration.rs` (+`id_migration/tests.rs`) | Migración al cambiar el id de `org.tlaku.app` a `io.github.jahziel1.tlaku`: `run` (gancho de `setup` en `lib.rs`) copia (no mueve) las carpetas de config, datos y datos locales del id viejo a las nuevas si están vacías (`migrate_dir`, `plan`, `migrate_all`), con marcadores `.migrado-desde-…`/`.migrando-desde-…`, sin pisar nada, con deshacer si falla; no copia `EBWebView` |
| `job_history.rs` | Historial de trabajos (mejora 4): archivo JSONL `job-history.jsonl` en `app_data_dir`; `load`/`append` (reemplaza por `id`)/`update`/`remove`/`clear`/`import`/`export_csv`, rotación por `max` (20 a 20 000), escritura atómica con candado, líneas dañadas a `.dañado`; comandos `job_history_*` (contrato en `app-api/preferencias.md`) |
| `backup/mod.rs` (+`backup/tests.rs`) | Respaldo de ajustes a una carpeta elegida (mejora 12): `BackupSettings`/`BackupConfig` en `backup.json`, `write_backup` (nombre fechado UTC, escritura atómica, rotación de los últimos N), `check_no_secrets`, `newer_backup` (respaldo más nuevo de otra instalación), `list_backups`; comandos `backup_config_get/set`, `backup_run`, `backup_safety`, `backup_list`, `backup_check_newer`, `backup_mark_seen`. `prefs.rs` guarda el paquete `.tlakubundle` (versiones 1 y 2) |
| `automation.rs`, `autosave.rs`, `prefs.rs`, `settings_backup.rs`, `templates.rs`, `probe.rs`, `netlink.rs`, `window.rs`, `live_window.rs` | UDP M54, autoguardado, preferencias y paquetes, plantillas, sondeo de puerto, red, ventanas (`live_window.rs`: la ventana aparte de la vista en vivo) |
| `tauri.conf.json`, `capabilities/{default,live}.json`, `build.rs` | Config y permisos de Tauri. `build.rs` lee `generate_handler!` de `lib.rs` y crea el permiso `allow-<comando>` de cada comando y el conjunto `main-window` (lo incluye `default.json`); `live.json` (ventana aparte `live`) no lo incluye y solo deja escuchar eventos y 2 comandos propios |

### 1.3 `apps/desktop/src/`

| Carpeta | Contenido |
|---|---|
| `api/` | `api.ts` (interfaz `LaserApi`, `COMMAND_NAMES`, `EVENT_NAMES`), `tauri.ts` (`invoke`/`listen`), `types.ts` (`JobArgs`, `EventMap`), `index.ts` (`getApi`, `VITE_API=mock\|tauri`), `mock/` (un archivo por área + `index.ts` que los une, `machine.ts` simula la máquina) |
| `store/` | Zustand: `index.ts` (une slices y conecta eventos `machine://*`), `document.ts`, `selection.ts`, `view.ts`, `machine.ts`, `prefs.ts`, `devices.ts`, `camera.ts`, `galvo.ts`, `preview.ts`, `notices.ts`, `selectors.ts`, `types.ts` |
| `features/<área>/` | `arrange art automation bundle camera canvas codes contextmenu devices feeder fileio help history images layers machine materials nest preview printcut queue recovery remote settings shell simple tabs text tiling trace variables versions`. Cada una: `index.ts(x)`, paneles/diálogos `.tsx`, lógica `.ts`, CSS y `*.test.ts(x)` junto al código |
| `commands/` | `types.ts` (`CommandDef`), `definitions/<área>.ts`, `index.ts` (`ALL_COMMANDS`), `menus.ts` (`MENUS`), `shortcuts.ts`, `registry.ts`, `useShortcuts.ts` |
| `i18n/` | `index.ts` (`t()`; `tn(base, n, params)` para plurales con claves `<base>.one` / `<base>.other`), `es.ts` y `en.ts` (unen las áreas), `es/<área>.ts`, `en/<área>.ts`, `i18n.test.ts` |
| `bindings/` | Generado por ts-rs desde `tlaku-doc`. **No editar ni leer** |
| `ui/` | `Modal`, `NumberField`, `LengthField`; accesibilidad: `Toolbar` (barra con una sola parada de Tab y flechas, sobre `useRoving.ts`), `useReturnFocus` (el foco vuelve a quien abrió un panel no modal) y `stepKey.ts` (`stepFromKey`: ↑/↓ con Mayús y Alt en campos numéricos); `a11y.test.tsx` audita con axe |
| `test/` | `dom.tsx`: utilidades de pruebas con jsdom y axe-core (`mount`, `press`, `click`, `audit`, `describeViolations`, `cleanup`); el archivo de prueba que las use declara `// @vitest-environment jsdom` en su primera línea |
| `lib/` | Puro y sin UI: `layers.ts` (`makeLayer`), `device.ts` (`FALLBACK_DEVICE`, `normalizeDevice`), `units`, `bezpath`, `geometry`, `viewport`, `groups`, `dockLayout`, `warp`, `hull`, `palette`, `platform` (`readPref`/`writePref`) |
| `styles/` | `tokens.css` (variables), `base`, `shell`, `panels`, `canvas`; cada feature trae su CSS |

## 2. Archivos grandes (más de 800 líneas)

Se partirán en la ola 6; si ya no existen o los números no cuadran, buscar con `grep -n`.
Lee con `offset`/`limit`. Cada fila: líneas totales y secciones (`línea:nombre`).

| Archivo (líneas) | Secciones |
|---|---|
| `crates/tlaku-galvo/src/compile.rs` (1416) | 34 `FieldOrigin`, 58 `CompileOptions`, 212 `Builder`, 691 `compile`, 793 `compile_light`, 850 tests |
| `crates/tlaku-pipeline/src/stroke.rs` (1401) | 53 `LeadSpec`, 118 `resolve`, 372 `Stroke`, 685 `push_shape`, 774 `apply_closed`, 849 `ramp`, 922 tests |
| `crates/tlaku-svg/src/lib.rs` (1342) | 60 `import_svg`, 135 `scan_xml`, 197 `build_warnings`, 580 `clip_path`, 662 `curves`, 785 tests |
| `crates/tlaku-grbl/src/gcode.rs` (1313) | 39 `GcodeOptions`, 121 `Writer`, 452 `generate`, 603 tests |
| `crates/tlaku-grbl/src/preflight.rs` (1304) | 95 `validate_gcode`, 310 `check_settings`, 455 `alarm_advice`, 585 `preflight`, 777 `preflight_link`, 889 tests |
| `crates/tlaku-cli/src/galvo.rs` (1683) + `galvo/seq.rs` (marcado dividido, repetido y rotativo por piezas) | 319 `GalvoCommand`, 418 `plan`, 603 `run`, 734 `light`, 903 `mark`, 1094 tests |
| `apps/desktop/src-tauri/src/edit/geometry.rs` (1152) | 63 `GeometryOp`, 202 `run`, 317 `cleanup`, 466 `geometry_op`, 484 tests |
| `crates/tlaku-pipeline/src/import_pdf.rs` (1146) | 36 `import`, 170 `page_geometry`, 213 `ColorSpace`, 437 `Interpreter`, 868 tests |
| `apps/desktop/src-tauri/src/remote/hub.rs` (1137) | 128 `Snapshot`, 224 `ActionError`, 290 `Core` (hasta 665), 746 tests |
| `crates/tlaku-pipeline/src/tile.rs` (1083) | 48 `Grid`, 122 `plan_tiles`, 321-493 recortes `clip_*`, 513 `apply`, 559 tests |
| `crates/tlaku-galvo/src/{codes,safety,session}.rs` (1059/1048/925) | codes: 169 tabla `GP`, 957 `lookup`; safety: 46 `SafetyLimits`, 335 `validate_list`, 546 `check_immediate`; session: 347 `GalvoSession` |
| `apps/desktop/src/features/machine/Galvo*` + `galvo{Profile,Calib,Calib9,Warp,WarpState}.ts` | Perfil galvo: `GalvoFields.tsx` (partes comunes en `GalvoFieldParts.tsx`; rotativo del galvo en `GalvoRotaryFields.tsx`), asistentes de calibración (`GalvoCalibration.tsx` dos cuadrados, `GalvoCalibration9.tsx` nueve puntos con el ajuste de `galvoCalib9.ts`, copia del modelo de `calib.rs` fijada por prueba), vista previa de la deformación de cilindro y cono (`GalvoWarpOverlay.tsx`, portal en `.canvas-area` montado desde `MachineDialogHost`; la fórmula en `galvoWarp.ts` copia a `cylinder.rs`). Textos: `i18n/{es,en}/galvo.ts` |
| `crates/tlaku-grbl/src/net.rs` (996) | 126-213 Telnet, 213 `TcpTransport`, 340 `WebSocketTransport`, 549 `Endpoint` |
| `crates/tlaku-cli/src/link.rs` (996) / `k40.rs` (815) | link: 102 `stream_lines`, 192 `send_job`, 366 `classify`, 619 `run_commands`; k40: 76 `K40Command`, 400 `export_egv`, 435 `send` |
| `apps/desktop/src-tauri/src/edit/shape_ops.rs` (928) | 49 `ShapeOp`, 273 `trim`, 397 `fillet`, 451 `cut_shapes`, 531 `resize`, 593 `shape_op` |
| `crates/tlaku-lhymicro/src/emu.rs` (892) / `tlaku-ruida/src/emu.rs` (837) | lhy: 116 `EmuBoard`; ruida: 112 `Emulator`, 425 `UdpEmulator`, 492 `replay` |
| `crates/tlaku-pipeline/src/material_test.rs` (889) | 40 `GLYPHS` (fuente de rótulos), 326 `apply_param`, 484 `material_test` |
| `crates/tlaku-raster/src/scan.rs` (869) | 8 `RasterSettings`, 90 `raster_to_block`, 196 `rotated_moves`, 276 `row_runs` |
| `crates/tlaku-pipeline/src/import_hpgl.rs` (820) | 24 `import`, 158 `Parser` |
| `apps/desktop/scripts/screenshots.mjs` (899) | 18-190 ayudantes (`shot`, `click`, `evaluate`), 191 fase 3, 423 ola 2, 532 ola 3 (39-47), 678 ola 4 (48-57) |
| `apps/desktop/src/features/machine/machine.css` (1059) | prefijos `.mc-*`; busca la clase con `grep -n` |

### Partidos en la ola 6 (carpetas por tema; ningún archivo pasa de ~600 líneas)

La API pública no cambió: cada `mod.rs` (o el `.rs` padre) reexporta con `pub use`. Busca el
nombre con `grep -rn` dentro de la carpeta.

| Carpeta / archivo | Contenido |
|---|---|
| `crates/tlaku-grbl/src/streamer/mod.rs` (166) | struct `Streamer`, `RunState`, `Command`/`Control`/`Job`, `mod`/`pub use`; submódulos: `realtime.rs` (`Realtime`, `override_plan`), `status.rs` (`MachineState`, `StatusReport` + parseo, `$#`), `firmware.rs` (`Firmware`, textos `error:`/`ALARM:`, fallas Creality), `lines.rs` (`prepare_line`, `StreamError`), `config.rs` (`StreamerConfig`, `ErrorPolicy`), `events.rs` (`Event`, `Outcome`), `run.rs` (`new`, `run`, `spawn`), `tick.rs` (bucle `tick`), `stop.rs` (`safe_stop`, `sequence_stop`, `absorb_line`), `incoming.rs` (`handle_line`, Marlin), `tests.rs` (523) |
| `crates/tlaku-emu/src/machine/mod.rs` (256) | struct `Machine`, `Options`, tipos internos; submódulos: `access.rs` (`new`, consultas), `link.rs` (`receive`), `motion.rs` (tiempo, cinemática, planificador de velocidades con `$11` y ventana `lookahead_blocks`, láser, límites), `pump.rs` (`exec_line`, `exec_gcode`), `dollar.rs` (`$`, jog, homing), `reprap.rs` (Marlin/Smoothie), `realtime.rs` (tiempo real, estado, alarmas, reset), `faults.rs` (fallas inyectadas) |
| `crates/tlaku-pipeline/src/build/mod.rs` (454) | `Builder`/`LineCtx`, `build`, `compute`, `compute_at`; submódulos: `builder.rs` (`push*`), `line.rs` (`line_layer`), `fill.rs` (`fill_layer`, partes), `image.rs`, `helpers.rs`, `plan.rs` (`plan_options`) |
| `crates/tlaku-pipeline/src/tests/mod.rs` (204) | ayudantes (`doc`, `device`, `rect_object`, `three_layers`...) y un archivo por tema: `import`, `layers`, `compute`, `placement`, `bounds`, `images`, `gcode`, `curves`, `tabs`, `zaxis`, `rotary`, `scan_offset`, `power_scale`, `manual` |
| `crates/tlaku-ruida/src/session/` | `mod.rs` (`RuidaSession`, ACK), `config.rs` (`SessionConfig`, `SessionError`), `types.rs` (`ProbeInfo`, `MachineStatus`, `UploadReport`...), `query.rs` (`status`, `controller_info`, `list_documents`), `settings.rs` (leer/escribir ajustes con respaldo y verificación), `docs.rs` (borrar/ejecutar documentos, nombre y reemplazo), `probe.rs`, `send.rs` (`send_job`, `try_stop`), `control.rs` (pausa, home, jog, `frame`), `tests/` |
| `crates/tlaku-ruida/src/cmd/` | `types.rs` (`Command`, `DecodeError`), `encode.rs`, `decode.rs` (`decode_body`, `disassemble`; también `encode_stream`), `display.rs`, `tests.rs` |
| `crates/tlaku-ruida/src/rd/` | `mod.rs` (`build_rd`, swizzle), `options.rs` (`MAX_LAYERS`, `HomeCorner`, `RuidaJobOptions`, `RdError`), `plan.rs` (`plan_layers`), `build.rs` (`build_commands`), `tests/` |
| `crates/tlaku-lhymicro/src/session/` | `mod.rs` (`LhySession`), `config.rs`, `control.rs` (`JobControl`), `types.rs` (errores, `JobReport`), `poll.rs` (`probe`, `status`), `realtime.rs` (`stop`), `packets.rs` (envío y CRC), `job.rs` (`send_job`), `motion.rs` (`home`, jog, `frame`), `tests/` |
| `crates/tlaku-lhymicro/src/encode/` | `mod.rs` (`encode`), `config.rs` (`EncodeConfig`, `HomeCorner`), `errors.rs`, `program.rs` (`Program`), `encoder.rs` (`Encoder`), `tests/` |
| `crates/tlaku-camera/src/calibration/` | `model.rs` (`LensModel`), `options.rs`, `calibrate.rs` (`calibrate_lens`, `calibrate_views`), `select.rs` (`select_model`), `fit.rs`, `intrinsics.rs`, `lm.rs` (`levenberg_marquardt`), `tests.rs`, `fisheye_tests.rs` |
| `crates/tlaku-camera/src/marks/` | `layout.rs` (`MarkLayout`), `report.rs` (`DetectOptions`), `candidates.rs`, `refine.rs`, `assign.rs`, `detect.rs` (`detect_marks`), `tests.rs` |
| `apps/desktop/src-tauri/src/machine.rs` (90) | Doc, `use` comunes, `NOT_CONNECTED`, `mod`/`pub use` de `connection`, `emit`, `handle`, `worker` |
| `apps/desktop/src-tauri/src/machine/emit.rs` (75) | `Emit`, `Sink`, `Timing` |
| `apps/desktop/src-tauri/src/machine/handle.rs` (239) | `Shared`, `Reply`, `RunKind`, `Request`, `Handle` |
| `apps/desktop/src-tauri/src/machine/connection.rs` (237) | `Connection`, `identify`, `probe_reprap`, `parse_m114`, `is_ack` |
| `apps/desktop/src-tauri/src/machine/worker.rs` (277) | `Worker` (`new`, `run`, `next_request`), `Run`, `Phase`, `Fail`, ayudantes |
| `apps/desktop/src-tauri/src/machine/worker/{status,idle,command,jog,start,stream}.rs` (124/152/162/166/104/239) | `on_status`/`pump`; `idle_step`/`poll_reprap`; `realtime`/`command`/`exchange`; `jog`/overrides; `try_start`; `stream_step`/`finish` |
| `apps/desktop/src-tauri/src/machine/worker/limits.rs` (261) + `tests/limits.rs` | Guarda de la cama del movimiento manual (`check_destination`: rechaza `jog`/`jog_to` fuera de la cama tras un homing en espacio positivo; el área viene de `Handle::set_bed`, puesto por `connect_inner`, o de `$130`/`$131`), lectura de `$$` (`note_settings`, aviso `$20=0`) y detección de reinicio de la placa en reposo (`detect_reboot`, `position_unconfirmed` en `StatusReport`; saludo esperado si hubo un `0x18` propio en ~3 s). Mid-job: `incoming.rs` del streamer hace la parada segura y `stream.rs::finish` marca la posición |
| `apps/desktop/src-tauri/src/machine/resume/` + `src/job_resume.rs` | Continuar un trabajo interrumpido: `walker.rs` (`Walker`, `Parsed`, `modal_at`: pasos de movimiento y estado modal del G-code), `store.rs` (`CheckpointStore`: `meta.json`, `progress.json`, `job.gcode`, escritura atómica), `writer.rs` (`CheckpointWriter`: hilo que guarda cada ~1 s), `plan.rs` (`locate`, `landing`, `build_program`, `frame_lines`), `assess.rs` (`assess`: compara la MPos de ahora con la guardada por si la máquina siguió sola con su búfer; `PathMeter`: porcentaje por posición; `locate_forward`), `start.rs` (`readiness`, `current`, `check_safety`, `resume_now`), `mod.rs` (`plan_for_job`, `summarize`, `view`, `prepare`); comandos de Tauri en `job_resume.rs`. El trabajador (`worker/{start,stream}.rs`) abre y cierra el punto de control; pruebas en `machine/tests/resume.rs` y `resume/tests.rs` |
| `apps/desktop/src-tauri/src/machine/tests/mod.rs` (454) + `{connection,jobs,firmware,virtual_machine,reprap,jog,overrides,network}.rs` | Apoyos (`Fake`, `Rig`, `connect_virtual`, `emu`) y pruebas por tema |
| `apps/desktop/src-tauri/src/machine/galvo.rs` (135) + `galvo/{safety,prepare,events,handle,connection,worker}.rs`, `galvo/worker/{light,job}.rs`, pruebas de la ola 6C en `galvo/tests/frame6c.rs` y de la 6E en `galvo/tests/wave6e.rs` | `SafetyAck` (safety), `prepare`/`preview` (prepare), `GalvoEvent` (events), `GalvoHandle`, `GalvoConnection`, `Worker` (`run_light` en light, `run_job` en job) |
| `apps/desktop/src-tauri/src/machine/ruida.rs` (91) + `ruida/{handle,connection,sim,worker,job,backup}.rs`, `ruida/worker/{status,requests,upload,settings}.rs` | `RuidaHandle`, `RuidaConnection`, `VirtualSim`, `Worker`, `build_upload`/`frame_corners` |
| `apps/desktop/src-tauri/src/machine/lhymicro.rs` (107) + `lhymicro/{config,handle,connection,worker}.rs`, `lhymicro/worker/{motion,job}.rs` | `session_config`/`encode_config`/`preview` (config), `LhyHandle`, `LhyConnection`, `Worker` |
| `apps/desktop/src-tauri/src/edit/camera.rs` (68) + `camera/{board,marks,setup,normalize,store,bed_image,circle,network}.rs` | calibración y tablero; marcas; `CameraSetup`, `CameraPrefs`, `CameraPreset` y tipos; `normalize_setup` (+ fuente/ajuste/configuraciones); archivo `load/save_camera_setup`; `compose_bed_image`; círculo; `fetch_camera_frame` (cliente HTTP mínimo, solo red local) |
| `apps/desktop/src-tauri/src/edit/camera/tests/mod.rs` + `{board,setup,marks,height,bed_image,network}.rs` | pruebas por tema (`network.rs`: servidor HTTP local de prueba) |
| `apps/desktop/src/store/machine/resume.ts` + `features/machine/{ResumeNotice,ResumeDialog}.tsx`, `resumeModel.ts`, `features/preview/resumeDraw.ts` | Continuar un trabajo interrumpido: `interrupted`, `checkInterrupted`, `discardInterrupted`; el aviso, el diálogo (vista previa hecho/falta, control del punto, home o confirmación, encuadre, lista) y su lógica pura; `RecoveryHost` lo ofrece al abrir y `onConnection` si se pierde la conexión; `jobLog.ts` marca los interrumpidos |
| `apps/desktop/src/store/machine/index.ts` (4) | Reexporta `reducer`, `selectors` y `slice`; los imports de fuera no cambian |
| `apps/desktop/src/store/machine/reducer.ts` (198) | 7 `JobUiState`, 19 `MachineDialog`, 37 `MachineData`, 66 `MachineEvent`, 112 `reduceMachine`, 194 `jobElapsedSeconds` |
| `apps/desktop/src/store/machine/selectors.ts` (120) | 15 `selectHeadDoc`, 45 `jogBlockedReason`, 59 `positionLaserBlockedReason`, 73 `overrideBlockedReason`, 83 `FrameMark`, 94 `frameMarkOf`, 103 `selectFramed`, 111 `sendBlockedReason` |
| `apps/desktop/src/store/machine/slice.ts` (582) | 27 `MachineSlice`, 126 `pickData`, 149 `commandErrorText`, 201 `createMachineSlice` |
| `apps/desktop/src/features/canvas/useCanvasTools.ts` (479) | Hook: 36 `useEffect`, 83 `cancel`, 120 `onDown`, 176 `update`, 209 `hoverAt`, 258 `onMove`, 286 `onUp`, 362 teclado, 387 menú contextual, 399 doble clic; gestos en `gestureState.ts`, `toolSnapping.ts`, `transformGesture.ts`, `shapeGesture.ts`, `polylineGesture.ts` |
| `apps/desktop/src/features/canvas/canvas2d.ts` (578) | `Canvas2DRenderer` (55-578); funciones de dibujo puras en `overlay2d.ts` (342) |

### Agregado en la ola 6B

| Dónde | Qué |
|---|---|
| `crates/tlaku-plan/src/seq.rs` | `Seq` (`ArraySeq`, `BlockSeq` con inversión perezosa) para 2-opt/Or-opt; `geom.rs` tiene `PolyIndex` y `grid.rs` `items_within`. `testgen.rs` y `equiv.rs` son solo de pruebas (rápido contra lento) |
| `crates/tlaku-raster/src/{load,downsample}.rs` | `load_for_process` (decodifica y reduce a la rejilla de grabado antes de procesar; PNG por filas, TIFF por franjas, YCbCr), `gray::dimensions` (solo cabecera), `convert_to_png` |
| `crates/tlaku-grbl/src/gcode.rs` | Fusión de cortes casi colineales: `GcodeOptions::merge_tolerance_mm` (`DEFAULT_MERGE_TOLERANCE_MM` = 0,0059; 0 la apaga), `compact_toolpath`, `generate_mapped` |
| `crates/tlaku-grbl/src/preflight.rs` + `tlaku-sim` | `LinkLoad`, `preflight_link_with_load`; `tlaku_sim::line_rate`; `tlaku_pipeline::link_load`. `tlaku_sim::estimate_with_times` planifica una sola vez |
| `crates/tlaku-ruida/src/rd/` | Fusión de capas vecinas iguales si hay más de 128 (`plan_summary`, `clamp_speeds`); tope 36 000 mm/min (`safety::MAX_SPEED_MM_MIN`). Avisos de velocidad en `tlaku-pipeline/src/layers.rs` (`speed_warnings`) |
| `src-tauri/src/edit/review.rs` + `src/features/review/` | Revisar diseño: comando `review_design`; `fixes.ts`, `useReview.ts`, `ReviewPanel.tsx`, `ReviewNotice.tsx` (diálogo de inicio), `SimpleReview.tsx` (modo sencillo) |
| `src/api/tauri.contract.test.ts` + `src/api/contract/rustCommands.ts` | Prueba de contrato: compara `generate_handler!`/firmas Rust con lo que manda `tauri.ts` |
| `src-tauri/src/smoke.rs`, `src-tauri/examples/smoke.rs`, `scripts/smoke-tauri.sh` | Prueba de humo con Tauri real (`npm run smoke:tauri`); `lib.rs` separa `build_app`/`app_context` |
| `src/features/canvas/{spatialIndex,objectDiff,layerBitmap}.ts`, `src/store/objectIndex.ts` | Rejilla para clic/hover/rectángulo, cambios incrementales, bitmap de capa y de selección (arrastre sin retrazar), índice id → posición (`translateById`) |

### Agregado en la ola 6C

| Dónde | Qué |
|---|---|
| `src/lib/dockLayout.ts` | `DockLayout` ahora lleva `side`, `toolsSide` y `floating` (paneles flotantes); `floatDockTab`, `dockFloatingTab`, `setFloatRect`, `fitFloatRect`, `shellColumns` |
| `src/features/shell/{dockPanels,FloatingPanels}.tsx` | `TABS`/`PanelBody` (contenido de cada pestaña, acoplada o flotante) y las ventanas flotantes; `AppShell.shellGrid` reparte las columnas según los bordes |
| `src/features/help/jobLog.ts` + `jobLogSnapshot.ts` | **Historial de trabajos** (mejora 4): `useJobLog` (copia en memoria + cola de escrituras hacia `job_history_*`; `load` migra el `localStorage` viejo `tlaku.jobLog` y marca como interrumpidos los «en curso» sin latido), `sanitizeJobLog`, `noteJobEvent`/`noteJobAlarm`/`touchJobLog` (los llama `onJob`/`onStatus` en `store/machine/slice.ts`; al empezar guarda una entrada `running` que el final reemplaza). `jobLogSnapshot.ts`: capas con ajustes, material (por coincidencia con la biblioteca), área/longitud (`jobMetrics`) y miniatura SVG (`buildThumbnail`, usa `cutRuns`/`simplify` de `remote/snapshot.ts`) de la pestaña de origen. `costs/costMetrics.ts` lee `useJobLog().entries` para calibrar tiempos |
| `src/features/help/JobLogDialog.tsx` + `JobLogDetail.tsx`, `jobLogFilter.ts`, `jobLogCsv.ts`, `jobLogRepeat.ts`, `jobLogFormat.ts` | Diálogo Historial (comando `help.jobLog`): filtros (`filterJobs`: texto sin acentos, máquina, material, resultado, fechas), detalle con capas y notas, **Repetir con estos ajustes** (`planRepeat`/`applyRepeat`/`repeatWithSettings`: confirma, un paso de deshacer, no envía nada), CSV (`jobsCsv`, `saveJobsCsv` → `job_history_export`). Preferencia `jobHistoryMax` |
| `src/features/queue/` | **Cola guiada de trabajos** (mejora 4). `queueModel.ts` (puro: `begin`, `jobStarted`, `jobEnded`, `pause`, `skipItem`, `moveItem`...; **solo `jobStarted` lleva a `running` y `jobEnded` nunca**), `queueStore.ts` (zustand), `queue.ts` (orquestación: `confirmStep` solo abre `requestStart`, `startQueueWatcher` observa `isJobBusy`/`jobState`/`jobOrigin`, `ensureItemTab`, `estimateOne`, persistencia en `queuePersist.ts`), `queueRemote.ts` (`buildQueueInfo`, `queueBlocksRemoteStart` para el celular), `QueueDialog.tsx` (gestión), `QueueStepDialog.tsx` (confirmación entre trabajos), `QueueHost.tsx` (montado en `AppShell`; píldora y `QueueLiveLine` en la vista en vivo), comandos `queue.open`/`queue.addTab` (`commands/definitions/queue.ts`), textos `i18n/{es,en}/queue.ts`. Pruebas: `queueModel.test.ts`, `queue.test.tsx` (con la máquina simulada), `pageQueue.test.ts` (la página del celular); verificación en Chrome: `scripts/queue-verify.mjs` |
| `src/lib/importOptions.ts`, `features/fileio/remember.ts` | Opciones de importación recordadas (pref `importOptions`) y DPI por omisión (`importDpi`, `effectiveImportDpi`) |
| `src/features/settings/persist.ts` | `startPrefsWindowSync`: las preferencias se comparten entre ventanas (evento `storage`) |

Pruebas de más de 800 líneas (no leer enteras; busca el nombre con `grep -n "fn "`):
`crates/tlaku-pipeline/tests/{orden,galvo}.rs`, `crates/tlaku-plan/tests/plan.rs`,
`crates/tlaku-cli/tests/cli.rs`, `crates/tlaku-emu/tests/{reprap,behavior,estimate}.rs`,
`crates/tlaku-grbl/tests/streamer.rs`, `crates/tlaku-text/tests/outline.rs`, `crates/tlaku-text/tests/opentype.rs`, `crates/tlaku-geom/src/tests.rs`,
`crates/tlaku-dxf/src/tests.rs`, `crates/tlaku-export/src/tests.rs`,
`apps/desktop/src-tauri/src/machine/{tests/,ruida/tests,lhymicro/tests,galvo/tests}.rs`, `remote/tests.rs`.

### Agregado en la ola 6D (publicación)

| Dónde | Qué |
|---|---|
| `.github/ISSUE_TEMPLATE/*.yml` (formularios es/en), `PULL_REQUEST_TEMPLATE.md`, `CODEOWNERS`, `dependabot.yml`, `labels.yml` (+ `scripts/sync-labels.sh`), `workflows/community.yml` | Comunidad: los ids de los formularios son los que prellena `features/help/githubForms.ts` (lo comprueba `githubForms.test.ts`); `community/materials/` y `scripts/community/issue-to-preset.mjs` convierten un issue de ajustes de material en un aporte |
| `src/features/help/{githubForms.ts,openExternal.ts,ShareDialogs.tsx}` | Menú Ayuda > «Reportar un problema…», «Compartir resultados de mi máquina…» y «Compartir ajustes de material…»: arman la dirección del formulario de GitHub y abren el navegador con `tauri-plugin-opener` (alcance en `capabilities/default.json`); nunca envían nada solos |
| `.github/workflows/{ci,release,pages}.yml`, `.github/FUNDING.yml` | CI (Rust en 3 sistemas, Rust y humo en Linux ARM64 `ubuntu-22.04-arm`, deny, interfaz, humo en Linux, sitio), publicación de instaladores (incluye Linux ARM64; ver `docs/publicar.md`), GitHub Pages (inactivo hasta `PAGES_ENABLED=true`) y botón Sponsor |
| `site/`, `scripts/build-site.mjs` | Sitio estático es/en (sección «Apoya Tlaku» y botón flotante en `site/support.js`, sin terceros; un enlace con marcador vacío se quita al construir); `site/config.json` llena `{{REPO_URL}}`, `{{SPONSORS_URL}}`, `{{KOFI_URL}}` (también en el README con `--readme`) |
| `scripts/configure-updater.sh`, `src-tauri/src/updater.rs` | Llena `plugins.updater` y activa `createUpdaterArtifacts`; el plugin solo se registra si hay `pubkey` y `endpoints` **y no hay sandbox de Flatpak** (`in_flatpak()`: `FLATPAK_ID` o `/.flatpak-info`; en Flatpak `lib.rs::build_app` inyecta `window.__TLAKU_SANDBOX__`, que lee `updatesManagedByStore` en `updater.ts`) |
| `packaging/flatpak/` (`io.github.jahziel1.tlaku.{yml,metainfo.xml,desktop,svg}`), `.github/workflows/flatpak.yml` | Paquete Flatpak (mejora 11b): manifiesto que reempaqueta el `.deb` de la Release (runtime `org.gnome.Platform//50`), AppStream, `.desktop`, ícono; `flatpak.yml` (llamado desde `release.yml`, o a mano con una etiqueta) construye x86_64 y aarch64 y adjunta los `.flatpak`. Permisos, estrategia B (desde fuentes) y pasos de Flathub en `docs/publicar.md#flatpak-y-flathub`. Sin probar en Linux real |
| `src/features/help/{updater.ts,links.ts}`, `UpdateDialog`/`DonateDialog` en `HelpDialogs.tsx` | Buscar actualizaciones (store `useUpdateUi`, pref `updateCheckOnStart`) y enlaces de donación (`PROJECT_LINKS`, vacíos por omisión) |

### Agregado en la ola 6E (máquina y periféricos)

| Dónde | Qué |
|---|---|
| `crates/tlaku-doc/src/layer.rs` (`u_offset`), `tlaku-grbl/src/gcode.rs` (`GcodeOptions::block_u_offset`), `tlaku-pipeline/src/{gcode.rs,job.rs,build/}` (`LayerReport::u_offset_mm`) | Desplazamiento de U por capa (M32): se emite solo la diferencia entre capas y se deshace el total al final. UI: campo «Desplazamiento U» en `features/layers/LayerFields.tsx` |
| `crates/tlaku-emu/src/probe.rs` + `parse.rs`/`machine/{pump,dollar,realtime}.rs` | Sonda de GRBL: `G38.2` a `G38.5`, `ProbeSurface` (plano, disco, caja; `Builder::probe_surface`), `[PRB:...]`, `Pn:P`, `ALARM:4/5`. Pruebas en `tests/probe.rs` |
| `src-tauri/src/machine/probe.rs`, `machine/worker/probe.rs`, comando `probe_touch` (`commands.rs`) | Un toque de sonda con límites de seguridad (100 mm, 500 mm/min, `G38.3`, láser apagado, `confirmed`). Las máquinas virtuales GRBL traen un disco de prueba en X100 Y100 (`virtual_port.rs`, `PROBE_DISC`) |
| `src/features/machine/{probeCenter.ts,ProbeCenterDialog.tsx}` | Buscador de centro con el cabezal (M48): modo sonda y modo puntero; reutiliza `camera/center.ts` y las acciones de `camera/centerFinder.ts`. Comando `machine.probeCenter` |
| `src/features/machine/Ruida{Section,Notice,SettingsDialog,Diff,DocDialog}.tsx` + `ruida{Settings,Start,BackupIO,Ports}.ts` | Panel de Ruida (EXPERIMENTAL): información, ajustes de la controladora (leer, editar, revisar con casilla de confirmación, escribir, restaurar, calibrar ejes, rotativo), respaldos `.tlakuruida`, borrar/ejecutar documentos (la ejecución usa la lista de comprobación de `checklist.ts`) y el nombre del documento en el diálogo Iniciar (`useRuidaStartGate`). Mock en `api/mock/ruida.ts`. Textos `i18n/{es,en}/ruida.ts`. GRBL (M25, ola 6E): `GrblAxisCalibration.tsx`, `grblCalibrate.ts` y el respaldo automático en `MachineSettingsDialog.tsx` |
| `src/features/machine/Ruida{Section,Notice,SettingsDialog,Diff,DocDialog}.tsx` + `ruida{Settings,Start,BackupIO,Ports}.ts` | Panel de Ruida (EXPERIMENTAL): información, ajustes de la controladora (leer, editar, revisar con casilla de confirmación, escribir, restaurar, calibrar ejes, rotativo), respaldos `.tlakuruida`, borrar/ejecutar documentos (la ejecución usa la lista de comprobación de `checklist.ts`) y el nombre del documento en el diálogo Iniciar (`useRuidaStartGate`). Mock en `api/mock/ruida.ts`. Textos `i18n/{es,en}/ruida.ts`. GRBL (M25, ola 6E): `GrblAxisCalibration.tsx`, `grblCalibrate.ts` y el respaldo automático en `MachineSettingsDialog.tsx` |
| `src/features/tiling/{guide.ts,MoveMaterialDialog.tsx}` | Tiling en varias columnas (M47): pausa guiada y marcas de registro; el alimentador avanza solo entre tramos de la misma columna (`run.ts`, `advanceFrom`) |
| `src/store/prefs.ts` (`cameraReopen`, `cameraFrameWidth`), `features/settings/SettingsDialog.tsx` (`CameraGroup`) | Preferencias de cámara (M51) |
| `crates/tlaku-doc/src/lbdev.rs`, `src-tauri/src/edit/devices.rs` (`read_lbdev`), `src/features/machine/{lbdevImport.ts,LbdevImportDialog.tsx}`, mock `api/mock/devices.ts` (`mockReadLbdev`), textos `devices.lbdev.*` | Importar perfil de LightBurn (`.lbdev`, M3): el motor lee y propone perfiles (nada se guarda ni se envía a la máquina) y el diálogo del gestor de dispositivos muestra resumen y avisos antes de guardar con `save_device`. Mapeo de claves y deducción del origen en la cabecera de `lbdev.rs` |

### Agregado: costos y precio por pieza (mejora 3 del plan de usuarios)

| Dónde | Qué |
|---|---|
| `src/features/costs/costModel.ts` | Fórmulas puras (`computeCost`, `priceBeforeRounding`, `roundMoney`, `roundUpTo`); la cabecera documenta costo y precio: la comisión y los impuestos se cobran sobre el precio final (`C(1+m)/(1−c−t)` o `C/(1−c−t−m)`), el precio se redondea siempre hacia arriba |
| `src/features/costs/costMetrics.ts` | Del diseño y la vista previa: tiempo, tiempo con láser encendido (`getModel().distances`), longitud de corte, caja envolvente y área real (`evenOddArea`), piezas; `calibrationFactor`/`resolveTime` con el registro de trabajos (`features/help/jobLog.ts`) |
| `src/features/costs/{currency,costProfiles,costStore}.ts` | Moneda y formato regional (`defaultCurrency`, `formatMoney`), saneado y persistencia en `localStorage` (`tlaku.costs`: valores por máquina, perfiles de material con nombre, venta), store de zustand con `openDialog` |
| `src/features/costs/{CostDialog,CostInputsPanel,CostResultPanel,useCostCalc}.tsx`, `costs.css` | Diálogo «Costos y precio» (se monta en `AppShell`); botones en `machine/JobSection.tsx` y `preview/PreviewBar.tsx`; comando `costs.open` en `commands/definitions/costs.ts` |
| `src/features/costs/{costReport,costFiles}.ts` | Desglose en texto y CSV (`reportText`, `reportCsv`, `csvField` neutraliza fórmulas) y guardado (`write_settings_backup` acepta `.csv`) |
| `src/i18n/{es,en}/costs.ts`; manual `docs/manual/07c-costos-y-precio.md` (+ `en/`) | Textos (prefijo `costs.`) y página del manual |

### Agregado: biblioteca comunitaria de materiales (mejora 5 del plan de usuarios)

| Dónde | Qué |
|---|---|
| `crates/tlaku-doc/src/community_catalog.rs` (+`community_catalog/tests.rs`), `examples/community_catalog.rs` | Contrato del catálogo publicado: `CatalogIndex`/`CatalogEntry`/`CatalogState`/`CatalogSource` (ts-rs), `CatalogOperation::of(role, mode)`, `build_index`/`build_index_from_dir` (desde `community/materials/`), `validate_index` (límites `MAX_INDEX_BYTES`, `MAX_ENTRIES`; reglas por entrada = `community::check_entry` más coherencia de `operation`/`verified_by`). `community.rs` agrega `date`, `tlaku_version`, `machine_model` y `Confirmation` al aporte |
| `src-tauri/src/community_catalog.rs` (+`community_catalog/tests.rs`) | **Única red de esta área:** comandos `community_catalog_cached` (sin red) y `community_catalog_refresh` (GET a un destino fijo del sitio, https, sin cookies, tope de tamaño al vuelo, solo mismo sitio en redirecciones, `If-None-Match`); caché `community-catalog.json` (carpeta de caché, se revalida al leer). Pruebas con un servidor local |
| `src/features/materials/{communityModel,communityStore,communityActions}.ts` | Lógica pura (`myMachine`, `catalogModelOf`, `compatOf` = mismo modelo/clase/tipo/otro, `filterEntries`, `presetFromEntry`), estado (`useCommunity`) y acciones (`openCommunity` = única vía de descarga, `copyEntryToLibrary`, `applyEntryToLayer` con `askChoice`, `shareFromActiveLayer`) |
| `src/features/materials/{CommunityBrowser.tsx,community.css}` | Diálogo «Explorar la comunidad…» (botón en `MaterialsPanel.tsx`, montado en `MaterialsDialogHost`); textos `materials.community.*` en `i18n/{es,en}/materials.ts`. Pruebas: `communityModel.test.ts`, `communityActions.test.ts`, `CommunityBrowser.test.tsx` |
| `src/features/help/{githubForms.ts,ShareDialogs.tsx}` | «Compartir este ajuste…»: `tlaku_version` en el formulario y `contributionDraft` (borrador JSON sin datos personales); prueba `shareMaterial.test.tsx` |
| `src/api/mock/communityCatalog.ts` | Catálogo simulado (ejemplos inventados, marcados); `?community=offline\|empty\|invalid` en la dirección del modo mock muestra los demás estados |
| `.github/workflows/{pages,ci}.yml`, `scripts/community/issue-to-preset.mjs`, `.github/ISSUE_TEMPLATE/material_settings.yml` | El flujo del sitio compila el catálogo con el ejemplo de Rust; el script agrega fecha, versión y `--machine-model`; el formulario trae «Versión de Tlaku». Proceso de revisión y confirmaciones en `community/materials/README*.md` y `CONTRIBUTING*.md`; verificación en Chrome: `scripts/community-verify.mjs` |

### Agregado: datos variables desde CSV (mejora 14)

| Dónde | Qué |
|---|---|
| `crates/tlaku-doc/src/variables.rs` (+ campos `VariableText.sources`, `ImageObject.variable`, `VectorObject.code`) | `DataSource` (nombre, ruta, `ShortRows`), `VariableImage`/`ImageFit`, `VariableCode`; `VariableText::normalize` (migra el `csv_path` único, nombres únicos); `relativize`/`absolutize` y `Document::{relativize,absolutize}_data_paths` (rutas relativas al `.tlaku`). Pruebas en el mismo archivo |
| `src-tauri/src/engine.rs` (`save_project`/`load_project`), `edit/variables.rs` (`probe_images`) | Rutas relativas al guardar y absolutas al abrir; comando que dice si existe cada imagen variable y su tamaño |
| `src/features/variables/{syntax,resolve}.ts` | `{csv.fuente:Columna}` y `{fila.fuente}`; `VarContext.sources` (la principal manda; las demás se repiten o bloquean) |
| `src/features/variables/{sources.ts,SourcesPanel.tsx,settings.ts,csvSync.ts,context.ts}` | Lista de fuentes (agregar, renombrar con reescritura de plantillas, hacer principal, quitar), lectura de cada CSV (`useVariablesUi.csv` = principal, `.extra` = las demás por nombre) |
| `src/features/variables/{varObjects.ts,imageVar.ts,codeVar.ts,jobResolve.ts}` | Plantillas de imágenes y códigos variables (`objectTemplates`, `shiftObject`), encaje en la caja (`fitImageObject`), código regenerado centrado (`placeCodeResult`) y la resolución al enviar |
| `src/features/variables/canvasDisplay.ts` (llamado desde `canvas/CanvasView.tsx` → `buildScene`) | Copia de presentación para el lienzo: imágenes y códigos variables con la fila actual (`canvasDisplayObjects`), sin tocar el documento; caché por ruta/contenido (sondeo y generación una sola vez), memo por objeto y aviso `onCanvasDisplayChange`; `scene.selected` sigue siendo el objeto real. Pruebas: `variables/canvasDisplay.test.ts` |
| `src/features/variables/{check.ts,DataCheck.tsx}` | «Revisar datos»: filas con celdas vacías, imágenes ausentes o códigos sin contenido |
| `src/features/variables/{ImageVariableField.tsx,ColumnPicker.tsx}` (usado desde `canvas/ShapeProps.tsx`), `features/codes/CodesDialog.tsx` | Marcar una imagen como variable; contenido variable en el diálogo de códigos |
| Pruebas: `variables/{multiSource.test.ts,sourcesUi.test.tsx}`, `codes/codesVariable.test.tsx` | Varias fuentes, filas distintas, imágenes ausentes, códigos, documentos viejos, vista previa por fila |

### Agregado: historial de versiones del proyecto (mejora 10)

| Dónde | Qué |
|---|---|
| `src-tauri/src/versions/` (+ comandos `versions_*` en `lib.rs`) | Motor: por proyecto (ruta absoluta) una carpeta con `<id>.tlaku.gz`, `<id>.thumb.svg.gz` y `<id>.meta.json` (el meta va al final y define que la versión existe). Por omisión en `<datos de la app>/versiones/`; opcional `.tlaku-versions` junto al archivo (`VersionsConfig.location`; listar, leer y borrar miran los dos). Retención en `retention.rs::plan` (pura, con reloj inyectado). Contrato: [app-api/versiones.md](app-api/versiones.md) |
| `src/features/versions/{VersionsDialog,VersionDetail,VersionsSettings,SaveVersionDialog,Thumb}.tsx`, `versions.css` | Diálogo «Historial de versiones» (lista con miniatura, vista previa, comparación, restaurar, borrar, ajustes) y «Guardar versión…»; `VersionsHost.tsx` los monta en `AppShell` (antes de `PromptHost`, para que las confirmaciones queden encima) y arranca las instantáneas |
| `src/features/versions/{actions,autoSnapshot,diff,diffText,useVersions,versionsUi,types}.ts` | `captureBeforeSave` (lo llama `writeTab` de `features/history/files.ts`; nunca lanza), `restoreAsNewTab`, `restoreInPlace` (confirma, guarda el estado como `restore`, un paso de deshacer con `applyDocChange` y `runInTab`), `AutoSnapshots` (por pestaña con archivo y cambios; sin archivo lo cubre `features/recovery`), `diffDocuments` (objetos por id, capas por color) |
| `src/commands/definitions/versions.ts`, menú Archivo (`menus.ts`) | `versions.open`, `versions.saveNamed` |
| `src/api/mock/versions.ts`, `scripts/versions-verify.mjs [--shots DIR]` | API simulada (dedupe, manual nombra a la última) y verificación en Chrome con capturas |
| Pruebas | Rust: `versions/tests.rs`; interfaz: `features/versions/{versions.test.ts,VersionsDialog.test.tsx}` |
| Manual | `docs/manual/03h-historial-de-versiones.md` (+ `en/`) |

### Agregado: nesting con varias hojas, retazos y veta (mejora 7 del plan de usuarios)

| Dónde | Qué |
|---|---|
| `crates/tlaku-nest/src/lib.rs` (`PieceRotation`: `Inherit`/`Fixed`/`Only(ángulos)`, `PieceRotation::grain()` = 0° y 180°) | Giros por pieza: la veta y «sin giro». `solve.rs::prepare` arma la tabla de ángulos de cada pieza; `engine.rs` y `search.rs` solo mutan/prueban los que ella admite |
| `crates/tlaku-nest/src/sheets.rs` (`nest_sheets`, `SheetsInput`, `SheetsOutcome`) | Varias hojas: lista de hojas (retazos) y hoja nueva `refill`, tope `max_sheets`; cada hoja es un `nest` independiente con su parte del tiempo; la que no admite nada se salta |
| `crates/tlaku-nest/src/leftover.rs` (`leftover`, `sheet_leftovers`) | Sobrante = hoja menos las piezas con margen, abierto (ancho mínimo) y partido en retazos conexos |
| `crates/tlaku-nest/examples/nest_bench.rs` | Banco de calidad y tiempo (instancias ESICUP en formato jagua-rs con `--dir`, más casos propios con piezas de `tlaku-parts`); `--evals N` lo hace reproducible |
| `src-tauri/src/edit/nest.rs` | `NestRequest.{max_sheets,remnants,leftover_min_mm}`, `NestPieceInput.rotation`; `NestStatus.sheets` y `NestPlacementOut.sheet` |
| `src-tauri/src/edit/remnants.rs` | Inventario local `remnants.json` (`load_remnants`, `save_remnants`; valida ids, nombres, medidas y contornos) |
| `src/features/nest/{remnants.ts,remnantStore.ts}` | Tipos y funciones puras del inventario (`remnantFromLeftover`, `matchingRemnants`) y su estado con guardado encadenado |
| `src/features/nest/{sheets.ts,apply.ts}` | Hojas en fila en el lienzo (`sheetOffsets`), contornos `hoja-N` en la capa de guías (`sheetReferences`) y `applyNest(..., layout)` |
| `src/features/nest/{SheetsPanels.tsx,RemnantsDialog.tsx,NestDialog.tsx,NestPreview.tsx}` | Sección «Hojas y retazos», resumen por hoja con «Guardar el sobrante», inventario (comando `arrange.remnants`), selector de giros por pieza y vista previa en fila |
| `src/api/mock/{nest.ts,remnants.ts}`, `scripts/nest-verify.mjs [--shots DIR]` | Mock con hojas, retazos y veta; verificación visual en Chrome headless |

### Agregado: vista previa «por tonos» (mejora 8 del plan de usuarios)

| Dónde | Qué |
|---|---|
| `crates/tlaku-doc/src/tonal.rs` + `data/tone-presets.json` | `ToneCurve` (`base`, `burn`, `dose_char`, `gamma`; `darkness`, `color`, `mix`, `validate`), `tone_dose`, `tone_preset(_ids)` (9 materiales de partida con `custom`), `ToneSample`, `ToneCalibration` (se guarda en `MaterialLibrary.tones`; `check_library` de `edit/materials.rs` las valida). Fórmula en la cabecera del archivo. Pruebas en el mismo archivo |
| `crates/tlaku-raster/src/tonal.rs` | `DoseMap`: rejilla de energía (recorrido exacto de la rejilla por segmento con recorte, `add_point`, `spread` = ancho del punto con promedio de caja, `to_rgba`) y `rgba_to_png`. No sabe de capas ni de materiales |
| `crates/tlaku-pipeline/src/tonal.rs` (+`tests/tonal.rs`) | `render_tonal(&ComputedJob, &DeviceProfile, &TonalOptions)` → `TonalImage`: recorre la trayectoria (cada movimiento con láser deposita potencia/velocidad), M3 (`device.power_mode`) sube la dosis en los extremos de cada cadena de cortes que sale del reposo (`deposit_chain_m3`, aceleración del perfil), hoja = caja de lo grabado + 3 mm, zona (`region_mm`) para el zoom. La medición de rendimiento es `large_image_job_renders_fast` (`--ignored --nocapture`, en release) |
| `src-tauri/src/edit/tonal.rs` | Comando `tonal_preview`: calcula el trabajo (como `compute_job`) y guarda el último (`LAST_JOB`, por hash de `doc+device+options+head`) para que acercar o cambiar de material no lo recalcule; devuelve el PNG en base64 |
| `src/features/tonal/curve.ts`, `calibration.ts` | Copia en TypeScript de la curva (fijada por `curve.test.ts` con los vectores de Rust), `fitCurve` (mínimos cuadrados de `ln(-ln(1-d)) = γ(ln D - ln D₀)`), `curveFor`, `withCalibration`; cuadrícula de la prueba de materiales (`gridCells`, dosis por celda, `samplesOf`, `marksFromSamples`) |
| `src/features/tonal/{plan,service,layers,store}.ts` | `planDetail` (qué zona pedir al acercar), `startTonalService`/`useTonalSync` (pide la hoja entera y, tras una pausa, el detalle; números de petición, descarta lo tardío), `layers.ts` (las dos imágenes y `drawTonal`, que llama `canvas/previewLayer.ts`; `onTonalChange` repinta el lienzo desde `CanvasView`), `store.ts` (modo, material y ancho del punto; `readPref`/`writePref` `preview.tonal.*`) |
| `src/features/tonal/{TonalControls,CalibrateDialog}.tsx`, `tonal.css` | Fila «Normal / Por tonos» en `preview/PreviewBar.tsx` y diálogo de calibración. Textos `preview.tonal.*` en `i18n/{es,en}/preview.ts` |
| `src/api/mock/tonal.ts` (+`mockComputeJob(..., collect)` en `mock/job.ts`, `simulatedDot` en `mock/images.ts`) | Versión reducida de la simulación para el modo mock (sin M3). `scripts/tonal-verify.mjs [--shots DIR]`: verificación en Chrome con prueba 5×5, foto y diseño mixto |
| Pruebas | `features/tonal/{curve,calibration,plan,service}.test.ts`, `tonalUi.test.tsx`; Rust: `tonal::tests` en los tres crates y `edit::tonal::tests` |
| Manual | `docs/manual/07-vista-previa.md` («Vista por tonos») y `11-materiales.md` («Calibrar el tono con la prueba») + `en/`; captura 74 en `screenshots.mjs` |

## 3. Recetas: "dónde agrego..."


Después de cualquiera: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`,
`npx tsc --noEmit && npx eslint .` y la prueba de tu área (sección 4).

### 3.1 Un comando de Tauri
1. Función `#[tauri::command]` en `src-tauri/src/edit/<área>.rs` (área nueva: `pub mod` en `edit/mod.rs`) o en `commands.rs` si es de conexión/trabajo. Pesados: `async` + `spawn_blocking`.
   **Si `api.ts` manda los argumentos en snake_case** (`power_pct`, `max_edge`): `#[tauri::command(rename_all = "snake_case")]` (ej. `frame_hull` en `commands.rs:~444`, `hull_outline` en `edit/design.rs`). Sin él Tauri espera camelCase y falla en la app real; el mock no lo detecta.
2. Registro en `src-tauri/src/lib.rs`, dentro de `generate_handler![...]` (`commands::x` o `edit::<área>::x`).
3. `src/api/api.ts`: método en `interface LaserApi` **y** entrada en `COMMAND_NAMES` (`miComando: 'mi_comando'`; está tipado con `satisfies`, el compilador avisa si falta).
4. `src/api/tauri.ts`: `miComando: (a) => call('miComando', a),`.
5. Mock: `src/api/mock/<área>.ts` (o crear `mockXxx(lag)` y esparcirlo con `...mockXxx(lag)` en `mock/index.ts:~202-220`).
6. Tipos nuevos de entrada/salida: contrato compartido en `tlaku-doc` (3.6); si solo los usa la UI, en `api/types.ts` o en `features/<área>/types.ts`.
7. `docs/app-api/<área>.md`: fila/sección del comando, y su fila en la tabla de `docs/app-api/README.md`.
8. Prueba: Rust en el mismo archivo (`mod tests`) y `api/mock/*.test.ts` o la de la feature.
9. **Contrato (automático):** `src/api/tauri.contract.test.ts` lee `lib.rs` y las firmas Rust (`src/api/contract/rustCommands.ts`), saca de `LaserApi` las claves que manda la interfaz y llama a `createTauriApi()` con `invoke` interceptado. Falla si un comando falta en `lib.rs` o en `COMMAND_NAMES`, si una clave no es la que Tauri espera (camelCase, o snake_case con `rename_all`) o si un argumento obligatorio de Rust no llega. Si no reconoce una firma, falla en voz alta: ajusta el lector, no la prueba.
10. **Humo de la app real:** `apps/desktop/scripts/smoke-tauri.sh` (o `npm run smoke:tauri`) arranca Tauri con una ventana oculta y llama por `invoke` a los comandos de solo lectura (`src-tauri/src/smoke.rs`). Si agregas un comando de solo lectura sin argumentos de entorno, añádelo a `calls()`.

### 3.2 Un evento de máquina (`machine://*`)
1. Carga serializable: tipo en `tlaku-doc/src/machine.rs` (derive `TS`, ver 3.6) o, si es de un solo driver, en su módulo (como `GalvoEvent` en `machine/galvo/events.rs`).
2. `src-tauri/src/machine/emit.rs`: variante en `enum Emit` **y** su nombre en `Emit::name()`; luego `TauriSink::emit` en `commands.rs:~71-79` (un `match` exhaustivo).
3. Emitirlo desde el `Worker` del driver (`self.sink.emit(Emit::...)`; `machine/{ruida,lhymicro,galvo}.rs`).
4. Frontend: `src/api/types.ts`, entrada en `EventMap` (~289; el tipo `GalvoEvent` vive allí). Si es del grupo común, agregar el nombre a `EVENT_NAMES` en `api/api.ts` (`mock.test.ts` cuenta 10) .
5. Suscribirlo en `src/store/index.ts:~62-67` (`api.on('machine://x', (p) => s().onX(p))`) y la acción `onX` en el slice (`store/machine/slice.ts`: `MachineSlice` ~27 + `createMachineSlice` ~201, o `store/galvo.ts`); si cambia el estado, el reductor es `reduceMachine` (`store/machine/reducer.ts:~112`).
6. Mock: `src/api/mock/machine.ts` (`this.emit('machine://x', payload)`).
7. `docs/app-api/conexion-y-trabajo.md`, tabla "Eventos (`listen`)".

### 3.3 Una preferencia
1. `src/store/prefs.ts`: campo en `interface Prefs` y entrada en `PREF_SPECS` (`bool`/`number`/`choice`/`json`, con valor de fábrica y límites). De ahí salen el saneado, `DEFAULT_PREFS`, `usePref`/`getPref` y el guardado en `preferences.json`.
2. Control en `src/features/settings/SettingsDialog.tsx` con `PrefCheck` / `PrefNumber` / `PrefLength` / `PrefSelect` (`fields.tsx`) dentro de su sección (`SETTINGS_SECTIONS` en `uiStore.ts`).
3. Textos `prefs.<clave>` en `i18n/es/prefs.ts` y `i18n/en/prefs.ts` (todas las claves llevan prefijo `prefs.`).
4. Paquete `.tlakuprefs`: entra solo (`PREF_KEYS`). Si un archivo ajeno **no** debe poder encenderla (abre el láser a la red, ejecuta algo), agrégala a `NEVER_IMPORTED` en `features/settings/bundle.ts:21` (hoy `remoteEnabled`).
5. Si el motor necesita el valor (como `automationPort`): pásalo con un comando existente (`automation_configure`, `remote_configure`) desde `features/<área>`; `src-tauri/src/prefs.rs` solo guarda el JSON, no conoce las claves.
6. Pruebas: `store/prefs.test.ts`, `features/settings/settings.test.ts`.

### 3.4 Un texto traducido
1. Clave `<área>.<nombre>` en `src/i18n/es/<área>.ts` **y** `src/i18n/en/<área>.ts` (el inglés es `{ [K in keyof typeof Source]?: string }`; no idéntico al español, salvo nombres propios listados en `IDENTICAL_OK` de `i18n.test.ts:~119`). Marcadores `{nombre}` iguales en ambos.
2. Área existente: nada más (`es.ts`/`en.ts` ya la esparcen). Área **nueva**: archivo en `es/` y `en/`, `import` + `...area` en `i18n/es.ts` (~273) y `i18n/en.ts`, y entradas en `i18n.test.ts` (imports, `AREAS`, lista de prefijos ~70).
3. Uso: `t('area.clave', { n })` (`I18nKey` sale de `keyof typeof es`; una clave inexistente no compila).
4. Correr: `npx vitest run src/i18n` (completitud, prefijos, marcadores).

### 3.5 Un comando de menú o atajo
1. `CommandDef` (`commands/types.ts`: `id` `categoria.nombre`, `label: I18nKey`, `category`, `shortcuts`, `run`, `enabled`, `checked`) en `src/commands/definitions/<categoría>.ts`. Archivo nuevo: arreglo exportado + `...xxxCommands` en `ALL_COMMANDS` de `commands/index.ts`.
2. Texto del `label` y motivos de `enabled` (`reason.*` o `<área>.reason.*`): ver 3.4.
3. Menú: sin tocar nada aparece al final del menú de su categoría; para ubicarlo, agrega el `id` a `items` en `commands/menus.ts` (`'-'` = separador; ej. `machine.tiling`).
4. Atajo: `shortcuts: ['Mod+Shift+K']` (formato en `commands/shortcuts.ts`); el registro rechaza ids o atajos repetidos al arrancar (`registry.test.ts`). El usuario los cambia en Preferencias (`prefs.shortcuts`).
5. Botón de barra: `features/shell/CommandButton.tsx` con el `id`; ícono en `features/shell/icons.ts`.

### 3.6 Un tipo del contrato (ts-rs)
1. Tipo en `crates/tlaku-doc/src/<archivo>.rs` con `#[derive(Serialize, Deserialize, TS)]`, `#[ts(export)]` y `#[serde(default)]` en campos nuevos (así los archivos viejos siguen abriendo); `pub use` en `tlaku-doc/src/lib.rs`.
2. Regenerar: `cargo test -p tlaku-doc` escribe `apps/desktop/src/bindings/*.ts` (ruta en `.cargo/config.toml`, `TS_RS_EXPORT_DIR`). Nunca editar `bindings/` a mano; se versiona.
3. Defaults en TypeScript (deben coincidir con `Default` de Rust): capas en `lib/layers.ts` (`factoryLayer`, `makeLayer`), perfiles en `lib/device.ts` (`DEFAULT_DIALECT`, `DEVICE_WAVE4_DEFAULTS`, `FALLBACK_DEVICE`, `normalizeDevice`), documento en `store/document.ts` (`emptyDocument`). El mock (`api/mock/*`) también lee estos tipos.
4. Importar el tipo con `import type { X } from '@/bindings/X'` (o desde `@/api`, que reexporta).
5. Prueba de ida y vuelta en `tlaku-doc/src/tests.rs`.

### 3.7 Un ajuste de capa
1. `tlaku-doc/src/layer.rs`: campo en `LayerSettings` (o `ImageSettings`/`TabSettings`) con `#[serde(default)]` y su valor en `impl Default` (~346); en `cut.rs`/`galvo.rs` si es de esos grupos. Regenerar bindings (3.6).
2. Motor: `tlaku-pipeline` (`build/` consume la capa: `line.rs`, `fill.rs`, `image.rs`; `layers.rs` valida; `stroke.rs`, `tabs.rs`, `zaxis.rs`... según el ajuste). Fill: `tlaku-fill`; imagen: `tlaku-raster`.
3. `apps/desktop/src/lib/layers.ts`: valor por defecto en `factoryLayer` (~77). Si no debe copiarse a plantillas, ver `layerTemplate` (117).
4. UI: `features/layers/fields.ts` (`NumericField` + rango en la tabla ~118), `rules.ts` (cuándo se deshabilita y por qué), `LayerFields.tsx` (o `ParamFields.tsx`/`SubLayers.tsx`), texto `layers.*` en `i18n/{es,en}/layers.ts`.
5. `api/mock/job.ts` si el ajuste cambia la vista previa simulada.
6. CLI si debe poder pedirse por línea de comandos: `crates/tlaku-cli/src/options.rs` / `cli.rs`.
7. Pruebas: `tlaku-pipeline/src/tests/<tema>.rs` o `tests/orden.rs`; `features/layers/*.test.ts(x)`. Si cambia el G-code dorado, ver sección 4.

### 3.8 Un driver o tipo de máquina nuevo
Modelo a imitar: Ruida (`tlaku-ruida`) o K40 (`tlaku-lhymicro`).
1. `tlaku-doc/src/device.rs`: variante en `DeviceKind` (+ struct de ajustes con `#[serde(default)]`); bindings (3.6). Si es solo otro dialecto de G-code, basta `GcodeDialect` (`device.rs`) y `tlaku-grbl/src/gcode.rs`.
2. Crate `crates/tlaku-<x>` (entra solo al workspace): `Toolpath` a protocolo, transporte detrás de un trait, sesión con parada segura, `safety.rs`, feature `emu` con emulador. No decide orden ni relleno (ver contrato en `arquitectura.md`).
3. `tlaku-pipeline`: módulo de salida (como `ruida.rs`/`galvo.rs`) + rama en `build/mod.rs` (por `device.kind`; `grep -n "device.kind"`) y `pub use` en `lib.rs`.
4. CLI: `tlaku-cli/src/<x>.rs` + subcomando en `cli.rs` (`Command::...` ~101, despacho ~447).
5. App Rust: `machine/<x>.rs` (`Handle`, `Connection`, `Worker`, `tests.rs`), variantes en `ActiveMachine` y `Machine` de `machine/active.rs`, rama en `commands.rs::connect` (~242-270, por `DeviceKind`), comandos propios en `lib.rs`, `virtual_port.rs` si lleva máquina virtual.
6. Frontend: `lib/device.ts` (defaults), `features/devices/{catalog,steps,AddMachineWizard}`, `features/machine/{DeviceKindField,ConnectionSection,<X>Section,<X>Fields,<X>Notice}.tsx`, `features/materials/machineClass.ts`, `store/machine/selectors.ts` (selectores por familia), `features/machine/virtualPort.ts` (`NAMES`), mock (`api/mock/index.ts`, `machine.ts`), textos `i18n/{es,en}/<x>.ts` (área nueva, ver 3.4).
7. Docs: `docs/hardware/<x>.md` o `controladoras.md`, `docs/app-api/`, tabla de crates de `arquitectura.md`.

### 3.9 Un formato de importación
1. Lector: crate propio si es grande (como `tlaku-lbrn`) o `tlaku-pipeline/src/import_<fmt>.rs` (`mod` en `lib.rs`); devuelve `ImportResult` con advertencias, nunca omisiones en silencio.
2. `tlaku-pipeline/src/import.rs`: variante en `InputKind` (~15), extensión en `input_kind` (~27-42) **y** el mensaje de error con la lista de formatos; rama en `import_file_extras` (~247-265).
3. `apps/desktop/src/features/history/paths.ts`: `IMPORT_EXTENSIONS` (abrir, arrastrar y soltar, recientes).
4. Si tiene opciones propias: campos en `ImportDialogOptions` (`src-tauri/src/edit/files.rs:19`), `features/fileio/types.ts` + `ImportOptionsDialog.tsx` (`kindsOf`), textos `fileio.*`, mock `api/mock/fileio.ts`.
5. Fixture pequeño bajo `crates/tlaku-pipeline/tests/fixtures/` y prueba en `import.rs` (`mod tests`) o `tests/golden.rs`. Texto de formatos soportados: `README.md` y manual (otra tarea).

### 3.10 Un preset de material de fábrica
1. `crates/tlaku-doc/data/materials-default.json`: en `presets` agrega `{"id":"factory.<material>.<grosor|x>.<rol>.<clase>", "material", "thickness"?, "role", "title", "machine_class", "speed", "power", "passes", "air"?, "source": "https://...", "confidence"}`. `id` único con prefijo `factory.`; **`source` (URL https) y `confidence` son obligatorios**; potencia 0-100, velocidad > 0. Sin fuente pública no se agrega (sala limpia).
2. Material nuevo: clave en `materials` del mismo JSON, entrada en `MATERIAL` de `features/materials/labels.ts` y textos `materials.factory.material.<clave>` en `i18n/{es,en}/materials.ts`. Restricciones (`restrictions`) igual: `source` obligatorio.
3. Validación: `check_factory` (`tlaku-doc/src/factory.rs:153`) corre al leer el JSON y en `factory.rs` `mod tests`: `cargo test -p tlaku-doc factory`. Sube `revision` si cambian valores que el usuario ya tenga copiados.
4. El mock lee el mismo JSON (`api/mock/materials.ts`): `npx vitest run src/features/materials`.

### 3.11 Una captura del manual
1. `apps/desktop/scripts/screenshots.mjs`: bloque con `await shot('NN-nombre.png')` en la fase/ola que corresponda (fase 3 ~191, ola 2 ~423, ola 3 ~532, ola 4 ~678; la última es la 71, `71-inicio-revisar-aviso.png`: sigue la numeración). Prepara el estado con `app()`/`evaluate(...)`, `click`, `clickText`, `key`, `closeDialog()`; empieza de `fresh` y tema oscuro. Los PNG se escriben al final (Vite recarga si no).
2. Correr: `npm run dev` en una terminal (puerto 1420, modo mock) y `npm run screenshots` en otra (necesita Chrome; variable `CHROME`). Salen en `apps/desktop/docs/screenshots/` (o en `SCREENSHOTS_OUT=<carpeta>`). Con `SCREENSHOTS_SKIP_PHONE=1` no se levanta la demo remota y se omiten las capturas del celular (63).
3. Enlazarla en `docs/manual/` (no lo leas entero: `grep -n` el capítulo) y en la ayuda de la app si aplica (`features/help/pages.ts`).

### 3.12 Un mensaje del motor traducido (Rust)
1. Catálogo `crates/tlaku-i18n/locales/{es,en}/<crate>.txt` (`<crate>.<módulo>.<qué> = plantilla {marcador}`) y en el código `tr!("<clave>", marcador = valor)` (`use tlaku_i18n::tr;`, dependencia `tlaku-i18n` en el `Cargo.toml` del crate). Guía completa: `crates/tlaku-i18n/README.md`.
2. Un crate nuevo agrega su par de archivos a `CATALOGS` en `tlaku-i18n/src/catalog.rs` (ya están todos los actuales).
3. Si la interfaz debe reconocer el error: `tlaku_i18n::coded("mi_codigo", tr!(..))` y en `lib/errors.ts` el código en `ENGINE_ERROR` (`errorCode(e)`); nunca comparar el texto. Ejemplo: `gcode_zero_power`.
4. Idioma: la app llama a `set_engine_locale` desde `AppShell` al arrancar y al cambiar `language`; la CLI usa `TLAKU_LANG`/`LANG`. En pruebas: `with_locale(Locale::En, || ..)`.
5. Correr: `cargo test -p tlaku-i18n -p tlaku-<crate>`.

## 4. Pruebas

| Tipo | Dónde | Correr solo una |
|---|---|---|
| Unitarias Rust | `mod tests` al final del archivo, o `tests.rs` al lado (`tlaku-doc`, `-geom`, `-pipeline`, `-dxf`, `-export`, `-sim`, `-codes`, `-nest`) | `cargo test -p tlaku-geom nombre_parcial` |
| Integración Rust | `crates/<x>/tests/*.rs` (`tlaku-cli`: `cli`, `galvo`, `k40`, `ruida_net`; `-emu`: `behavior`, `protocol`, `reprap`, `streamer`, `net`; `-grbl`: `streamer`, `dialects`, `vectors`) | `cargo test -p tlaku-grbl --test streamer nombre` |
| Dorados (G-code, rellenos, bitmaps) | `crates/<x>/tests/fixtures/` + `golden.rs` en `-grbl`, `-fill`, `-raster`, `-pipeline` (`golden`, `golden_orden`, `stroke_golden`), `-cli` | `UPDATE_GOLDEN=1 cargo test -p <crate> --test golden` y revisar el diff; nunca regenerar para tapar un fallo ([CONTRIBUTING.md](../CONTRIBUTING.md)) |
| App Rust | `src-tauri/src/**/tests.rs` (machine, ruida, lhymicro, galvo, remote, engine) con `Sink` en memoria y máquinas `virtual:*` | `cargo test -p tlaku-desktop machine::tests::nombre` |
| Frontend (vitest) | `*.test.ts(x)` junto al código (config en `vite.config.ts`; `include: src/**/*.test.ts[x]`) | `cd apps/desktop && npx vitest run src/features/layers` o `... -t "nombre"` |
| Emuladores | `tlaku-emu` (`VirtualMachine` implementa `Transport`; perfiles `Profile::{grbl_1_1h,falcon2_pro,grbl_hal,fluidnc,grbl_lpc,marlin,smoothie}`; fallas con `Fault`/`Scheduled`), `tlaku-ruida` y `tlaku-lhymicro` con feature `emu`, `tlaku-galvo/src/emu.rs` | `cargo test -p tlaku-emu --test behavior` |
| Máquinas `virtual:*` | Puerto `virtual:<perfil>[@vel]` (`falcon2 grbl11 grblhal fluidnc marlin smoothie`; en la UI también `ruida`, `k40`, `galvo`; `virtualnet:` = por TCP local). Se elige en el panel Máquina; sin hardware | ver `machine/virtual_port.rs`, `features/machine/virtualPort.ts` |
| Estrés | `tlaku-bench` (`run`/`gen`, smoke en `crates/tlaku-bench/tests/smoke.rs`), `scripts/run-stress.sh [dir]` (descarga corpus con `fetch-corpus.sh`, compila release; resultados en `docs/pruebas-estres*.{md,csv}`), lienzo: `apps/desktop/scripts/canvas-stress.mjs` | `cargo test -p tlaku-bench`; `scripts/run-stress.sh /tmp/x` es lento |
| Licencias | `deny.toml` | `cargo deny check licenses sources` |

Todo antes de entregar: bloque "Verificar" de [CLAUDE.md](../CLAUDE.md).

## 5. Docs

| Archivo | Para qué / cuándo leerlo |
|---|---|
| `arquitectura.md` | Pipeline, responsabilidad de cada crate, contrato `Toolpath`, decisiones (Z, rotativo, sub-capas, orden), convenciones. Leer al tocar el motor |
| `app-api/` | Contrato app-motor: `README.md` (convenciones y tabla de comandos por área) + un archivo por área. Abre solo el del comando buscado |
| `mapa.md` | Este archivo |
| `plan-completo.md` | Plan por fases hasta las 214 funciones; consultar al elegir qué falta |
| `ideas-mejoras.md` | Backlog de ideas sobre LightBurn; no son compromisos |
| `publicar.md` | Cómo publicar una versión (etiqueta `v*`, `release.yml`, borrador de GitHub Release), actualizador, Flatpak y Flathub, sitio, CI y la lista de pasos que le tocan al usuario |
| `pruebas-estres.md` (+ `.csv`) | Resultados y método de las pruebas con archivos reales; leer al tocar rendimiento |
| `manual/` | Manual de usuario (capturas incluidas). No leer salvo tarea de manual |
| `spec/00-resumen.md` | Inventario de funciones y prioridades P0-P2 de LightBurn (sala limpia) |
| `spec/01-diseno-edicion.md` | Inventario: interfaz y herramientas de diseño/edición |
| `spec/02-capas-corte-imagen.md` | Inventario: capas, ajustes de corte, procesamiento de imagen |
| `spec/03-maquinas-avanzadas.md` | Inventario: conexión y control de máquinas, funciones avanzadas |
| `spec/cobertura/` | Qué funciones están cubiertas: `README.md` (totales y áreas) + un archivo por área. Solo en tareas de cobertura |
| `hardware/grbl.md` | Protocolo GRBL 1.1 y vectores de prueba; al tocar `tlaku-grbl`/streamer/preflight |
| `hardware/dialectos-gcode.md` | Marlin, Smoothie, grblHAL, FluidNC, GRBL-LPC, GRBL-M3: diferencias; al tocar `GcodeDialect` |
| `hardware/falcon2.md` | Creality Falcon2: evidencia pública y comparación; al tocar su perfil/emulador |
| `hardware/controladoras.md` | Qué controladoras soporta LightBurn y estado en Tlaku; catálogo de firmware y marcas |
| `hardware/ruida.md` | Protocolo Ruida y viabilidad; al tocar `tlaku-ruida` |
| `hardware/dsp-co2.md` | DSP de CO2 no Ruida y K40 M2 Nano/Moshiboard; al tocar `tlaku-lhymicro` |
| `hardware/galvo.md` | Galvo JCZ/BJJCZ: viabilidad, seguridad láser clase 4; al tocar `tlaku-galvo` |
| `hardware/taller.md` | Validación de rotativo, eje Z, corrección de barrido, puentes y cámara |

### Agregado en la ola 6C (cámara y paquetes de usuario)

| Dónde | Qué |
|---|---|
| `src/features/camera/{netSource,networkUrl,NetworkSection}.ts(x)` | Cámara de red: bucle de cuadros (`runNetworkLoop`, `drawNetworkFrame`), validar/ocultar/quitar credenciales de la URL, campos del panel (tipo de cámara, URL, configuraciones). `session.ts` usa `rawSource()` (video o canvas de la red) |
| `src/features/camera/{presets,adjust,AdjustSection}.ts(x)` | Configuraciones con nombre (`savePreset`, `applyPreset`) y ajuste manual del fondo (`adjustMatrix`, `autoDpm`), aplicado en `session.publishBackdrop` y `mapping.currentMapping` |
| `src/features/camera/{headMarks,headMarksState,HeadMarksBlock}.ts(x)` | Alineación automática del cabezal con marcas (`detectHeadMarks`; `head.ts fitHeadCalibration` acepta una marca por observación) |
| `src/features/camera/{zone,ZoneOverlay}.ts(x)` | Zona del recorrido dibujada en el lienzo (portal en `.canvas-area`); `headScan.ts` `currentScanArea`/`scanRegion(useSelection, zone)` |
| `src/features/bundle/{extras,sources}.ts` | Paquete versión 2 (mejora 12): `extras.ts` = secciones `costs`, `remnants`, `templates`, `history` (armar, sanear, comparar nuevo/distinto/igual) y la política de conflictos (`ConflictPolicy`: reemplazar, conservar ambos, omitir; `diffByKey`, `mergeByKey`); `sources.ts` = de dónde sale cada sección (`collectSource`, `buildCurrentBundle`). `model.ts` suma las secciones al plan; `apply.ts` guarda un respaldo previo (`saveSafetyBackup`) y aplica con las políticas; `BundleDialog.tsx` muestra la vista previa por categoría con el selector de conflictos; `Frame.tsx` es el marco común |
| `src/features/backup/` | Respaldo a carpeta (mejora 12): `actions.ts` (`runBackup`, `runBackupStartup`, `isBackupDue`, `backupOnClose`, `saveBackupSettings`), `BackupDialog.tsx` (carpeta, cuándo, cuántos, lista y «Restaurar…»), `NewerBackupPrompt.tsx` (pregunta al abrir), `startup.ts` (`useBackupStartup`, lo monta `BundleHost`), `store.ts`. El cierre lo engancha `useCloseGuard` (`features/history/windowEvents.ts`). Mock: `api/mock/backup.ts` (`backupSim`: unidad ausente, respaldo de «otra computadora»). Textos `bundle.backup.*` en `i18n/{es,en}/bundle.ts`. Comandos `backup.open` y `backup.now` |
| `src/features/bundle/{cameras,art}.ts` | Secciones `cameras` y `art` del paquete (armar, planear, sin credenciales); `apply.ts`/`BundleDialog.tsx` las aplican y muestran. Comando nuevo `art_save_user` (`edit/art.rs`) |

### Agregado: vista en vivo del trabajo (escritorio y celular)

Simulación dibujada, sin cámara, que sigue a la máquina real: ubica la posición que reporta (`machine://status`) en la ruta de la vista previa (`move_times`), interpola entre reportes y dibuja lo quemado / lo pendiente / el cabezal. Solo lectura. Apagada de fábrica (`Prefs.liveView`); apagada no ubica ni anima nada. En el escritorio se ve en una **ventana flotante propia** (`LiveWindow`, «imagen en imagen»; NO toca el lienzo del diseño: cama simulada con su propia vista, zoom y paneo) o en una **ventana nativa aparte** (`live`, de solo lectura); el celular conserva su página.

| Dónde | Qué |
|---|---|
| `src/features/live/route.ts` | `LiveRoute` (`buildLiveRoute`, modelo de la vista previa + punto de partida), `locate` (hacia delante sin retroceder, ventana de 3 s de trabajo, tolerancia 0,5 mm y empates como `resume/plan.rs`; el avance del streamer desempata sin ubicación previa) y `RouteLocator` (reporte a reporte: reintento en toda la ruta tras 3 fallos, aviso de retroceso real) |
| `src/features/live/tracker.ts` | `LiveTracker`: interpolación del cabezal (extrapola a la velocidad medida, filtro de primer orden, nunca retrocede salvo retroceso real, Hold = quieto, cancelado = congelado, fin = al final). Lo copia el celular (bloque `live-core` de `page.html`; `pageLive.test.ts` fija que den lo mismo) |
| `src/features/live/service.ts` (+`capture.ts`, `progress.ts`, `frame.ts`) | `LiveService` (sin React; `startLive`, `useLive`, `getLiveService`, `addLiveConsumer`, `onLiveRoute`): sigue el store, elige la fuente (`position` / `progress` / `clock`), `frame()` para dibujar, `report()` para el celular y `reading()`/`onLiveReading` (último reporte ubicado, para la ventana aparte). `capture.ts`: `noteJobStart` (lo llama `startJob`), `frame.ts`: `reportToDoc` (origen, swap, rotativo), `progress.ts`: `jobFraction` |
| `src/features/live/layers.ts`, `draw.ts` | Capas fuera de pantalla en coordenadas del documento: pendiente (se pre-renderiza una vez, por tandas) y quemado (incremental, se acumula); un cuadro = fondo + cama + dos `drawImage` + el cabezal; se rearman al cambiar el zoom de la ventana ×2. Se dibuja en el lienzo propio de la ventana: nada sobre el diseño |
| `src/features/live/{LiveWindow,LivePanel,LiveToggle,index}.tsx`, `live.css`, `text.ts` | `LiveWindow`: la ventana flotante (arrastrar por la barra, esquina para el tamaño, agrandar/restaurar, abrir aparte, cerrar = apagar; mecánica y estilos de `shell/FloatingPanels.tsx`; se monta desde `MachineDialogHost`). `LivePanel`: cama encajada con zoom/paneo propios (rueda, arrastre, teclado, «encajar»), bucle de dibujo y franja de avance; lo usan la flotante y la aparte. `LiveToggle` (interruptor del panel Trabajo), `LiveHost` (arranca el servicio y el puente a la ventana aparte desde `AppShell`). Textos `i18n/{es,en}/live.ts` |
| `src/features/live/{source,localSource,remoteSource,follow,wire,detach,DetachedLive,windowMode}.ts(x)` | `LiveSource`: de dónde sale lo que se dibuja. `localSource` (servicio + stores de la ventana principal) y `remoteSource` (la ventana aparte: recibe la ruta y el estado y arma su propio `LiveTracker`). `follow.ts`: `frameAt`/`emittingAt` (puros, los comparten servicio y ventana aparte). `wire.ts`: formato de los mensajes `live://route` (arreglos en base64, tope 1,5 M de movimientos) y `live://state` (foto + `LiveReading`), validados al decodificar. `detach.ts`: abrir/cerrar la ventana aparte y mandarle la ruta (una vez por trabajo) y el estado. `DetachedLive`: raíz de la ventana `live` (`main.tsx` la elige con `isLiveWindow()`: etiqueta `live` en Tauri o `?liveWindow=1` en el modo simulado); no carga preferencias ni toca la máquina |
| `src/lib/liveWindow.ts` | Rectángulo de la ventana flotante: `sanitizeLiveWindow` (preferencia `Prefs.liveWindow`), `fitLiveRect` (entera dentro de la app, se reacomoda al achicar), `resizeLiveRect`, `defaultLiveRect`, `maxLiveRect` (tamaño mínimo 280 × 220) |
| `src-tauri/src/live_window.rs` + `capabilities/live.json` | Ventana aparte `live` (una sola; de solo lectura). Comandos `live_window_open`, `live_window_close`, `live_window_send` (solo canales `route`/`state`, tope 48 MiB) y `live_window_hello` (solo desde `live`); la ventana dueña recibe `live://ready` y `live://closed`. La capacidad `live` solo permite `core:event:allow-listen/unlisten`, `allow-live-window-hello` y `allow-live-window-close`: ningún comando de máquina (lo prueba `live_window::tests` leyendo las capacidades y `smoke.rs` invocando de verdad desde la ventana `live`) |
| `src/features/live/remoteRoute.ts` | Ruta simplificada para el celular: trazos con láser, Douglas-Peucker en mm, tope de ~300 KB, líneas agrupadas en rellenos (`buildLivePayload`). `remote/remote.ts` la manda una vez por trabajo (`remoteSetLive`) y `remote/snapshot.ts` añade `live` al informe |
| `src-tauri/src/remote/live.rs` (+`hub.rs` `set_live`/`live_json`, `server.rs` `GET /api/live`, `mod.rs` `remote_set_live`) | Valida y sirve la ruta (tope 320 KB / 30 000 trazos / 60 000 vértices) y `LiveReport`; el estado trae `live_v` y `live`. `page.html`: interruptor, `localStorage` `tlaku.remote.live`, lienzo con cabezal interpolado |
| `src-tauri/src/machine/tests/live_view.rs` + `src/features/live/fixtures/emulator-trace.json` | Corre trabajos reales en `virtual:falcon2` con tres perfiles y comprueba que cada reporte cae sobre la ruta; `UPDATE_LIVE_TRACE=1` regenera el rastro que reproduce `liveEmulator.test.ts` |
| `src/features/live/*.test.ts(x)`, `testRoutes.ts`, `fakeCanvas.ts`, `src/test/jsdom.d.ts` | Pruebas (ubicar, interpolar, capas, ruta del celular, servicio, lienzo, página del celular en jsdom, máquina simulada del mock). `scripts/live-bench.mjs`: rendimiento en Chrome headless; `scripts/live-verify.mjs [--shots DIR]`: verificación de píxeles y de ratón real en el escritorio (Chrome headless + mock: el lienzo del diseño no se tapa, lo quemado crece al 30 %/70 %/100 % dentro de la ventana flotante y coincide con el diseño, arrastrar/redimensionar/agrandar/cerrar, zoom propio, posición recordada tras recargar, activada a mitad, ventana aparte) y capturas |

### Agregado: pestañas de documentos (varios documentos en una ventana)

Los slices vivos (`doc`, `past`, `selectedIds`, `zoom`...) siempre son los de la pestaña **activa**, así que todo lo que lee `s.doc` sigue igual; las inactivas son una foto (`TabData`) que se intercambia en un solo `set`. Cada pestaña tiene un `id`; las operaciones asíncronas capturan el id (`pinTab`) y aplican su resultado con `runInTab`. Detalle para usuarias: [manual/03f](manual/03f-pestanas-de-documentos.md).

| Dónde | Qué |
|---|---|
| `src/store/tabs.ts` (+`tabs.test.ts`, `tabsJob.test.ts`, `tabsMemory.test.ts`) | `TabsSlice` (`tabs`, `activeTabId`, `tabSwitches`, `jobOrigin`; `newTab`, `openInTab`, `activateTab`, `cycleTab`, `removeTab`, `moveTab`, `duplicateTab`, `runInTab`, `noteJobOrigin`), `captureTab`/`restoreOf` (qué es por pestaña), `trimInactive` (`INACTIVE_HISTORY_STEPS` = 30, `INACTIVE_POINT_BUDGET` = 200 000 puntos), `tabViews`, `tabLabel`/`activeTabLabel` («Sin título N»), `findTabByPath`, `isPristine`, `pinTab`. El portapapeles (`clipboard`) y la herramienta son de la ventana, no de la pestaña. `selectJobPreview` (`store/selectors.ts`): vista previa del trabajo que corre (la de su pestaña de origen) |
| `src/store/tabExtras.ts` | Registro de estado POR PESTAÑA que vive fuera del store principal (`registerTabExtra`, `captureExtras`, `restoreExtras`); `captureTab`/`restoreOf` lo guardan en `TabData.extras`. Hoy lo usa `features/variables/context.ts` (clave `variables.csv`: las tablas CSV leídas, `useVariablesUi.csv` y `.extra`, son de la pestaña activa y viajan con ella). `csvSync.ts` fija la pestaña (`pinTab`) al leer y escribe el resultado con `runInTab`; `startCsvSync` solo lee lo que la pestaña activa pide y aún no tiene. Pruebas: `variables/tabsAsync.test.tsx`, `text/tabsAsync.test.tsx`, `costs/costTabs.test.tsx`, `costs/costJobOrigin.test.ts` |
| `src/features/tabs/` | `TabBar.tsx` (`TabBar` con `role=tablist/tab`, menú contextual, arrastre con el puntero; `TabPanel` = `tabpanel` del lienzo; `tabs.css`), `actions.ts` (`closeTab`, `closeOtherTabs`, `confirmCloseWindow`, `saveAllTabs`, diálogo con la lista de sucias), `handoff.ts` («Mover a ventana nueva»: copia `handoff-…` + `?handoff=` en la ventana nueva), `session.ts` (sesión `localStorage` `tlaku.tabs.session`, `restoreSession`, preferencia `restoreTabs`), `TabsHost.tsx` (arranque: recoge el handoff, restaura la sesión y la guarda; solo la ventana `main`) |
| `src/commands/definitions/tabs.ts` | `tabs.new` (`Mod+T`), `tabs.close` (`Mod+W`), `tabs.next` / `tabs.prev` (`Mod+Tab` / `Mod+Shift+Tab`; en macOS `Ctrl+Tab`, ver `eventToCombo`/`formatCombo` en `shortcuts.ts`), `closeOthers`, `duplicate`, `moveToWindow`, `saveAll`. **No** hay `Mod+1..9` ni `Mod+Alt+←/→`: ya son de los paneles y del movimiento del cabezal |
| `src/features/history/files.ts` | `openProject`/`openRecent`/`newProject` abren en pestaña (sin preguntar); `saveTab`, `saveProject` y `importPaths` fijan la pestaña de origen. `documentName()` es el de la pestaña activa |
| `src/features/recovery/autosave.ts` | `Autosaver` con una sesión de copia por pestaña (`tick(force, tabId)`, `endSession(tabId)`, `endAll`); `recover.ts` abre cada copia en su pestaña (`recoverAll`) |
| `src-tauri/src/window.rs` (`handoff_url`), `autosave.rs` (`Limits.max_copies` = 40), `lib.rs` (`without_close_window`: macOS sin «Cerrar ventana» Cmd+W) | Lado Rust; pruebas en cada archivo y `smoke.rs` (`second_window`: `handoff` llega a la dirección) |
| `scripts/queue-verify.mjs [--shots DIR]` | Verificación en Chrome headless con el mock de la cola guiada (se detiene entre trabajos, el paso de confirmación, cancelación a media cola, línea en la vista en vivo) y del historial (capas, miniaturas, notas, filtro, «Repetir con estos ajustes») |
| `scripts/tabs-verify.mjs [--shots DIR]` | Verificación en Chrome headless con el mock: tres documentos, deshacer por pestaña, teclado, arrastre, menú, cerrar con cambios, trabajo desde una pestaña y sesión |
| `scripts/nest-verify.mjs [--shots DIR]` | Verificación en Chrome headless con el mock del nesting: varias hojas, retazos (inventario), veta y sin giro por pieza |
| `src/features/text/{advanced.ts,capabilities.ts,OpenTypeSection.tsx,FeaturePreview.tsx}` + `src-tauri/src/edit/text.rs` (`font_capabilities`, `to_spec`) | Texto avanzado (mejora 13): `advanced.ts` (lógica pura: qué funciones OpenType están encendidas, `withFeature`/`withAxis`, grupos, instancias con nombre), `capabilities.ts` (`useFontCapabilities`, memoria por cara+ejes con debounce), `OpenTypeSection.tsx` (sección plegable del panel Texto: casillas con vista previa apagada/encendida y deslizadores de ejes) y `FeaturePreview.tsx` (SVG). Campos de `TextObject`: `features`, `variations`, `uppercase`, `valign`, `word_spacing_mm` (todos opcionales); `paramsKey` en `signature.ts` los incluye. Textos `text.ot.*`/`text.var.*` en `i18n/{es,en}/text.ts`. Pruebas `advanced.test.ts`, `OpenTypeSection.test.tsx` |

### Agregado: generadores de piezas (mejora 2 del plan de usuarios)

Cajas con dientes (con divisores, zócalo y patas), bandejas y cajones, bisagras flexibles, engranajes, uniones sueltas, cajas redondeadas y pieza de prueba de encaje. Cada generador es una función pura `parámetros -> Result<Generated, PartsError>` en `crates/tlaku-parts`; la interfaz arma el formulario a partir de su lista de campos, así que **un generador nuevo no toca el núcleo**.

| Dónde | Qué |
|---|---|
| `crates/tlaku-parts/src/{boxes,tray,hinge,gears,joints,rounded_box,fit_test}` | Un módulo por generador: struct de parámetros (`#[serde(default)]`), función, `schema()` (campos del formulario) y pruebas. La caja se prueba en 3D (cada punto de las losas lo tiene un solo panel) |
| `crates/tlaku-parts/src/registry.rs` | `GENERATORS` (lista), `ParamSpec`/`ParamKind` (formulario), `generate(id, params, max_width, gap)` (valida, genera y acomoda en filas), `list()`. Tipos exportados a `bindings/` con ts-rs |
| `crates/tlaku-i18n/locales/{es,en}/parts.txt` | Mensajes del motor (validación, avisos, nombres de piezas, rótulos). Clave `parts.<generador>.<qué>` |
| `apps/desktop/src-tauri/src/edit/parts.rs` | Comandos `list_generators` y `generate_part` (parámetros inválidos vuelven en `error`, no como fallo) |
| `apps/desktop/src/features/parts/` | `PartsDialog.tsx` (lista, formulario, vista previa en vivo, capas, Insertar/Regenerar), `ParamForm.tsx` (formulario desde `ParamSpec`), `PartsBar.tsx` (barra «Regenerar…» al seleccionar el grupo), `partsModel.ts` (puro: objetos, hueco libre, inserción, registro), `actions.ts` (`commitParts` con `pinTab`), `partsStore.ts`, `usePartsPreview.ts` |
| `crates/tlaku-doc/src/document.rs` | `Document.generators: Vec<GeneratorRecord>` (grupo, generador, parámetros JSON, capas): lo que «Regenerar» necesita; vacío no se escribe |
| `apps/desktop/src/api/mock/{parts.ts,partsEngine.ts,partsGears.ts,partsSchema.json}` | Mock: el formulario sale del mismo JSON que el motor (una prueba de Rust lo mantiene al día: `UPDATE_FIXTURES=1 cargo test -p tlaku-parts schema_fixture`); la geometría la aproxima `partsEngine.ts` (engranajes en `partsGears.ts`, sin filete ni aligeramiento) |
| `apps/desktop/scripts/parts-verify.mjs [--shots DIR]` | Verificación visual en Chrome headless con el mock |
| `cargo run -p tlaku-parts --example svg -- DIR` | SVG de cada generador para revisarlos a ojo |

**Cómo añadir un generador (engranajes, bandejas, cola de milano...):**
1. `crates/tlaku-parts/src/<id>.rs`: struct de parámetros (`Serialize + Deserialize + Default`, `#[serde(default)]`), `fn <id>_parts(&Params) -> Result<Generated, PartsError>` que devuelve `Part::new(key, nombre, cortes, grabados)` (coordenadas nominales en mm; la pieza se lleva sola al origen) y `pub fn schema() -> Vec<ParamSpec>` con `ParamSpec::{length,int,boolean,choice}` (`.when(...)` para campos condicionales). Valida con `PartsError::field("clave", tr!(...))`.
2. `registry.rs`: una entrada en `GENERATORS` (`id`, `schema`, `defaults: to_value::<Params>`, `run: |v| run::<Params>(v, <id>_parts)`) y `pub mod` en `lib.rs`.
3. Mensajes en `locales/{es,en}/parts.txt` (`parts.<id>.*`) y, en la interfaz, textos `parts.generator.<id>`, `parts.generator.<id>.desc`, `parts.<id>.<campo>` y `parts.<id>.<campo>.<opción>` en `i18n/{es,en}/parts.ts` (una prueba de `PartsDialog.test.tsx` revisa que no falte ninguno), y el ícono en `ICONS` de `PartsDialog.tsx`.
4. Regenerar el JSON del mock (`UPDATE_FIXTURES=1 cargo test -p tlaku-parts schema_fixture`) y, si quieres vista previa en el mock, una función en `partsEngine.ts` (si no, el mock avisa con su error de generador desconocido).
5. Para uniones entre piezas reutiliza `fingers::Cells` (reparto con número impar de celdas), `rectilinear::rect_minus_rects` (rectángulo menos muescas, exacto) y `font::text_*` (rótulos); la prueba 3D de `boxes/tests.rs` (`every_slab_point_is_owned_by_exactly_one_panel`) es el modelo para comprobar que las piezas encajan.

