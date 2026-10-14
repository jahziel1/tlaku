# Mapa del código

Léelo antes de tocar nada. Responde "¿dónde está X?" y "¿dónde agrego Y?". Qué hace cada crate y
las decisiones de diseño están en [arquitectura.md](arquitectura.md) (no se repiten aquí); el
contrato de comandos y eventos, en [app-api.md](app-api.md) (busca con `grep -n`, no lo leas
entero). Las líneas son aproximadas (2026-10); si el código cambió, `grep -n` el nombre.

## 1. Estructura de arriba abajo

Workspace Cargo: `crates/*` y `apps/desktop/src-tauri` (crate `tlaku-desktop`). Otros: `scripts/`
(`run-stress.sh`, `fetch-corpus.sh`), `tests/corpus/` (archivos reales; no leer), `.cargo/config.toml`
(`TS_RS_EXPORT_DIR` de los bindings), `.github/workflows/{ci,release}.yml`.

### 1.1 `crates/*` (entrada `src/lib.rs` salvo que se indique)

| Crate | Qué es / módulos clave |
|---|---|
| `tlaku-core` | `Point`, `Polyline`, `Rgb`, `Job`, `Toolpath`. `bezpath.rs`, `optimize.rs`, `toolpath.rs` |
| `tlaku-doc` | Contrato con la app (serde + ts-rs). `document.rs` (`Document`, `DesignObject`), `layer.rs` (`LayerSettings`), `device.rs` (`DeviceProfile`, `DeviceKind`, `GcodeDialect`), `job.rs` (`JobOptions`, `JobPreview`), `machine.rs` (`StatusReport`, eventos), `cut.rs`, `galvo.rs`, `materials.rs`, `factory.rs` + `data/materials-default.json`, `text.rs`, `virtual_array.rs`, `tests.rs` |
| `tlaku-svg` | `lib.rs` (`import_svg`), `clip.rs` (clip-path/mask) |
| `tlaku-dxf` | `lib.rs`, `aci.rs` (colores), `geom.rs` |
| `tlaku-lbrn` | Lector neutro de proyectos `.lbrn/.lbrn2`: `parse.rs`, `pathdata.rs` |
| `tlaku-geom` | `boolean`, `offset`, `kerf`, `join`, `dedupe`, `tabs`, `fillet`, `trim`, `split`, `slots`, `knife`, `hull`, `tangent`, `warp`, `simplify`, `nesting` (par-impar) |
| `tlaku-plan` | Orden de corte: `planner.rs`, `sched.rs`, `improve.rs`, `contain.rs`, `overlap.rs`, `explain.rs`, `describe.rs` |
| `tlaku-fill` | Relleno: `scan.rs` (barrido), `offset.rs` (contornos), `flood.rs`, `cluster.rs` |
| `tlaku-raster` | Imagen a potencia: `adjust`, `dither`, `screen` (semitono), `sketch`, `mask`, `resample`, `scan.rs` (barrido a bloque) |
| `tlaku-sim` | Tiempo/overscan: `profile.rs`, `plan.rs`, `sample.rs` |
| `tlaku-text` | Texto a contornos: `library.rs`, `layout.rs`, `linefont.rs` (Hershey en `data/`), `shx.rs`, `effects.rs`, `path_sampler.rs` |
| `tlaku-trace` | Vectorizar imagen: `binarize`, `polygon`, `spline` |
| `tlaku-codes` | QR y barras: `encode.rs`, `geometry.rs` |
| `tlaku-nest` | Nesting: `engine.rs`, `search.rs`, `prep.rs`, `region.rs`, `convex.rs`, `ops.rs` |
| `tlaku-camera` | Lente/homografía/marcas sin OpenCV: `calibration.rs`, `marks.rs`, `chessboard`, `circle`, `homography`, `height`, `mosaic`, `undistort` |
| `tlaku-export` | Salida SVG/DXF/PNG/PDF: `svg.rs`, `dxf_out.rs`, `png_out.rs`, `pdf.rs`, `scene.rs` |
| `tlaku-pipeline` | `Document`+`DeviceProfile`+`JobOptions` a `ComputedJob`. `build.rs` (núcleo), `layers.rs`, `place.rs`, `stroke.rs`, `tabs.rs`, `zaxis.rs`, `rotary.rs`, `scan_offset.rs`, `tile.rs`, `split.rs`, `slice3d.rs`, `cylinder.rs`, `gcode.rs`, `ruida.rs`, `galvo.rs`, `preview.rs`, `material_test.rs`, `registration.rs`, `processed.rs`; importadores: `import.rs` (despacha por extensión), `import_pdf.rs`, `import_hpgl.rs`, `import_gcode.rs`, `import_lbrn.rs` + `import_lbrn/{layers,bitmap,text}.rs` |
| `tlaku-grbl` | G-code y enlace: `gcode.rs`, `streamer.rs`, `preflight.rs`, `frame.rs`, `serial.rs`, `net.rs` (TCP/Telnet/WebSocket), `link.rs` (`LinkProfile`) |
| `tlaku-ruida` | Experimental: `rd.rs` (archivo), `cmd.rs` (comandos), `packet.rs`, `swizzle.rs`, `link.rs` (UDP), `session.rs`, `safety.rs`, `emu.rs` |
| `tlaku-lhymicro` | Experimental (K40): `encode.rs`, `packet.rs`, `crc.rs`, `speed.rs`, `status.rs`, `transport.rs`, `usb.rs`, `session.rs`, `emu.rs`, `decode.rs` |
| `tlaku-galvo` | Experimental (JCZ/BJJCZ): `compile.rs`, `cmd.rs`, `codes.rs`, `params.rs`, `calib.rs`, `cor.rs`, `safety.rs`, `session.rs`, `transport.rs`, `usb.rs`, `wire.rs`, `emu.rs` |
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
| `machine/virtual_port.rs` | Puertos `virtual:<perfil>[@vel]` (6 perfiles GRBL/Marlin/Smoothie en `KINDS`) |
| `machine/{extras,frame}.rs` | Ejes extra, encuadre |
| `edit/` | Un archivo por área: `text`, `variables`, `codes`, `geometry`, `shape_ops`, `design`, `nest`, `trace`, `art`, `camera`, `materials`, `devices`, `files` (registro en `edit/mod.rs`) |
| `remote/` | Control remoto por celular (axum): `server.rs`, `hub.rs`, `auth.rs`, `net.rs`, `page.html`, `tests.rs` |
| `automation.rs`, `autosave.rs`, `prefs.rs`, `settings_backup.rs`, `templates.rs`, `probe.rs`, `netlink.rs`, `window.rs` | UDP M54, autoguardado, preferencias y paquetes, plantillas, sondeo de puerto, red, ventanas |
| `tauri.conf.json`, `capabilities/default.json` | Config y permisos de Tauri |

