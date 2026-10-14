# Grabado 3D por cortes, cilindro y cono

Tres funciones para trabajos que no son planos: el **grabado 3D por cortes** (un relieve a partir de una imagen en grises), la **corrección de cilindro** (marcar sobre una taza o una botella con un láser galvo, sin rotativo) y la **corrección de cono** (marcar sobre un vaso que se ensancha).

> **Estado: viven en el motor.** Estas tres funciones existen en el motor de Tlaku y tienen pruebas automáticas, pero **no hay banderas para ellas en la línea de comandos**, y los controles de la app están en camino. El galvo se maneja hoy desde la línea de comandos; la interfaz en la app está en camino. Esta página explica qué hacen y cómo se calculan, para que sepas qué esperar. **Nada se probó con una máquina real**, y la corrección de cilindro y cono es un **modelo geométrico propio**, no verificado con hardware.

## Grabado 3D por cortes

### La idea

Una imagen en **escala de grises** se interpreta como un **mapa de profundidad**: el **blanco puro** es la superficie (no se graba) y cuanto más **oscuro**, más hondo. En vez de variar la potencia, el relieve se logra con **varias pasadas** de blanco y negro: cada pasada graba solo los puntos *más oscuros* que un umbral, y el umbral baja de blanco a negro en pasos iguales. Los puntos más hondos se graban en más pasadas, así que quedan más profundos.

Con **N cortes**, la pasada `k` (empezando en 0) usa el umbral `(N − k) / N`. Con 4 cortes:

| Corte | Umbral | Qué graba |
|---|---|---|
| 1 | 100 % | Todo lo que no sea blanco puro |
| 2 | 75 % | Los puntos más oscuros que 75 % de brillo |
| 3 | 50 % | Los más oscuros que 50 % |
| 4 | 25 % | Solo lo muy oscuro |

El negro puro se graba en **los cuatro cortes**: es lo más hondo. Un relieve con 16 cortes tiene 16 escalones de profundidad.

### Ajustes

| Ajuste | Qué hace |
|---|---|
| **Cortes** | De 2 a 256. Más cortes, relieve más suave y trabajo más largo |
| **Ángulo por corte** | Grados que se suman al ángulo de barrido en cada corte (0 = el mismo en todos). Un ángulo distinto en cada pasada reparte las líneas y alisa el fondo |
| **Z por corte** | Si la capa usa eje Z con un paso por pasada, cada corte baja ese paso: el foco acompaña a la profundidad |

Cada corte sale como su propio bloque en el trabajo y en la vista previa (con el nombre «imagen · corte 3/16»). Si la capa tiene pausas, solo van antes del primer corte y después del último. Los cortes más hondos suelen quedar vacíos si la imagen no tiene negros: Tlaku lo resume en un solo aviso («imagen: 5 de 16 cortes 3D no tienen ningún punto que grabar») y no es un error.

### Cómo preparar la imagen

1. Trabaja en **gris, 8 bits**. Los mapas de profundidad de 16 bits todavía no se aprovechan: solo hay 8 bits (256 niveles).
2. **Blanco = superficie, negro = fondo.** Si tu mapa está al revés, invierte la imagen.
3. Un degradado suave da mejor resultado que una foto con mucho detalle: es un relieve, no una foto.
4. Empieza con **pocos cortes** (8 o 16) en un material de desecho y compara con el modelo.

Este grabado **sirve también en máquinas de pórtico** (por ejemplo, un CO2 o un diodo): en el motor, los cortes son pasadas de umbral normales. El galvo no es necesario para usarlo.

## Corrección de cilindro

### El problema

Un galvo marca sobre un **plano** de enfoque. Si pones una taza, la superficie se **curva** y se aleja del espejo a los lados: sin corrección, un cuadrado de 40 mm queda deformado, y fuera de foco hacia los bordes. Con la corrección de cilindro, dibujas el diseño **plano** y Tlaku calcula dónde tiene que caer el haz para que la marca quede donde la dibujaste sobre la superficie.

### Los datos que pide

| Dato | Qué es |
|---|---|
| **Diámetro** | El de la taza, en mm |
| **Distancia del espejo** | De los espejos del galvo a la línea más cercana de la superficie (la cima de un cilindro convexo). Con la pieza **enfocada**, es la distancia focal de tu lente |
| **Eje** | **X** si el cilindro va a lo largo de X (la superficie se curva en Y, como un rotativo), o **Y** |
| **Cóncavo** | Para marcar el interior de un anillo en vez del exterior |

