# Imágenes

Cómo grabar fotos y dibujos de mapa de bits: importarlos, elegir el tramado, ajustar los tonos, recortarlos con una forma y, si lo prefieres, convertirlos en vectores con **Vectorizar imagen** (o varios a la vez). También cómo hacer el camino inverso, de vectores a imagen.

## Importar una imagen

1. **Archivo > Importar diseño…** (`Ctrl/Cmd+I`), o arrastra el archivo sobre la ventana.
2. Formatos de imagen: PNG, JPG, BMP, **WebP**, **TIFF**, **GIF** y **TGA**. Para elegir la resolución con la que se interpretan, usa **Archivo > Importar con opciones…** (ver [Importar y exportar](03b-importar-y-exportar.md)). Un TIFF o un TGA se convierte a un PNG junto al original, y Tlaku lo avisa.
3. La imagen aparece con su tamaño a 96 puntos por pulgada (un ancho de 600 px mide 158,75 mm) y va a una capa propia llamada **Imagen**, en modo **Imagen**, a 3000 mm/min.

Cambia el tamaño con **Ancho** y **Alto** de la barra numérica (el candado mantiene la proporción) o arrastrando las manijas. Las imágenes se mueven y se escalan, pero **no se pueden rotar**: gírala en un editor de imágenes antes de importarla.

> **Ojo:** el tamaño físico manda sobre los píxeles. Una imagen de pocos píxeles escalada a un tamaño grande saldrá pixelada por mucho que bajes el intervalo.

## Modo Imagen

Una capa en modo **Imagen** graba punto a punto, variando la potencia según el tono. Sus ajustes están en el editor de la capa (ver [Capas y ajustes de corte](05-capas-y-ajustes.md)):

| Ajuste | Qué hace |
|---|---|
| **Potencia máxima** | Potencia para los puntos más oscuros. |
| **Potencia mínima** | Potencia para los más claros. Con 0 % no marcan nada. |
| **Intervalo** y **Resolución equivalente** | Distancia entre líneas (0,1 mm = 254 DPI). Más fino, más detalle y más tiempo. Debe parecerse al ancho del punto del haz. |
| **Ángulo de barrido** | Con 90° la imagen se graba por columnas. |
| **Incremento de ángulo** | Grados que se suman al ángulo de barrido en cada pasada; solo se nota con 2 o más pasadas (ver [Capas y ajustes de corte](05-capas-y-ajustes.md)). |
| **Bidireccional**, **Overscan** | Igual que en Relleno. |

## Tramado

El **Tramado** (campo del grupo **Imagen** en el editor de la capa) convierte los tonos de la foto en puntos que el láser pueda grabar. Por defecto se usa **Stucki**.

| Tramado | Cuándo usarlo |
|---|---|
| **Umbral** | Logotipos y dibujos de línea: cada punto es negro o blanco según el **Umbral**. No sirve para fotos. |
| **Floyd–Steinberg** | Mucho detalle, algo de ruido. |
| **Jarvis** | Transiciones suaves, menos contraste, más lento. |
| **Stucki** | Parecido a Jarvis pero más nítido: una buena opción general para fotos. |
| **Atkinson** | Más contraste y zonas limpias; se pierden los extremos de luces y sombras. |
| **Trama ordenada 4×4** | Patrón regular, rápido y predecible; el patrón se nota. |
| **Escala de grises** | Sin trama: la potencia varía con el tono. Pide ajustar bien las potencias mínima y máxima al material. |
| **Semitono** | Puntos más grandes en las zonas oscuras, sobre una rejilla girada (como una revista impresa). Usa **Tamaño de celda** y **Ángulo de la rejilla**. |
| **Líneas de periódico** | Líneas más gruesas en las zonas oscuras. También usa celda y ángulo. |
| **Boceto** | Graba solo los bordes, como un dibujo a lápiz. |

## Ajustes de tono

Se aplican a la imagen antes del tramado:

- **Brillo** (-1 a 1): aclara u oscurece. 0 = sin cambio.
- **Contraste** (-1 a 1): separa o junta los tonos alrededor del gris medio.
- **Gamma** (0,1 a 5): curva de tonos. Mayor que 1 oscurece los medios tonos; menor que 1 los aclara.
- **Invertir**: intercambia claro y oscuro, útil con negativos o materiales que se aclaran al quemarse.
- **Ancho del punto** (0 a 1 mm): acorta cada tramo encendido para que las zonas oscuras no engorden. 0 = sin corrección.

## Presets de imagen

Arriba de los ajustes de tono, el selector **Preset de imagen** aplica de una vez el brillo, el contraste, el gamma, el realce de bordes y quitar motas (un solo paso de deshacer). No cambia el tramado ni el intervalo: esos dependen de tu máquina y del material, no de la foto.

De fábrica vienen **Sin cambios**, **Foto en madera**, **Logo de alto contraste**, **Retrato suave**, **Foto oscura**, **Escaneo sucio** y **Detalle fino**; cada uno explica debajo para qué sirve. Son puntos de partida: pruébalos con la vista previa y ajusta a mano si hace falta, con lo que el selector pasa a **Personalizado**.

