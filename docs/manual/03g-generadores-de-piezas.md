# Generadores de piezas: cajas, bandejas, bisagras flexibles, engranajes, uniones y prueba de encaje

**Herramientas > Generadores de piezas…** dibuja por ti piezas que se cortan con láser y se arman: una **caja con dientes** (finger joints) con divisores y patas, una **bandeja o cajón**, una **caja redondeada** con lateral flexible, una **bisagra flexible** (living hinge), **engranajes** de evolvente, **uniones** sueltas (cola de milano y pestaña-ranura) y una **pieza de prueba de encaje** para calibrar tu máquina. Pones las medidas, ves las piezas en vivo y **Insertar** las deja en el diseño como objetos vectoriales normales, que puedes editar, mover o anidar como cualquier otro.

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

## Divisores, zócalo y patas de la caja

En la caja, la sección **Divisores** reparte el interior en compartimentos.

- **Divisores a lo ancho (reparten X)** y **a lo fondo (reparten Y)**: cuántas láminas lleva la caja en cada dirección (0 a 10). Con 2 y 1 salen seis compartimentos. Los divisores van a **todo el alto** del interior (en la caja cerrada llegan hasta la tapa; en la de tapa deslizante, hasta la tapa; en la abierta, hasta el borde).
- **Anchos de los compartimentos**: vacío = todos iguales. Si escribes medidas (`40, 60`; también sirven `;` o espacios) valen para los **primeros** compartimentos y los que falten se reparten por igual lo que sobra; si escribes **todos**, deben sumar lo que mide la cavidad menos los divisores (el campo te dice cuánto). Un compartimento no puede medir menos de dos grosores.
- **Sujeción**: con *Pestañas en ranuras* cada divisor tiene pestañas, del grosor exacto de la pared o la base que las recibe, que entran en **ranuras** de las paredes laterales, de la base y de la tapa (si hay); las pestañas usan el mismo reparto de dientes de la caja y siempre dejan un trozo liso entre la ranura y el borde. Con *Sueltos* no hay ranuras en las paredes: cada divisor se acorta el **juego de los divisores sueltos** en largo y en alto y se mete entre las paredes (un punto de pegamento lo fija si hace falta).
- **Cruces de media madera**: donde un divisor de X cruza uno de Y, el de X pierde la mitad de arriba y el de Y la mitad de abajo, en ranuras de un grosor (más la holgura) y de **exactamente media altura**. Se arman metiendo uno en el otro, y no se solapan ni dejan hueco. En los cruces no hay pestañas.
- La **holgura de encaje** de la caja también vale aquí: las ranuras miden la pestaña (o el grosor del divisor que cruza) más esa holgura, repartida a los dos lados. Con 0 es lo nominal; el kerf lo compensa la capa, igual que en los dientes.

La sección **Zócalo o patas** eleva la caja. Es una pila de **capas planas del grosor del material** que se **pegan** entre sí y debajo de la base (no tienen uniones, así que no cambian la caja): *Zócalo* es un marco retranqueado un grosor del borde de la base y *Cuatro patas* son cuatro cuadrados por capa, una pata en cada esquina. La **altura** se redondea a un número de capas (hasta 10) y el aviso dice cuántas capas y cuántos mm salen.

## Bandeja y cajón

El generador **Bandeja y cajón** usa la misma caja abierta de dientes (mismas reglas de arista, holgura y kerf) y le añade asas y, si quieres, un frente y una caja exterior. Las medidas son las del **cuerpo**.

