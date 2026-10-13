# Dibujar y editar

Cómo dibujar formas, seleccionarlas, transformarlas y combinarlas. Todo lo que hagas se puede deshacer (`Ctrl/Cmd+Z`) y queda anotado en el panel **Historial**.

## Herramientas de dibujo

Lo que dibujas va a la **capa activa** (la marcada en el panel Capas o en la franja de colores). Si la capa activa es de imagen, va a la primera capa de líneas o relleno. Mientras arrastras, una etiqueta muestra la medida en vivo.

| Herramienta | Tecla | Cómo se usa |
|---|---|---|
| Dibujar línea | `L` | Clic a clic para una polilínea. `Intro`, doble clic o clic derecho la terminan; `C` la cierra; `Retroceso` quita el último punto; `Mayús` fija ángulos de 45°. Arrastrar de un punto a otro dibuja una sola línea. |
| Rectángulo | `R` | Arrastra. `Mayús` hace un cuadrado. |
| Elipse | `E` | Arrastra. `Mayús` hace un círculo. |
| Texto | `T` | Clic en un punto vacío crea un texto (ver [Texto](04-texto.md)). |

Con **Rectángulo** activo aparece en la barra numérica el campo **Radio de esquinas** (en mm) para los rectángulos que dibujes a continuación.

Polígono, estrella y curvas Bézier dibujadas a mano son **(próximamente)**.

## Seleccionar

La herramienta **Seleccionar** (`V`) hace todo esto:

- Clic selecciona un objeto; `Mayús`+clic lo suma o lo quita de la selección.
- Arrastrar sobre un espacio vacío dibuja una caja. De izquierda a derecha (línea continua) selecciona solo lo que queda **completamente dentro**; de derecha a izquierda (línea discontinua), todo lo que **toque** la caja.
- `Ctrl/Cmd+A` selecciona todo; `Ctrl/Cmd+Shift+I` invierte la selección.
- `Esc` cancela el arrastre en curso; sin arrastre, quita la selección.

Las otras selecciones especiales, bloquear objetos y el menú contextual con clic derecho sobre el lienzo son **(próximamente)**.

## Mover, escalar y girar

- **Mover:** arrastra el objeto, usa las flechas (1 mm; `Mayús` 10 mm; `Alt` 0,1 mm) o escribe **X** e **Y** en la barra numérica. `Mayús` al arrastrar limita el movimiento a horizontal o vertical.
- **Escalar:** arrastra una manija. En las esquinas conserva la proporción; `Mayús` la suelta; `Alt` escala desde el centro. En la barra numérica, **Ancho** y **Alto** con el candado puesto escalan los dos.
- **Girar:** con un solo objeto seleccionado, un segundo clic sobre él cambia las manijas a modo giro. Arrastra la perilla superior; `Mayús` gira de 15° en 15°; `Alt` gira alrededor del punto de referencia en vez del centro. O escribe los grados en **Rotación**.

Las imágenes se mueven y se escalan, pero no se giran. Voltear (espejo horizontal y vertical) y la rotación rápida son **(próximamente)**.

## Editar nodos

Elige **Editar nodos** (`N`) y haz clic en un trazo para ver sus nodos y manejadores.

![Editor de nodos con un contorno redondeado, tres nodos elegidos y la barra de nodos arriba](../../apps/desktop/docs/screenshots/11-editor-de-nodos.png)

- Clic elige un nodo; `Mayús`+clic suma o quita; clic en un segmento elige sus dos nodos; arrastrar sobre vacío elige con una caja.
- Arrastra para mover los nodos elegidos (`Mayús` en una sola dirección, `Alt` sin ajuste magnético). Las flechas también los mueven y `Supr` los borra. La barra fija **X** e **Y** exactas.
- Doble clic sobre un segmento agrega un nodo sin cambiar la forma.

Las teclas de nodos solo valen con esta herramienta y nodos elegidos:

| Acción | Tecla |
|---|---|
| Insertar nodo en el punto medio | `I` |
| Segmento a recta / a curva | `Mayús+L` / `Mayús+U` |
| Nodo en esquina / suave / simétrico | `Mayús+E` / `Mayús+S` / `Mayús+M` |
| Romper el trayecto en el nodo | `B` |
| Unir extremos | `J` |
| Cerrar trayecto | `O` |

