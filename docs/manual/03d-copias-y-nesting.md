# Copias, arreglo virtual y nesting

Cómo repetir piezas y acomodarlas para aprovechar el material: copiar a lo largo de un trazo, el **arreglo virtual** (copias que no son objetos), acoplar piezas entre sí, agrupar automáticamente por contención y el **nesting** (acomodo automático). Los comandos están en el menú **Organizar** (el de agrupar, en **Editar**). Cada uno es un solo paso de **Deshacer**. Los arreglos en rejilla y circulares de siempre están en [Dibujar y editar](03-dibujar-y-editar.md).

## Copiar a lo largo de un trazo

**Copiar a lo largo de un trazo…** reparte copias de la selección sobre otro trazo, como cuentas en un collar.

1. Selecciona lo que quieres copiar **y** el trazo (puede ser abierto o cerrado).
2. En el diálogo, **Cuál es el trazo** dice qué objeto es la guía (la lista marca el papel de cada uno: **Trazo** o **Copiar**). De fábrica es el último de la selección.
3. Elige el **Reparto**:
   - **Por número de copias** (8 de fábrica), o
   - **Por distancia** entre copias (10 mm de fábrica); el diálogo muestra el **largo del trazo**.
4. **Girar según la tangente del trazo** apunta cada copia en la dirección del trazo; el **Giro adicional** se suma a esa dirección. Se toma la forma como orientada hacia la derecha.
5. **Conservar los originales** (apagado de fábrica): apagado, los originales se quitan y quedan solo las copias; encendido, se quedan donde estaban además de las copias.
6. La vista previa muestra las copias sobre el trazo. El máximo es de 2000 copias.

**Ejemplo: una guirnalda de estrellas sobre una curva.** Dibuja una estrella pequeña y una curva con la herramienta de línea. Selecciona las dos, abre el comando, pon **Por número de copias** en 12 y apaga el giro para que todas queden derechas (o enciéndelo para que sigan la curva). **Ejemplo: agujeros de ventilación en un arco**: un círculo de 4 mm, un arco de guía y **Por distancia** de 9 mm.

Las imágenes no se pueden girar: si las incluyes, apaga el giro o quítalas. No hay escala progresiva a lo largo del trazo.

## Arreglo virtual

Un **arreglo virtual** hace que un solo objeto se grabe o corte varias veces **sin crear los objetos**. El objeto se sigue editando como uno solo; las copias se ven como **fantasmas** tenues en el lienzo y se generan al enviar el trabajo.

1. Selecciona **un** objeto (si pertenece a un grupo, desagrúpalo o suéldalo antes).
2. **Arreglo virtual…**
3. Elige el tipo:
   - **Rejilla**: **Columnas**, **Filas** y el **Hueco horizontal** y **vertical** entre los bordes de cada copia (negativo = se solapan). 3 × 3 y 5 mm de fábrica.
   - **Circular (alrededor de un centro)**: copias en total, **Ángulo total** (360° de fábrica), centro X e Y y si las copias **giran** hacia el centro.
4. **Asignar arreglo.** Para cambiarlo después, ábrelo otra vez (**Actualizar arreglo**).

Para quitarlo o volverlo real hay dos comandos: **Quitar arreglo virtual** y **Convertir arreglo en objetos** (cada copia pasa a ser un objeto de verdad).

**Ejemplo: 40 posavasos en una lámina.** Dibuja un solo posavasos (un contorno, no un grupo), ponle un arreglo virtual de 5 columnas y 8 filas con 3 mm de hueco, y tendrás el trabajo completo sin 40 objetos en el documento. Si cambias la forma del original, las 40 copias cambian a la vez. Con **Convertir arreglo en objetos** podrás tocar una copia suelta.

![Un objeto con arreglo virtual: las copias aparecen como fantasmas punteados alrededor del original](../../apps/desktop/docs/screenshots/42-arreglo-virtual.png)

> **Ojo:** el lienzo dibuja hasta 600 fantasmas a la vez, pero el trabajo lleva todas las copias (hasta 2000). **Ajustar al diseño**, el tamaño de la selección y el aviso de salirse de la cama **no cuentan las copias**: mira la [vista previa](07-vista-previa.md) para comprobar que todas caben en la cama. De una imagen se graban las copias, pero el lienzo no dibuja su fantasma.

## Acoplar y mover juntos

**Acoplar y mover juntos…** pega piezas entre sí sin medirlas a mano.

1. Selecciona la **forma de referencia** y las que se van a mover. En el diálogo, **Forma de referencia (no se mueve)** elige cuál se queda quieta (la primera de fábrica); la lista marca **Fija** y **Mueve**.
2. Elige el **Modo**:
   - **Deslizar hasta el borde**: cada forma se desliza en la **Dirección** elegida (izquierda, derecha, arriba o abajo) hasta tocar la referencia. Avanzan solo las que quedan frente a ella.
   - **Juntar hacia la referencia**: cada forma avanza en línea recta hacia el centro de la referencia hasta tocarla.
3. **Hueco**: separación que queda al tocar (puede ser negativa, para que se solapen).
4. **Mover todas juntas**: conservan su posición relativa y se mueven como un bloque.
5. La vista previa marca la referencia y las posiciones nuevas. Las formas que no llegan a tocar la referencia se quedan donde están y se avisa cuántas.

**Ejemplo: acercar piezas para ahorrar viajes.** Ves cuatro piezas sueltas a la izquierda de un panel. Con el panel como referencia, **Deslizar hasta el borde** hacia la derecha y 2 mm de hueco: quedan pegadas al panel con 2 mm de separación.

