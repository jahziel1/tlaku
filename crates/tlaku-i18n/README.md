# tlaku-i18n: mensajes del motor en español e inglés

Los avisos y errores que el motor (Rust) le muestra al usuario viven en catálogos de texto, no en
`format!`. La interfaz (`apps/desktop/src/i18n`) ya tiene los suyos; esto es solo para el motor.

## Cómo funciona

- Catálogo: `locales/{es,en}/<crate>.txt`, un archivo por crate de origen. `<crate>` es el nombre sin
  `tlaku-` (`pipeline`, `grbl`, `cli`...); `apps/desktop/src-tauri` es `desktop`. Ya existen vacíos para
  todos los crates y están registrados en `CATALOGS` (`src/catalog.rs`); un crate nuevo agrega una línea ahí.
- Uso: `tr!("pipeline.import_hpgl.no_strokes", name = path.display())` devuelve un `String` en el
  idioma activo. El crate que lo usa agrega `tlaku-i18n = { version = "0.1.0", path = "../tlaku-i18n" }`
  y `use tlaku_i18n::tr;`.
- Idioma activo: global (`set_locale(Locale::En)`, lo pone la app con el comando `set_engine_locale`)
  o solo del hilo (`with_locale(Locale::En, || ...)`, para pruebas). La CLI lo toma de `TLAKU_LANG` o
  `LANG`. Por omisión, español.
- Si falta una clave en un idioma se usa el otro; si falta en ambos sale la clave. Nunca hay pánico.
  `take_fallbacks()` devuelve lo que cayó en el respaldo.

## Formato del catálogo

```text
# comentario
pipeline.import_hpgl.no_strokes = {name}: el archivo HPGL no contiene trazos
pipeline.import_rd.variable_power = La capa {n} varía su potencia: se usó el máximo ({pct} %).
pipeline.import_lbrn.text_note_one = Texto: {note} (en 1 texto)
```

- Una línea por mensaje: `clave = plantilla`. Una línea que empieza con espacios continúa la anterior
  (se une con un espacio).
- Marcadores con nombre: `{capa}`, `{max}`. Mismos nombres en es y en. `{{` y `}}` son llaves literales;
  `\n` es un salto de línea.
- Sin espacios al inicio ni al final del texto: el texto va completo, no en fragmentos.
- Agrupa por archivo fuente con un comentario (`# import_hpgl.rs`).

## Claves

`<crate>.<módulo>.<qué>`, todo en minúsculas y con `_`:

- `pipeline.import_hpgl.no_strokes`, `grbl.stream.timeout`, `cli.gcode.missing_output`.
- `<módulo>` es el archivo o carpeta fuente (`import_hpgl`, `stream`), no la función.
- `<qué>` dice qué pasó, no el texto (`no_strokes`, no `el_archivo_no_contiene_trazos`).
- Plural con palabras que cambian: dos claves, `..._one` y `..._other`, y el código elige con
  `if n == 1`. Si basta con `(s)` ("{n} capa(s)"), úsalo y ahórrate el par.

## Convertir un `format!`

```rust
// Antes
return Err(format!("{name}: el .rd no contiene cortes (no hay A8/A9/AA/AB con el láser encendido)"));
// Después
return Err(tr!("pipeline.import_rd.no_cuts", name = name));
```

```text
# locales/es/pipeline.txt
pipeline.import_rd.no_cuts = {name}: el .rd no contiene cortes (no hay A8/A9/AA/AB con el láser encendido)
# locales/en/pipeline.txt
pipeline.import_rd.no_cuts = {name}: the .rd file contains no cuts (there is no A8/A9/AA/AB with the laser on)
```

- Cada argumento nombrado se escribe con `Display` (`path.display()`, números, `String`).
- Un formato con precisión se calcula antes: `pct = format!("{pct:.0}")`.
- Texto armado con `if`/`join`/fragmentos (`" (solo trae un grabado...)"`): conviértelo en mensajes
  completos (`no_cut` y `no_cut_raster`) o en claves aparte para cada trozo (`quoted_name`, `and_more`).
  Nunca concatenes frases traducidas: el orden de las palabras cambia entre idiomas.
