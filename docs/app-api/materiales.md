# Contrato app-motor: Biblioteca de materiales

Parte de [el contrato app-motor](README.md). Comandos y contrato de la biblioteca de materiales y la prueba de material.

Comandos nuevos:

| Comando | Argumentos | Resultado |
|---|---|---|
| `load_material_library` | — | `MaterialLibrary` (vacía si no hay archivo) |
| `load_factory_materials` | — | `MaterialLibrary` de fábrica (solo lectura; instala o actualiza su copia `materials-factory.json`, nunca toca `materials.json`) |
| `save_material_library` | `library` | — |
| `import_material_library` | `path` | `MaterialLibrary` del archivo `.tlakumat` |
| `export_material_library` | `path`, `library` | — |
| `material_test` | `params: MaterialTestParams` | `Document` con la cuadrícula |

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
