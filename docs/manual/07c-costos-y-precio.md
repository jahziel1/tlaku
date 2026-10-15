# Costos y precio por pieza

Si vendes lo que haces (Etsy, Mercado Libre, ferias, pedidos a medida), **Costos y precio** te dice cuánto te cuesta hacer una pieza y a cuánto venderla para que, después de la comisión de la plataforma y de los impuestos, te quede la ganancia que quieres. Usa los datos que Tlaku ya calcula del trabajo (tiempo, tiempo con el láser encendido, área y longitud de corte), así que se actualiza solo cuando cambias el diseño.

Es una **ayuda para cotizar, no una promesa**: el tiempo es una estimación y los precios los escribes tú. **Todo se queda en tu computadora; no se envía nada a internet** (ni siquiera hay tipo de cambio). Los atajos se escriben con `Ctrl/Cmd`: `Cmd` en macOS y `Ctrl` en Windows y Linux.

## Abrirla

- Botón **Costos y precio…** en la pestaña **Máquina** (sección Trabajo), junto a **Ajustes de optimización…**.
- El mismo botón en la barra de la **vista previa**.
- El menú **Máquina > Costos y precio…**.

Se abre un diálogo con tus datos a la izquierda y el **resultado** a la derecha: el **precio sugerido por pieza** en grande, el costo, el desglose y el precio del lote. Mientras está abierto, el resultado sigue al trabajo: si el diseño, las capas, la máquina activa o la selección cambian, vuelve a calcular.

## Qué se le pide

### Tiempo de máquina

Elige qué tiempo usar:

- **Estimado de Tlaku**: el de la vista previa. Es una estimación y suele quedar corta frente a lo real; el resultado lo avisa («Tiempo estimado sin calibrar»).
- **Estimado calibrado con tus trabajos**: el estimado multiplicado por cuánto tardaron de verdad tus trabajos terminados en esa máquina. Usa el [historial de trabajos](09f-cola-e-historial-de-trabajos.md) (la mediana de tiempo real ÷ estimado de los últimos 20 trabajos) y necesita **al menos 3**. El diálogo muestra el factor («los trabajos duraron 1,35× lo estimado»).
- **Tiempo real del último trabajo**: el tiempo del último trabajo terminado en esa máquina, sin contar pausas. Útil si repites siempre la misma pieza.

Si eliges una opción y todavía no hay historial suficiente, queda deshabilitada y se usa el estimado (el diálogo lo dice).

### Máquina y electricidad

Estos valores **se recuerdan por máquina**: al cambiar de máquina activa aparecen los suyos, y una máquina nueva empieza en cero.

- **Costo de la máquina por hora**: lo que quieres recuperar de la compra, el taller y el mantenimiento por cada hora de trabajo. Se cobra sobre el tiempo total de la pasada.
- **Precio** y **vida útil** del tubo (CO2) o del diodo, en horas con el láser encendido. El **desgaste** es precio ÷ vida útil, y **solo se cobra por el tiempo con el láser encendido** (el de cortar y grabar, no el de moverse en vacío). Con vida útil 0 no se cuenta.
- **Consumo de la máquina** y **de accesorios** (extractor, compresor de aire, enfriador de agua), en W, y el **precio del kWh**. La electricidad es (W de la máquina + W de accesorios) ÷ 1000 × horas × precio del kWh.

### Material

Tres maneras de cobrarlo:

- **Por área (rectángulo que envuelve el diseño)**: el área de la caja que rodea todo lo que sale en el trabajo, más el **desperdicio**. Es lo más simple, y si acomodas piezas sobra material; el resultado lo avisa.
- **Por área real de las formas**: la suma de las áreas de las formas **cerradas** (un anillo o la «o» de un texto descuentan su hueco). Los trazos abiertos no tienen área; si el diseño no tiene ninguna forma cerrada se usa la caja envolvente y se avisa. Dos objetos que se encimen se cuentan por separado.
- **Por hoja (piezas por hoja)**: precio de la hoja ÷ las piezas que salen de cada hoja con tu acomodo. Es la más justa si las acomodas con [nesting](03d-copias-y-nesting.md). Se muestra cuántas **hojas hacen falta** para la cantidad.

En las dos primeras escribes el **precio y el tamaño de la hoja** (ancho × alto en mm) y el costo es el área usada × (1 + desperdicio) × precio ÷ área de la hoja.

**Perfiles de material.** Escribe un nombre y pulsa **Guardar perfil** para recordar el precio, el tamaño, el desperdicio, las piezas por hoja y la forma de cobrar; elígelo después en la lista **Perfil de material**. Guardar con un nombre que ya existe lo reemplaza. **Eliminar perfil** pide confirmación. Hay hasta 100 perfiles.

### Mano de obra y extras

- **Tarifa de mano de obra por hora**.
- **Preparación del lote** (minutos de diseño, ajustes y pruebas): se paga **una sola vez** y se reparte entre todas las piezas.
- **Acabado por pieza** (minutos de lijar, pegar, armar).
- **Empaque y consumibles por pieza**: un costo fijo (bolsa, tarjeta, pegamento).

### Venta

