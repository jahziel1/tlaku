# Texto variable

Para grabar números de serie, nombres de una lista o la fecha del día sin escribir cada pieza a mano, un texto puede llevar **variables** entre llaves. El texto que guardas en el proyecto es la **plantilla** (`Pieza {serie:4}`); al previsualizar, enviar o exportar, Tlaku la cambia por los valores de ese momento (`Pieza 0007`) y genera el contorno de las letras con ellos. El proyecto guardado conserva siempre la plantilla.

## Escribir variables

Selecciona un texto y usa el menú **Insertar variable** del panel de texto: pone la variable donde está el cursor. También puedes escribirlas a mano.

| Variable | Qué pone |
|---|---|
| `{serie}` (o `{serial}`) | El número de serie actual. `{serie:6}` lo rellena con ceros hasta 6 dígitos; `{serie:hex}` y `{serie:HEX}` lo escriben en hexadecimal (`{serie:hex4}` con 4 dígitos). |
| `{csv:Nombre}` | El dato de la columna **Nombre** en la fila actual del CSV. También se acepta el número de columna desde 1: `{csv:2}`. Con varios CSV, `{csv.fuente:Nombre}` elige la fuente (ver más abajo). |
| `{fila}` (o `{row}`) | El número de la fila actual del CSV, desde 1. `{fila.fuente}` es la que le toca a esa fuente. |
| `{fecha}` (o `{date}`) | La fecha de hoy, `dd/MM/yyyy` de fábrica. `{fecha:formato}` la escribe a tu gusto. |
| `{hora}` (o `{time}`) | La hora, `HH:mm` de fábrica. `{hora:formato}` cambia el formato. |
| `{copia}` (o `{copy}`) | El número de copia, desde 1. |

Los nombres valen en español y en inglés, y no importan las mayúsculas.

**Desplazamiento.** Cualquier variable de serie, CSV, fila o copia admite `@N` al final: `{csv:Nombre@2}` es el dato de dos filas más adelante, `{serie@3}` el número tres pasos adelante. Es lo que usa **Una copia por fila** (más abajo).

**Llaves literales.** Para escribir una llave, duplícala: `{{` da `{` y `}}` da `}`. Así `{{serie}}` se graba como `{serie}`.

**Formato de fecha y hora.**

| Marca | Significado |
|---|---|
| `d`, `dd`, `ddd`, `dddd` | Día (1 o 2 dígitos), día de la semana abreviado y completo. |
| `M`, `MM`, `MMM`, `MMMM` | Mes (1 o 2 dígitos), abreviado y completo. |
| `yy`, `yyyy` (o `aa`, `aaaa`) | Año con 2 o 4 dígitos. |
| `H`, `HH` / `h`, `hh` | Hora de 0 a 23 / de 1 a 12. |
| `m`, `mm` / `s`, `ss` | Minutos / segundos. |
| `AP` / `ap` | AM o PM / am o pm. |
| `'texto'` | Texto tal cual: `{fecha:d 'de' MMMM 'de' yyyy}` da `3 de septiembre de 2025`. |

Una `m` minúscula es de **minutos** si va junto a una hora o a segundos (`HH:mm`, `mm:ss`) y de **mes** en los demás casos: `{fecha:dd/mm/aaaa}` también funciona. Los nombres de meses y días salen en el idioma de la interfaz.

**Si algo no se entiende** (un nombre que no existe, una llave sin cerrar, una columna que no está en el CSV) esa parte se deja escrita tal cual, el panel de texto lo avisa y **el envío se bloquea** hasta corregirlo: nunca se graba un texto a medias.

## El diálogo «Texto variable»

Se abre desde **Herramientas > Texto variable…** o desde el botón del panel de texto.

![Diálogo Texto variable con el número de serie, la tabla del CSV y Una copia por fila](../../apps/desktop/docs/screenshots/43-texto-variable.png)

- **Número de serie.** El **próximo número**, el **paso** (cuánto sube por trabajo) y los **dígitos** mínimos (rellena con ceros). Con **Anterior** y **Siguiente** mueves el número un paso.
- **Fuentes de datos (CSV).** **Elegir CSV…** (o **Agregar otro CSV…**), **Volver a leer** y **Quitar esta fuente**; con varios archivos ver «Varios CSV en un proyecto». Se aceptan archivos UTF-8 (con o sin BOM), con separador coma, punto y coma o tabulador (se detecta solo), campos entre comillas dobles y saltos de línea dentro de un campo. La primera fila son los encabezados. Hasta 100 000 filas.
- **La tabla** muestra las filas y resalta la actual, la que se grabará en el próximo trabajo. Un clic en una fila la elige; también puedes usar **Fila anterior**, **Fila siguiente**, **Al inicio** o **Ir a la fila** (grupo **Vista previa por fila**).
- **Avance.** Con **Avanzar solo al terminar un trabajo** activo, cuando un trabajo termina bien la serie sube un paso y el CSV pasa a la fila siguiente. Un trabajo cancelado o con error no avanza. El avance se puede deshacer. Si el trabajo usó N copias (ver más abajo) avanza N pasos.
- **Fin del CSV.** Al grabar la última fila la app avisa. Con la fila ya pasada del final, el siguiente trabajo no sale (diría que no hay datos) hasta que elijas otra fila o vuelvas al inicio, así no se repite la última etiqueta por descuido.
- **Ver la plantilla en el lienzo.** Por defecto el lienzo muestra los valores de ahora. Esta opción (también en el panel de texto) muestra las llaves, para poder editarlas. Al enviar siempre salen los valores.
- **Objetos con variables del diseño.** Lista cada plantilla (de textos, imágenes y códigos) con lo que se grabaría ahora y los problemas que tenga.
- **Revisar datos.** Lista las filas del CSV con datos que faltan (ver más abajo).

