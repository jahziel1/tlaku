# Contrato app-motor: Aplicación: preferencias, atajos y autoguardado

Parte de [el contrato app-motor](README.md). Preferencias, paquetes `.tlakuprefs` y `.tlakubundle`, respaldo a carpeta, autoguardado, plantillas y ventanas.

## Aplicación: preferencias, atajos y autoguardado

Código: `src-tauri/src/prefs.rs`, `src-tauri/src/autosave.rs`, `src-tauri/src/job_history.rs`; en la interfaz `features/settings/`
y `features/recovery/`.

| Comando | Argumentos | Resultado |
|---|---|---|
| `load_preferences` | — | `PrefsFile` o `null` si no hay archivo. Uno dañado se aparta como `.dañado`; una versión desconocida es un error y no se toca |
| `save_preferences` | `data` (`PrefsFile`) | Escribe `preferences.json` de forma atómica |
| `export_preferences` | `path`, `bundle` | Escribe un paquete `.tlakuprefs` |
| `import_preferences` | `path` | `PrefsBundle` (valida `format` y `version`; máximo 1 MB) |
| `paths_exist` | `paths` | `bool[]`: si cada archivo existe (limpia «recientes») |
| `set_engine_locale` | `locale` (`"es"` \| `"en"`) | Fija el idioma de los avisos y errores del motor (`tlaku-i18n`, global). La interfaz lo llama al arrancar y al cambiar `language` (`store/prefs.ts`); un valor desconocido es un error |
| `autosave_write` | `id`, `name`, `path`, `doc` | `RecoveryInfo`; reemplaza la copia `id` y recorta las más viejas (tope de 40 copias). La interfaz usa **una copia por pestaña** (cada sesión de copia de cada pestaña tiene su `id`) y una copia con id `handoff-…` para «Mover a ventana nueva»; todas quedan marcadas como vivas de la ventana que escribe |
| `autosave_list` | — | `RecoveryInfo[]` pendientes, de la más nueva a la más vieja; borra las superadas por un guardado posterior |
| `autosave_read` | `id` | `Document` |
| `autosave_discard` | `id` | Borra la copia (no es error que no exista) |

### Historial de trabajos (mejora 4 del plan de usuarios)

Código: `src-tauri/src/job_history.rs`; en la interfaz `features/help/jobLog*.ts`, `JobLogDialog.tsx` y `JobLogDetail.tsx`.
Un archivo JSONL (`job-history.jsonl`, un trabajo por línea) en la **carpeta de datos de la app** (`app_data_dir`).
Rust no conoce el significado de los campos (los define y sanea la interfaz): exige un objeto con `id` (texto) y `at`
(ms desde 1970, número) de a lo más 48 KiB; las líneas dañadas se saltan al leer (y antes de reescribir sin ellas se
guarda el original como `job-history.jsonl.dañado`). Toda escritura es atómica (`write_atomic`) y se serializa con un
candado (varias ventanas anotan a la vez). La rotación borra las entradas más viejas (por `at`) al pasar el tope
`max` (20 a 20 000, por omisión 1000; la interfaz manda la preferencia `jobHistoryMax`). Todos los argumentos son de una
sola palabra, así que no hace falta `rename_all`.

| Comando | Argumentos | Resultado |
|---|---|---|
| `job_history_load` | `max` (opcional) | `object[]`, de la entrada más vieja a la más nueva; rota si el archivo pasa de `max` |
| `job_history_append` | `entry` (objeto con `id` y `at`), `max` (opcional) | Agrega o, si ya hay una con ese `id`, **reemplaza** (la entrada «en curso» que el final completa); rota |
| `job_history_update` | `id`, `patch` (objeto) | `bool`: si existía. Mezcla campos de primer nivel; un `null` borra el campo; `id` y `at` no cambian (notas, marca de interrupción) |
| `job_history_remove` | `ids` | Cuántas entradas quitó |
| `job_history_clear` | — | Borra el archivo (no es error que no exista) |
| `job_history_import` | `entries`, `max` (opcional) | Cuántas entraron: migración del registro viejo de `localStorage` (`tlaku.jobLog`); salta las inválidas y las de `id` repetido |
| `job_history_export` | `path` (`.csv`), `text` | Escribe el CSV (hasta 32 MiB; atómico). Lo arma la interfaz |

Forma de una entrada (`JobLogEntry`, `features/help/jobLog.ts`): `id`, `at`, `machine`, `document`, `documentPath?`,
`tab?`, `estimatedS`, `actualS`, `result` (`finished` \| `cancelled` \| `error` \| `interrupted` \| `running`), `message`,
`interrupted?` (punto de control), `layers?` (`color`, `name`, `mode`, `material?`, `settings` con `speed`, `power`,
`min_power`, `passes`, `interval`, `angle`, `crosshatch`, `bidirectional`, `air`, `kerf`), `material?`, `areaMm2?`,
`lengthMm?`, `thumb?` (SVG de hasta 12 000 caracteres) y `notes?`. Al empezar un trabajo se guarda con `result:
"running"` y el final la reemplaza; una entrada «en curso» que ninguna ventana sigue ejecutando (no hay latido en
`localStorage` `tlaku.jobLog.live`) se marca `interrupted` al leer. En el modo simulado vive en memoria
(`api/mock/jobHistory.ts`).

