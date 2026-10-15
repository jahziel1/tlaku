# Mapa del código

Léelo antes de tocar nada. Responde "¿dónde está X?" y "¿dónde agrego Y?". Qué hace cada crate y
las decisiones de diseño están en [arquitectura.md](arquitectura.md) (no se repiten aquí); el
contrato de comandos y eventos, en [app-api/](app-api/README.md) (su tabla dice en qué archivo de
área está cada comando; abre solo ese). Las líneas son aproximadas (2026-10, tras partir archivos en la ola 6); si el código cambió, `grep -n` el nombre.

## 1. Estructura de arriba abajo

Workspace Cargo: `crates/*` y `apps/desktop/src-tauri` (crate `tlaku-desktop`). Otros: `scripts/`
(`run-stress.sh`, `fetch-corpus.sh`), `tests/corpus/` (archivos reales; no leer), `.cargo/config.toml`
(`TS_RS_EXPORT_DIR` de los bindings), `.github/workflows/{ci,release}.yml`.

### 1.1 `crates/*` (entrada `src/lib.rs` salvo que se indique)

| Crate | Qué es / módulos clave |
|---|---|
| `tlaku-core` | `Point`, `Polyline`, `Rgb`, `Job`, `Toolpath`. `bezpath.rs`, `optimize.rs`, `toolpath.rs` |
| `tlaku-doc` | Contrato con la app (serde + ts-rs). `document.rs` (`Document`, `DesignObject`), `layer.rs` (`LayerSettings`), `device.rs` (`DeviceProfile`, `DeviceKind`, `GcodeDialect`), `lbdev.rs` (+`lbdev/tests.rs`: `parse_lbdev`, lector de perfiles `.lbdev` de LightBurn, `LbdevImport`/`LbdevNotice`), `job.rs` (`JobOptions`, `JobPreview`), `machine.rs` (`StatusReport`, eventos), `cut.rs`, `galvo.rs`, `materials.rs`, `factory.rs` + `data/materials-default.json`, `community.rs` + `build.rs` (aportes de `community/materials/*.json`, validados por `cargo test -p tlaku-doc community`), `text.rs`, `virtual_array.rs`, `tests.rs` |
| `tlaku-svg` | `lib.rs` (`import_svg`), `clip.rs` (clip-path/mask) |
| `tlaku-dxf` | `lib.rs`, `aci.rs` (colores), `geom.rs` |
| `tlaku-lbrn` | Lector neutro de proyectos `.lbrn/.lbrn2`: `parse.rs`, `pathdata.rs` |
| `tlaku-geom` | `boolean`, `offset`, `kerf`, `join`, `dedupe`, `tabs`, `fillet`, `trim`, `split`, `slots`, `knife`, `hull`, `tangent`, `warp`, `simplify`, `nesting` (par-impar) |
| `tlaku-plan` | Orden de corte: `planner.rs`, `sched.rs`, `improve.rs`, `contain.rs`, `overlap.rs`, `explain.rs`, `describe.rs`; los textos de las razones y avisos salen de `crates/tlaku-i18n/locales/{es,en}/plan.txt` |
| `tlaku-fill` | Relleno: `scan.rs` (barrido), `offset.rs` (contornos), `flood.rs`, `cluster.rs` |
| `tlaku-raster` | Imagen a potencia: `adjust`, `dither`, `screen` (semitono), `sketch`, `mask`, `resample`, `warp` (giro afín de una imagen), `preview` (vista previa entera o por trozo), `scan.rs` (barrido a bloque); carga sin materializar la imagen: `load.rs` (cabecera, PNG por filas, TIFF por franjas/teselas con YCbCr, `convert_to_png`) y `downsample.rs`; `load_for_process` reduce a la rejilla antes de ajustar |
| `tlaku-sim` | Tiempo/overscan: `profile.rs`, `plan.rs`, `sample.rs` |
| `tlaku-text` | Texto a contornos: `library.rs`, `layout.rs`, `linefont.rs` (Hershey en `data/`), `shx.rs`, `effects.rs`, `path_sampler.rs` |
| `tlaku-trace` | Vectorizar imagen: `binarize`, `polygon`, `spline` (usa `visioncortex` mediante `vtracer/`, la parte de biblioteca de vtracer 0.6.5 adaptada, solo modo binario; ver `THIRD_PARTY.md`); `golden.rs` compara byte a byte con `tests/fixtures/golden_*` |
| `tlaku-codes` | QR y barras: `encode.rs`, `geometry.rs` |
| `tlaku-nest` | Nesting: `engine.rs`, `search.rs`, `prep.rs`, `region.rs`, `convex.rs`, `ops.rs` |
| `tlaku-camera` | Lente/homografía/marcas sin OpenCV: `calibration.rs`, `marks.rs`, `chessboard`, `circle`, `homography`, `height`, `mosaic`, `undistort` |
| `tlaku-export` | Salida SVG/DXF/PNG/PDF/AI: `svg.rs`, `dxf_out.rs`, `png_out.rs`, `pdf.rs` (`export_pdf`, `export_ai`: PDF 1.5 con una capa por color), `scene.rs` |
| `tlaku-pipeline` | `Document`+`DeviceProfile`+`JobOptions` a `ComputedJob`. `build.rs` (núcleo), `layers.rs`, `place.rs`, `stroke.rs`, `tabs.rs`, `zaxis.rs`, `rotary.rs`, `scan_offset.rs`, `tile.rs` (`plan_grid`), `split.rs` (`galvo_rotary`, `mark_sequence`), `split_shapes.rs` (formas enteras, `doc_mark_sequence`), `slice3d.rs`, `cylinder.rs`, `gcode.rs`, `ruida.rs`, `galvo.rs`, `preview.rs`, `material_test.rs`, `registration.rs` (`fit_with_choice`, imágenes que giran), `processed.rs` (`processed_png`, `processed_preview`); importadores: `import.rs` (despacha por extensión), `import_pdf.rs`, `import_hpgl.rs`, `import_gcode.rs`, `import_rd.rs` (`.rd` de Ruida), `import_egv.rs` (`.egv` del K40), `import_lbrn.rs` + `import_lbrn/{layers,bitmap,text}.rs` |
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
| `edit/` | Un archivo por área: `text`, `variables`, `codes`, `geometry`, `shape_ops`, `design`, `nest`, `trace`, `art`, `camera`, `materials`, `devices`, `files`, `imaging` (`preview_image`), `printcut` (`fit_registration`, `plan_tiles`) (registro en `edit/mod.rs`) |
| `remote/` | Control remoto por celular (axum): `server.rs`, `hub.rs`, `auth.rs`, `net.rs`, `page.html`, `tests.rs` |
| `automation.rs`, `autosave.rs`, `prefs.rs`, `settings_backup.rs`, `templates.rs`, `probe.rs`, `netlink.rs`, `window.rs` | UDP M54, autoguardado, preferencias y paquetes, plantillas, sondeo de puerto, red, ventanas |
| `tauri.conf.json`, `capabilities/default.json` | Config y permisos de Tauri |