> **Ojo:** la serie y la fila viven en el proyecto. Para que el siguiente número sobreviva al cerrar el programa, **guarda el proyecto** después de grabar (el avance marca el proyecto como modificado).

## Una copia por fila

Con una etiqueta seleccionada (por ejemplo el texto y su marco), **Una copia por fila** crea el número de copias que indiques en una rejilla (de fábrica, las filas que quedan del CSV). Cada copia toma la fila siguiente: en la tercera, `{csv:Nombre}` pasa a ser `{csv:Nombre@2}`, y lo mismo con `{serie}`. Así grabas muchas etiquetas en **un solo trabajo**. Es un solo paso de deshacer.

## Varios CSV en un proyecto

Un proyecto puede usar **varios CSV a la vez**: por ejemplo uno con las personas y otro con los productos. En el diálogo, **Fuentes de datos (CSV)** lista cada archivo con un **nombre** corto (de fábrica, el del archivo sin la extensión; solo letras, números, `_` y `-`). Con **Agregar otro CSV…** sumas más; con **Cambiar archivo…**, **Hacer principal** y **Quitar esta fuente** los administras. Si renombras una fuente, los textos, imágenes y códigos que la nombraban se actualizan solos.

Cada variable elige su fuente con el nombre: `{csv.productos:Nombre}` es la columna **Nombre** de la fuente `productos`, y `{fila.productos}` la fila que le toca. Sin nombre (`{csv:Nombre}`) es la **fuente principal**, la primera de la lista. El menú **Insertar variable** ofrece las columnas de todas las fuentes ya leídas.

**Qué pasa cuando tienen distinto número de filas.** La regla es una sola: **la fuente principal manda**.

- La fila actual y el número de filas del trabajo son los de la principal; las demás fuentes se leen con **la misma fila**. Pasada la última fila de la principal se acabaron los datos para todas.
- Una fuente secundaria **más corta** se **repite desde el inicio** (con 3 filas, la fila 4 de la principal lee su fila 1). Si prefieres que no se repita, elige **Bloquear el envío** en esa fuente: al pasar de su última fila el envío se detiene y el aviso nombra la fuente y la fila.
- Una fuente secundaria **más larga** se corta: las filas que sobran no se usan.
- El diálogo avisa de cada caso en cuanto lees los archivos («tiene 2 filas y la principal 3: se repite desde el inicio»).

## Imágenes y logotipos variables

Selecciona una **imagen** y, en sus propiedades, marca **Imagen variable (el archivo sale de una columna)**. Escribe la plantilla de la ruta (`{csv:Logo}`) o elígela con **Columna con la ruta**.

- La ruta de la celda puede ser **absoluta** o **relativa a la carpeta del CSV** de donde sale la columna (`logos/taza.png`).
- Al previsualizar, enviar o exportar, la imagen se cambia por el archivo de cada fila y **se encaja en la caja de la imagen marcada**: **Entera, centrada** la deja completa (puede sobrar espacio) y **Llenar y recortar** cubre la caja y recorta lo que sobra. El tamaño de la caja es el que tiene la imagen de muestra en el diseño.
- La imagen conserva su **capa**, así que se graba con el **modo de imagen** de esa capa (umbral, tramado, escala de grises...). Ajusta la imagen de muestra con el color de capa que quieras.
- Si falta un archivo, la celda está vacía o el archivo no es una imagen legible, **el envío se bloquea** y el aviso dice cuál, en qué fila y en qué plantilla. Nunca se graba otra cosa por error.
- Con **Una copia por fila** cada copia toma el logotipo de su fila.

## QR y códigos de barras variables

En **Herramientas > Insertar código…** el **Contenido** puede llevar variables: `https://tienda.example/{csv:Id}`, o `{csv.productos:Sku}` en un código de barras. La vista previa del diálogo usa la fila actual (y el selector **Insertar columna del CSV** agrega la variable). El código que se inserta recuerda sus ajustes y **se vuelve a generar al enviar** con los datos de cada fila, centrado donde estaba. Si el contenido de una fila queda vacío o el generador lo rechaza (un EAN-13 con dígitos de menos, por ejemplo), el envío se bloquea y dice la fila. Unas llaves sueltas sin variable (un JSON, por ejemplo) se codifican tal cual; para escribir una llave junto a una variable, duplícala (`{{`).