Los textos y las imágenes no tienen nodos: convierte el texto a trayectos para editarlo así.

## Agrupar

`Ctrl/Cmd+G` agrupa dos o más objetos (sin bloquear); `Ctrl/Cmd+Shift+G` desagrupa. Un grupo se mueve, escala y alinea como un solo objeto.

## Menú Organizar

### Booleanas

Trabajan con **formas cerradas** de vectores (y textos con contorno); no con imágenes ni con trazos abiertos (ciérralos antes con `C` o con **Cerrar trayectos…**).

| Comando | Atajo | Resultado |
|---|---|---|
| Unión | `Alt+U` | Una sola forma con todo el área. |
| Diferencia (A menos B)… | `Alt+D` | A menos el área de las demás. Un diálogo te deja elegir cuál es A. |
| Intersección | | Solo lo que comparten. |
| Diferencia simétrica (XOR) | | Lo que está en una forma pero no en ambas. |
| Soldar | `Alt+W` | Funde en una región las formas que se solapan. Una forma que queda entera dentro de otra cuenta como agujero. |

Intersección y XOR con exactamente dos formas se aplican directo; con más, piden elegir A.

![Diálogo Diferencia: A menos B para elegir la forma principal](../../apps/desktop/docs/screenshots/13-diferencia.png)

### Offset

**Offset…** (`Alt+O`) crea un contorno paralelo a distancia fija. Positivo crece hacia afuera (los agujeros se encogen); negativo hacia adentro. Elige las **Esquinas** (**Redondas**, **En ángulo**, **Biseladas**) y si **Conservar el original**. Hay vista previa en el diálogo.

### Unir, cerrar y quitar duplicados

Abren un diálogo con una **Tolerancia** en mm (valor inicial entre paréntesis). Solo trabajan con trayectos vectoriales: no con imágenes ni con textos sin convertir. Unir y cerrar piden además que haya algún trazo abierto.

- **Unir trazos…** (0,05): junta trazos abiertos cuyos extremos estén a esa distancia, aunque sean de objetos distintos (de la misma capa).
- **Cerrar trayectos…** (0,5): cierra cada trayecto abierto cuyo final esté cerca de su inicio.
- **Quitar duplicados…** (0,01): borra los tramos repetidos o superpuestos de cada capa, para que el láser no corte dos veces la misma línea.

### Alinear y distribuir

Los seis comandos de alinear (izquierda, centro horizontal, derecha, arriba, centro vertical, abajo) piden al menos dos objetos, o uno si la referencia es la cama. **Distribuir** con espacio igual pide tres. **Alinear respecto a** elige la referencia: la selección, un objeto o la cama. **Centrar en la cama** es `Alt+C`. **Alinear y distribuir…** abre todo en un diálogo.

### Arreglos

- **Arreglo en rejilla…**: **Columnas** y **Filas**, con **Separación** medida entre bordes o entre centros.
- **Arreglo circular…**: **Piezas en total** (con la original), **Ángulo total**, **Radio** y centro; puede girar las copias hacia el centro.

Ambos muestran una vista previa y avisan si el arreglo se sale de la cama. El máximo es de 2000 piezas, y puedes **Agrupar el resultado**.

## Ajuste magnético

`S` lo enciende o apaga. Atrae a la cuadrícula visible y a los bordes y centros de los demás objetos y de la cama, a 8 píxeles de pantalla. Los interruptores **Ajustar a la cuadrícula** y **Ajustar a objetos y cama** (abajo a la izquierda del lienzo) eligen a qué. `Alt` mientras arrastras lo suspende en ese gesto.

## Copiar, pegar y deshacer

`Ctrl/Cmd+C`, `X` y `V` copian, cortan y pegan dentro de Tlaku; **Pegar en el mismo lugar** es `Ctrl/Cmd+Shift+V`; `Ctrl/Cmd+D` duplica. Copiar y pegar entre Tlaku y otros programas es **(próximamente)**. **Deshacer** y **Rehacer** recorren los últimos 200 pasos que guarda el **Historial**.

Siguiente: [Texto](04-texto.md)