Con **Guardar como…** guardas los valores de ahora con el nombre que quieras (hasta 50; si repites un nombre, reemplaza el anterior) y aparecen en **Míos**. **Borrar** quita el que tienes elegido. Tus presets se guardan en este equipo; no viajan dentro del proyecto.

## Vista previa de la imagen

El botón **Vista previa de la imagen…**, al final de los ajustes de la imagen, abre un panel flotante con lo que se grabaría de una imagen de la capa (la seleccionada o la primera; si hay varias, eliges cuál). Se actualiza solo cuando cambias un ajuste, y no calcula todo el trabajo.

- **Entera** ajusta la imagen al visor. Si la rejilla del trabajo es más fina de lo que cabe en pantalla, la vista usa una más gruesa y lo dice: es una aproximación rápida.
- **Tamaño real**, **200 %** y **400 %** muestran un trozo con la rejilla exacta del trabajo (cada punto es un punto grabado). Arrastra la imagen para moverte.

![El editor de la capa Foto con el preset «Foto en madera» y, abajo a la derecha, la vista previa tramada de la imagen](../../apps/desktop/docs/screenshots/68-vista-previa-imagen.png)

> **Ojo:** la captura se tomó con la aplicación en modo de demostración, así que la imagen y su tramado son de ejemplo; en tu equipo la vista previa muestra tu imagen con tu tramado.

La vista previa no incluye el efecto de la máquina al barrer (sobrebarrido, compensación de offsets). Para eso está la vista previa del trabajo.

## Imagen avanzada

En la sección plegable **Imagen avanzada** del editor de la capa (oculta en el **Modo principiante**) hay tres ajustes más. Se aplican antes del tramado, junto con los de tono.

| Ajuste | Qué hace | Rango |
|---|---|---|
| **Realce de bordes: radio** y **fuerza** | Acentúa los bordes (como el "enfoque" de un editor de fotos). El radio es el tamaño, en píxeles de la imagen, de lo que se considera un borde; la fuerza va de 0 (nada) a 1 (máximo). **Hacen falta los dos mayores que 0**; con uno en 0 no pasa nada y el editor lo avisa. | Radio 0 a 20 px; fuerza 0 a 1 |
| **Quitar motas** | Borra las motas aisladas de hasta ese tamaño, en píxeles, antes de tramar. 0 = apagado. | 0 a 20 px |
| **Enviar sin tramar (pass-through)** | Cada punto de la imagen ya es la potencia: se envía tal cual, sin tramado ni ajustes de tono. Con esta casilla el tramado, el umbral, el brillo, el contraste, el gamma, invertir, el realce y quitar motas quedan desactivados. | sí o no |

**Ejemplos prácticos**

- **Foto vieja o escaneada, con polvo y manchitas.** Antes de grabar sube **Quitar motas** a 2 o 3 px: las manchas aisladas desaparecen en lugar de salir como puntitos quemados en la madera. Los bloques grandes y los bordes rectos se conservan. Súbelo poco a poco: un valor alto empieza a borrar detalles reales.
- **Foto blanda o desenfocada.** Un **Realce de bordes** de radio 1 a 2 px y fuerza 0,5 devuelve definición a los contornos (ojos, letras, orillas). Si ves halos o bordes duros, baja la fuerza.
- **Imagen preparada en otro programa.** Si ya hiciste el tramado o las curvas de tono en un editor de fotos y la imagen en escala de grises es exactamente lo que quieres grabar, activa **Enviar sin tramar**: Tlaku no la toca. El negro se graba a la **Potencia máxima** y el blanco a la mínima. La imagen se sigue remuestreando al **Intervalo** de la capa.

La [vista previa](07-vista-previa.md) muestra las líneas de barrido y el tiempo estimado; con imágenes, el tiempo real puede diferir más que con vectores.

## Vectorizar una imagen

**Herramientas > Vectorizar imagen…** convierte una imagen en trazados que luego puedes cortar o editar con nodos. Solo está disponible con **una** imagen seleccionada.

![Diálogo Vectorizar imagen con la vista previa del trazado y los deslizadores](../../apps/desktop/docs/screenshots/15-vectorizar.png)

1. Selecciona la imagen y abre el diálogo.
2. Ajusta los deslizadores; la vista previa se recalcula y muestra cuántas formas y nodos salen.
3. Decide si marcas **Borrar la imagen original** (apagado por defecto; no se puede si la imagen está bloqueada).
4. Pulsa **Vectorizar**. Es un solo paso de **Deshacer**.

| Ajuste | Qué hace |
|---|---|
| **Umbral** | Se traza lo que sea más oscuro que este brillo. |
| **Suavizado** | Más alto: menos nodos y curvas más largas. |
| **Eliminar manchas** | Descarta manchas y huecos más chicos que este lado (muestra el equivalente en mm). |
| **Esquinas** | Los giros más cerrados que este ángulo quedan como esquina viva. Solo en modo **Curvas**. |
| **Desenfoque** | Alisa los bordes ruidosos antes de trazar. |
| **Modo** | **Curvas** (Bézier) o **Polígono** (segmentos rectos). |
| **Invertir (trazar lo claro)** | Traza las zonas claras en vez de las oscuras. |

