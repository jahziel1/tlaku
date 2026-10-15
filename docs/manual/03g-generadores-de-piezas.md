# Generadores de piezas: cajas, bisagras flexibles y prueba de encaje

**Herramientas > Generadores de piezas…** dibuja por ti piezas que se cortan con láser y se arman: una **caja con dientes** (finger joints), una **bisagra flexible** (living hinge) y una **pieza de prueba de encaje** para calibrar tu máquina. Pones las medidas, ves las piezas en vivo y **Insertar** las deja en el diseño como objetos vectoriales normales, que puedes editar, mover o anidar como cualquier otro.

Los atajos se escriben con `Ctrl/Cmd`: `Cmd` en macOS y `Ctrl` en Windows y Linux. Las medidas usan la unidad de **Preferencias** (mm o pulgadas); por dentro todo es mm.

## El diálogo

A la izquierda eliges el **generador**; en el centro están sus **parámetros** (con la unidad al lado de cada medida) y a la derecha la **vista previa**, que se actualiza al teclear. Un valor que no sirve se marca en rojo con el motivo (por ejemplo «la caja es demasiado pequeña para dientes de este grosor») y **Insertar** queda apagado hasta corregirlo. Debajo de la vista previa:

- **Capa de corte**: la capa de líneas donde van los cortes. Por omisión, la capa activa (si es de líneas) o la primera de líneas.
- **Capa de los rótulos grabados** (solo si pides rótulos): una capa existente, **Sin rótulos** o una **capa nueva** («Rótulos de piezas», de líneas, con poca potencia y más velocidad para que **grabe sin atravesar**). Revisa su potencia con tu material.
- Los **avisos** del generador (dientes más angostos que el grosor, puentes muy finos, etc.).

**Insertar** pone un objeto de corte por pieza (y uno de grabado por pieza con rótulo), **todos en un grupo**, en la pestaña activa, dentro de la cama y **sin tapar lo que ya hay**: las piezas se acomodan en filas del ancho de la cama y el conjunto va al primer hueco libre. Si no hay hueco, queda en la esquina y un aviso te lo dice. Todo es **un solo paso de deshacer**.

## Caja con dientes

Los paneles se unen con **dientes alternados**: en cada arista, un panel tiene diente donde el otro tiene muesca, y la profundidad es el **grosor** del material. Las medidas salen exactas.

- **Tipo**: *Cerrada* (6 paneles), *Abierta* (sin tapa, 5) o *Con tapa deslizante*: la tapa corre por ranuras de los laterales y se mete por detrás; el panel trasero es más bajo. Delante de la tapa queda una rendija de dos grosores (un bloque de cada lateral sostiene la franja de arriba) y los laterales llevan una ranura de un grosor más el **juego de la tapa**.
- **Las medidas son** *Exteriores* o *Interiores*. Con interiores, la caja resultante mide dos grosores más por lado (y, en la abierta, un grosor más de alto; en la de tapa deslizante, el alto interior es el que queda **bajo la tapa**).
- **Grosor del material**: el de tu hoja. Mídelo con un calibrador; una hoja de «3 mm» suele medir 2,8 a 3,2.
- **Ancho de diente**: el que prefieres. Tlaku lo **ajusta** en cada arista a un número **impar** de celdas iguales, así los dientes quedan simétricos y las dos puntas de la arista son del mismo panel. Un diente más angosto que el grosor se rompe fácil: el generador avisa.
- **Holgura de encaje** (ranura − diente): mira abajo. **0** es lo nominal; positivo es más flojo, negativo, más apretado.
- **Grabar el nombre de cada panel**: rotula cada panel (FRENTE, ATRAS, IZQ, DER, BASE, TAPA) en su **cara exterior**, con letras que se leen bien si pones la pieza con esa cara hacia arriba. Si no cabe entre los dientes, se omite y avisa.

## Kerf y holgura: dos cosas distintas

El láser quita material: el haz tiene un ancho (el **kerf**, de 0,1 a 0,3 mm según la máquina y el material). Por eso una unión «perfecta» en el dibujo sale floja o apretada al cortar.

- Las piezas se dibujan **nominales**: dientes del ancho exacto y muescas de la profundidad exacta, sin kerf.
- El **kerf lo compensa la capa**: en **Capas** (editor de la capa de corte) está el campo **Kerf**. Con un valor positivo la capa agranda los contornos exteriores y encoge los agujeros y las muescas, para que la pieza **física** salga del tamaño dibujado. **Para que las uniones encajen, activa el kerf en la capa de corte** con el valor que mediste (mira «Calibrar»). El diálogo te lo recuerda y, si la capa ya tiene kerf, te avisa que lo está usando.
- La **holgura de encaje** es un ajuste fino de la ranura contra el diente que se dibuja en la geometría (ensancha las muescas y adelgaza los dientes por igual en las dos piezas). Úsala **o** el kerf para el mismo efecto, no los dos: el generador avisa si pones holgura. Lo normal: kerf medido en la capa y holgura 0; si queda una pizca floja o apretada, ajusta la holgura 0,02 a 0,05 mm.

