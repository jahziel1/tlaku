# Contrato app-motor: Biblioteca de materiales

Parte de [el contrato app-motor](README.md). Comandos y contrato de la biblioteca de materiales, la prueba de material y el catálogo de la comunidad.

Comandos nuevos:

| Comando | Argumentos | Resultado |
|---|---|---|
| `load_material_library` | — | `MaterialLibrary` (vacía si no hay archivo) |
| `load_factory_materials` | — | `MaterialLibrary` de fábrica (solo lectura; instala o actualiza su copia `materials-factory.json`, nunca toca `materials.json`) |
| `save_material_library` | `library` | — |
| `import_material_library` | `path` | `MaterialLibrary` del archivo `.tlakumat` |
| `export_material_library` | `path`, `library` | — |
| `material_test` | `params: MaterialTestParams` | `Document` con la cuadrícula |
| `community_catalog_cached` | — | `CatalogState` con la última copia guardada del catálogo de la comunidad. **No usa la red** |
| `community_catalog_refresh` | — | `CatalogState` tras bajar el catálogo del sitio del proyecto (único comando con red de esta área) |

Contrato de la biblioteca de materiales (ola 5.5): cada `MaterialPreset` puede traer, todos
opcionales para no romper bibliotecas guardadas, `role` (`cut`/`engrave`/`score`), `machine_class`
(`diode_5|10|20|40`, `co2_40|60|80|100`, `fiber_20|30|50`), `source` (dirección https de donde salen
los valores), `confidence` (`official`/`guide`/`forum`) y `warnings`. Los datos de fábrica viven en
`crates/tlaku-doc/data/materials-default.json` (formato compacto, validado por `check_factory`: todo
ajuste exige fuente y confianza) y `tlaku_doc::factory_library()` los expande. La interfaz elige el
ajuste de una máquina con `pickPreset` (`features/materials/pick.ts`): primero el de la máquina,
luego el de su clase, luego uno sin clase y al final la clase más cercana por potencia del mismo
tipo de láser, avisando que hay que probar.

La biblioteca vive en `materials.json` dentro de la carpeta de configuración de la app (escritura
atómica; un archivo dañado se aparta como `materials.json.dañado`, uno de versión desconocida da
error sin tocarlo). Los archivos `.tlakumat` son el mismo JSON de `MaterialLibrary`.

Curvas de tono (vista previa «por tonos», mejora 8): `MaterialLibrary.tones` (opcional, solo en la
biblioteca del usuario) es una lista de `ToneCalibration { material, curve: ToneCurve, samples }`, una
por material de la vista por tonos (`light_wood`, `mdf`, `plywood`, `leather`, `acrylic`, `cardboard`,
`paper`, `anodized`, `custom`; los valores de partida están en `crates/tlaku-doc/data/tone-presets.json`).
`save_material_library` y las importaciones validan cada curva (`ToneCalibration::validate`: dosis
característica > 0, gamma entre 0,2 y 4, muestras finitas). Ver `tonal_preview` en
[conexion-y-trabajo.md](conexion-y-trabajo.md).

## Catálogo de ajustes de la comunidad (mejora 5)

Sin servidor propio: el sitio estático del proyecto (GitHub Pages) publica `catalog/v1/index.json`, que el
flujo `pages.yml` compila desde `community/materials/*.json` con
`cargo run -p tlaku-doc --example community_catalog` (el mismo validador que usa la app). El formato, el
proceso de revisión y las confirmaciones están en [community/materials/README.es.md](../../community/materials/README.es.md).

Tipos (generados con ts-rs desde `crates/tlaku-doc/src/community_catalog.rs`): `CatalogIndex`
(`schema` = `tlaku.community-materials`, `version` = 1, `generated_at` `AAAA-MM-DD`, `entries`),
`CatalogEntry` (los campos del aporte más `operation`, `mode` y `verified_by` calculados; `Confirmation`
para cada confirmación), `CatalogState` y `CatalogSource`.

`CatalogState`: `{ index: CatalogIndex | null, source: 'network' | 'cache' | 'none', fetched_at: number | null (ms desde 1970), error: string | null }`.
Con una copia buena el índice se devuelve aunque falle la descarga (`source: 'cache'` y el motivo en `error`);
`source: 'none'` es «no hay copia ni se pudo bajar».

**Red (lo único que sale de la app en esta área).** Todo está en `src-tauri/src/community_catalog.rs`:

- Destino **fijo** (`https://jahziel1.github.io/tlaku/catalog/v1/index.json`): los comandos no reciben ninguna
  dirección. Solo `GET`, solo HTTPS, sin cookies ni `Referer`, con `User-Agent: Tlaku/<versión> (catalog)`;
  no se manda ningún identificador ni dato del usuario, y el sitio solo ve la IP. Redirecciones solo al mismo
  sitio (máx. 3), conexión de 10 s y total de 20 s, tope de 4 MiB aplicado mientras llega la respuesta.
- La interfaz **sigue sin poder hacer `fetch`** (CSP `connect-src 'self' ipc:`): no se amplió la CSP ni el
  alcance de `opener`. Se usa `reqwest` 0.13 con rustls (`ring`) y las raíces del sistema, las mismas versiones y
  funciones que ya trae el actualizador, así que no suma dependencias nuevas al árbol (`cargo deny` pasa igual).
- **Solo a pedido:** `community_catalog_refresh` lo llama la interfaz al abrir «Explorar la comunidad…» o al pulsar
  «Actualizar». Nunca al arrancar ni en segundo plano. Si ya hay una caché se manda `If-None-Match` (el sitio
  contesta 304 y solo se renueva la fecha).
- **Validación antes de mostrar o guardar** (`tlaku_doc::validate_index`): tamaño, versión mayor conocida (otra
  da «actualiza Tlaku»), hasta 5000 entradas, y por entrada las reglas del aporte (potencia > 0 y ≤ 100 %,
  velocidad positiva y acotada por tipo de láser, clase contra watts, pasadas, grosor, intervalo y frecuencia en
  rango, licencia CC0, dirección del issue, confirmaciones sin repetir ni de quien aportó) y la coherencia de
  `operation`/`verified_by`. Un solo error descarta todo y se conserva la última copia buena. Los campos
  desconocidos de la misma versión se ignoran (compatibilidad hacia adelante).
- **Caché:** `community-catalog.json` en la carpeta de caché de la app (`{ fetched_at, etag, index }`, escritura
  atómica); se revalida al leerla, así que un archivo dañado o alterado se ignora.
- El catálogo son solo datos. Nada se ejecuta ni se envía a la máquina: «Usar» copia un ajuste a la
  biblioteca local (`save_material_library`), «Aplicar a la capa» pasa por una pregunta de confirmación y un
  paso de deshacer, y compartir solo abre el formulario de GitHub en el navegador (alcance de `opener` sin cambios).
- Limitación conocida: no hay firma criptográfica del índice (el plan evaluaba minisign). La integridad descansa
  en HTTPS hacia GitHub Pages, en la validación de arriba y en que nada del catálogo actúa solo.
