# Importar y exportar

Qué archivos entran a Tlaku, con qué opciones, y en qué formatos puedes sacar tu diseño para llevarlo a otro programa, compartirlo o imprimirlo a tamaño real. Los atajos se escriben con `Ctrl/Cmd`: `Cmd` en macOS, `Ctrl` en Windows y Linux.

## Importar

Hay dos caminos:

- **Archivo > Importar diseño…** (`Ctrl/Cmd+I`) o **arrastrar** el archivo sobre la ventana: importa **sin preguntar**, con las opciones automáticas. Crea una capa por cada color, selecciona lo importado y ajusta la vista.
- **Archivo > Importar con opciones…**: igual, pero abre antes un diálogo con las opciones de abajo. Puedes elegir **varios archivos** a la vez.

Si el importador no pudo traer algo, aparece un **aviso persistente** que dice qué faltó: léelo antes de enviar nada a la máquina. Tlaku prefiere avisar a perder algo en silencio.

### Formatos que se importan

| Formato | Extensiones | Qué trae |
|---|---|---|
| SVG | `.svg` | Curvas Bézier, grupos, recortes y texto (con las fuentes instaladas). Una capa por color. |
| DXF | `.dxf` | Líneas, arcos, círculos, elipses, splines y bloques, con las unidades del archivo. El texto se ignora con aviso. |
| Imágenes | `.png`, `.jpg`, `.jpeg`, `.bmp`, `.webp`, `.tif`, `.tiff`, `.gif`, `.tga` | Una imagen en su propia capa de modo **Imagen** (ver [Imágenes](06-imagenes.md)). |
| PDF | `.pdf` | Los **trayectos** de una página, en milímetros, una capa por color. |
| Adobe Illustrator | `.ai` | Igual que un PDF, si se guardó con **Crear archivo compatible con PDF**. |
| HPGL (plotters y cortadoras de vinilo) | `.plt`, `.hpgl`, `.hpg` | Los trazos, **una capa por pluma**. |
| G-code | `.nc`, `.gcode`, `.gc`, `.ngc` | Los trazos con el láser encendido, como geometría. |

Notas por formato:

- **PDF y AI.** Solo se importan trayectos. **No** se importan el texto con fuentes (conviértelo a curvas en el programa de origen), las imágenes incrustadas, los degradados, los patrones ni los recortes; Tlaku lo avisa. Si el PDF tiene varias páginas, importa la primera y lo dice; elige otra en las opciones. Un AI antiguo (PostScript, anterior a Illustrator CS) no se puede leer: la app explica cómo convertirlo (guardarlo desde Illustrator con **Crear archivo compatible con PDF**, o exportarlo a PDF o SVG).
- **HPGL.** El formato no declara su escala; Tlaku supone **40 unidades por milímetro** (0,025 mm, lo habitual) y lo avisa. Si el tamaño no coincide, corrígelo con la **Escala** de las opciones de importación.
- **G-code.** Solo cuentan los movimientos que graban: `G1`, `G2` y `G3` con el láser encendido (`M3` o `M4` y `S` mayor que 0). Los viajes rápidos (`G0`) no se dibujan. De fábrica se hace **una capa por nivel de potencia** (hasta 8, de menor a mayor); elige **Una sola capa** en las opciones si prefieres una sola. Entiende `G90`/`G91`, `G20`/`G21` y arcos con `I J` o con `R`. Sirve para recuperar un diseño desde un G-code, no para reproducir los ajustes de velocidad del original.
- **Imágenes.** La ventana no sabe mostrar TIFF ni TGA: Tlaku los convierte a un PNG (`nombre.tlaku.png`) junto al original y el diseño usa esa copia, que debes conservar junto al proyecto; si esa carpeta no se puede escribir, la copia va a la carpeta temporal y hay que volver a importar la imagen al reiniciar. En ambos casos lo avisa. El tamaño sale de la resolución (96 ppp por omisión).
- **Proyectos de LightBurn** (`.lbrn`, `.lbrn2`). Se importan con sus capas y ajustes. Lo que trae y lo que avisa está en [Venir desde LightBurn](14-desde-lightburn.md).

### Opciones de importación

El diálogo **Opciones de importación** muestra solo las que tienen sentido para los archivos elegidos:

| Opción | Qué hace |
|---|---|
| **Escala (%)** | Multiplica todo el diseño. 100 lo deja igual. Útil para corregir un HPGL o un SVG de otro tamaño. |
| **Resolución (ppp) de SVG sin unidades e imágenes** | Cuántos píxeles son una pulgada cuando el archivo no lo dice. **Automática (96)**. Illustrator suele usar 72: si el tamaño sale distinto del original, prueba con 72. |
| **Unidad de un DXF sin unidades** | **Milímetros (aviso)**, **Pulgadas** o **Centímetros**. Solo cuenta si el DXF no declara sus unidades. |
| **Agrupar todo lo importado** | Une todo en un solo grupo para moverlo como una pieza. |
| **Capa destino de los vectores** | **Una capa por color (automático)** o una capa existente donde meter todos los vectores. |
| **Página del PDF o AI** | Qué página importar, desde 1. |
| **G-code** | **Una capa por potencia** o **Una sola capa**. |