## Calibrar con la pieza de prueba de encaje

La **Prueba de encaje** es una **placa con trece ranuras** de anchos distintos y un **macho** (una lengüeta con asa). Se corta con **la misma capa y el mismo material** que la caja y dice cuál holgura te sirve.

1. Elige el modo:
   - **Ranura contra un diente cortado**: es el de los dientes de una caja. Las ranuras miden el ancho de diente más un paso (−0,30 … +0,30 mm con los valores de fábrica), y el macho es un diente de ese ancho.
   - **Ranura contra el canto del material**: para uniones de pestaña en ranura. Las ranuras miden el **grosor** más un paso y el macho se mete **de canto**.
2. Escribe el **grosor** (o el ancho de diente), el **paso** (0,05 mm es un buen comienzo; súbelo si tu haz es ancho) y cuántas ranuras a cada lado (6 cubren hasta ±0,30 mm). Deja **Grabar los valores** encendido: cada ranura lleva su desfase grabado (`-0.10`, `0.00`, `+0.05`…).
3. En la capa de corte pon el **kerf** que usas (o 0 si todavía no lo conoces) y **Insertar**. Corta con la **capa y el material reales**. No uses la misma hoja para otra cosa antes: el grosor varía de hoja en hoja.
4. Prueba el macho en cada ranura. Busca la que entra **con un leve apriete**: sin forzar, sin que baile ni se caiga. Su valor grabado es lo que buscabas.
5. Con el modo de dientes tienes dos formas de aplicar el resultado (usa **una**):
   - Escribe el valor en **Holgura de encaje** de la caja y deja el kerf de la capa en 0; o
   - Deja la holgura en 0 y pon en el **kerf** de la capa **la cuarta parte** del valor, sin signo. Razón: el campo Kerf es cuánto se desplaza cada línea (la mitad del ancho del haz) y en una unión de diente con ranura el haz se come material de los dos lados, así que la ranura menos el diente se abre **dos anchos de haz**: si ajusta la ranura `-0.20`, tu haz mide unos 0,10 mm y el kerf de la capa debe ser de **0,05 mm**. Repite la prueba con ese kerf puesto: ahora debe ajustar la ranura `0.00`.

Un valor negativo que ajusta es lo normal: significa que tu corte deja las ranuras **más anchas** y los dientes **más angostos** de lo dibujado. Apunta el valor por **material y grosor**: cada uno tiene el suyo.

## Bisagra flexible

Una zona de **cortes alternados** deja doblar madera, contrachapado o acrílico delgado. Elige la **forma de los cortes**: *Rectas escalonadas* (el patrón clásico), *Onda* o *Zigzag*; la **dirección** (los cortes corren a lo largo de X o de Y; el material se curva en el otro sentido); el **tamaño de la zona**; el **largo de cada corte**, el **puente** que queda sin cortar entre dos cortes, la **separación entre filas** y el **margen** sin cortes junto al borde. Con onda o zigzag, además la **altura** y la **longitud de onda**. Las filas pares empiezan en un corte y las impares en un puente (medio paso de desfase) y todo queda simétrico respecto al centro.

- **Radio de las esquinas** redondea el contorno; **Incluir el contorno como corte** lo quita si solo quieres el patrón dentro de una pieza que tú dibujas (mueve el grupo adonde lo necesites).
- Más puentes y filas más separadas = más rígida; cortes más largos y filas más juntas = más flexible. La flexión real depende del material y de la veta: **prueba un trozo** antes de cortar la pieza final. Puentes menores de 1 mm o filas a menos de 1 mm se rompen con facilidad y el generador avisa.
- Cada corte es una **línea abierta** sin ancho; el kerf de la capa no los modifica (los puentes ya cuentan con que el haz se come un poco).

## Regenerar

Los parámetros se guardan **en el proyecto**. Al seleccionar el grupo aparece sobre el lienzo la barra **Piezas generadas… [Regenerar…]** (también en **Herramientas > Regenerar piezas…**): reabre el diálogo con los parámetros guardados y **reemplaza las piezas en el mismo lugar** (misma esquina, mismo grupo). Es como las formas paramétricas: si borras piezas o editas sus nodos, el vínculo se rompe y el diálogo avisa que regenerar vuelve a hacer el conjunto completo. Un proyecto guardado con una versión anterior abre igual: lo que falte toma su valor de fábrica.

## Límites

- Las cajas son de **paneles planos con dientes**: sin divisores, patas ni esquinas redondeadas (de momento). La **tapa deslizante** es una solución sencilla: cuida que la franja de arriba de los laterales (dos grosores) aguante con tu material.
- La geometría y el encaje se comprueban con pruebas automáticas, pero el **ajuste real depende de tu máquina y de tu material**: corta siempre primero la pieza de prueba y una caja chica.
- Todo se calcula en tu computadora; no se envía nada a internet.