### 1.3 `apps/desktop/src/`

| Carpeta | Contenido |
|---|---|
| `api/` | `api.ts` (interfaz `LaserApi`, `COMMAND_NAMES`, `EVENT_NAMES`), `tauri.ts` (`invoke`/`listen`), `types.ts` (`JobArgs`, `EventMap`), `index.ts` (`getApi`, `VITE_API=mock\|tauri`), `mock/` (un archivo por área + `index.ts` que los une, `machine.ts` simula la máquina) |
| `store/` | Zustand: `index.ts` (une slices y conecta eventos `machine://*`), `document.ts`, `selection.ts`, `view.ts`, `machine.ts`, `prefs.ts`, `devices.ts`, `camera.ts`, `galvo.ts`, `preview.ts`, `notices.ts`, `selectors.ts`, `types.ts` |
| `features/<área>/` | `arrange art automation bundle camera canvas codes contextmenu devices fileio help history layers machine materials nest preview printcut recovery remote settings shell simple text tiling trace variables`. Cada una: `index.ts(x)`, paneles/diálogos `.tsx`, lógica `.ts`, CSS y `*.test.ts(x)` junto al código |
| `commands/` | `types.ts` (`CommandDef`), `definitions/<área>.ts`, `index.ts` (`ALL_COMMANDS`), `menus.ts` (`MENUS`), `shortcuts.ts`, `registry.ts`, `useShortcuts.ts` |
| `i18n/` | `index.ts` (`t()`), `es.ts` y `en.ts` (unen las áreas), `es/<área>.ts`, `en/<área>.ts`, `i18n.test.ts` |
| `bindings/` | Generado por ts-rs desde `tlaku-doc`. **No editar ni leer** |
| `ui/` | `Modal`, `NumberField`, `LengthField` |
| `lib/` | Puro y sin UI: `layers.ts` (`makeLayer`), `device.ts` (`FALLBACK_DEVICE`, `normalizeDevice`), `units`, `bezpath`, `geometry`, `viewport`, `groups`, `dockLayout`, `warp`, `hull`, `palette`, `platform` (`readPref`/`writePref`) |
| `styles/` | `tokens.css` (variables), `base`, `shell`, `panels`, `canvas`; cada feature trae su CSS |

