# Materiales

La biblioteca guarda los ajustes que ya sabes que funcionan para cada material, y la prueba de materiales te ayuda a descubrirlos con una cuadrícula de celdas. Así no vuelves a adivinar velocidad y potencia cada vez.

> **Ojo:** Tlaku trae una biblioteca **de fábrica** con 134 ajustes publicados por fabricantes y guías, pero son **puntos de partida, no una garantía**: no son de tu máquina ni se han probado con ella. Los ajustes reales dependen de tu máquina, tu láser, su estado y tu material. Haz siempre la prueba y guarda los tuyos.

## La pestaña Materiales

Abre la pestaña **Materiales** (`Ctrl/Cmd+6`). Arriba están las acciones (**Guardar capa como material…**, **Importar…**, **Exportar…** y **Prueba de materiales…**) y debajo la lista, con un buscador. La lista tiene dos secciones: **Mis materiales** (los que guardas tú, que puedes editar y borrar) y **De fábrica** (solo lectura, ver más abajo). Un aviso fijo, **Son puntos de partida: haz una prueba de materiales con tu máquina**, recuerda que ningún ajuste es una garantía.

![Biblioteca de materiales agrupada por material y grosor](../../apps/desktop/docs/screenshots/19-materiales.png)

La lista se agrupa por material (sin distinguir mayúsculas ni acentos) y, dentro, por grosor, de menor a mayor; los que no tienen grosor van al final. Cada ajuste muestra su operación (por ejemplo "Corte"), un resumen como `250 mm/min · 100 %`, su modo (**Línea**, **Relleno** o **Imagen**), sus notas y etiquetas:

- La máquina para la que se probó, si lo indicaste.
- **En uso: N**, si N capas del documento tienen exactamente esos ajustes.

El buscador (**Buscar**) filtra por material, operación, notas o grosor. Marca **Solo esta máquina** para ver los ajustes de la máquina activa y los que valen para cualquiera.

## La biblioteca de fábrica

La sección **De fábrica** reúne 134 ajustes de corte y de grabado de materiales comunes (madera, contrachapado, MDF, acrílico, cuero, cartón, papel, corcho, vidrio, acero y aluminio anodizado, entre otros). **No es una lista inventada**: cada ajuste se copió de una tabla o guía publicada y **dice de dónde salió**.

![Panel de materiales con la clase de máquina elegida (Diodo 20 W) y el aviso de puntos de partida](../../apps/desktop/docs/screenshots/61-materiales-clase.png)

### Fuente y confianza

Cada ajuste de fábrica muestra dos datos:

- **Fuente:** el sitio de donde viene (por ejemplo, la tabla de materiales de un fabricante). Pulsa el enlace para abrirla en el navegador, o el icono de cadena para copiar la dirección. Léela: ahí está el contexto (lente, óptica, aire) con el que se obtuvo el valor.
- **Confianza:** qué tan seria es esa fuente.
  - **Oficial**: una tabla publicada por el fabricante del láser.
  - **Guía**: una guía o un artículo publicado; no es de tu máquina ni está verificado con ella.
  - **Foro**: un dato de un foro o de la comunidad, sin verificar.

Que un valor sea **Oficial** quiere decir que lo publicó el fabricante, **no que sirva tal cual en tu máquina**.

![Lista De fábrica: cada ajuste con su clase de máquina, su confianza y su fuente](../../apps/desktop/docs/screenshots/61b-materiales-de-fabrica.png)

### Clases de máquina

Un láser de diodo de 5 W y uno de CO2 de 100 W no se ajustan igual, así que cada ajuste de fábrica dice para **qué clase de máquina** es: **Diodo 5, 10, 20 y 40 W**, **CO2 40, 60, 80 y 100 W** y **Fibra 20, 30 y 50 W**. En **Clase de mi máquina** eliges la tuya (solo salen las clases de tu tipo de láser); Tlaku la recuerda para esa máquina. Si no la eliges, se muestran todos los ajustes. Con **Solo mi clase** ocultas los de otras clases (los que no dicen para qué máquina son se siguen viendo).

Si no sabes la potencia, mira la etiqueta del láser o su caja. En el [modo sencillo](01b-modo-sencillo.md), Tlaku te lo pregunta ("¿De cuántos watts es tu láser?") y, si contestas **No lo sé**, usa los ajustes más prudentes.

### Avisos y restricciones

Algunos ajustes traen un aviso (por ejemplo, que el acero inoxidable suele necesitar un recubrimiento para que la marca se vea). Y hay materiales que **no se pueden trabajar con ciertos láseres**: el **acrílico transparente no se corta con diodo** (la luz lo atraviesa); con una máquina de diodo, la lista lo avisa y el modo sencillo no te deja elegir **Cortar**.

### Usarlos

- **Aplicar a la capa** funciona igual que con tus materiales.
- Los de fábrica **no se editan ni se borran**. **Duplicar** (el icono de copiar) crea una copia en **Mis materiales** que sí puedes editar.
- **Exportar…** guarda solo **tus** materiales; los de fábrica vienen con la aplicación.
- Los ajustes de fábrica son una **referencia para empezar**: haz una **prueba de materiales** con tu máquina y tu material de desecho (ver abajo) y guarda el resultado como tuyo.

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

Tu biblioteca (**Mis materiales**) se guarda sola en tu equipo, en un archivo `materials.json` dentro de la carpeta de configuración de la app. Si el archivo está dañado, la app lo aparta (como `materials.json.dañado`) y no guarda nada hasta que reintentes, para no pisarlo.

- **Exportar…** crea un archivo `.tlakumat` (por defecto `materiales.tlakumat`) con todos tus ajustes. Está desactivado si la biblioteca está vacía.
- **Importar…** agrega los ajustes de un `.tlakumat`. Lo que ya tienes no se pisa: un ajuste idéntico a uno existente se omite, así que importar el mismo archivo dos veces no duplica nada; si el identificador choca con otro ajuste distinto, entra como copia con identificador nuevo. Al terminar dice cuántos entraron, cuántos cambiaron de id y cuántos ya estaban.

Los `.tlakumat` sirven para compartir tus ajustes con otras personas o pasarlos a otro equipo. No se importan bibliotecas de otros programas.

**Compartir con la comunidad.** Si un ajuste tuyo te funcionó, usa el botón **Compartir…** de ese ajuste (o Ayuda > «Compartir ajustes de material…»): la app abre tu navegador con el formulario de GitHub ya lleno con la máquina, el material, la velocidad y la potencia. Tú lo revisas y decides si lo publicas; Tlaku no envía nada por su cuenta. Tras una revisión, los ajustes entran a la sección **Comunidad (sin verificar)** de la biblioteca con licencia CC0. Son valores de una persona en una máquina: haz siempre una prueba antes de usarlos.

Siguiente: [Rotativo y eje Z](12-rotativo-y-eje-z.md)
