# Tiling y alimentador automático

El **tiling** divide un trabajo **más grande que la cama** en tramos que sí caben, los envía de uno en uno y te dice cómo mover el material entre un tramo y el siguiente. Sirve para un letrero de 1,2 m en un láser de 40 cm, un mural de piezas encajadas o un rollo de tela. **LightBurn no tiene tiling**: allí se resuelve a mano partiendo los vectores y registrando cada pieza con Print & Cut.

El **alimentador automático** es una cinta o unos rodillos que avanzan el material solos. Tlaku lo usa dentro del tiling para no tener que mover el material a mano entre tramos.

> **Ojo:** el tiling y el alimentador se probaron con la máquina virtual y con pruebas automáticas. **No se han probado con una máquina real.** Los tramos se alinean con el material colocado a mano o con un alimentador: la precisión de cada empalme depende de lo bien que coloques el material, no de Tlaku. Empieza con un diseño de prueba en cartón y con el láser a potencia baja.

## Cuándo usarlo

- El diseño **no cabe en la cama** en uno de los dos ejes. Entonces el diálogo de iniciar y la sección **Trabajo** de la pestaña Máquina muestran un aviso con las medidas del diseño y de la cama («…: no cabe de una vez. Puedes enviarlo por tramos») y el botón **Dividir en tramos…**.
- Tu máquina tiene paso a través (una puerta o una ranura para que el material salga por delante o por detrás), o el material se puede mover sobre la cama y volver a fijar.
- Aceptas que cada empalme dependerá de cómo coloques el material: para piezas que tienen que encajar al décimo de milímetro, mejor que cada pieza quepa entera en la cama.

Si solo se pasa un poco (un par de centímetros) revisa antes si basta con girar el diseño 90 º o reducirlo.

## Paso a paso: un letrero de 600 × 300 mm en una cama de 400 × 415

1. Importa el diseño y, si solo quieres una parte, selecciónala (el tiling respeta **Solo la selección** del trabajo).
2. **Máquina > Dividir en tramos…** abre el panel (también el botón del aviso de arriba). Muestra el tamaño del diseño y de la cama.
3. Ajusta las opciones:

| Opción | Qué hace | De fábrica |
|---|---|---|
| **Ancho del tramo** y **Alto del tramo** | Tamaño máximo de cada tramo | La cama menos el margen por cada lado (390 × 405 mm con una cama de 400 × 415 y 5 mm de margen) |
| **Solape** | Cuánto se repite del diseño en cada empalme | 5 mm |
| **Margen a la cama** | Aire entre el borde de la cama y el tramo (el overscan lo necesita) | 5 mm |
| **Orden** | **Por filas** (siempre de izquierda a derecha) o **Serpentina** (cada fila impar al revés: se recorre menos distancia) | Por filas |
| **Marcas de alineación en el solape** | Pequeñas cruces en la zona de solape, grabadas en los dos tramos | Activadas |

4. El lienzo dibuja el **plano de los tramos** y el panel dice cuántos son: aquí, **2 tramos (2 columnas × 1 filas)**: el primero cubre de 0 a 390 mm de ancho y el segundo de 385 a 600 mm; los 5 mm de 385 a 390 se repiten.
5. **Empezar.** El asistente pasa a **Tramo 1 de 2: coloca el material así…** y te dice qué zona del diseño debe quedar en la cama y que su **esquina inferior izquierda** debe quedar en el origen más el margen.
6. Coloca el material, comprueba con un encuadre que está donde debe y pulsa **Enviar tramo 1**. Cuando termina, el asistente pasa al tramo 2 y te pide colocar el material.
7. Mueve el material, **alinea las cruces** (ver abajo) y pulsa **Enviar tramo 2**. Al final dice **Listo** y cuántos tramos se enviaron y cuántos se saltaron.

Los tramos se calculan en coordenadas absolutas de la cama: **haz home antes** y no cambies el origen entre tramos.

## El solape y las marcas de alineación

El **solape** es la franja que se corta o graba en los **dos** tramos. Sirve para que un error de colocación de uno o dos milímetros no deje un hueco: lo que caiga en la franja se repite. Pero lo que cae en la franja se corta o se graba dos veces: en un corte casi no se nota, y en un grabado de relleno la franja queda más marcada. Por eso:

- Usa un solape **pequeño pero mayor que tu error de colocación**: 3 a 5 mm es un punto de partida razonable. Con cero no hay cruces.
- Para **corte**, el solape no suele dar problemas: el trazo ya cortado no se vuelve a ver. Para **grabado de relleno**, la franja se nota; prueba con una potencia más baja.

Las **marcas de alineación** son cruces pequeñas (miden el 80 % del solape, con un máximo de 6 mm) puestas en las zonas de solape, **también grabadas en los dos tramos**. Hacen de referencia: tras mover el material, el segundo tramo volverá a grabar las cruces, y si el material está bien puesto caerán **encima de las del primer tramo**. Cuando lo estén, las líneas coinciden; si ves dos cruces, mueve el material hasta que se junten.

- Las cruces están en su propia capa («Marcas de alineación», azul claro `#00a0ff`, grabado suave a 3000 mm/min y 12 % de potencia). **Se graban sobre el diseño**, así que si molestan, usa un solape más grande y una potencia baja, o desactívalas.
- Si ya tienes una capa de ese color, Tlaku usa tus ajustes tal cual; así puedes darles tu propia potencia y velocidad.
- Para ver dónde cae una cruz sin quemar nada, usa el puntero rojo de tu máquina o el encuadre.

