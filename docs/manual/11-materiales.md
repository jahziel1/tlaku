# Materiales

La biblioteca guarda los ajustes que ya sabes que funcionan para cada material, y la prueba de materiales te ayuda a descubrirlos con una cuadrícula de celdas. Así no vuelves a adivinar velocidad y potencia cada vez.

> **Ojo:** la biblioteca empieza vacía; Tlaku no trae valores de fábrica para ningún material. Los ajustes dependen de tu máquina, tu láser y tu material: encuéntralos con la prueba y guárdalos tú.

## La pestaña Materiales

Abre la pestaña **Materiales** (`Ctrl/Cmd+6`). Arriba están las acciones (**Guardar capa como material…**, **Importar…**, **Exportar…** y **Prueba de materiales…**) y debajo la lista, con un buscador.

![Biblioteca de materiales agrupada por material y grosor](../../apps/desktop/docs/screenshots/19-materiales.png)

La lista se agrupa por material (sin distinguir mayúsculas ni acentos) y, dentro, por grosor, de menor a mayor; los que no tienen grosor van al final. Cada ajuste muestra su operación (por ejemplo "Corte"), un resumen como `250 mm/min · 100 %`, su modo (**Línea**, **Relleno** o **Imagen**), sus notas y etiquetas:

- La máquina para la que se probó, si lo indicaste.
- **En uso: N**, si N capas del documento tienen exactamente esos ajustes.

El buscador (**Buscar**) filtra por material, operación, notas o grosor. Marca **Solo esta máquina** para ver los ajustes de la máquina activa y los que valen para cualquiera.

## Guardar un ajuste

1. En la pestaña **Capas**, deja la capa con los ajustes que te funcionaron y selecciónala. Sin capa activa, el botón avisa "Selecciona una capa en la pestaña Capas".
2. En **Materiales**, pulsa **Guardar capa como material…**.
3. Rellena **Material** (por ejemplo "Contrachapado de abedul"), **Operación** (por ejemplo "Corte"), **Grosor** (opcional; vacío para grabados) y **Notas**. En **Máquina** eliges **Cualquier máquina** o una en concreto.
4. **Guardar**. Se copian todos los ajustes de la capa.

Cada fila tiene iconos para **Editar**, **Duplicar** y **Borrar** (que pide confirmación y no se puede deshacer). Al editar puedes actualizar los ajustes con los de la capa activa.

## Aplicar un ajuste

1. Selecciona la capa en la pestaña **Capas**.
2. En la fila del material, pulsa **Aplicar a la capa**.

Se copian todos los ajustes del material a la capa, menos su color, su nombre, si está visible, si tiene salida y su posición en el orden. Se puede deshacer (aparece como **Aplicar material a la capa** en el historial). Si la capa ya tiene esos ajustes, el botón queda desactivado. Si el material trae ajustes de Z pero tu máquina activa no tiene eje Z, la app avisa que el trabajo los ignorará.

## Prueba de materiales

Una cuadrícula de cuadrados donde cada celda es una capa con su propia combinación de ajustes. La grabas sobre un retazo y eliges la celda que mejor quedó. Ábrela desde **Herramientas > Prueba de materiales…** o desde la pestaña **Materiales**.

![Diálogo de prueba de materiales con la vista previa de la cuadrícula](../../apps/desktop/docs/screenshots/20-prueba-materiales.png)

1. **Modo de las celdas**: **Línea** o **Relleno**.
2. En **Filas** y **Columnas** elige qué **Parámetro** varía en cada eje: **Velocidad**, **Potencia**, **Pasadas** o **Intervalo**. Deben ser parámetros distintos, y el intervalo solo existe en relleno. Indica la **Cantidad** (de 1 a 30) y el rango **Desde** y **Hasta**. Por defecto, las filas varían la potencia (de 10 a 100 %) y las columnas la velocidad (de 500 a 3000 mm/min).
3. En **Celdas y rótulos** fija el tamaño (por defecto 10 × 10 mm) y la **Separación** (3 mm). Con **Rotular los valores de filas y columnas** se graban los números junto a la cuadrícula, con su propia **Velocidad de rótulos** y **Potencia de rótulos**.
4. En **Valores fijos** pones lo que no varía. Marca **Partir de los ajustes de la capa** para heredar el resto (aire, ángulo, overscan...) de la capa activa.
5. Revisa la vista previa: abajo ves el número de celdas, el tamaño total y el **Tiempo estimado**. Si la cuadrícula no cabe en la cama de la máquina activa, el diálogo lo dice.
6. Pulsa **Crear documento de prueba**. Se abre como documento nuevo (si tienes cambios sin guardar, pregunta antes). Envíalo como cualquier otro trabajo: [Enviar trabajos con seguridad](09-enviar-con-seguridad.md).

> **Consejo:** empieza con un rango amplio y pocas celdas, y después repite con un rango estrecho alrededor de la mejor celda. Anota el resultado con **Guardar capa como material…**. Haz la prueba siempre con tu máquina real y material de desecho; en el emulador no se graba nada.

## Importar y exportar

La biblioteca se guarda sola en tu equipo, en un archivo `materials.json` dentro de la carpeta de configuración de la app. Si el archivo está dañado, la app lo aparta (como `materials.json.dañado`) y no guarda nada hasta que reintentes, para no pisarlo.

- **Exportar…** crea un archivo `.tlakumat` (por defecto `materiales.tlakumat`) con todos tus ajustes. Está desactivado si la biblioteca está vacía.
- **Importar…** agrega los ajustes de un `.tlakumat`. Lo que ya tienes no se pisa: un ajuste idéntico a uno existente se omite, así que importar el mismo archivo dos veces no duplica nada; si el identificador choca con otro ajuste distinto, entra como copia con identificador nuevo. Al terminar dice cuántos entraron, cuántos cambiaron de id y cuántos ya estaban.

Los `.tlakumat` sirven para compartir tus ajustes con otras personas o pasarlos a otro equipo. No se importan bibliotecas de otros programas.

Siguiente: [Rotativo y eje Z](12-rotativo-y-eje-z.md)
