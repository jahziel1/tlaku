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
   (o usa *Compartir…* sobre un ajuste propio en el panel Materiales de la app, que abre el formulario ya
   lleno). Confirma que probó los valores y que autoriza publicarlos con licencia CC0.
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
4. **Biblioteca.** Al integrarse, el archivo se compila dentro de la app (`crates/tlaku-doc/build.rs`) y
   aparece en el panel Materiales con la etiqueta *Comunidad (sin verificar)*, junto a los ajustes de fábrica.

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

El validador (`cargo test -p tlaku-doc community`) comprueba campos, unidades, rangos razonables, ids
únicos, la dirección del issue y la licencia. Un archivo que no lo pasa no se puede integrar, porque la
CI corre la misma prueba.

## Licencia

Los valores de esta carpeta los publican sus autores con
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) (dominio público), para que la biblioteca
pueda redistribuirse con Tlaku. El código del proyecto sigue siendo MIT.

## Retirar un aporte

Abre un issue o pídelo en el original. Borrar el archivo lo quita de la siguiente versión (las copias ya
distribuidas no se pueden recoger, por eso se exige CC0).
