# Contrato app-motor: historial de versiones del proyecto

Parte de [el contrato app-motor](README.md). Mejora 10 del [plan de usuarios](../plan-mejoras-usuarios.md#10-historial-de-versiones-del-proyecto). Manual: [Historial de versiones](../manual/03h-historial-de-versiones.md).

Código: `apps/desktop/src-tauri/src/versions/` (`store.rs` almacén de un proyecto, `retention.rs` política de
retención, `config.rs`, `project.rs` ruta a carpeta, captura, lectura y borrado, `commands.rs` los comandos); en la
interfaz `apps/desktop/src/features/versions/`. Todos los comandos son `async` (hilo aparte, un solo acceso al
almacén a la vez). Los argumentos son de una sola palabra: no hace falta `rename_all`.

## Cómo funciona

- **Identidad.** Un proyecto se identifica por la ruta absoluta de su `.tlaku` (el documento no tiene un id propio).
  Mover o renombrar el archivo empieza un historial nuevo; `versions_projects` sigue listando el viejo.
- **Dónde.** `VersionsConfig.location`: `app` (por omisión) = `<datos de la app>/versiones/<nombre>-<huella de la ruta>/`;
  `beside` = `<carpeta del proyecto>/.tlaku-versions/<archivo>/`. Se escribe en el lugar elegido; **listar, leer y borrar
  miran los dos** (cambiar el ajuste no esconde el historial viejo).
- **Una versión** son tres archivos: `<id>.tlaku.gz` (el JSON del proyecto, gzip), `<id>.thumb.svg.gz` (miniatura SVG sin
  imágenes, opcional, hasta 250 KB) y `<id>.meta.json` (`VersionInfo`). Todo se escribe de forma atómica y el `meta` va
  al final: sin `meta` la versión no existe y la limpieza borra los restos. Id = `<created_ms 13 dígitos>-<8 hex de la huella>`.
- **Duplicados.** Una versión con la misma huella (FNV-1a de 64 bits) y tamaño que la más reciente no se guarda; si la
  nueva es `manual`, la existente pasa a ser manual con ese nombre. Un id repetido (mismo instante y contenido) tampoco.
- **Tope por versión:** 128 MB sin comprimir.
- **Cuándo las crea la interfaz.** Antes de cada guardado (`versions_capture`), cada `auto_minutes` por pestaña con archivo y
  cambios (`versions_snapshot` con `auto`), con «Guardar versión…» (`manual`) y antes de reemplazar el documento al
  restaurar (`restore`). **El guardado principal no depende de esto:** `features/history/files.ts` llama a
  `versions_capture` por separado y, si falla, avisa y guarda igual.
- **Retención** (`retention.rs::plan`, función pura; se aplica tras cada escritura): todas hasta `recent_hours`; una por
  día UTC (la más nueva) hasta `daily_days`; una por semana UTC hasta `weekly_weeks`; lo más viejo se borra; luego los
  topes `max_versions` y `max_mb` borran primero lo más viejo. **Nunca** se borran la más reciente ni las `manual`.
- **Rutas de datos** (CSV de los datos variables): se guardan relativas a la carpeta del proyecto, igual que al guardar,
  y `versions_read` las devuelve absolutas. Las imágenes se referencian por ruta y **no** viajan en la versión.

## Comandos

| Comando | Argumentos | Resultado |
|---|---|---|
| `versions_list` | `path` | `VersionListing { versions: VersionInfo[], total_bytes, dir }`, de la más nueva a la más vieja |
| `versions_thumb` | `path`, `id` | Texto SVG de la miniatura o `null` |
| `versions_read` | `path`, `id` | `Document` de esa versión (no toca el documento abierto) |
| `versions_capture` | `path` | `VersionInfo` de lo que tenía el archivo (tipo `saved`, con la fecha del archivo) o `null` (sin archivo, historial apagado, sin cambios) |
| `versions_snapshot` | `path`, `doc`, `kind` (`auto` \| `manual` \| `restore`), `label?` | `VersionInfo` o `null` (sin cambios; `auto` con el historial apagado o `auto_minutes` = 0). `manual` exige `label` (se recorta, máximo 120 caracteres); `saved` no se acepta |
| `versions_delete` | `path`, `ids` | Cuántas versiones se borraron (las que no existan se ignoran) |
| `versions_clear` | `path` | Cuántas versiones había; quita todo el historial del proyecto y la carpeta si queda vacía |
| `versions_projects` | — | `ProjectSummary[]` (`path`, `name`, `versions`, `total_bytes`, `latest_ms`, `exists`) de la carpeta de datos de la app, el más reciente primero |
| `versions_config_get` | — | `VersionsConfig` (archivo `versions.json` de la carpeta de configuración; dañado o ausente = de fábrica) |
| `versions_config_set` | `config` | `VersionsConfig` ya acotada a rangos válidos |

`VersionInfo`: `id`, `kind` (`saved` \| `auto` \| `manual` \| `restore`), `label`, `created_ms`, `size_bytes`,
`stored_bytes`, `hash`, `objects`, `layers`, `has_thumb`.
`VersionsConfig` (de fábrica): `enabled` true, `location` `app`, `auto_minutes` 5 (0..240), `recent_hours` 24 (1..720),
`daily_days` 30 (0..365), `weekly_weeks` 26 (0..520), `max_mb` 200 (1..10240), `max_versions` 500 (10..5000).
Los tipos de la interfaz están en `features/versions/types.ts` (no vienen de `tlaku-doc`: solo los usa la interfaz).