- **Bandeja abierta**: cinco paneles de lados bajos. Con **Asa** en *Muesca redondeada* los laterales izquierdo y derecho llevan un recorte en U con el fondo redondeado, abierto en el borde superior; con *Ojal* es una ranura en forma de píldora cerca del borde. **Ancho del asa** y **hondo de la muesca / alto del ojal** se validan para que no toquen los dientes de las esquinas ni de la base.
- **Cajón con frente grande**: el cuerpo más un **frente liso** que sobresale por cada lado, por arriba y por abajo lo que pidas en **Cuánto sobresale el frente por cada lado**. Se **pega por fuera** de la pared frontal, centrado, y tapa el hueco del mueble. El asa va en el frente, y el mismo recorte atraviesa la pared frontal del cuerpo que queda detrás para que se pueda meter el dedo.
- **Cajón y caja exterior**: además sale una caja abierta por el frente cuyo interior es el cajón más el **juego entre cajón y caja** por cada lado y de fondo; es la caja abierta de dientes con los ejes cambiados (cinco piezas: base, tapa, dos laterales y fondo). El cajón entra por el frente y su frente queda por fuera; el generador avisa si el frente no alcanza a tapar la rendija (sobrante menos juego menor de 2 mm).

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

Una zona de **cortes alternados** deja doblar madera, contrachapado o acrílico delgado. Elige la **forma de los cortes**: *Rectas escalonadas* (el patrón clásico), *Onda*, *Zigzag*, *Ladrillo* o *Rombo*; la **dirección** (los cortes corren a lo largo de X o de Y; el material se curva en el otro sentido); el **tamaño de la zona**; el **largo de cada corte**, el **puente** que queda sin cortar entre dos cortes, la **separación entre filas** y el **margen** sin cortes junto al borde. Con onda o zigzag, además la **altura** y la **longitud de onda**. Las filas pares empiezan en un corte y las impares en un puente (medio paso de desfase) y todo queda simétrico respecto al centro.

- **Radio de las esquinas** redondea el contorno; **Incluir el contorno como corte** lo quita si solo quieres el patrón dentro de una pieza que tú dibujas (mueve el grupo adonde lo necesites).
- Más puentes y filas más separadas = más rígida; cortes más largos y filas más juntas = más flexible. La flexión real depende del material y de la veta: **prueba un trozo** antes de cortar la pieza final. Puentes menores de 1 mm o filas a menos de 1 mm se rompen con facilidad y el generador avisa.
- **Ladrillo**: filas escalonadas como las rectas, pero cada corte lleva un **remate corto** en cada extremo (forma de ⊓, como las juntas de una pared de ladrillos). Flexiona también al torcer. El remate mide la mitad de la separación entre filas y deja al menos 0,3 mm de material contra la fila vecina; si no cabe, el generador te pide más separación.
- **Rombo**: una **retícula de rombos**. Cada lado de rombo es un corte y el puente queda en los vértices. Aquí el **largo del corte** es el ancho del rombo (su diagonal a lo largo de los cortes), la **separación entre filas** es la **mitad de su diagonal** perpendicular (rombos más altos con más separación) y el **puente** es el material que une los rombos en cada vértice. Si el puente es tan largo que se come el lado, o los lados quedan a menos de 0,3 mm entre sí en un vértice, el generador marca el campo y explica por qué.
- Cada corte es una **línea abierta** sin ancho; el kerf de la capa no los modifica (los puentes ya cuentan con que el haz se come un poco).

## Engranajes

Engranajes **rectos de evolvente**, listos para cortar. En **Qué generar** eliges: *Un engranaje*, *Par de engranajes* (dos que engranan), *Piñón y corona* (engranaje interior: el piñón gira dentro de un aro dentado), *Cremallera* o *Piñón y cremallera*.

