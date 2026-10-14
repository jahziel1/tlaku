# Seguridad del galvo

Esta página es la más importante de todo lo que hay sobre el galvo. Léela entera **antes** de conectar una tarjeta, aunque pienses probar solo con el emulador, y vuelve a ella cada vez que cambies de fuente láser o de montaje.

> **Qué es esto:** un láser de **fibra** (o UV, o CO2 galvo) de uso industrial. No es un diodo de 5 W con una tapa de acrílico. Puede dañar **de forma permanente** la vista de quien esté en la habitación, iniciar fuegos y perforar la pieza, el soporte y el propio cabezal en cuestión de milisegundos. El soporte galvo de Tlaku es **experimental y no se ha probado con una tarjeta real**: el software no puede garantizar tu seguridad. Las protecciones de esta página son físicas y son tuyas.

> **Dónde se maneja:** el galvo se maneja desde la app (ver [Galvo (experimental)](13c-galvo.md), «El galvo en la app») y desde la línea de comandos (`tlaku galvo`). Todo lo que dice esta página vale igual en los dos.

## Los peligros, uno por uno

### 1. El haz es invisible y es de clase 4

Una fibra típica emite a 1064 nm, **infrarrojo cercano que no se ve**. No hay un punto brillante que te avise: el haz puede estar apuntando a tu cara y no lo sabrías. Es láser **clase 4**: incluso una **reflexión difusa** (el haz rebotando en un metal pulido, un vidrio, una pared) puede dañar el ojo. El UV y el CO2 galvo tienen sus propios riesgos y longitudes de onda; el principio es el mismo.

- **Nunca mires el haz**, directo ni reflejado, ni "un momentito" para comprobar si sale.
- **Nunca pongas una mano, un papel ni una herramienta** en la trayectoria para "ver" si marca.
- El **puntero rojo** es una luz aparte: indica **dónde irá** el haz, pero **no indica que el láser esté apagado**. Que el puntero esté encendido o apagado no te dice nada sobre el estado de la fuente.

### 2. Gafas con la longitud de onda y la densidad óptica correctas

Las gafas de seguridad **tienen que corresponder a tu láser**: se rotulan con la **longitud de onda** que filtran y con su **densidad óptica (DO, o OD en inglés)**.

- Una gafa para CO2 (10 600 nm) **no protege** de una fibra (1064 nm), ni una de diodo azul de una de UV. Comprueba el rótulo contra la ficha de tu fuente.
- La **DO mínima** que necesitas depende de la potencia de tu láser, de la duración del pulso y de la distancia: la fijan las normas (ANSI Z136.1, IEC 60825-1, EN 207) y el fabricante de tu fuente o un responsable de seguridad láser. **Tlaku no te la puede calcular y no inventa un número**: pregunta a quien vendió la fuente y exige un rótulo que lo cubra.
- Las gafas se **rayan, se quiebran y caducan**. Una gafa dañada no es una gafa.
- **Todas las personas** que estén en la habitación mientras la fuente está conectada necesitan las suyas, aunque no toquen la máquina.

### 3. Recinto cerrado, con enclavamiento y obturador

- Trabaja con el cabezal dentro de un **recinto cerrado** que bloquee el haz, con una **tapa con enclavamiento** (que apague el láser al abrirla).
- Usa un **obturador** (*shutter*): el propio manual de la tarjeta lo recomienda. Es una barrera física que corta el haz aunque el software o la tarjeta fallen.
- Mantén a mano el **paro de emergencia**, a un brazo de distancia y sin que tengas que pasar por delante del haz.
- Ten un **extintor** adecuado cerca y no dejes la máquina sola mientras marca.

### 4. El software no es el último seguro

Hay reportes de usuarios de otros programas de galvo en los que el espejo se detiene pero el haz **sigue encendido**, o el láser queda encendido al terminar un trabajo. Un haz fijo en un punto **perfora** la pieza, el soporte o el cabezal, y es la forma típica en que un galvo provoca un incendio. Si se cae el cable USB a mitad de un trabajo, **lo que hace la tarjeta sin que el equipo le diga nada no se puede garantizar por software**: hay que medirlo en tu máquina (ver el plan de prueba, más abajo).

### 5. Parámetros que dañan la fuente

Una fuente de fibra tiene reglas de arranque y de pulso. Con una frecuencia, un ancho de pulso o una potencia fuera de lo que admite tu fuente se puede **dañar el módulo láser** (retropropagación). Por eso Tlaku te obliga a escribir el **rango de frecuencia** de tu fuente (`--freq-range`), los **anchos de pulso** permitidos (`--pulse-widths`) y una **potencia máxima** (`--max-power`), y **rechaza** todo lo que esté fuera, antes de conectar. Esos valores salen del manual de **tu** fuente, no de los de fábrica de Tlaku.