El **eje del cilindro** pasa por el **centro del campo** (el centro de la cama), y la coordenada que va **a lo largo de la curvatura** se mide en **milímetros de arco sobre la superficie**, no en milímetros planos. Eso significa que, sobre una taza de 70 mm (220 mm de circunferencia), un diseño de 60 mm de ancho ocupa 60 mm *de superficie* (algo más de un cuarto de vuelta), no 60 mm de proyección.

### Un ejemplo con números

Una taza de 70 mm de diámetro (radio 35 mm), con los espejos a 160 mm de la cima. Para marcar un punto a distancia `c` de arco del centro, Tlaku lo proyecta en perspectiva desde el espejo:

| Arco sobre la taza | Dónde cae el haz en el campo | Escala a lo largo del eje |
|---|---|---|
| 0 mm (la cima) | 0,00 mm | 100 % |
| 10 mm | 9,78 mm | 99,1 % |
| 20 mm | 18,29 mm | 96,6 % |
| 30 mm | 24,60 mm | 93,0 % |
| 40 mm | 28,23 mm | 88,7 % |

Fíjate en que los 20 mm de superficie caen a 18,29 mm del centro: el haz tiene que ir **menos lejos** que lo dibujado porque la superficie se curva y se aleja. Cerca de la cima la diferencia es mínima; a los lados, cada milímetro de superficie ocupa cada vez menos campo, hasta que a unos 55 mm de arco (un cuarto de vuelta de esta taza) la superficie ya **no se ve desde arriba** y los puntos se recortan al borde, con un aviso. En la práctica, **no marques más allá de unos 60 º a cada lado de la cima** (unos 36 mm de arco en esta taza): el haz llega muy rasante y la marca se vuelve débil y borrosa.

Con un cilindro **cóncavo** pasa lo contrario: la superficie se acerca al espejo hacia los bordes, y 20 mm de arco caen a 19,61 mm.

### Qué no corrige

- Es un **modelo geométrico propio**: proyección desde un espejo único, sin considerar que son dos espejos separados ni la forma real del campo de la lente. **No está verificado con hardware.** Haz una prueba de una rejilla en un cilindro del mismo diámetro y mide.
- **No corrige el enfoque.** El láser enfocado en la cima queda desenfocado a los lados; con superficies muy curvas, usa un cilindro grande o una lente con más profundidad de campo.
- Para marcar sobre un cilindro con un láser de pórtico, el camino es el rotativo: mira [Rotativo y eje Z](12-rotativo-y-eje-z.md).

## Corrección de cono

Para un vaso o una copa que se ensancha. Se dibuja con el **diámetro mayor** y el ancho se reduce hacia el extremo más delgado.

| Dato | Qué es |
|---|---|
| **Diámetro superior** e **inferior** | En mm |
| **Largo** | De un extremo al otro del cono, a lo largo de su eje, centrado en la cama |
| **Eje** | El mismo que el del cilindro (si usas ambos, tienen que coincidir) |
| **Lado superior** | Si el extremo «superior» está en el lado positivo del eje |

A cada posición a lo largo del eje, Tlaku escala la coordenada de la curvatura con el cociente entre el diámetro de ahí y el diámetro mayor, de modo que un mismo ángulo abarca menos milímetros de superficie donde el cono es más delgado. Con un cono de 70 mm arriba, 50 mm abajo y 100 mm de largo:

| Posición en el eje | Diámetro | Factor sobre el ancho |
|---|---|---|
| Extremo superior (+50 mm) | 70 mm | 100 % |
| Centro (0 mm) | 60 mm | 85,7 % |
| Extremo inferior (−50 mm) | 50 mm | 71,4 % |

El cono **solo trabaja en milímetros** y, como el cilindro, **no corrige la distancia focal**. Se puede usar solo (sin la corrección de cilindro): entonces solo escala el ancho; o junto con ella, y entonces el radio local del cilindro es el del cono en cada punto.

Volver a: [Rotativo y eje Z](12-rotativo-y-eje-z.md). Ver también: [Galvo (experimental)](13c-galvo.md). Siguiente: [Controladoras soportadas](13-controladoras.md)
