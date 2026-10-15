# Ajustes de material de la comunidad

[English](README.md) · Español

Ajustes de material compartidos por personas que los probaron en sus propias máquinas. Cada archivo de
esta carpeta es **un aporte** (un issue de GitHub) y termina en la biblioteca de materiales de la app,
con la etiqueta **Comunidad (sin verificar)**.

> Son los valores de una persona en una máquina. Los láseres, los lentes, el enfoque, el aire y los
> materiales varían. **Haz siempre una prueba en material de desecho**, empezando con menos potencia
> que la indicada.

## Cómo entra un ajuste a la biblioteca

1. **Issue.** Alguien abre el formulario [Compartir ajustes de material](https://github.com/jahziel1/tlaku/issues/new?template=material_settings.yml)
   (o usa *Compartir este ajuste…* sobre un ajuste propio en el panel Materiales de la app, o *Compartir
   un ajuste mío…* en el navegador de la comunidad; los dos abren el formulario ya lleno y no se envía nada
   hasta que la persona lo publica en GitHub). Confirma que probó los valores y que autoriza publicarlos con licencia CC0.
2. **Revisión.** Una persona del proyecto comprueba que los valores tienen sentido para la máquina y el
   material, que la descripción y la foto del resultado coinciden y que no se sugiere nada inseguro (por
   ejemplo, un corte que pide más potencia de la que da la máquina, o un material que no debe cortarse con láser).
3. **Pull request.** Esa persona genera el archivo a partir del issue y abre un pull request:

   ```sh
   node scripts/community/issue-to-preset.mjs 42        # lee el issue #42 con `gh` y escribe community/materials/42.json
   cargo test -p tlaku-doc community                     # valida todos los archivos de esta carpeta
   ```

   El script convierte mm/s a mm/min si quien aportó escribió la unidad, elige la clase de máquina según el
   tipo de láser y los watts, y guarda la dirección del issue como `source`.
4. **Biblioteca y catálogo.** Al integrarse, el archivo llega a la gente de dos maneras:
   - Se compila dentro de la siguiente versión de la app (`crates/tlaku-doc/build.rs`) y aparece en el
     panel Materiales con la etiqueta *Comunidad (sin verificar)*, junto a los ajustes de fábrica.
   - El flujo **Sitio** (`.github/workflows/pages.yml`) compila todos los archivos de esta carpeta en el
     catálogo publicado `https://jahziel1.github.io/tlaku/catalog/v1/index.json`, **sin esperar una versión
     nueva de la app**. La app lo descarga solo cuando la persona abre *Explorar la comunidad…* en el
     panel Materiales (ver [el catálogo publicado](#el-catálogo-publicado)).

## Revisar un aporte (quien mantiene el proyecto)

Nada entra a la biblioteca sin que una persona lo lea. Antes de abrir el pull request:

- [ ] Los valores tienen sentido para la máquina y la potencia del láser (el validador también comprueba
      la clase contra los watts). Se rechaza un corte que pide más potencia de la que da la máquina.
- [ ] La descripción del resultado y la foto coinciden con los valores.
- [ ] No se sugiere nada inseguro (materiales que no deben cortarse con láser: PVC y vinilo, cuero curtido
      al cromo, fibra de vidrio con recubrimiento...; ninguna instrucción de saltarse la prueba).
- [ ] El texto son pruebas propias de quien aporta, no una tabla copiada de un manual o sitio con derechos de autor.
- [ ] `notes` no lleva enlaces, publicidad ni datos personales (la app se lo muestra a todo el mundo).
- [ ] `machine_model` se pone **solo** si la máquina que escribió la persona nombra con claridad un modelo del
      catálogo de máquinas de la app (`apps/desktop/src/features/devices/catalog*.ts`): usa su `id`
      (`--machine-model creality-falcon2-22w`). Si hay duda, se omite: la clase igual se compara.
- [ ] No existe ya un aporte casi idéntico (mismo material, grosor, operación y clase de máquina).

El pull request agrega `community/materials/<issue>.json` y nada más. La CI lo valida
(`cargo test -p tlaku-doc community` y la compilación del catálogo).

### Confirmaciones («verificado por N personas»)

Una confirmación es **otra** persona que dice haber probado los mismos valores en su máquina.

1. La persona comenta en el issue original: máquina, potencia del láser, material y qué salió
   (una foto ayuda). Una reacción (pulgar arriba) no cuenta.
2. Quien mantiene el proyecto lee el comentario y agrega una entrada a `confirmations` del archivo de ese
   aporte, en un pull request:

   ```json
   "confirmations": [
     { "by": "usuario-de-github", "machine": "Máquina como la escribió", "machine_class": "diode_20",
       "date": "2026-10-20", "source": "https://github.com/jahziel1/tlaku/issues/42#issuecomment-123456" }
   ]
   ```

3. El validador rechaza una confirmación de la propia persona que aportó y la misma persona dos veces. El
   catálogo cuenta las entradas (`verified_by`) y la app muestra *Verificado por N personas* con la lista.

«Verificado» significa *otras personas dicen que les funcionó*. No es una certificación de seguridad: el
aviso de probar primero en un retazo se muestra siempre.

### «Misma máquina» se declara, no se demuestra

Nadie puede demostrar qué máquina tiene otra persona. Lo que el catálogo guarda (`machine`, `laser_watts`,
`machine_class` y `machine_model` cuando quien revisa pudo asociar el texto con el catálogo de la app) es lo
que la persona **declaró**. La app compara tu máquina activa con eso (mismo modelo, misma clase, mismo tipo
de láser con otra potencia, otro láser) y lo dice; todo lo que no sea el mismo modelo se muestra como «solo
orientativo». Incluso el mismo modelo puede variar en lente, enfoque, aire y material.

## El catálogo publicado

`catalog/v1/index.json` lo genera `cargo run -p tlaku-doc --example community_catalog -- --out <archivo>`
(lo hace el flujo Sitio; la CI lo comprueba en cada cambio). Formato (versión 1 del esquema):

```json
{
  "schema": "tlaku.community-materials",
  "version": 1,
  "generated_at": "2026-10-15",
  "entries": [ /* un objeto por aporte, ordenados por id */ ]
}
```

Cada entrada tiene los campos del archivo de aporte (abajo) más `operation` calculada (`cut`, `score`,
`engrave`, `fill` o `image`, a partir de `role` y `mode`), el `mode` efectivo (`line`, `fill` o `image`) y
`verified_by`. Los tipos de Rust son `CatalogIndex`/`CatalogEntry` en
`crates/tlaku-doc/src/community_catalog.rs` (de ahí se generan los tipos de TypeScript). La versión va en la
ruta: un cambio que rompa a las apps viejas se publica como `catalog/v2/` y `v1/` se queda como está. Los
campos que se agreguen después a `v1` son opcionales y las apps viejas los ignoran.

La app **valida el catálogo antes de mostrarlo** y lo rechaza entero ante el primer problema: tamaño de
hasta 4 MiB, hasta 5000 entradas, potencia mayor que 0 y hasta 100 %, velocidades positivas y acotadas,
pasadas, grosor, intervalo y frecuencia en rango, clase del láser contra watts, licencia CC0, dirección del
issue y coherencia de los campos calculados. Un catálogo son solo datos: nada de él se ejecuta y aplicar un
ajuste pasa siempre por la biblioteca o por una confirmación explícita para cambiar una capa. Nunca envía
nada a la máquina. Las reglas de red están en `docs/app-api/materiales.md`.

**Todavía no hay aportes**, así que el catálogo se publica vacío y la app lo dice e invita a compartir uno.
Ejemplo de archivo (**inventado para la documentación; no es un ajuste real y no debe usarse como tal**):

```json
{
  "id": "community.basswood.3.cut.diode_20.999",
  "material": "basswood",
  "thickness": 3,
  "role": "cut",
  "machine_class": "diode_20",
  "laser_watts": 22,
  "machine": "Máquina de EJEMPLO 22 W",
  "speed": 500,
  "power": 100,
  "passes": 2,
  "air": true,
  "notes": "EJEMPLO, no es un ajuste real.",
  "source": "https://github.com/jahziel1/tlaku/issues/999",
  "confidence": "forum",
  "license": "CC0-1.0"
}
```

## Formato del archivo

Un objeto JSON por archivo, llamado `<número del issue>.json`. Usa el mismo formato compacto que
[`crates/tlaku-doc/data/materials-default.json`](../../crates/tlaku-doc/data/materials-default.json),
más campos de procedencia:

| Campo | Significado |
|---|---|
| `id` | `community.<material>.<grosor>.<operación>.<clase>.<issue>`; único |
| `material`, `material_name` | Clave del material (`basswood`, `mdf`...). Si la clave no está en la biblioteca de fábrica, `material_name` trae el nombre |
| `thickness` | mm; se omite al grabar la superficie |
| `role` | `cut`, `engrave` o `score` |
| `machine_class`, `laser_watts` | Tipo y clase de potencia del láser (`diode_20`...) y los watts ópticos reales; deben coincidir |
| `machine` | Máquina en la que se probaron los valores |
| `speed` | **mm/min** |
| `power` | De 1 a 100 % |
| `passes`, `interval`, `air`, `frequency_khz` | Datos opcionales (`interval` en mm) |
| `notes` | Resultado y enfoque, texto libre (hasta 400 caracteres) |
| `source` | Dirección del issue: `https://github.com/jahziel1/tlaku/issues/<número>` |
| `confidence` | Siempre `forum` (comunidad, sin verificar) |
| `license` | Siempre `CC0-1.0` |
| `contributor` | Usuario de GitHub (opcional) |
| `date` | Día en que se abrió el issue, `AAAA-MM-DD` (opcional; lo llena el script) |
| `tlaku_version` | Versión de Tlaku con la que se probó, `0.1.0` (opcional; lo llena el formulario) |
| `machine_model` | `id` del modelo en el catálogo de máquinas de la app (opcional; lo pone quien revisa, declarado) |
| `confirmations` | Personas que lo confirmaron, ver arriba (opcional; las agrega quien revisa) |

El validador (`cargo test -p tlaku-doc community`) comprueba campos, unidades, rangos razonables, ids
únicos, la dirección del issue, la licencia y las confirmaciones. Un archivo que no lo pasa no se puede integrar, porque la
CI corre la misma prueba.

## Licencia

Los valores de esta carpeta los publican sus autores con
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) (dominio público), para que la biblioteca
pueda redistribuirse con Tlaku. El código del proyecto sigue siendo MIT.

## Retirar un aporte

Abre un issue o pídelo en el original. Borrar el archivo lo quita de la siguiente versión (las copias ya
distribuidas no se pueden recoger, por eso se exige CC0).