## 2. Archivos grandes (más de 800 líneas)

Se partirán en la ola 6; si ya no existen o los números no cuadran, buscar con `grep -n`.
Lee con `offset`/`limit`. Cada fila: líneas totales y secciones (`línea:nombre`).

| Archivo (líneas) | Secciones |
|---|---|
| `crates/tlaku-grbl/src/streamer.rs` (2402) | 35 `Realtime`, 103 `override_plan`, 152-410 `MachineState`/`StatusReport` + parseo, 429-650 `Firmware` + mensajes de `error:`/`ALARM:`, 656-692 errores Creality, 702 `prepare_line`, 740 `StreamError`, 851 `StreamerConfig`, 925 `Event`/`Outcome`, 1030-1810 `Streamer`/`RunState` (bucle de envío), 1837 `absorb_line`, 1879 tests |
| `crates/tlaku-emu/src/machine.rs` (2301) | 34 `Options`, 120 `Machine`, 230-2285 `impl Machine` (RX, planificador, `$`, tiempo real, homing), 2285 tests |
| `apps/desktop/src-tauri/src/edit/camera.rs` (1987) | 60-134 `detect_board`/`calibrate_lens`, 139-245 marcas, 252-700 `CameraSetup` + `normalize_setup` + archivo, 705-917 `compose_bed_image`, 923 círculo, 939 tests |
| `crates/tlaku-ruida/src/session.rs` (1848) | 37 `addr`, 85 `SessionConfig`, 136 `SessionError`, 281 `SendMode`, 352-1091 `RuidaSession`, 1105 tests |
| `apps/desktop/src-tauri/src/machine.rs` (1789) | 80 `Emit`, 114 `Timing`, 220 `Handle`, 396 `Connection`, 522 `identify`, 715 `Worker`; en `impl Worker`: 801 `run`, 853 `on_status`, 970 `idle_step`, 1157 `command`, 1262 `jog`, 1409 `try_start`, 1510 `stream_step`, 1682 `finish` |
| `apps/desktop/src-tauri/src/machine/galvo.rs` (1726) | 123 `SafetyAck`, 263 `prepare`, 459 `GalvoEvent`, 614 `GalvoHandle`, 869 `GalvoConnection`, 1151 `Worker` (1430 `run_light`, 1541 `run_job`) |
| `crates/tlaku-lhymicro/src/session.rs` (1643) | 58 `SessionConfig`, 129 `JobControl`, 185 `SessionError`, 385 `LhySession` (497 `probe`, 777 `send_job`, 930 `home`, 1004 `frame`), 1045 tests |
| `crates/tlaku-camera/src/calibration.rs` (1603) | 38 `LensModel`, 297 `calibrate_lens`, 383 `calibrate_views`, 673-870 ajuste, 1037 `levenberg_marquardt`, 1160 tests |
| `crates/tlaku-ruida/src/cmd.rs` (1549) | 124 `Command`, 360-720 encode, 728 `decode_body`, 1073 `Display`, 1171 tests |
| `crates/tlaku-lhymicro/src/encode.rs` (1542) | 39 `HomeCorner`, 110 `EncodeConfig`, 151 `Warning`, 397 `Encoder`, 654 `encode`, 798 tests |
| `crates/tlaku-ruida/src/rd.rs` (1539) | 89 `HomeCorner`, 186 `RuidaJobOptions`, 335 `plan_layers`, 521 `build_commands`, 721 `build_rd`, 786 tests |
| `crates/tlaku-camera/src/marks.rs` (1480) | 44 `MarkLayout`, 186 `DetectOptions`, 302 `find_candidates`, 659-777 asignación, 863 `detect_marks`, 1075 tests |
| `crates/tlaku-galvo/src/compile.rs` (1416) | 34 `FieldOrigin`, 58 `CompileOptions`, 212 `Builder`, 691 `compile`, 793 `compile_light`, 850 tests |
| `crates/tlaku-pipeline/src/stroke.rs` (1401) | 53 `LeadSpec`, 118 `resolve`, 372 `Stroke`, 685 `push_shape`, 774 `apply_closed`, 849 `ramp`, 922 tests |
| `crates/tlaku-svg/src/lib.rs` (1342) | 60 `import_svg`, 135 `scan_xml`, 197 `build_warnings`, 580 `clip_path`, 662 `curves`, 785 tests |
| `crates/tlaku-pipeline/src/build.rs` (1332) | 33 `FillPart`, 163 `LineCtx`, 229 `Builder` (248-886), 898 `build` (898-1250), 1250 `plan_options`, 1314 `compute` |
| `crates/tlaku-grbl/src/gcode.rs` (1313) | 39 `GcodeOptions`, 121 `Writer`, 452 `generate`, 603 tests |
| `crates/tlaku-grbl/src/preflight.rs` (1304) | 95 `validate_gcode`, 310 `check_settings`, 455 `alarm_advice`, 585 `preflight`, 777 `preflight_link`, 889 tests |
| `apps/desktop/src-tauri/src/machine/ruida.rs` (1289) | 81 `RuidaTiming`, 175 `RuidaHandle`, 307 `RuidaConnection`, 591 `VirtualSim`, 659 `Worker`, 1241 `build_upload` |
| `apps/desktop/src-tauri/src/machine/lhymicro.rs` (1267) | 131 config, 339 `LhyHandle`, 556 `LhyConnection`, 825 `Worker` |
| `crates/tlaku-cli/src/galvo.rs` (1184) | 209 `GalvoCommand`, 276 `plan`, 452 `run`, 564 `light`, 677 `mark`, 802 tests |
| `apps/desktop/src-tauri/src/edit/geometry.rs` (1152) | 63 `GeometryOp`, 202 `run`, 317 `cleanup`, 466 `geometry_op`, 484 tests |
| `crates/tlaku-pipeline/src/import_pdf.rs` (1146) | 36 `import`, 170 `page_geometry`, 213 `ColorSpace`, 437 `Interpreter`, 868 tests |
| `apps/desktop/src-tauri/src/remote/hub.rs` (1137) | 128 `Snapshot`, 224 `ActionError`, 290 `Core` (hasta 665), 746 tests |
| `crates/tlaku-pipeline/src/tile.rs` (1083) | 48 `Grid`, 122 `plan_tiles`, 321-493 recortes `clip_*`, 513 `apply`, 559 tests |
| `crates/tlaku-galvo/src/{codes,safety,session}.rs` (1059/1048/925) | codes: 169 tabla `GP`, 957 `lookup`; safety: 46 `SafetyLimits`, 335 `validate_list`, 546 `check_immediate`; session: 347 `GalvoSession` |
| `crates/tlaku-grbl/src/net.rs` (996) | 126-213 Telnet, 213 `TcpTransport`, 340 `WebSocketTransport`, 549 `Endpoint` |
| `crates/tlaku-cli/src/link.rs` (996) / `k40.rs` (815) | link: 102 `stream_lines`, 192 `send_job`, 366 `classify`, 619 `run_commands`; k40: 76 `K40Command`, 400 `export_egv`, 435 `send` |
| `apps/desktop/src-tauri/src/edit/shape_ops.rs` (928) | 49 `ShapeOp`, 273 `trim`, 397 `fillet`, 451 `cut_shapes`, 531 `resize`, 593 `shape_op` |
| `crates/tlaku-lhymicro/src/emu.rs` (892) / `tlaku-ruida/src/emu.rs` (837) | lhy: 116 `EmuBoard`; ruida: 112 `Emulator`, 425 `UdpEmulator`, 492 `replay` |
| `crates/tlaku-pipeline/src/material_test.rs` (889) | 40 `GLYPHS` (fuente de rótulos), 326 `apply_param`, 484 `material_test` |
| `crates/tlaku-raster/src/scan.rs` (869) | 8 `RasterSettings`, 90 `raster_to_block`, 196 `rotated_moves`, 276 `row_runs` |
| `crates/tlaku-pipeline/src/import_hpgl.rs` (820) | 24 `import`, 158 `Parser` |
| `apps/desktop/src/features/canvas/useCanvasTools.ts` (920) | un solo hook `useCanvasTools` (91-920): ratón, herramientas, atajos del lienzo |
| `apps/desktop/src/features/canvas/canvas2d.ts` (914) | `Canvas2DRenderer` (54-914) |
| `apps/desktop/src/store/machine.ts` (884) | 125 `reduceMachine`, 207-320 selectores (`jogBlockedReason`, `sendBlockedReason`), 503 `createMachineSlice` |
| `apps/desktop/scripts/screenshots.mjs` (899) | 18-190 ayudantes (`shot`, `click`, `evaluate`), 191 fase 3, 423 ola 2, 532 ola 3 (39-47), 678 ola 4 (48-57) |
| `apps/desktop/src/features/machine/machine.css` (1059) | prefijos `.mc-*`; busca la clase con `grep -n` |