## Vista previa por fila y revisión de datos

- **Vista previa por fila.** El grupo del mismo nombre muestra **Fila X de N** y tiene **Fila anterior**, **Fila siguiente**, **Al inicio** e **Ir a la fila**. La fila actual es la que se ve en el lienzo, en la vista previa del trabajo y en lo que se envía. Con varias fuentes, el selector **Ver la fuente** muestra la tabla de cada una con la fila que le toca resaltada.
- **Revisar datos.** El botón **Revisar todas las filas** recorre el CSV con lo que usa el diseño y **lista las filas con datos que faltan**: celdas vacías, imágenes que no existen o con ruta vacía, códigos sin contenido y filas que pasan del final de una fuente en «bloquear». Un clic en una fila la deja como fila actual para verla. Una columna o fuente que no existe se avisa una sola vez.
- **Al enviar.** Siempre se comprueban las filas de ese trabajo: los archivos de imagen que faltan y los códigos que no se pueden generar bloquean el envío; las celdas vacías en un texto no lo bloquean pero se avisa con la fila.

## En el proyecto

Las fuentes se guardan en el `.tlaku` con **ruta relativa** a la carpeta del proyecto cuando el CSV está en esa carpeta o cerca (hasta tres niveles arriba), así puedes mover la carpeta entera; si queda lejos se guarda la ruta absoluta. Al abrir el proyecto las rutas vuelven a ser absolutas. Un proyecto de una versión anterior, con un solo CSV, se abre con ese archivo como fuente principal. Las rutas de imagen de las celdas se resuelven contra la carpeta del CSV, así que viajan con él.

## Ejemplos prácticos

**Veinte etiquetas numeradas en un solo trabajo.** Escribe `Pieza {serie:4}` dentro de un marco, selecciona el texto y el marco, y en el diálogo **Texto variable** pon el **Próximo número** en 1. Usa **Una copia por fila** con 20 copias y 5 **por fila**: salen `Pieza 0001` a `Pieza 0020`. Con **Avanzar solo al terminar un trabajo**, al terminar bien el trabajo el próximo número pasa a 21; guarda el proyecto para conservarlo.

**Una etiqueta por persona.** En una hoja de cálculo haz las columnas **Nombre** y **Puesto**, guárdala como CSV, y en Tlaku elige ese archivo con **Elegir CSV…**. Con el texto `{csv:Nombre}` y, abajo, `{csv:Puesto}`, cada trabajo graba una persona y pasa a la siguiente fila. Para grabar todas de una vez, **Una copia por fila** con tantas copias como filas (de fábrica propone las que quedan).

**Fecha de fabricación en cada pieza.** `Hecho el {fecha:d 'de' MMMM 'de' yyyy}` graba, por ejemplo, "Hecho el 13 de octubre de 2026" con la fecha del día en que envías el trabajo, sin tocar nada.

**Etiqueta con logotipo y QR por producto.** Un CSV `productos.csv` con las columnas **Producto**, **Logo** (ruta de la imagen) y **Sku**. Pon un texto `{csv:Producto}`, una imagen de muestra marcada como variable con `{csv:Logo}` y un QR con `https://tienda.example/{csv:Sku}`. Selecciona los tres, usa **Una copia por fila** y envía: cada etiqueta sale con su nombre, su logotipo y su QR. Antes, **Revisar todas las filas** te dice si algún logotipo no existe.

**Una llave que sí es una llave.** `Código {{A}}` se graba como `Código {A}`.

## Qué falta o es distinto

- Las variables funcionan en **textos**, en **imágenes** (la ruta del archivo) y en **QR y códigos de barras** (el contenido). No en el relleno de formas ni en los vectores.
- En el lienzo, una imagen o un código variable muestra los de **la fila actual** (igual que el texto), siempre que el CSV esté leído y el archivo exista; si no, ves la muestra. Es solo lo que ves: el proyecto guarda la muestra, y al seleccionar o editar el objeto trabajas con ella. **Ver la plantilla en el lienzo** vuelve a mostrar la muestra. La vista previa del trabajo y lo que se envía usan los mismos datos de la fila. Revisa la fila en la vista previa antes de grabar.
- Las fuentes secundarias se leen con el mismo número de fila que la principal; no hay una fila propia por fuente ni una unión por clave (como en una base de datos).
- No hay selección de filas (rango, filtro) ni buscar y reemplazar en bloque; ni QR con numeración propia (para eso usa `{serie}` en el contenido).
- No hay un modo de variable de ajuste de capa (velocidad, potencia) ni «hornear» el texto a valores fijos; para fijar un texto, conviértelo a trayectos.
- La sintaxis de Tlaku es propia: los archivos de LightBurn con texto variable no se traducen automáticamente.
- El texto variable avanzando en trabajos reales no se ha probado todavía en una máquina real; se probó con el emulador y la máquina virtual.

Siguiente: [Capas y ajustes de corte](05-capas-y-ajustes.md)