- El texto en español debe quedar **idéntico** al de antes (las pruebas lo comparan). Una cadena con
  `\` al final de línea en Rust se une sin espacios sobrantes: copia el resultado, no los saltos.
- Comillas: en español `«...»`, en inglés `"..."`.
- Si el mensaje es una lista de errores o avisos, cada elemento es su propia clave.

## Qué no traducir

- `panic!`, `expect`, `unreachable!`, `debug_assert!`, `log`/`debug` y mensajes de pruebas.
- Errores de otras bibliotecas (`{e}` de `std::io`, `serde`...): se pasan como argumento tal cual.
- Nombres de capa, perfiles o archivos que son identificadores (`K40 1`, `C01`, `{stem}_imagenes`).
- Texto que no es para el usuario: G-code, claves de protocolo, nombres de campos, JSON, comentarios.
- Formatos sin palabras propias (`format!("{}: {e}", path.display())`): no hay nada que traducir.
- Lo que el usuario ve y **sí** se traduce: avisos, errores devueltos (`Err(String)`), nombres de
  estados que llegan a la interfaz como texto, salida de la CLI.

## Errores que la interfaz reconoce

La interfaz **nunca** compara el texto de un error del motor (cambia con el idioma). Si necesita
reconocer uno, el error lleva un código estable:

```rust
Err(tlaku_i18n::coded("gcode_zero_power", tr!("pipeline.import_gcode.zero_power", name = n, n = k)))
```

El mensaje sale como `[gcode_zero_power] texto`. En la interfaz: `errorCode(e) === ENGINE_ERROR.x`
(`src/lib/errors.ts`; agrega el código a `ENGINE_ERROR`); `errorMessage(e)` ya quita el prefijo. En
Rust, `split_code` separa ambos. Entre crates, para reconocer un aviso ajeno sin depender del idioma,
`msg_is(mensaje, "otro.modulo.clave")`.

Pendientes de este tipo (texto de otro crate que alguien compara): busca `i18n-acoplado` en el código.

## Cómo escribir el inglés

- Claro y directo, para quien usa un láser: "Move the design inside the bed", no una traducción palabra
  por palabra. Usa la palabra que usa el oficio: *bed* (cama), *kerf*, *overscan*, *layer*, *tabs*
  (puentes), *air assist*, *home*, *jog*, *frame*.
- Mismo tono que el español (impersonal o de tú -> "you"/imperativo). Frases completas con punto final
  si el español lo lleva.
- No traduzcas nombres propios ni comandos (G-code, M3, HPGL, LightBurn, Ruida, K40, `S`, `PD`).
- Decimales con punto ("0.025 mm"); unidades igual que en español.
- El inglés no puede ser igual al español (una prueba lo exige). Si es un nombre propio o un código,
  agrega la clave a `locales/identical_ok.txt`.

## Pruebas

```sh
cargo test -p tlaku-i18n            # catálogo: claves en es y en, marcadores, duplicados, claves usadas
cargo test -p tlaku-<crate>         # las del crate que conviertes (siguen en español por omisión)
```

`tests/catalog.rs` comprueba: toda clave existe en es y en, los marcadores coinciden, el inglés difiere
del español (salvo `identical_ok.txt`), no hay claves duplicadas ni mal formadas (`<crate>.<módulo>.<qué>`,
prefijo igual al archivo), toda clave de un `tr!("...")` del workspace está en el catálogo y toda clave
del catálogo se usa en alguna parte. La clave del `tr!` tiene que ser un literal (así la encuentra el escaneo).

Para probar el inglés de lo que conviertes, sin tocar el idioma global (las pruebas corren en paralelo):

```rust
let err = tlaku_i18n::with_locale(tlaku_i18n::Locale::En, || import_file(&path).unwrap_err());
assert!(err.contains("contains no strokes"));
```

Ejemplo completo: `crates/tlaku-pipeline/tests/import_i18n.rs`.

## Lista de verificación por crate

1. `Cargo.toml`: dependencia `tlaku-i18n`; en el archivo, `use tlaku_i18n::tr;`.
2. Convierte los mensajes (es idéntico al original) y escribe el inglés en `locales/en/<crate>.txt`.
3. `cargo test -p tlaku-i18n -p tlaku-<crate>`; `cargo clippy -p tlaku-<crate> --all-targets -- -D warnings`.
4. Busca textos que otra parte compare (`grep -rn "starts_with(\"\|contains(\"" ...`, y en la interfaz
   `grep -rn "includes(\|\.test(" apps/desktop/src`) y cámbialos por código o `msg_is`.
5. Los comandos de Tauri que devuelven `Err(String)` ya llevan el idioma activo: no hace falta más.