Pruebas de más de 800 líneas (no leer enteras; busca el nombre con `grep -n "fn "`):
`crates/tlaku-pipeline/src/tests.rs`, `tests/{orden,galvo}.rs`, `crates/tlaku-plan/tests/plan.rs`,
`crates/tlaku-cli/tests/cli.rs`, `crates/tlaku-emu/tests/{reprap,behavior}.rs`,
`crates/tlaku-grbl/tests/streamer.rs`, `crates/tlaku-text/tests/outline.rs`, `crates/tlaku-geom/src/tests.rs`,
`crates/tlaku-dxf/src/tests.rs`, `crates/tlaku-export/src/tests.rs`,
`apps/desktop/src-tauri/src/machine/{tests,ruida/tests,lhymicro/tests,galvo/tests}.rs`, `remote/tests.rs`.

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
7. `docs/app-api.md`: fila/sección del comando (`grep -n "^## \|^### "` para ubicar el área).
8. Prueba: Rust en el mismo archivo (`mod tests`) y `api/mock/*.test.ts` o la de la feature.

### 3.2 Un evento de máquina (`machine://*`)
1. Carga serializable: tipo en `tlaku-doc/src/machine.rs` (derive `TS`, ver 3.6) o, si es de un solo driver, en su módulo (como `GalvoEvent` en `machine/galvo.rs:~459`).
2. `src-tauri/src/machine.rs`: variante en `enum Emit` (~80) **y** su nombre en `Emit::name()`; luego `TauriSink::emit` en `commands.rs:~71-79` (un `match` exhaustivo).
3. Emitirlo desde el `Worker` del driver (`self.sink.emit(Emit::...)`; `machine/{ruida,lhymicro,galvo}.rs`).
4. Frontend: `src/api/types.ts`, entrada en `EventMap` (~289; el tipo `GalvoEvent` vive allí). Si es del grupo común, agregar el nombre a `EVENT_NAMES` en `api/api.ts` (`mock.test.ts` cuenta 10) .
5. Suscribirlo en `src/store/index.ts:~62-67` (`api.on('machine://x', (p) => s().onX(p))`) y la acción `onX` en el slice (`store/machine.ts` interfaz ~420 + `createMachineSlice` 503, o `store/galvo.ts`); si cambia el estado, el reductor es `reduceMachine` (125).
6. Mock: `src/api/mock/machine.ts` (`this.emit('machine://x', payload)`).
7. `docs/app-api.md`, tabla "Eventos (`listen`)".

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
2. Motor: `tlaku-pipeline` (`build.rs` consume la capa; `layers.rs` valida; `stroke.rs`, `tabs.rs`, `zaxis.rs`... según el ajuste). Fill: `tlaku-fill`; imagen: `tlaku-raster`.
3. `apps/desktop/src/lib/layers.ts`: valor por defecto en `factoryLayer` (~77). Si no debe copiarse a plantillas, ver `layerTemplate` (117).
4. UI: `features/layers/fields.ts` (`NumericField` + rango en la tabla ~118), `rules.ts` (cuándo se deshabilita y por qué), `LayerFields.tsx` (o `ParamFields.tsx`/`SubLayers.tsx`), texto `layers.*` en `i18n/{es,en}/layers.ts`.
5. `api/mock/job.ts` si el ajuste cambia la vista previa simulada.
6. CLI si debe poder pedirse por línea de comandos: `crates/tlaku-cli/src/options.rs` / `cli.rs`.
7. Pruebas: `tlaku-pipeline/src/tests.rs` o `tests/orden.rs`; `features/layers/*.test.ts(x)`. Si cambia el G-code dorado, ver sección 4.