### 1.3 `apps/desktop/src/`

| Carpeta | Contenido |
|---|---|
| `api/` | `api.ts` (interfaz `LaserApi`, `COMMAND_NAMES`, `EVENT_NAMES`), `tauri.ts` (`invoke`/`listen`), `types.ts` (`JobArgs`, `EventMap`), `index.ts` (`getApi`, `VITE_API=mock\|tauri`), `mock/` (un archivo por área + `index.ts` que los une, `machine.ts` simula la máquina) |
| `store/` | Zustand: `index.ts` (une slices y conecta eventos `machine://*`), `document.ts`, `selection.ts`, `view.ts`, `machine.ts`, `prefs.ts`, `devices.ts`, `camera.ts`, `galvo.ts`, `preview.ts`, `notices.ts`, `selectors.ts`, `types.ts` |
| `features/<área>/` | `arrange art automation bundle camera canvas codes contextmenu devices feeder fileio help history images layers machine materials nest preview printcut recovery remote settings shell simple text tiling trace variables`. Cada una: `index.ts(x)`, paneles/diálogos `.tsx`, lógica `.ts`, CSS y `*.test.ts(x)` junto al código |
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
| `src/features/help/jobLog.ts` + `JobLogDialog.tsx` | Registro de trabajos en `localStorage` (`tlaku.jobLog`); `noteJobEvent`/`noteJobAlarm` los llama `onJob`/`onStatus` en `store/machine/slice.ts`; comando `help.jobLog` |
| `src/lib/importOptions.ts`, `features/fileio/remember.ts` | Opciones de importación recordadas (pref `importOptions`) y DPI por omisión (`importDpi`, `effectiveImportDpi`) |
| `src/features/settings/persist.ts` | `startPrefsWindowSync`: las preferencias se comparten entre ventanas (evento `storage`) |