### Paquetes `.tlakubundle` y respaldo a carpeta (mejora 12)

Código: `src-tauri/src/prefs.rs` (paquete) y `src-tauri/src/backup/` (respaldo a carpeta); en la interfaz `features/bundle/` (armar,
comparar y aplicar) y `features/backup/` (diálogo, programación, pregunta al abrir).

El **paquete `.tlakubundle`** es un JSON con `format: "tlaku-bundle"` y `version`. **Versión 1**: `devices` (con macros y posiciones),
`active_device`, `materials`, `cameras`, `art`, `prefs`. **Versión 2** agrega `costs` (`machines`, `sales`, `materials`), `remnants`
(`RemnantLibrary`), `templates` (`items: {name, doc}[]`) y `history` (`entries`). La interfaz escribe la 1 si el paquete no lleva ninguna
sección nueva, así lo abre cualquier versión; el motor lee la 1 y la 2 y rechaza otra con «versión no soportada». Cada sección es opcional al
importar. El motor solo valida el sobre (máximo 32 MiB); el contenido lo sanea la interfaz.

| Comando | Argumentos | Resultado |
|---|---|---|
| `export_user_bundle` | `path` (con extensión `.tlakubundle`), `bundle` | Escribe el paquete de forma atómica |
| `import_user_bundle` | `path` | `UserBundle` (valida `format`, `version` y que `devices` sea una lista) |
| `backup_config_get` | — | `BackupConfig`: `folder`, `on_close`, `every_days`, `keep`, `include_history` y el estado `last_backup_ms`, `last_seen_ms`, `install_id`. Crea el identificador de esta instalación la primera vez. Vive en `backup.json` (no viaja en los paquetes: la ruta es de esta computadora) |
| `backup_config_set` | `settings` (`BackupSettings`) | `BackupConfig`; acota `keep` a 1–100 y `every_days` a 0–365 y quita los espacios de `folder` |
| `backup_run` | `bundle` | `BackupWrite` (`name`, `path`, `created_ms`, `removed`). Escribe `tlaku-respaldo-AAAA-MM-DD_HH-MM-SS.tlakubundle` (UTC) en la carpeta configurada, añade `install_id`, rota (conserva los `keep` más nuevos, solo borra archivos con ese nombre y `.tmp` huérfanos propios de más de 10 min) y anota `last_backup_ms`. Errores: sin carpeta, carpeta no disponible (no se crea), paquete con un secreto, más de 32 MiB |
| `backup_safety` | `bundle` | `BackupWrite`; igual que `backup_run` pero siempre en `<app_config_dir>/respaldos-locales/` con el prefijo `tlaku-antes-de-importar` y conservando 5. Lo llama la interfaz antes de aplicar una importación |
| `backup_list` | — | `BackupList` (`configured`, `available`, `entries[]` con `name`, `path`, `size_bytes`, `created_ms`), el más nuevo primero. Una carpeta ausente es `available: false`, no un error |
| `backup_check_newer` | — | `BackupEntry` o `null`: el respaldo más nuevo de la carpeta si es posterior a `max(last_backup_ms, last_seen_ms)` y lo hizo otra instalación (`install_id` distinto o ausente) |
| `backup_mark_seen` | `created_ms` | Sube `last_seen_ms`: el respaldo ya se importó o se descartó y no se vuelve a ofrecer |

**Secretos.** `backup_run` y `backup_safety` rechazan (`desktop.backup.secret_found`) cualquier paquete que tenga un campo llamado
`token`, `password`, `secret`, `apikey`, `privatekey`, `authorization`, `cookie`, `credentials`… (también si solo termina en `token`,
`password` o `secret`) con valor no nulo, o una cadena con forma `esquema://usuario[:clave]@host` (salvo dentro de `templates` e
`history`, que son texto libre del usuario). Es la segunda barrera: la interfaz ya quita las credenciales de las URL de cámara
(`stripCredentials`), no incluye el acceso del control remoto y `remoteEnabled` está en `NEVER_IMPORTED`.

**Escritura.** Atómica (`write_atomic`: archivo `.tmp` en la misma carpeta y renombrar). Si falla, no se rota ni se borra nada. La
rotación nunca toca archivos ajenos a la carpeta. La carpeta debe existir (una unidad desconectada no se sustituye por una carpeta local).

