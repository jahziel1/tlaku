# Tlaku: reglas para agentes

Reemplazo open source (MIT) de LightBurn. Motor en Rust (`crates/`), app Tauri + React
(`apps/desktop`). Español en comentarios y docs; identificadores en inglés.

## Antes de leer código

1. Lee **[docs/mapa.md](docs/mapa.md)** para ubicar dónde vive cada cosa. No explores el
   repositorio entero.
2. Abre solo los archivos de tu tarea. Busca con `grep -n` y lee el fragmento que necesitas
   (offset/limit); no leas archivos de más de ~500 líneas completos si no hace falta.
3. **No leas** salvo que la tarea sea sobre ellos: `docs/manual/`, `docs/spec/04-cobertura.md`,
   `docs/app-api.md` completo (busca el comando con grep), `apps/desktop/src/bindings/` (generado
   por ts-rs), capturas, `crates/*/tests/fixtures/`, `tests/corpus/`, `target/`, `node_modules/` ni los textos de otras áreas
   en `src/i18n/` (solo `es/<área>.ts` y `en/<área>.ts` de la tuya).
4. Entrega un informe corto: qué cambiaste (archivos), qué verificaste y qué quedó pendiente.

## Reglas que no se negocian

- **Sala limpia:** nada de código, textos, íconos ni capturas de LightBurn; no descompilar.
- **Terceros:** leer cualquier licencia sí; copiar solo MIT/Apache/BSD/ISC/Zlib con crédito en
  `THIRD_PARTY.md`; GPL se lee pero no se copia ni se traduce línea por línea. Detalle en
  [CONTRIBUTING.md](CONTRIBUTING.md).
- **Unidades:** mm, mm/min, potencia 0–100 %, origen abajo a la izquierda, Y hacia arriba
  ([docs/arquitectura.md](docs/arquitectura.md#convenciones)).
- **Seguridad de la máquina:** un error detiene la máquina; nada se envía sin preflight; el
  control remoto y la IA nunca arrancan la máquina sin las confirmaciones definidas.
- **Textos de la interfaz** siempre en `es` y `en` (no idénticos entre sí).
- **Comandos de Tauri:** si los argumentos llegan en snake_case desde `api.ts`, el comando lleva
  `#[tauri::command(rename_all = "snake_case")]`. El modo simulado (mock) no detecta este error.
- Trabaja solo en la zona de archivos que te asignaron. **No hagas commits.**

## Verificar

```sh
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check licenses sources
cd apps/desktop && npx tsc --noEmit && npx eslint . && npx vitest run
```

Corre solo lo de tu área mientras trabajas (`cargo test -p <crate>`, `npx vitest run <ruta>`) y
todo al final.