- **Tamaño del diente:** por **módulo** (mm; el diámetro primitivo es `m·z` y el exterior `m·(z+2)`) o por **paso diametral** (dientes por pulgada: `m = 25,4/DP`). Con módulo 2 y 20 dientes sale un engranaje de 40 mm primitivo y 44 mm exterior. Para que dos engranajes engranen deben tener **el mismo módulo y el mismo ángulo de presión**; el generador usa el mismo para todo el conjunto.
- **Ángulo de presión:** 14,5° (antiguo), **20° (estándar)** o 25° (diente más robusto, de pie más ancho).
- **Distancia entre centros:** en un par es `m·(z1+z2)/2`; en la corona, `m·(z2−z1)/2`. Con **Engranando, en un solo objeto** las piezas salen ya colocadas a esa distancia y con el diente de uno metido en el hueco del otro (un solo objeto, para ver el engrane; mueve el grupo adonde lo necesites). Con **Separadas** sale una pieza por engranaje, en filas, para cortar o mover por separado.
- **Juego (backlash)** y **holgura de cabeza:** el juego adelgaza cada diente la mitad del valor sobre el círculo primitivo, de modo que el par queda con ese juego entre flancos **sin cambiar la distancia entre centros**; 0,1 mm es un punto de partida para madera de 3 mm, pero depende de tu kerf y de tu máquina (con el kerf de la capa activado, el juego pedido es el que queda de verdad). La holgura de cabeza (0,25 del módulo por omisión) deja libre el fondo para la punta del otro engranaje.
- **Pie del diente:** con **filete** (más resistente) o **simple**. La **tolerancia de las curvas** (0,01 mm) es cuánto puede apartarse el polígono del evolvente exacto; más baja = más puntos.
- **Agujero central:** redondo, con **chavetero** (ancho y profundidad) o **eje en D** (un plano); 0 = sin agujero. El segundo engranaje de un par tiene su propio diámetro. Si el agujero deja menos de 1 mm de pared bajo el fondo de los dientes, se rechaza.
- **Aligeramiento:** **agujeros redondos** o **radios** (aberturas entre rayos de ancho constante), en la cantidad que pidas, entre el cubo y el fondo de los dientes; el **material mínimo** fija la pared del cubo y del aro y el ancho de los rayos. Si no cabe, se omite con un aviso.
- **Corona:** el **espesor del aro** exterior es aparte. **Cremallera:** número de dientes y espesor del cuerpo bajo el fondo; el largo es `dientes × π·m`.

**Avisos:** con pocos dientes hay **socavado** (el pie del diente se debilita y el engranaje puede trabarse): el mínimo práctico es `2/sin²α` (17 dientes a 20°, 32 a 14,5°, 11 a 25°) y el aviso lo sugiere. También avisa de puntas muy finas, de una **relación de contacto** menor de 1,2 (el par engrana a tirones) y de una corona con menos de 8 dientes más que el piñón. Los dientes que acaban en punta o un agujero que no cabe son errores y marcan el campo. El generador no sabe el tamaño de tu cama: el diálogo avisa si el conjunto no cabe.

Los dientes pierden precisión con módulos menores de 1 mm en madera o acrílico (el haz ya es del orden del diente): empieza con módulo 2 o más y corta primero un par de prueba.

## Uniones: cola de milano y pestaña-ranura

Piezas **sueltas** para unir dos placas por una arista: sirven de prueba o para usarlas en diseños propios (las cajas ya traen sus dientes). Elige el **tipo de unión**; sale un par de placas y se inserta como cualquier otro generador. El **largo de la arista** y el **fondo** de cada placa son los de la pieza de ejemplo; copia solo la forma de la unión a tu diseño.

- **Cola de milano plana (mismo plano)**: las dos placas quedan en el mismo plano, como piezas de un rompecabezas. Una placa lleva **colas** trapezoidales repartidas a intervalos iguales y la otra, las **ranuras** del mismo trapecio. Parámetros: **número de colas**, **ancho del cuello** (en la arista), **profundidad**, **ángulo** (qué tanto se ensancha la cola; 8 a 15° es lo habitual, más ángulo traba más) y **holgura**. La holgura (ranura − cola) deja la mitad por cada lado: con **0** las dos placas juntas rellenan el rectángulo completo sin huecos ni solapes. Se ensamblan **deslizando** una placa sobre la otra a lo largo de la arista; la forma impide separarlas tirando en perpendicular, pero no al deslizar: fija con un tope o una gota de pegamento. Si las colas no caben (no queda material entre ranuras) o la ranura pasa del fondo de la placa, el generador lo marca.
- **Pestaña y ranura (perpendiculares)**: una placa se apoya de canto sobre la cara de la otra. La primera lleva **pestañas** del **ancho** pedido y de un largo igual al **grosor**; la segunda, las **ranuras pasantes** de `(pestaña + holgura) × (grosor + holgura)`, a la **distancia del borde** que elijas. Las pestañas se reparten a intervalos iguales y van centradas. Una pestaña más angosta que el grosor se rompe fácil y el generador avisa.