## Lo que Tlaku hace para protegerte (y sus límites)

Esto es lo que el código garantiza, **verificado contra la tarjeta simulada** y no con una real:

- **Rechaza lo desconocido.** Solo acepta la tarjeta `9588:9899`. Los clones y las tarjetas sin firmware se rechazan con el motivo.
- **Pide que lo confirmes.** Con hardware real, todo menos `tlaku galvo list` exige la bandera `--i-understand-laser-safety`; sin ella ni siquiera se abre el USB. Además, `mark` pregunta si tienes las gafas puestas, el recinto cerrado, el obturador y el paro de emergencia listos.
- **Lista blanca de comandos.** Solo se envían los comandos que Tlaku necesita. **Nunca** se escribe la EEPROM, el firmware, la tabla de corrección ni la configuración de la tarjeta, aunque algo lo pidiera.
- **Revisa cada lista** antes de enviarla: que empiece y termine bien, que no haya una marca antes de fijar velocidad, retardos, frecuencia y potencia, y que la potencia, la frecuencia y el ancho de pulso estén dentro del perfil.
- **El láser empieza desarmado.** Cada marcado arranca con la fuente desarmada y la arma justo antes de ejecutar la lista. El **encuadre con el puntero rojo nunca habilita el láser**.
- **Apagado garantizado.** Salga como salga el trabajo (termine bien, falle o lo canceles con **Ctrl+C**), Tlaku ejecuta una **secuencia de apagado completa y la repite si falla**, y la repite también si la sesión se cierra sucia. Si no puede confirmar que apagó la fuente, **te lo dice** («NO hubo confirmación: puede seguir habilitada»); en ese caso usa el paro de emergencia y abre el recinto. Si lo que se cae es el cable USB, la orden de apagado **no tiene por dónde llegar** (ver el punto 4 de los peligros).
- **Verificación sin láser al dar de alta.** En la app, el asistente de **Agregar máquina** termina con **Verificar sin láser**: lee la tarjeta y comprueba el perfil sin mover los espejos ni encender el puntero o la fuente (con una tarjeta USB real, pide antes la confirmación de seguridad). Ver [Galvo (experimental)](13c-galvo.md).
- **Confirmación en cada conexión.** En la app, conectar con una tarjeta USB real abre **Seguridad láser antes de conectar** y exige las tres casillas (gafas, recinto, paro de emergencia) **cada vez**.
- **Enclavamiento por software (opcional).** En la app, el perfil tiene **Enclavamiento por entrada** (y la línea de comandos, `--require-input MASK=VALOR`) exige **antes de cada trabajo y de cada encuadre** que las entradas de la tarjeta cumplan `(entradas & MASK) == VALOR`. Es útil si la puerta o el paro de emergencia de tu máquina llegan a una entrada de la tarjeta. **Pero los bits del paro de emergencia y de las alarmas de la fuente no están documentados**: tienes que medirlos tú, y mientras no los pongas, Tlaku no puede comprobar el enclavamiento (y lo avisa).

**Lo que no puede hacer:** impedir un daño por un USB desconectado, un dato de unidades equivocado, una fuente mal configurada ni una persona sin gafas. Tlaku no sustituye a la protección física.

## El modo `--red-only`

`tlaku galvo mark --red-only` recorre **todo el trabajo**, con las mismas posiciones, velocidades y retardos que tendría el marcado, pero con el **láser deshabilitado** y solo el **puntero rojo** encendido. Sirve para ver el recorrido real sobre la pieza antes de disparar:

```
tlaku galvo mark pieza.svg --red-only --frequency 30 --i-understand-laser-safety
```

Es una buena prueba, pero **no es una garantía de seguridad**:

- Depende de que el puntero rojo y el bit de salida del láser sean los que Tlaku supone (el bit 8 para el puntero y el 0 para el láser, de otros proyectos, **sin verificar en una tarjeta**).
- Si tu cabezal no tiene puntero rojo, no verás nada.
- Un `--red-only` no pregunta la confirmación de siempre porque el láser no se arma; **sigue necesitando** la bandera de seguridad con hardware real.

Y el comando hermano `tlaku galvo light` dibuja con el puntero rojo el rectángulo (o el contorno con `--contour`) en bucle hasta Ctrl+C, sin habilitar nunca el láser.

## Por qué no hay pausa

En la interfaz de otros programas hay un botón **Pausa**. En Tlaku, con el galvo, **no existe**: solo se puede **detener**.