- **Moneda**: de fábrica la de la región del sistema (`es-MX` da pesos mexicanos; sin región, MXN en español y USD en inglés). Los importes salen con el símbolo y el formato de tu región. Monedas sin centavos (CLP, PYG) se redondean al peso.
- **Cantidad a vender** y **piezas por pasada del diseño**: si el diseño ya trae 12 llaveros, pon 12, y el tiempo, la electricidad y el material se reparten entre ellos. El botón **Usar las N del diseño** cuenta las piezas por ti (cada grupo cuenta como una y las copias del [arreglo virtual](03d-copias-y-nesting.md) suman).
- **Margen de ganancia** y sobre qué se calcula (el costo o el precio de venta).
- **Comisión de la plataforma** e **impuestos**, en %.
- **Redondear el precio hacia arriba a**: el centavo, 0,50, 1, 5 o 10.

## Cómo se calcula

Todo es por pieza. Cada línea del costo se redondea al centavo de la moneda, así que el desglose suma exacto.

```
costo = máquina + desgaste + electricidad + material + acabado + preparación ÷ cantidad + consumibles
```

El **precio** se calcula a partir del costo `C`, el margen `m`, la comisión `c` y los impuestos `t` (todos como fracción: 30 % es 0,30). La comisión y los impuestos se cobran sobre **el precio final** (lo que paga el cliente), no sobre el costo; por eso se **dividen** en lugar de sumarse. Lo que te queda es `precio × (1 − c − t)`, y eso debe cubrir el costo más tu ganancia:

```
margen sobre el costo:   precio = C × (1 + m) ÷ (1 − c − t)
margen sobre el precio:  precio = C ÷ (1 − c − t − m)
```

El diálogo trae la sección **Cómo se calcula** con la fórmula y **tus números** ya puestos (`$60.00 × (1 + 0.3) ÷ (1 − 0.1 − 0) = 86.6667 → $86.67`). Después el precio se **redondea siempre hacia arriba** al paso elegido, así que la ganancia real nunca baja de la pedida.

> **Ejemplo.** Una pasada de 1 h (30 min con el láser encendido), caja de 100 × 100 mm sobre una hoja de 600 × 400 mm que cuesta 120, máquina a 60 por hora, tubo de 1 200 con 2 000 h de vida, 120 W a 2,50 el kWh, mano de obra a 100 por hora con 30 min de preparación y 6 min de acabado, 2 de empaque y 10 piezas: máquina 60 + desgaste 0,30 + electricidad 0,30 + material 5 + acabado 10 + preparación 5 + empaque 2 = **82,60** de costo. Con 30 % de margen sobre el costo y 10 % de comisión: 82,60 × 1,3 ÷ 0,9 = 119,31 → **119,32** por pieza (1 193,20 el lote), de los cuales 11,93 son la comisión y 24,79 tu ganancia. Sumar «30 % + 10 %» al costo daría 118,18, que no alcanza.

**Impuestos que se suman encima del precio** (el IVA de 16 %): la casilla es el porcentaje **del precio final**. Si tu impuesto se suma, escribe 16 ÷ 1,16 = **13,79 %**.

Si la comisión, los impuestos (y el margen, cuando se calcula sobre el precio) suman **100 % o más**, ningún precio los cubre: el resultado dice **No hay precio posible** y qué bajar.

## Avisos que muestra

- **Tiempo estimado sin calibrar** (suele ser menor que el real) y **tiempo aproximado por distancia** (cuando el motor no dio los tiempos exactos).
- **El material se calcula con el rectángulo que envuelve el diseño**, **no hay formas cerradas**, **falta el tamaño de la hoja** o **las piezas por hoja**.
- **Todavía no hay tiempo ni área del trabajo**: sin objetos en capas con salida activa solo cuentan los gastos fijos.

Con **Solo la selección** encendida (en el panel del trabajo), la calculadora también cuenta solo lo seleccionado.

## Copiar y guardar el desglose

En el pie del diálogo:

- **Copiar desglose**: texto listo para pegar en un mensaje, una cotización o una nota, con el trabajo, el costo por línea, el precio y el lote.
- **Copiar CSV** y **Guardar CSV…**: las mismas filas como `Sección,Concepto,Valor,Unidad`, en el idioma de la interfaz. Los números llevan **punto decimal y sin separador de miles** para que cualquier hoja de cálculo los lea; el archivo lleva la marca UTF-8 para que Excel muestre bien los acentos. Si tu hoja de cálculo espera punto y coma (algunas configuraciones regionales), usa **Datos > Texto en columnas** y elige la coma.

## Qué se recuerda

En `localStorage` de esta computadora (clave `tlaku.costs`), sin salir de ella: los valores de cada máquina, los perfiles de material, la moneda, las tarifas, el margen, la comisión, los impuestos, el redondeo, la cantidad y la fuente de tiempo. **No** viajan en el proyecto ni en los paquetes de preferencias.

## Lo que no hace

- **No promete un precio**: el tiempo es una estimación y no sabe cuánto vale tu hora ni qué cobra tu plataforma. Mira el desglose y ajusta.
- No conoce los precios de los materiales ni las comisiones de las plataformas, y no consulta tipos de cambio.
- No calcula cuántas piezas caben en una hoja: eso lo hace el [nesting](03d-copias-y-nesting.md); tú escribes el resultado en **Piezas por hoja**.
- El área real no descuenta lo que se encima entre objetos distintos.

Siguiente: [Conectar tu máquina](08-conectar-maquina.md)