La alineación fina del material es **física**: usa topes, escuadras o una plantilla, y pega el material. Es lo que más calidad da al empalme.

## Enviar por tramos

En cada tramo el asistente muestra:

- **Tramo n de total: coloca el material así…**, con la zona del diseño que cae en la cama.
- **Enviar tramo n**: lo manda a la máquina; mientras sale verás **Enviando tramo n de total…** y puedes **Cancelar el tramo**.
- **Saltar tramo n**: lo da por hecho sin enviarlo (por ejemplo, si ese tramo ya lo cortaste).
- Un **plano** donde tocas un tramo para **repetirlo o ir a él**.

Si un tramo no sale (error, cancelación, la máquina no está lista), el asistente dice **El tramo n no salió**, con el motivo, y te deja **Reintentar tramo n** o **Volver**. El tramo queda pendiente; no se pierde.

El motor recorta cada tramo antes de planificar: los trazos se parten en el borde del tramo (no queda ninguna línea a medias), los rellenos se recortan como una región y las imágenes se enmascaran.

## Retomar

Si se cierra la app, se va la luz o se corta la conexión a mitad, el avance queda guardado en tu equipo. Al abrir **Dividir en tramos…** con el **mismo diseño, las mismas opciones y la misma máquina**, el asistente ofrece **Retomar** («Hay un trabajo por tramos sin terminar (3 de 6 tramos listos)») o **Descartar**. El tramo que estaba en curso vuelve a pedir confirmación antes de enviarse. Si cambias el diseño o las opciones, el avance guardado ya no corresponde y no se ofrece.

## Alimentador automático

Si tu máquina tiene un alimentador (una cinta transportadora o unos rodillos que mueven el material con un motor), puedes decirle a Tlaku cómo avanzarlo y el tiling lo hará solo entre tramos.

### Configurarlo

**Máquina > Gestionar dispositivos…**, elige el perfil y busca el grupo **Alimentador automático**:

1. Marca **Tengo un alimentador automático**.
2. **G-code de avance:** una línea por orden; `{mm}` se sustituye por la distancia de cada avance. De fábrica es:

   ```
   G91
   G0 Y{mm}
   G90
   ```

   (relativo, mover, absoluto). Ajústalo al eje que mueve tu alimentador. Debajo ves **«Con este avance se enviaría: …»** con el G-code resultante.
3. **Avance por defecto:** la distancia (300 mm de fábrica) cuando no se conoce otra. Entre tramos se usa el alto del tramo menos el solape.
4. **Guardar cambios.**

Tlaku valida el G-code (no puede estar vacío, tiene que llevar `{mm}`, no admite otro marcador, ninguna línea de más de 120 caracteres) y el avance (positivo y de 5000 mm como máximo). Con una controladora Ruida o un K40 el grupo no aparece: no usan G-code.

### Cómo lo usa el tiling

El alimentador solo se usa cuando los tramos van **en una sola columna**, es decir, cuando el diseño cabe a lo ancho y se divide a lo largo. Si hay varias columnas, el panel avisa «El alimentador solo se usa cuando los tramos van en una sola dirección (una columna)» y el material lo mueves tú.

Con una sola columna, el flujo cambia:

1. El primer tramo **sí pide confirmación**: colocas el material y el asistente te dice «después el alimentador avanza solo con: …» y el G-code exacto.
2. Al terminar un tramo, la app espera a que la máquina vuelva a reposo, manda el avance (**Avanzando el material…**), vuelve a esperar y envía el tramo siguiente **sin pedir confirmación**.
3. Si el avance falla («El alimentador no pudo avanzar el material» o «la máquina no volvió a reposo»), el asistente se detiene en ese tramo y te deja **Reintentar**.

**Ejemplo.** Un rollo de tela: un diseño de 300 × 1000 mm en una cama de 400 × 415. Con el margen de 5 mm y el solape de 5 mm salen **3 tramos de una columna** (405, 405 y 200 mm de alto). Entre el primero y el segundo el alimentador avanza **400 mm** (405 menos 5 de solape), y entre el segundo y el tercero **195 mm** (el último es más corto).

> **Antes de usarlo, prueba el avance.** Manda a mano un avance corto (por ejemplo `G91`, `G0 Y10`, `G90`) y comprueba **hacia dónde se mueve el material**. El material tiene que avanzar en el sentido que deja bajo el láser la parte siguiente del diseño. Si va al revés, cambia el signo en el G-code (`G0 Y-{mm}`). Tlaku no puede saber cómo está montado tu alimentador. También comprueba que las tres líneas dejan la máquina en modo absoluto (`G90`) al terminar.

## Lo que no hace (todavía)

- No hay tiling desde la línea de comandos; es del asistente de la app.
- El alimentador solo se usa dentro del tiling con los tramos en una columna. **No** repite un trabajo N veces con avance entre copias (como el *Feeder* de LightBurn con las controladoras Ruida y Trocen) ni usa el alimentador propio de una controladora DSP.
- No parte formas enteras al elegir el borde del tramo: una forma que cruza el borde se corta en dos partes, una en cada tramo.
- Nada de esto se ha probado con un alimentador real.

Siguiente: [Control desde el celular](09e-control-desde-el-celular.md)