Trabaja con las **cajas envolventes** (no con el contorno real) y no acopla a los bordes de la cama (para eso están **Alinear respecto a** la cama y **Centrar en la cama**).

## Agrupar automáticamente por contención

**Editar > Agrupar automáticamente por contención** forma un grupo por cada pieza, con los huecos y grabados que quedan dentro de su contorno exterior, como lo entiende el motor (profundidad par-impar). Una isla dentro de un hueco es una pieza nueva.

**Ejemplo: un DXF con 30 piezas con sus agujeros.** Al importarlo, todo viene suelto: mover una pieza deja los agujeros atrás. Selecciónalo todo y usa el comando: cada pieza queda en su grupo con sus agujeros, y ya puedes moverla, alinearla o usar el relleno por grupos y **Terminar cada grupo antes del siguiente** (ver [Capas y ajustes de corte](05-capas-y-ajustes.md)). Si ya sabes que lo necesitas, activa **Agrupar por contención** en **Archivo > Importar con opciones…** y el archivo llega ya agrupado.

Pide al menos dos formas cerradas. Usa el contorno cerrado más grande de cada objeto; lo que ya está agrupado, las líneas abiertas, los textos y las imágenes no entran.

## Nesting: acomodar piezas

**Organizar > Acomodar piezas (nesting)…** coloca las piezas seleccionadas para desperdiciar lo menos posible: sirve para sacar muchas piezas de una lámina o ajustar el diseño a un trozo de material que ya está usado.

### Qué entra

- Cada **objeto suelto** o **grupo** completo cuenta como una pieza. Los objetos bloqueados u ocultos no se mueven.
- **Copias**: en la lista de piezas cada una tiene su número de copias (o **Copias de todas**). Hasta 2000 piezas en total.
- **Contenedor**: **Cama** (con su tamaño) o **Material: un objeto del documento**. El objeto material tiene que ser una forma cerrada; no se mueve ni se acomoda, las piezas caben dentro de su contorno y esquivan sus huecos.

### Opciones

| Opción | Qué hace |
|---|---|
| **Separación entre piezas** | Distancia mínima entre piezas (1 mm de fábrica). Pon al menos el ancho de tu corte (kerf) más un poco de margen. |
| **Margen al borde** | Distancia mínima al borde del contenedor (0 de fábrica). |
| **Rotaciones** | **Ninguna**, **0 / 90 / 180 / 270°** (de fábrica), **Cada cierto ángulo** (**Paso de giro**, 15° de fábrica) o **Libre**. Más rotaciones aprovechan más y tardan más. |
| **Permitir espejo** | Las piezas se pueden voltear. Apágalo si la pieza tiene lado bueno. |
| **Usar huecos de otras piezas** | Piezas pequeñas pueden caer dentro de los agujeros de las grandes. |
| **Criterio** | **Gravedad** (compacta hacia una esquina) o **Abajo a la izquierda**. |
| **Tope de tiempo** | De 1 a 600 s (20 de fábrica). Es lo máximo que se dedica a buscar un acomodo mejor. |
| **Semilla** | Otro número, otra variante del acomodo; útil para probar si sale mejor. |

![Diálogo de nesting con las piezas, el contenedor, las opciones y la vista previa con el aprovechamiento](../../apps/desktop/docs/screenshots/39-nesting.png)

### Cómo se usa

1. Selecciona las piezas y abre el comando. La vista previa muestra cómo están ahora.
2. Elige el contenedor y las opciones. Pulsa **Empezar**.
3. Mientras calcula, la vista previa se actualiza y la ventana muestra el **aprovechamiento** (área de las piezas contra la del contenedor), cuántas piezas están **colocadas** y el tiempo. **Detener** corta la búsqueda y deja el mejor acomodo hasta ese momento; si llega al tope de tiempo, lo avisa.
4. Si alguna pieza no cupo, la ventana la lista (**No cupieron**). Cambia el contenedor, las rotaciones o la separación.
5. Cuando te guste, **Aplicar**: las piezas se mueven (y se crean las copias). Es un solo paso de **Deshacer**. **Cancelar** no cambia nada.

**Ejemplo: 24 llaveros de acrílico en una lámina de 300 × 200 mm.** Dibuja un llavero, selecciónalo, y en el diálogo pon **Copias** en 24, separación de 2 mm, rotaciones de 0 / 90 / 180 / 270° y **Empezar**. En unos segundos ves el aprovechamiento (por ejemplo, 71 %) y las piezas ordenadas; si dos no caben, cambia a rotaciones de 15° y vuelve a probar. **Aplicar** y revisa la [vista previa](07-vista-previa.md).

**Ejemplo: aprovechar un retazo irregular.** Importa o dibuja el contorno del retazo (por ejemplo, con huecos de los cortes anteriores) en una capa sin salida, elige **Material: un objeto del documento** y el nesting coloca las piezas dentro, esquivando los huecos.

> **Consejo:** el motor trabaja con una versión ligeramente simplificada de cada contorno, pero siempre conservadora (nunca más chica que la pieza real), así que las piezas no se solapan y quedan al menos a la separación pedida. Deja una separación mayor que el kerf, y si el material tiene veta, apaga el espejo y no uses rotación libre: Tlaku no conoce la veta, tú decides qué rotaciones son aceptables.

> **Ojo:** el acomodo no cambia el orden de corte; ese lo decide el [planificador](07-vista-previa.md) como siempre. El nesting solo coloca las piezas, sin dividir el trabajo en varias hojas.

Siguiente: [Guías, puentes, punto de inicio y proyecto](03e-guias-puentes-y-proyecto.md)