El resultado queda exactamente sobre la imagen, en la capa activa (o en la primera capa de líneas si la activa es de imagen). El diálogo te dice a qué capa se agregará. Si salen demasiadas formas pequeñas o el borde queda áspero, sube **Eliminar manchas** o el suavizado, o prueba con algo de **Desenfoque**. **Restablecer ajustes** vuelve a los valores iniciales, pensados para logotipos y dibujos.

## Recortar una imagen con una forma

Para grabar solo una parte de la imagen, con la silueta que quieras, usa una **máscara**.

1. Dibuja la forma que recorta (un círculo, un corazón, un texto...) encima de la imagen.
2. Selecciona **la imagen y la forma** (una sola imagen; puedes sumar varias formas).
3. **Herramientas > Recortar imagen con forma.**

La imagen se graba solo dentro de la forma y el lienzo la recorta igual. Las formas **se consumen** al aplicarla (pasan a ser la máscara de la imagen). Para volver a la imagen entera, selecciónala y usa **Herramientas > Quitar máscara de la imagen**. Es un paso de **Deshacer**.

**Ejemplo: un retrato en un posavasos redondo.** Importa la foto, dibuja un círculo del diámetro del posavasos centrado en la cara, selecciona los dos y aplica la máscara: se graba solo el círculo.

La máscara es siempre una **forma vectorial** (no hay máscara por brillo). Con una máscara nueva se reemplaza la anterior, y si **reemplazas la imagen** por otro archivo la máscara se descarta.

## Convertir vectores a mapa de bits

El camino inverso a vectorizar: **Herramientas > Convertir a mapa de bits…** dibuja las formas vectoriales seleccionadas en una imagen PNG que se graba como imagen. Sirve para grabar un dibujo vectorial como imagen (con tramado, por ejemplo) o para combinarlo con una foto antes de grabar.

| Ajuste | Qué hace |
|---|---|
| **Resolución** | En puntos por pulgada; el diálogo muestra el tamaño en mm y en píxeles, y avisa si la imagen sería demasiado grande. |
| **Dibujo** | **Relleno** (las formas cerradas salen macizas) o **Contorno** (solo el borde, con su **Grosor del contorno**). |
| **Negativo** | Formas en blanco sobre negro. |
| **Reemplazar los vectores por la imagen** | Quita las formas originales (las bloqueadas no se quitan). |

El PNG se guarda **junto al proyecto**. Si el proyecto aún no está guardado, queda en la carpeta temporal y la app lo avisa: guarda el proyecto antes y vuelve a convertir para que la imagen quede a su lado. El PNG sale en negro sobre blanco.

**Ejemplo: grabar una silueta con tramado.** Dibujas una forma, la conviertes a 254 ppp y mandas la imagen a una capa en modo **Imagen** con el tramado que quieras.

## Guardar la imagen procesada y reemplazar la imagen

Con **una** imagen seleccionada hay dos comandos más en **Herramientas**:

- **Guardar imagen procesada…**: escribe un PNG con lo que se grabaría, con los ajustes de brillo, contraste y tramado de su capa y su máscara. Pide que la imagen esté en una capa de modo **Imagen**. Útil para ver el resultado exacto o llevarlo a otro programa.
- **Reemplazar imagen…**: cambia el archivo de la imagen por otro sin mover nada: conserva el **ancho** y la **posición** y sigue la proporción del nuevo archivo, en la misma capa. La máscara se descarta. Es la forma de corregir una foto en un editor externo y volver a grabarla sin rehacer el trabajo. (La imagen se vuelve a leer al reemplazarla; no hay un comando de "actualizar imagen" aparte.)

## Vectorizar varios archivos

**Herramientas > Vectorizar varios archivos…** aplica los **mismos ajustes** de **Vectorizar una imagen** (más arriba, en esta página) a muchas imágenes de una vez. No hace falta tener nada seleccionado.

1. **Agregar imágenes…** y elige los archivos; los que no se leen como imagen se avisan y no entran.
2. Los ajustes son los de **Vectorizar imagen** (umbral, suavizado, manchas, esquinas, desenfoque, modo, invertir), con vista previa del archivo que elijas en la lista. Debajo, la **Resolución de las imágenes** (ppp) que define su tamaño.
3. El diálogo dice a qué capa se agregan. **Vectorizar N imagen(es).**
4. Una barra muestra el avance; **Detener** corta el lote y no agrega nada.

Cada resultado se coloca en una **fila**: 10 mm de separación y hasta 400 mm de ancho, antes de pasar a la siguiente fila. Los resultados no se escalan al tamaño de ninguna imagen del diseño. Un archivo que falla no detiene al resto: al final se lista cuáles no se vectorizaron. Es un solo paso de **Deshacer**.

**Ejemplo: 12 logotipos de clientes para una placa.** Agrega los 12 PNG, ajusta el umbral viendo uno de ellos, y obtienes los 12 vectores en filas, listos para cortar.


Siguiente: [Vista previa y orden de corte](07-vista-previa.md)