### 3.8 Un driver o tipo de máquina nuevo
Modelo a imitar: Ruida (`tlaku-ruida`) o K40 (`tlaku-lhymicro`).
1. `tlaku-doc/src/device.rs`: variante en `DeviceKind` (+ struct de ajustes con `#[serde(default)]`); bindings (3.6). Si es solo otro dialecto de G-code, basta `GcodeDialect` (`device.rs`) y `tlaku-grbl/src/gcode.rs`.
2. Crate `crates/tlaku-<x>` (entra solo al workspace): `Toolpath` a protocolo, transporte detrás de un trait, sesión con parada segura, `safety.rs`, feature `emu` con emulador. No decide orden ni relleno (ver contrato en `arquitectura.md`).
3. `tlaku-pipeline`: módulo de salida (como `ruida.rs`/`galvo.rs`) + rama en `build.rs` (por `device.kind`, ~107) y `pub use` en `lib.rs`.
4. CLI: `tlaku-cli/src/<x>.rs` + subcomando en `cli.rs` (`Command::...` ~101, despacho ~447).
5. App Rust: `machine/<x>.rs` (`Handle`, `Connection`, `Worker`, `tests.rs`), variantes en `ActiveMachine` y `Machine` de `machine/active.rs`, rama en `commands.rs::connect` (~242-270, por `DeviceKind`), comandos propios en `lib.rs`, `virtual_port.rs` si lleva máquina virtual.
6. Frontend: `lib/device.ts` (defaults), `features/devices/{catalog,steps,AddMachineWizard}`, `features/machine/{DeviceKindField,ConnectionSection,<X>Section,<X>Fields,<X>Notice}.tsx`, `features/materials/machineClass.ts`, `store/machine.ts` (selectores por familia), `features/machine/virtualPort.ts` (`NAMES`), mock (`api/mock/index.ts`, `machine.ts`), textos `i18n/{es,en}/<x>.ts` (área nueva, ver 3.4).
7. Docs: `docs/hardware/<x>.md` o `controladoras.md`, `docs/app-api.md`, tabla de crates de `arquitectura.md`.

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
1. `apps/desktop/scripts/screenshots.mjs`: bloque con `await shot('NN-nombre.png')` en la fase/ola que corresponda (fase 3 ~191, ola 2 ~423, ola 3 ~532, ola 4 ~678; última 57). Prepara el estado con `app()`/`evaluate(...)`, `click`, `clickText`, `key`, `closeDialog()`; empieza de `fresh` y tema oscuro. Los PNG se escriben al final (Vite recarga si no).
2. Correr: `npm run dev` en una terminal (puerto 1420, modo mock) y `npm run screenshots` en otra (necesita Chrome; variable `CHROME`). Salen en `apps/desktop/docs/screenshots/`.
3. Enlazarla en `docs/manual/` (no lo leas entero: `grep -n` el capítulo) y en la ayuda de la app si aplica (`features/help/pages.ts`).

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
| `app-api.md` (877) | Contrato app-motor: comandos, eventos, conexión, Ruida/K40, remoto. Solo el comando buscado (`grep -n`) |
| `mapa.md` | Este archivo |
| `plan-completo.md` | Plan por fases hasta las 214 funciones; consultar al elegir qué falta |
| `ideas-mejoras.md` | Backlog de ideas sobre LightBurn; no son compromisos |
| `publicar.md` | Cómo publicar una versión (etiqueta `v*`, `release.yml`, borrador de GitHub Release) |
| `pruebas-estres.md` (+ `.csv`) | Resultados y método de las pruebas con archivos reales; leer al tocar rendimiento |
| `manual/` | Manual de usuario (capturas incluidas). No leer salvo tarea de manual |
| `spec/00-resumen.md` | Inventario de funciones y prioridades P0-P2 de LightBurn (sala limpia) |
| `spec/01-diseno-edicion.md` | Inventario: interfaz y herramientas de diseño/edición |
| `spec/02-capas-corte-imagen.md` | Inventario: capas, ajustes de corte, procesamiento de imagen |
| `spec/03-maquinas-avanzadas.md` | Inventario: conexión y control de máquinas, funciones avanzadas |
| `spec/04-cobertura.md` | Qué funciones están cubiertas y cuáles no. Solo en tareas de cobertura |
| `hardware/grbl.md` | Protocolo GRBL 1.1 y vectores de prueba; al tocar `tlaku-grbl`/streamer/preflight |
| `hardware/dialectos-gcode.md` | Marlin, Smoothie, grblHAL, FluidNC, GRBL-LPC, GRBL-M3: diferencias; al tocar `GcodeDialect` |
| `hardware/falcon2.md` | Creality Falcon2: evidencia pública y comparación; al tocar su perfil/emulador |
| `hardware/controladoras.md` | Qué controladoras soporta LightBurn y estado en Tlaku; catálogo de firmware y marcas |
| `hardware/ruida.md` | Protocolo Ruida y viabilidad; al tocar `tlaku-ruida` |
| `hardware/dsp-co2.md` | DSP de CO2 no Ruida y K40 M2 Nano/Moshiboard; al tocar `tlaku-lhymicro` |
| `hardware/galvo.md` | Galvo JCZ/BJJCZ: viabilidad, seguridad láser clase 4; al tocar `tlaku-galvo` |
| `hardware/taller.md` | Validación de rotativo, eje Z, corrección de barrido, puentes y cámara |