**Importar** aplica las opciones a todos los archivos elegidos; **Cancelar** no importa nada.

## Exportar

Con algo visible en el diseño, el menú **Archivo** ofrece:

- **Exportar G-code…** (`Ctrl/Cmd+Shift+E`): el trabajo para la máquina. Ver [Enviar trabajos con seguridad](09-enviar-con-seguridad.md).
- **Exportar SVG…**, **Exportar DXF…**, **Exportar imagen PNG…** y **Exportar PDF…**: tu **diseño** en formatos abiertos. Sin objetos visibles, estos comandos están deshabilitados y dicen **No hay objetos que exportar**.

Los cuatro abren un diálogo; después **Exportar…** pide dónde guardar, y al terminar el aviso dice la ruta, el tamaño del dibujo en mm y el peso del archivo. Los objetos ocultos y los de capas ocultas **no** se exportan.

![Diálogo Exportar SVG con las opciones de exportación y la elección de exportar solo la selección](../../apps/desktop/docs/screenshots/30-exportar.png)

En los cuatro, **Qué exportar** deja elegir **Todo el diseño** o **Solo la selección** (con cuántos objetos). Un texto sale como trayectos; si todavía no tiene contorno generado se omite y se avisa.

| Formato | Opciones | Lo que conviene saber |
|---|---|---|
| **SVG** | **Imágenes** (**Incrustar**, **Enlazar** o **No incluir**), **Margen**, **Grosor del trazo**, **Conservar las coordenadas de la cama**, **Rellenar las formas de las capas de relleno** | Medidas reales en mm (1 unidad = 1 mm), curvas Bézier tal cual y **un grupo por capa**, con el color de la capa. Por omisión la página mide lo que el diseño; con **Conservar las coordenadas de la cama**, el origen de la cama es la esquina del SVG. |
| **DXF** | **Curvas** (**Curvas exactas (SPLINE)** o **Aplanar a polilíneas**), **Tolerancia**, **Guardar el color exacto (DXF 2004)** | En mm, con **una capa por color**. Los trayectos de solo curvas salen como SPLINE exactos; los que mezclan rectas y curvas, como una polilínea aplanada. Sin el color exacto, queda el color ACI más cercano. |
| **PNG** | **Resolución (ppp)**, **Margen**, **Grosor del trazo**, **Fondo transparente**, **Rellenar las formas de las capas de relleno** | El diseño dibujado con el color de cada capa. 300 ppp por omisión; hay un tope de tamaño para no agotar la memoria con un valor absurdo. |
| **PDF** | **Hoja** (**Ajustar al diseño**, **A4**, **A3**, **Carta**, **Oficio**), **Margen**, **Grosor del trazo**, **Incluir las imágenes**, **Rellenar las formas de las capas de relleno** | PDF **vectorial a escala 1:1**: 1 mm del diseño es 1 mm en el papel. |

> **Consejo:** el SVG y el DXF son la salida para llevar piezas a otros programas o a otra cortadora, y la mejor forma de no depender de un solo programa. El PDF se abre en Illustrator y en casi cualquier editor vectorial; no hay exportación a `.ai`.

### Imprimir

**Archivo > Imprimir…** (`Ctrl/Cmd+P`) abre el mismo diálogo que **Exportar PDF…** y, al aceptar, genera el PDF a escala 1:1 y lo **abre en el visor del sistema**. Desde ahí imprímelo con **tamaño real (100 %)** y sin **ajustar a la página**; si no, la plantilla sale a otra escala. Tlaku no imprime directamente a la impresora.

### Copiar y pegar entre programas

**Copiar** y **Cortar** ponen la selección en el portapapeles del sistema como SVG; **Editar > Copiar como SVG** (`Ctrl/Cmd+Alt+C`) hace lo mismo y avisa que lo hizo. **Pegar** (`Ctrl/Cmd+V`) lee el portapapeles del sistema:

- Un **SVG** de otro programa (Inkscape, Illustrator y similares) entra como dibujo; se avisa si se omitieron elementos que no son formas (texto, imágenes, referencias).
- Un **texto** cualquiera entra como un objeto de texto.
- Lo que copió Tlaku mismo se pega desde su portapapeles interno y **conserva** capas, textos editables y formas.

Si el sistema bloquea el acceso al portapapeles, Tlaku lo dice y sigue funcionando con su portapapeles interno.

Siguiente: [Texto](04-texto.md)
