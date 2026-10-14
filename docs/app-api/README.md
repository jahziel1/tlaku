# Contrato entre la app (Tauri) y el motor

Fase 2. La interfaz (TypeScript) es dueña del documento que se edita; el motor (Rust) no
guarda estado del documento: recibe el documento completo y devuelve resultados. La única
excepción es la conexión con la máquina, que vive en Rust.

Los tipos se definen en Rust (crate `tlaku-doc`) con `serde` y se exportan a TypeScript con
`ts-rs` en `apps/desktop/src/bindings/` (`cargo test -p tlaku-doc` los regenera; la ruta sale de
`.cargo/config.toml`). Este archivo describe la forma; el código manda.
JSON con `snake_case`, unidades en mm, mm/min y %, origen abajo a la izquierda, Y hacia arriba.

Este contrato se partió por área (antes era un solo archivo de 55 KB). Busca el comando en la tabla de abajo y abre solo el archivo del área. Los tipos exactos están en `crates/tlaku-doc` y en `apps/desktop/src/bindings/` (generado).

## Índice

| Archivo | Contenido | Comandos con tabla |
|---|---|---:|
| [documento.md](documento.md) | El documento, los objetos, las capas, el perfil de dispositivo y las opciones de trabajo. | 0 |
| [conexion-y-trabajo.md](conexion-y-trabajo.md) | Comandos de importar, calcular, exportar y enviar el trabajo; conexión con la máquina; eventos `machine://*`; imágenes. | 19 |
| [edicion.md](edicion.md) | Curvas, grupos, texto, vectorizado, operaciones de geometría y de formas y arte; incluye los primeros comandos de cámara de la fase 3 (`detect_board`, `calibrate_lens`, marcas). | 18 |
| [taller.md](taller.md) | Campos nuevos de capas y perfiles (puentes, Z, rotativo, dialectos G-code) de las fases de taller. | 0 |
| [materiales.md](materiales.md) | Comandos y contrato de la biblioteca de materiales y la prueba de material. | 6 |
| [maquinas.md](maquinas.md) | Comandos de máquinas de la fase 4, oleada C (`device_defaults`, `probe_port`, `simulate_fault`). | 4 |
| [preferencias.md](preferencias.md) | Preferencias, paquetes `.tlakuprefs`, autoguardado, plantillas y ventanas. | 14 |
| [red.md](red.md) | GRBL por TCP y WebSocket. | 0 |
| [ruida.md](ruida.md) | Controladoras Ruida por UDP. | 0 |
| [k40.md](k40.md) | Placa M2 Nano (`tlaku-lhymicro`) por USB. | 3 |
| [automatizacion.md](automatizacion.md) | Servidor UDP local con comandos de texto. | 4 |
| [control-remoto.md](control-remoto.md) | Servidor HTTP, seguridad, API y aviso al terminar. | 8 |
| [camara.md](camara.md) | Comandos de la cámara (ola 4). | 3 |

## Comandos por área

Los nombres salen de la primera columna de las tablas de comandos de cada archivo (en `automatizacion.md` son los comandos de texto del UDP). Los eventos (`listen`) están en [conexion-y-trabajo.md](conexion-y-trabajo.md); las rutas HTTP del control remoto, en [control-remoto.md](control-remoto.md). Los comandos de red y Ruida se describen en el texto de [red.md](red.md) y [ruida.md](ruida.md).

| Archivo | Comandos |
|---|---|
| [documento.md](documento.md) | (sin tabla de comandos; ver el texto del archivo) |
| [conexion-y-trabajo.md](conexion-y-trabajo.md) | `import_file`, `compute_job`, `export_gcode`, `save_project`, `load_project`, `list_devices`, `save_device`, `delete_device`, `list_ports`, `connect`, `disconnect`, `machine_command`, `machine_realtime`, `jog`, `start_job`, `pause_job`, `resume_job`, `cancel_job`, `frame_job` |
| [edicion.md](edicion.md) | `list_fonts`, `text_outline`, `geometry_op`, `shape_op`, `trace_image`, `vectors_to_bitmap`, `save_processed_image`, `art_import_svg`, `art_list_user`, `art_add_user`, `art_remove_user`, `detect_board`, `calibrate_lens`, `load_camera_setup`, `save_camera_setup`, `save_board_file`, `camera_marks`, `detect_camera_marks` |
| [taller.md](taller.md) | (sin tabla de comandos; ver el texto del archivo) |
| [materiales.md](materiales.md) | `load_material_library`, `load_factory_materials`, `save_material_library`, `import_material_library`, `export_material_library`, `material_test` |
| [maquinas.md](maquinas.md) | `device_defaults`, `rotary_test`, `probe_port`, `simulate_fault` |
| [preferencias.md](preferencias.md) | `load_preferences`, `save_preferences`, `export_preferences`, `import_preferences`, `paths_exist`, `autosave_write`, `autosave_list`, `autosave_read`, `autosave_discard`, `list_templates`, `save_template`, `load_template`, `delete_template`, `new_window` |
| [red.md](red.md) | (sin tabla de comandos; ver el texto del archivo) |
| [ruida.md](ruida.md) | (sin tabla de comandos; ver el texto del archivo) |
| [k40.md](k40.md) | `lhy_preview`, `lhy_home`, `lhy_state` |
| [automatizacion.md](automatizacion.md) | `PING`, `START`, `STATUS`, `STOP` |
| [control-remoto.md](control-remoto.md) | `remote_configure`, `remote_status`, `remote_regenerate`, `remote_kick`, `remote_update`, `remote_set_route`, `remote_camera_frame`, `remote_start_result` |
| [camara.md](camara.md) | `calibrate_lens`, `compose_bed_image`, `detect_circle_in_image` |