La razón es de seguridad: en los reportes de usuarios de galvos, **pausar detiene los espejos pero el haz puede seguir encendido** (a veces al 95 % de la potencia). Un espejo quieto con el haz encendido perfora la pieza en un punto y puede encender un fuego. Detener, en cambio, ejecuta la secuencia de apagado completa. Si necesitas parar, detén el trabajo (**Ctrl+C**), o usa el paro de emergencia.

## Plan de prueba con la tarjeta sin fuente

Antes de conectar la fuente láser, haz esta prueba con **la tarjeta y el cabezal alimentados y la fuente láser desconectada** (o con el cable DB25 de la fuente desconectado). Es el plan que usa el proyecto para la primera validación con hardware (`docs/hardware/galvo.md`, sección 9). Lo ideal es tener un osciloscopio o un analizador lógico para medir las señales: sin eso, la prueba sigue siendo útil para el movimiento, pero no para comprobar que el láser queda apagado.

1. **Montaje.** La tarjeta LMCV4-FIBER-M con el cabezal galvo alimentado. **Enciende primero la tarjeta y la fuente del cabezal, antes que el láser**: si no, el haz puede desviarse sin control (aviso del manual de la tarjeta). El osciloscopio en las señales del cabezal (reloj y sincronía) y, en el conector del láser, en las señales del oscilador, del amplificador, de la frecuencia y del puntero rojo.
2. **Identificar.** `tlaku galvo list` y luego `tlaku galvo status --i-understand-laser-safety`: debe aparecer la tarjeta `9588:9899`, con su serie y su versión, y el estado «en reposo». Comprueba que **no se movió nada**.
3. **Mover sin láser.** `tlaku galvo light diseño.svg --i-understand-laser-safety --loops 3`: el rectángulo de un cuadrado de 20 mm debe verse (con el puntero rojo, o midiendo las señales del cabezal) con el **centro** en el centro del campo y el tamaño correcto. Compáralo con el campo de tu lente.
4. **Comprobar que el láser queda apagado.** Con el osciloscopio, durante y después de `tlaku galvo mark diseño.svg --red-only --frequency 30 --i-understand-laser-safety`, las señales de la fuente (oscilador, amplificador, frecuencia) deben **permanecer en reposo**. Prueba también **cancelar con Ctrl+C a la mitad** y comprueba que siguen apagadas. Y, **lo más importante**, **desconecta el cable USB a mitad de un trabajo** y mira si las señales quedan apagadas: ninguna fuente de software puede garantizarlo, hay que medirlo en tu tarjeta.
5. **Medir la frecuencia y los retardos.** La frecuencia generada en la señal de frecuencia contra la pedida con `--frequency`, y los retardos de encendido y apagado contra lo pedido (resolución, signo, unidad). Esto es lo que comprueba que las **conversiones de unidades** son las correctas, y es la mayor incógnita de todo el soporte.

Solo si todo lo anterior sale bien, y **con la fuente conectada**:

6. **Potencia mínima, recinto cerrado, gafas con la DO correcta y obturador**, sobre un material que no importe. Verifica un **punto**, una **línea** y un **cuadrado**. Compara con el mismo trabajo hecho en EZCAD2 y mide la distorsión del campo con una rejilla grabada.
7. **Cuéntalo.** Un resultado real, bueno o malo, es lo más valioso que puedes aportar: **Ayuda > Reportar un problema…** con la salida de los comandos, la tarjeta, la fuente y la lente.

Hasta que alguien complete estas pruebas con una tarjeta real, el galvo seguirá marcado **experimental**.

## Resumen en diez líneas

1. Fibra = clase 4, invisible: **no mires el haz, nunca**.
2. Gafas con la **longitud de onda** y la **DO** de tu láser; todos las usan.
3. **Recinto cerrado, enclavamiento, obturador y paro de emergencia** al alcance.
4. Sin pausa: solo **Detener** (Ctrl+C) y el paro de emergencia.
5. Primero `--emu` y `--dry-run`; después la tarjeta **sin fuente**; después `--red-only`; después la fuente a potencia mínima.
6. Los rangos de frecuencia, de pulso y de potencia son los de **tu fuente**.
7. `--i-understand-laser-safety` es una confirmación de que leíste esto, no un trámite.
8. No uses `--yes` en las primeras pruebas.
9. Nada de esto se ha probado con una tarjeta real.
10. En duda, apaga la fuente y abre el recinto.

Volver a: [Galvo (experimental)](13c-galvo.md). Siguiente: [Venir desde LightBurn](14-desde-lightburn.md)
