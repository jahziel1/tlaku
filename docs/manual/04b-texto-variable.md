# Texto variable

Para grabar números de serie, nombres de una lista o la fecha del día sin escribir cada pieza a mano, un texto puede llevar **variables** entre llaves. El texto que guardas en el proyecto es la **plantilla** (`Pieza {serie:4}`); al previsualizar, enviar o exportar, Tlaku la cambia por los valores de ese momento (`Pieza 0007`) y genera el contorno de las letras con ellos. El proyecto guardado conserva siempre la plantilla.

## Escribir variables

Selecciona un texto y usa el menú **Insertar variable** del panel de texto: pone la variable donde está el cursor. También puedes escribirlas a mano.

| Variable | Qué pone |
|---|---|
| `{serie}` (o `{serial}`) | El número de serie actual. `{serie:6}` lo rellena con ceros hasta 6 dígitos; `{serie:hex}` y `{serie:HEX}` lo escriben en hexadecimal (`{serie:hex4}` con 4 dígitos). |
| `{csv:Nombre}` | El dato de la columna **Nombre** en la fila actual del CSV. También se acepta el número de columna desde 1: `{csv:2}`. |
| `{fila}` (o `{row}`) | El número de la fila actual del CSV, desde 1. |
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
- **Datos de un CSV.** **Elegir CSV…**, **Volver a leer** y **Quitar**. Se aceptan archivos UTF-8 (con o sin BOM), con separador coma, punto y coma o tabulador (se detecta solo), campos entre comillas dobles y saltos de línea dentro de un campo. La primera fila son los encabezados. Hasta 100 000 filas.
- **La tabla** muestra las filas y resalta la actual, la que se grabará en el próximo trabajo. Un clic en una fila la elige; también puedes usar **Fila anterior**, **Fila siguiente**, **Al inicio** o **Ir a la fila**.
- **Avance.** Con **Avanzar solo al terminar un trabajo** activo, cuando un trabajo termina bien la serie sube un paso y el CSV pasa a la fila siguiente. Un trabajo cancelado o con error no avanza. El avance se puede deshacer. Si el trabajo usó N copias (ver más abajo) avanza N pasos.
- **Fin del CSV.** Al grabar la última fila la app avisa. Con la fila ya pasada del final, el siguiente trabajo no sale (diría que no hay datos) hasta que elijas otra fila o vuelvas al inicio, así no se repite la última etiqueta por descuido.
- **Ver la plantilla en el lienzo.** Por defecto el lienzo muestra los valores de ahora. Esta opción (también en el panel de texto) muestra las llaves, para poder editarlas. Al enviar siempre salen los valores.
- **Textos con variables del diseño.** Lista cada plantilla con lo que se grabaría ahora y los problemas que tenga.

> **Ojo:** la serie y la fila viven en el proyecto. Para que el siguiente número sobreviva al cerrar el programa, **guarda el proyecto** después de grabar (el avance marca el proyecto como modificado).

## Una copia por fila

Con una etiqueta seleccionada (por ejemplo el texto y su marco), **Una copia por fila** crea el número de copias que indiques en una rejilla (de fábrica, las filas que quedan del CSV). Cada copia toma la fila siguiente: en la tercera, `{csv:Nombre}` pasa a ser `{csv:Nombre@2}`, y lo mismo con `{serie}`. Así grabas muchas etiquetas en **un solo trabajo**. Es un solo paso de deshacer.

## Ejemplos prácticos

**Veinte etiquetas numeradas en un solo trabajo.** Escribe `Pieza {serie:4}` dentro de un marco, selecciona el texto y el marco, y en el diálogo **Texto variable** pon el **Próximo número** en 1. Usa **Una copia por fila** con 20 copias y 5 **por fila**: salen `Pieza 0001` a `Pieza 0020`. Con **Avanzar solo al terminar un trabajo**, al terminar bien el trabajo el próximo número pasa a 21; guarda el proyecto para conservarlo.

**Una etiqueta por persona.** En una hoja de cálculo haz las columnas **Nombre** y **Puesto**, guárdala como CSV, y en Tlaku elige ese archivo con **Elegir CSV…**. Con el texto `{csv:Nombre}` y, abajo, `{csv:Puesto}`, cada trabajo graba una persona y pasa a la siguiente fila. Para grabar todas de una vez, **Una copia por fila** con tantas copias como filas (de fábrica propone las que quedan).

**Fecha de fabricación en cada pieza.** `Hecho el {fecha:d 'de' MMMM 'de' yyyy}` graba, por ejemplo, "Hecho el 13 de octubre de 2026" con la fecha del día en que envías el trabajo, sin tocar nada.

**Una llave que sí es una llave.** `Código {{A}}` se graba como `Código {A}`.

## Qué falta o es distinto

- Las variables solo funcionan en **textos**. Los códigos QR y de barras se generan con su contenido ya fijo y no son variables.
- No hay un modo de variable de ajuste de capa (velocidad, potencia) ni «hornear» el texto a valores fijos; para fijar un texto, conviértelo a trayectos.
- La sintaxis de Tlaku es propia: los archivos de LightBurn con texto variable no se traducen automáticamente.
- Nada de esto se ha probado todavía con una máquina real; se probó con el emulador y la máquina virtual.

Siguiente: [Capas y ajustes de corte](05-capas-y-ajustes.md)