Pruebas de más de 800 líneas (no leer enteras; busca el nombre con `grep -n "fn "`):
`crates/tlaku-pipeline/tests/{orden,galvo}.rs`, `crates/tlaku-plan/tests/plan.rs`,
`crates/tlaku-cli/tests/cli.rs`, `crates/tlaku-emu/tests/{reprap,behavior,estimate}.rs`,
`crates/tlaku-grbl/tests/streamer.rs`, `crates/tlaku-text/tests/outline.rs`, `crates/tlaku-geom/src/tests.rs`,
`crates/tlaku-dxf/src/tests.rs`, `crates/tlaku-export/src/tests.rs`,
`apps/desktop/src-tauri/src/machine/{tests/,ruida/tests,lhymicro/tests,galvo/tests}.rs`, `remote/tests.rs`.

### Agregado en la ola 6D (publicación)

| Dónde | Qué |
|---|---|
| `.github/ISSUE_TEMPLATE/*.yml` (formularios es/en), `PULL_REQUEST_TEMPLATE.md`, `CODEOWNERS`, `dependabot.yml`, `labels.yml` (+ `scripts/sync-labels.sh`), `workflows/community.yml` | Comunidad: los ids de los formularios son los que prellena `features/help/githubForms.ts` (lo comprueba `githubForms.test.ts`); `community/materials/` y `scripts/community/issue-to-preset.mjs` convierten un issue de ajustes de material en un aporte |
| `src/features/help/{githubForms.ts,openExternal.ts,ShareDialogs.tsx}` | Menú Ayuda > «Reportar un problema…», «Compartir resultados de mi máquina…» y «Compartir ajustes de material…»: arman la dirección del formulario de GitHub y abren el navegador con `tauri-plugin-opener` (alcance en `capabilities/default.json`); nunca envían nada solos |
| `.github/workflows/{ci,release,pages}.yml`, `.github/FUNDING.yml` | CI (Rust en 3 sistemas, deny, interfaz, humo en Linux, sitio), publicación de instaladores, GitHub Pages (inactivo hasta `PAGES_ENABLED=true`) y botón Sponsor |
| `site/`, `scripts/build-site.mjs` | Sitio estático es/en; `site/config.json` llena `{{REPO_URL}}`, `{{SPONSORS_URL}}`, `{{KOFI_URL}}` (también en el README con `--readme`) |
| `scripts/configure-updater.sh`, `src-tauri/src/updater.rs` | Llena `plugins.updater` y activa `createUpdaterArtifacts`; el plugin solo se registra si hay `pubkey` y `endpoints` |
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
| `publicar.md` | Cómo publicar una versión (etiqueta `v*`, `release.yml`, borrador de GitHub Release), actualizador, sitio, CI y la lista de pasos que le tocan al usuario |
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
| `src/features/bundle/{cameras,art}.ts` | Secciones `cameras` y `art` del paquete (armar, planear, sin credenciales); `apply.ts`/`BundleDialog.tsx` las aplican y muestran. Comando nuevo `art_save_user` (`edit/art.rs`) |
