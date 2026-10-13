# Capas y ajustes de corte

En Tlaku cada color es una **capa**, y cada capa es una operación con sus propios ajustes: cuánto se mueve, con qué potencia y de qué manera. Esta página explica el panel **Capas**, el editor de ajustes y el orden en que se ejecutan.

> **Ojo:** una capa nueva nace con 1000 mm/min, 20 % de potencia y 1 pasada. Son valores de partida, no de ningún material. Prueba siempre en un retazo antes de cortar la pieza buena (la [prueba de materiales](11-materiales.md) ayuda a encontrar los valores).

## El panel Capas

Se abre con `Ctrl/Cmd+1`. Cada fila muestra el color y el nombre, cuántos objetos tiene, un resumen (velocidad, potencia, intervalo, pasadas) y una etiqueta: **Grabado**, **Línea** o **Corte**. A la derecha hay tres botones:

- El **ojo** muestra u oculta la capa en el lienzo. No cambia lo que se envía.
- El **rayo** decide si la capa se **envía a la máquina**. Apágalo para probar el resto sin borrarla.
- Los **tres puntos** abren un menú (también con clic derecho sobre la fila): **Ajustes de la capa…**, **Seleccionar sus objetos**, **Renombrar**, **Ejecutar antes**, **Ejecutar después** y **Fijar en esta posición**.

Con el teclado: flechas para moverte por la lista, `Intro` o doble clic para abrir los ajustes, `F2` para renombrar y `Alt+↑`/`Alt+↓` para reordenar. `Alt+Intro` abre los ajustes de la capa activa desde cualquier parte.

Para mover objetos a una capa, selecciónalos y elige un color de la franja de abajo. La capa de **Guías** (el último cuadro, punteado) sirve de referencia al diseñar y nunca se envía a la máquina.

## Editor de ajustes

Abre el editor con `Intro` en una fila. Arriba, el botón **Ayudas** muestra u oculta una explicación bajo cada parámetro. Los campos inválidos se marcan en rojo y no se aplican; cada cambio tiene su propio paso de **Deshacer**.

![Editor de una capa de corte con velocidad, potencia, pasadas y asistencia de aire](../../apps/desktop/docs/screenshots/03-editor-de-capa.png)

### Modo de la capa

| Modo | Qué hace |
|---|---|
| **Línea** | Sigue los contornos de los objetos: para cortar o marcar. |
| **Relleno** | Barre el interior de las formas cerradas con líneas paralelas: para grabar superficies. |
| **Imagen** | Graba fotos y mapas de bits variando la potencia punto a punto (ver [Imágenes](06-imagenes.md)). |

### Velocidad y potencia

| Ajuste | Rango y unidad | Notas |
|---|---|---|
| **Velocidad** | 1 a 60000 mm/min | Más lento deja más energía en cada milímetro. Avisa si supera el máximo de la máquina activa. |
| **Potencia** | 0 a 100 % | En modo Imagen se llama **Potencia máxima**. |
| **Potencia mínima** | 0 a 100 % | Solo en Imagen; no puede superar la máxima. |
| **Pasadas** | 1 a 1000 | Dos pasadas a media potencia suelen cortar más limpio que una a tope. |
| **Asistencia de aire** | sí o no | Enciende el aire de la máquina durante la capa. |

### Contorno: kerf y puentes (modo Línea)

- **Kerf** (-5 a 5 mm): compensa lo que el haz se come del material. Con un valor positivo el recorrido se desplaza hacia fuera en las piezas y hacia dentro en los agujeros, para que la pieza salga del tamaño dibujado. Solo afecta a contornos cerrados.
- **Puentes**: con **Dejar puentes sin cortar** la capa deja pequeños tramos sin cortar para que la pieza no se suelte. Defines el **Ancho del puente**, la **Cantidad** por forma cerrada (o una **Separación** fija en mm; con 0 se usa la cantidad) y si se **Omiten formas interiores**. Los puentes aparecen como huecos en la vista previa.

![Sección Puentes del editor de capa](../../apps/desktop/docs/screenshots/21-capa-puentes.png)

### Barrido (Relleno e Imagen)

| Ajuste | Qué hace |
|---|---|
| **Intervalo** (0,01 a 5 mm) | Distancia entre líneas de barrido. Menos intervalo, más densidad y más tiempo. Debajo ves el equivalente en DPI y en líneas por cm: son el mismo dato (DPI = 25,4 / intervalo). |
| **Ángulo** | Dirección de las líneas, en grados antihorario (0 = horizontal). |
| **Trama cruzada** | Rellena dos veces, la segunda a 90°. Solo en Relleno. |
| **Bidireccional** | Graba de ida y de vuelta: casi el doble de rápido. Si la máquina tiene holgura, apágalo para que las líneas no se desalineen. |
| **Overscan automático** / **Overscan** (0 a 50 mm) | Distancia que el cabezal recorre fuera de la pieza, con el láser apagado, para llegar a velocidad antes de marcar. En automático se calcula con la velocidad y la aceleración. |
| **Relleno** | **Barrido** (líneas paralelas) o **Contornos** (anillos concéntricos hacia adentro; entonces no se usan ángulo, trama cruzada, bidireccional ni overscan). |

### Eje Z

El grupo **Eje Z** (**Desplazamiento Z** y **Bajada por pasada**, en mm) solo aparece si la máquina activa tiene eje Z motorizado, o si la capa ya trae valores de Z. Si no hay eje Z, esos valores se ignoran al enviar y el editor ofrece **Poner Z en 0**. Más en [Rotativo y eje Z](12-rotativo-y-eje-z.md).

## Orden de ejecución

La propia lista de capas es el orden, de arriba (primero) a abajo (último). El número de cada fila es su posición.

- **Automático** (el modo por defecto): primero los grabados (Relleno e Imagen), luego las líneas y al final los cortes, para que la pieza no se mueva antes de terminar de grabar. Una capa de línea cuenta como **Corte** si tiene kerf distinto de 0, o potencia de 80 % o más con varias pasadas.
- **Del archivo**: respeta la lista tal como está.

Cambia entre los dos con el selector **Automático** / **Del archivo** del panel. Para reordenar, arrastra el asa de la fila, usa `Alt+↑`/`Alt+↓` o **Ejecutar antes** y **Ejecutar después**. **Fijar en esta posición** (o **Fijar la posición de ejecución** en el editor) obliga a una capa a ocupar un lugar; dos capas no pueden pedir el mismo.

Una capa sin posición no se ejecuta, y su etiqueta dice por qué: **Guía**, **Sin salida** o **Sin objetos**. Pasa el cursor sobre la etiqueta para ver la explicación. Cómo se ordena el recorrido *dentro* de cada capa está en [Vista previa y orden de corte](07-vista-previa.md).

Siguiente: [Imágenes](06-imagenes.md)
