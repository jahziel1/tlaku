# Contrato app-motor: Aplicación: preferencias, atajos y autoguardado

Parte de [el contrato app-motor](README.md). Preferencias, paquetes `.tlakuprefs`, autoguardado, plantillas y ventanas.

## Aplicación: preferencias, atajos y autoguardado

Código: `src-tauri/src/prefs.rs`, `src-tauri/src/autosave.rs`; en la interfaz `features/settings/`
y `features/recovery/`.

| Comando | Argumentos | Resultado |
|---|---|---|
| `load_preferences` | — | `PrefsFile` o `null` si no hay archivo. Uno dañado se aparta como `.dañado`; una versión desconocida es un error y no se toca |
| `save_preferences` | `data` (`PrefsFile`) | Escribe `preferences.json` de forma atómica |
| `export_preferences` | `path`, `bundle` | Escribe un paquete `.tlakuprefs` |
| `import_preferences` | `path` | `PrefsBundle` (valida `format` y `version`; máximo 1 MB) |
| `paths_exist` | `paths` | `bool[]`: si cada archivo existe (limpia «recientes») |
| `set_engine_locale` | `locale` (`"es"` \| `"en"`) | Fija el idioma de los avisos y errores del motor (`tlaku-i18n`, global). La interfaz lo llama al arrancar y al cambiar `language` (`store/prefs.ts`); un valor desconocido es un error |
| `autosave_write` | `id`, `name`, `path`, `doc` | `RecoveryInfo`; reemplaza la copia `id` y recorta las más viejas |
| `autosave_list` | — | `RecoveryInfo[]` pendientes, de la más nueva a la más vieja; borra las superadas por un guardado posterior |
| `autosave_read` | `id` | `Document` |
| `autosave_discard` | `id` | Borra la copia (no es error que no exista) |

### Plantillas y ventanas (ola 3)

Código: `src-tauri/src/templates.rs` y `src-tauri/src/window.rs`; en la interfaz `features/shell/` (`templates.ts`, `TemplatesDialog.tsx`).

| Comando | Argumentos | Resultado |
|---|---|---|
| `list_templates` | — | `TemplateInfo[]` (`name`, `saved_at_ms`, `size_bytes`), por nombre |
| `save_template` | `name`, `doc` | `TemplateInfo`; reemplaza la plantilla del mismo nombre. El nombre es el del archivo `<nombre>.tlakutemplate` y no admite separadores de ruta ni caracteres reservados |
| `load_template` | `name` | `Document` |
| `delete_template` | `name` | Borra la plantilla (no es error que no exista) |
| `new_window` | — | Etiqueta de la ventana nueva (`doc-N`). Cada ventana tiene su propio documento; comparten la máquina y las preferencias |

Las plantillas viven en `<app_config_dir>/plantillas/`. La capacidad `capabilities/default.json` cubre las
ventanas `main` y `doc-*`.

- **Preferencias**: `preferences.json` en `app_config_dir()` (sobrevive a reinstalar y se puede copiar).
  Forma: `{ "version": 1, "prefs": { ... }, "shortcuts": { "<id de comando>": ["Mod+K", ...] } }`.
  El catálogo de claves, valores de fábrica y límites está en `src/store/prefs.ts` (`PREF_SPECS`); Rust
  no conoce cada clave, solo valida el sobre. `localStorage` guarda una copia para arrancar sin parpadeo.
- **Paquete `.tlakuprefs`**: el mismo JSON más `"format": "tlaku-prefs"`. Puede ser parcial (por ejemplo solo
  `shortcuts`): al importar, lo que falta se deja como está y lo que no se entiende se descarta con aviso.
- **Atajos**: `shortcuts` es el reemplazo completo de los atajos de un comando (`[]` = sin atajo); los
  comandos que no aparecen conservan los de fábrica. Se guardan con `Mod` (Cmd en macOS, Ctrl en el resto).
- **Recuperación**: `<datos de la app>/recuperacion/<id>.tlaku` y `<id>.meta.json`. Máximo 10 copias, 128 MB
  por copia y 512 MB en total. Una copia de un proyecto con ruta se descarta si el archivo tiene fecha igual o
  posterior; las de documentos sin guardar se ofrecen siempre.