**Detección.** Compara la marca del nombre del archivo (UTC) con las del estado local: una computadora con el reloj adelantado puede
parecer «más nueva». No es sincronización: solo ofrece y la persona decide en la vista previa.

### Plantillas y ventanas (ola 3)

Código: `src-tauri/src/templates.rs` y `src-tauri/src/window.rs`; en la interfaz `features/shell/` (`templates.ts`, `TemplatesDialog.tsx`).

| Comando | Argumentos | Resultado |
|---|---|---|
| `list_templates` | — | `TemplateInfo[]` (`name`, `saved_at_ms`, `size_bytes`), por nombre |
| `save_template` | `name`, `doc` | `TemplateInfo`; reemplaza la plantilla del mismo nombre. El nombre es el del archivo `<nombre>.tlakutemplate` y no admite separadores de ruta ni caracteres reservados |
| `load_template` | `name` | `Document` |
| `delete_template` | `name` | Borra la plantilla (no es error que no exista) |
| `new_window` | `handoff` (opcional) | Etiqueta de la ventana nueva (`doc-N`). Cada ventana tiene sus propias pestañas y documentos; comparten la máquina y las preferencias. `handoff` es un texto ya codificado con `encodeURIComponent` (solo `A-Za-z0-9%-_.~`, hasta 2048 caracteres; otro valor es error) que se añade como `?handoff=…` a la dirección de la ventana: así recibe la pestaña que se movió (`features/tabs/handoff.ts`: JSON `{id, path, dirty}` con el id de una copia `autosave_write`) |
| `live_window_open` | — | Abre la ventana aparte de la vista en vivo (etiqueta `live`, una sola; si ya está abierta la trae al frente). La ventana que llama queda como dueña: a ella le llegan `live://ready` y `live://closed` |
| `live_window_close` | — | Cierra la ventana `live` si está abierta |
| `live_window_send` | `kind` (`route` \| `state`), `data` (texto JSON, hasta 48 MiB) | `bool`: si la ventana `live` existe. La reenvía como evento `live://route` o `live://state`; otro canal es error. Formato en `src/features/live/wire.ts` |
| `live_window_hello` | — | Solo la ventana `live`: ya escucha, que la dueña reenvíe la ruta y el estado (`live://ready`) |

Las plantillas viven en `<app_config_dir>/plantillas/`. La capacidad `capabilities/default.json` cubre las
ventanas `main` y `doc-*`, con el conjunto de permisos `main-window` (un `allow-<comando>` por cada comando de
`lib.rs`, que arma `build.rs`).

**Ventana aparte de la vista en vivo (`live`).** Es de solo lectura y su capacidad (`capabilities/live.json`) es la
mínima: escuchar eventos (`core:event:allow-listen` y `allow-unlisten`) y los comandos `live_window_hello` y
`live_window_close`. No puede llamar a ningún otro comando de la app (máquina, trabajo, archivos, preferencias),
ni a diálogos, rutas, el actualizador o el navegador. Eventos para la ventana `live`: `live://route` (la ruta del
trabajo) y `live://state` (porcentaje, estado, tiempos y último reporte de la máquina); para la dueña:
`live://ready` y `live://closed`. Código: `src-tauri/src/live_window.rs`; en la interfaz `features/live/` (`detach.ts`,
`DetachedLive.tsx`).

- **Preferencias**: `preferences.json` en `app_config_dir()` (sobrevive a reinstalar y se puede copiar).
  Forma: `{ "version": 1, "prefs": { ... }, "shortcuts": { "<id de comando>": ["Mod+K", ...] } }`.
  El catálogo de claves, valores de fábrica y límites está en `src/store/prefs.ts` (`PREF_SPECS`); Rust
  no conoce cada clave, solo valida el sobre. `localStorage` guarda una copia para arrancar sin parpadeo.
  Las carpetas de la app dependen del `identifier` de Tauri (`io.github.jahziel1.tlaku`); al arrancar,
  `id_migration.rs` copia las del id anterior `org.tlaku.app` si las nuevas están vacías.
- **Paquete `.tlakuprefs`**: el mismo JSON más `"format": "tlaku-prefs"`. Puede ser parcial (por ejemplo solo
  `shortcuts`): al importar, lo que falta se deja como está y lo que no se entiende se descarta con aviso.
- **Atajos**: `shortcuts` es el reemplazo completo de los atajos de un comando (`[]` = sin atajo); los
  comandos que no aparecen conservan los de fábrica. Se guardan con `Mod` (Cmd en macOS, Ctrl en el resto).
- **Recuperación**: `<datos de la app>/recuperacion/<id>.tlaku` y `<id>.meta.json`. Máximo 10 copias, 128 MB
  por copia y 512 MB en total. Una copia de un proyecto con ruta se descarta si el archivo tiene fecha igual o
  posterior; las de documentos sin guardar se ofrecen siempre.