Las dos usan la **holgura de encaje** de la misma forma que la caja: con el kerf en la capa, déjala en 0; si no, calíbrala con la **Prueba de encaje** (su modo «Ranura contra el canto del material» sirve justo para las pestañas).

## Caja redondeada

**Base y tapa con esquinas redondeadas y un solo lateral** que da la vuelta y se **dobla en las esquinas** gracias a una bisagra flexible, unido con dientes a la base y a la tapa. Salen **tres piezas**: *Base*, *Tapa* (si la pides) y *Lateral con bisagra*.

- **Medidas**: **ancho**, **fondo** y **alto exteriores**, **radio de las esquinas** (al menos el grosor más 2 mm y no más de la mitad del lado menor) y **grosor**. **Con tapa** apagado deja la caja abierta (base y lateral).
- **Largo del lateral**: es el **perímetro de la línea media del grosor** del lateral (el contorno corrido medio grosor hacia adentro, de radio `R − grosor/2`): al curvar la tira esa línea no se estira ni se comprime, así que conserva su largo. El contorno exterior mide `2π × grosor/2` más y el interior, menos. El diálogo te dice el largo exacto.
- **Bisagra**: solo en los **arcos** de las cuatro esquinas; los lados rectos quedan planos y sin cortes. Los cortes son verticales (paralelos al eje de flexión) con el patrón, el largo, el puente, la separación y el margen que elijas, como en la bisagra suelta (también ladrillo y rombo).
- **Dientes**: el contorno se reparte en un número **par** de celdas iguales a lo largo de la línea media. Las celdas pares son **dientes del lateral** (entran en muescas de la base y de la tapa) y las impares, lengüetas de la placa. En las curvas las muescas son sectores de corona de ancho igual al grosor; su área coincide con la del diente plano. La **holgura** ensancha cada muesca la mitad por lado.
- **Cierre del lateral**: empieza y termina en el centro del lado frontal. Con **cola de milano** (una o dos, según el alto) los dos extremos se traban; con **a tope** quedan rectos y se pegan. Las colas del cierre sobresalen del largo de la tira. Si el tramo recto del frente es muy corto para el cierre, el generador te lo dice.

**Importante:** la bisagra de las esquinas **no está probada en material real** con esta geometría. Antes de cortar la caja, corta una **bisagra de prueba** con el mismo patrón y comprueba que dobla al radio de la esquina sin romperse; un radio muy chico exige cortes más largos y puentes más delgados. En modo simulado (sin el motor) la vista previa de la caja redondeada es solo **aproximada**.

## Regenerar

Los parámetros se guardan **en el proyecto**. Al seleccionar el grupo aparece sobre el lienzo la barra **Piezas generadas… [Regenerar…]** (también en **Herramientas > Regenerar piezas…**): reabre el diálogo con los parámetros guardados y **reemplaza las piezas en el mismo lugar** (misma esquina, mismo grupo). Es como las formas paramétricas: si borras piezas o editas sus nodos, el vínculo se rompe y el diálogo avisa que regenerar vuelve a hacer el conjunto completo. Un proyecto guardado con una versión anterior abre igual: lo que falte toma su valor de fábrica.

## Límites

- Las cajas son de **paneles planos con dientes**; las esquinas redondeadas son la **Caja redondeada**, de un solo lateral con bisagra. Los divisores y el zócalo funcionan con medidas y holgura nominales; la prueba de encaje te da el valor para tu material. Las uniones sueltas no se aplican todavía a una arista de un objeto de tu diseño: salen como piezas de ejemplo con el largo que pidas. La **tapa deslizante** es una solución sencilla: cuida que la franja de arriba de los laterales (dos grosores) aguante con tu material.
- La geometría y el encaje se comprueban con pruebas automáticas, pero el **ajuste real depende de tu máquina y de tu material**: corta siempre primero la pieza de prueba y una caja chica.
- Todo se calcula en tu computadora; no se envía nada a internet.
