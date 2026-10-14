# Control avanzado de la máquina

Lo que la pestaña **Máquina** ofrece además de mover, encuadrar e iniciar: **macros**, mover los ejes **Z** y **U**, **posiciones guardadas**, **enfocar**, el **disparo de prueba**, los **ajustes `$$`** de la controladora con respaldo, el **encuadre a baja potencia y en bucle**, el **láser 2** y los **paquetes `.tlakubundle`** para llevar tu configuración a otro equipo.

> **Ojo:** nada de esto se ha probado con una máquina real; se validó con el emulador y las máquinas virtuales ([Conectar tu máquina](08-conectar-maquina.md)). Todo lo que enciende el láser o escribe en la controladora pide confirmación, y aun así conviene probarlo primero en una máquina virtual. Casi todo esto es de la **familia GRBL** (GRBL, grblHAL, FluidNC, GRBL-LPC); con Marlin y Smoothieware varias funciones cambian o no existen, y con Ruida no aplican (ver [Ruida y K40](13b-ruida-y-k40.md)).

## Macros

Una **macro** es un botón que manda a la máquina una o varias líneas de G-code que guardaste en el perfil. Sirve para lo que haces siempre: encender el aire, llevar el cabezal a la esquina donde sacas la pieza, pedir el estado.

**Crear una macro.** Menú **Máquina**, **Gestionar dispositivos…**, elige el perfil y baja al grupo **Macros**:

1. Pulsa **Agregar macro** (hasta 10 por perfil).
2. Dale un **nombre** (aparece en el botón).
3. En **G-code (una línea por comando)** escribe las líneas, hasta 50.
4. Marca **Pedir confirmación antes de enviarla** si quieres verla antes de que salga.
5. **Guardar cambios**.

Los botones aparecen en la pestaña **Máquina**, sección **Macros**, y cada macro es también un comando (**Macro 1** a **Macro 10**) al que puedes asignar un atajo en **Editar > Editar atajos de teclado…** (ver [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md)).

**Ejemplos.**

| Macro | G-code | Qué pasa al pulsarla |
|---|---|---|
| Aire encendido | `M8` | Pide confirmación (no es de solo lectura) y manda la línea |
| Aire apagado | `M9` | Igual |
| Esquina de descarga | `$J=G90 G21 X0 Y380 F3000` | Pide confirmación, porque mueve la máquina; lleva el cabezal a X0 Y380 a 3000 mm/min |
| Información | `$I` y en otra línea `$G` | Son de solo lectura: salen directo, sin preguntar |

**Reglas de seguridad.** Las macros pasan por las mismas reglas que la [consola segura](09-enviar-con-seguridad.md):

- Solo las líneas de solo lectura (`$I`, `$$`, `$#`, `$G`, `$N` y `M5`) salen sin preguntar. Si cualquier línea mueve la máquina, enciende el láser o cambia un ajuste, **siempre se muestran las líneas exactas** y hay que confirmar, aunque no hayas marcado la casilla.
- Una macro **no puede** llevar `?`, `!` ni `~` (comandos de tiempo real): para pausar o reiniciar usa los botones del panel. El editor te dice qué línea no vale.
- **Nunca se envían durante un trabajo.**
- Las líneas salen una a una y la macro **se detiene en la primera que la máquina rechace**, diciendo cuál fue y qué contestó.
- Las macros no existen con Ruida.

## Mover con Z y U

Si el perfil declara un eje, la sección **Mover cabezal** suma el bloque **Ejes extra** con los botones **Subir** y **Bajar**.

- **Eje Z:** se activa en el perfil con **Eje Z motorizado** (grupo **Ejes y accesorios**, ver [Rotativo y eje Z](12-rotativo-y-eje-z.md)). **Subir** aleja el cabezal del material y **Bajar** lo acerca; con **Invertir Z** activado Tlaku ya compensa el sentido.
- **Eje U:** se activa con la casilla **La máquina tiene un eje U (rotativo u otro eje extra)**. Como los firmwares lo llaman distinto, elige la **Letra del eje U en el G-code** (`U`, `A`, `B` o `C`). Esta casilla solo habilita el movimiento a mano: **no cambia el G-code de los trabajos**.
- Cada clic mueve el mismo **paso** que el pad (de `0,01` a `500 mm`) a la **Velocidad** de **Mover cabezal**. Con un trabajo en curso, sin conexión o en alarma, los botones se deshabilitan y dicen por qué.

> **Ejemplo:** tu mesa motorizada baja con +Z. Marca **Eje Z motorizado**, pon el paso en `1`, pulsa **Bajar** tres veces para acercar el cabezal al material y confirma a ojo la distancia al enfoque.

## Enfocar

Con el eje Z activo, **Enfocar** lleva Z a una altura de enfoque que guardas una vez:

1. Enfoca a mano tu material (con **Subir** y **Bajar** o con el panel de la máquina) y anota la posición Z que informa tu controladora. La barra de estado de Tlaku solo muestra X e Y; la Z la ves en los reportes de estado de la **Consola**, si tu firmware la informa.
2. Escribe esa altura en **Altura de enfoque (Z)** (en mm, en las coordenadas de trabajo de la controladora).
3. Cuando cambies de material, ajusta el valor; después **Enfocar** lleva Z hasta ahí.

El comando también está en el menú de comandos como **Enfocar (mover Z a la altura de enfoque)**. Falta el autoenfoque y optimizar Z.

## Posiciones guardadas

Para volver rápido a lugares que usas: la esquina de carga, el centro de la mesa, la posición del rotativo.

1. Mueve el cabezal al lugar.
2. En **Posiciones guardadas** escribe un **Nombre de la posición** y pulsa **Guardar aquí** (necesita la máquina conectada). Se guardan hasta 20 por perfil, y un nombre repetido **reemplaza** la anterior.
3. Para volver, pulsa **Ir** junto al nombre: el cabezal se mueve con el **láser apagado** a la Velocidad de **Mover cabezal**. **Borrar posición** la quita.

Se guardan en el perfil (y viajan en los paquetes) en **coordenadas de la máquina**, así que siguen valiendo aunque cambies el origen del trabajo.

## Disparo de prueba

Un pulso corto del láser a baja potencia para ver **dónde cae el punto** sin lanzar un trabajo (por ejemplo, para comprobar el enfoque o el origen sobre un cartón).

> **Atención, seguridad.** El disparo **enciende el láser de verdad**. Ponte las gafas de protección adecuadas, aparta las manos, ten el paro de emergencia a mano y comprueba que debajo del punto no hay nada que no quieras marcar. No lo uses con una máquina que no esté a tu vista.

- En **Disparo de prueba** ajusta la **Potencia** (de 0,1 a **20 %**, de fábrica 2 %) y la **Duración** (de 10 a **2000 ms**, de fábrica 200 ms). Los límites no se pueden pasar.
- **Disparo de prueba…** abre un diálogo de **doble confirmación**: dice «Se encenderá el láser al 2 % durante 200 ms y se apagará solo», repite la advertencia y **no deja disparar hasta que marques «Llevo las gafas puestas y el área está despejada»**. Después pulsa **Disparar**.
- Si necesitas cortarlo, **Apagar ya** envía un reinicio inmediato y un `M5`.
- Por dentro, Tlaku manda `M3` con una S baja, una pausa y **siempre `M5` al final**, falle lo que falle. Si ni siquiera puede confirmar que el láser se apagó, te lo avisa con claridad y te pide reiniciar la máquina o cortar la energía.
- Está deshabilitado sin conexión, durante un trabajo o si la máquina no está en reposo (alarma, pausa, en marcha).

## Ajustes `$$` de la controladora, con respaldo

**Ajustes de la máquina…** (botón en la sección **Ajustes de la máquina** del panel, o el comando `machine.settings`) lee los `$n` de la controladora GRBL y los muestra con **nombre en español, descripción, unidad** y una marca **Crítico** en los que, con un valor equivocado, pueden mover mal la máquina, quitarle protecciones o apagar el modo láser. Cubre GRBL 1.1 y los ajustes conocidos de grblHAL; un ajuste que Tlaku no conoce sale como «Ajuste sin descripción».

- **Releer** vuelve a leer la máquina. El buscador filtra por número o nombre.
- **Editar** un valor lo valida (número, rango, 0 o 1) y pasa a **Revisar el cambio**, que muestra el **valor actual** y la **línea exacta** que se enviará, con la advertencia de que se guarda en la memoria permanente (EEPROM). Solo **Escribir `$n=valor`** la envía.
- Al escribir, **espera el `ok` de cada línea**, se detiene en el primer error y al final **relee `$$`** para comprobar; si un valor no coincide, te lo dice (a veces hace falta reiniciar la controladora).
- **FluidNC** guarda su configuración en `config.yaml`: aquí solo ves una vista, con una nota. Con Ruida el botón no aparece.

**Respaldo.** Antes de tocar nada, pulsa **Exportar respaldo**: guarda un archivo `.tlakuset` con todos los ajustes. Para volver atrás, **Restaurar desde archivo…**:

1. Elige el `.tlakuset`.
2. Ves una tabla de **diferencias** («Ahora → respaldo»), con una casilla por ajuste. Desmarca lo que no quieras cambiar. Los ajustes que la máquina no tiene o los valores no válidos se omiten con un aviso; si el respaldo es de otra familia de firmware, te lo advierte.
3. **Escribir N ajuste(s)** los envía de uno en uno, con las mismas comprobaciones.

> **Consejo:** exporta un respaldo la primera vez que conectes una máquina nueva y después de dejarla bien calibrada. Si un ajuste estropea el comportamiento, restaurar es cuestión de un minuto.

No hay formato `.lbset` de LightBurn ni respaldo automático.

## Encuadre a baja potencia y en bucle

Debajo de los botones de encuadre (sección **Trabajo**) hay dos opciones nuevas (ver también [Enviar trabajos con seguridad](09-enviar-con-seguridad.md)):

- **Encuadrar con el láser encendido a baja potencia:** el encuadre recorre el borde con el láser encendido para **verlo sobre el material**. Eliges la **Potencia del encuadre** (de 0,1 a **20 %**, de fábrica 1 %). El láser se enciende justo antes del primer tramo del borde, así que el traslado hasta la primera esquina sigue siendo con el láser apagado; el `M5` final lo apaga. Solo está disponible si el dialecto de tu máquina lleva la **potencia dentro de cada movimiento** (GRBL 1.1 y similares); si no, la casilla está deshabilitada y dice por qué. Con Ruida el encuadre es siempre con el láser apagado. Se advierte que el láser puede marcar o quemar: **las gafas siguen siendo obligatorias**. El interruptor **no se recuerda** al reiniciar la aplicación (la potencia sí).
- **Repetir el encuadre hasta detener:** al terminar un recorrido empieza otro, para que puedas colocar el material con calma. Un aviso («Encuadre en bucle…») y el botón **No repetir más** lo muestran; también se corta con **Detener**, al desconectar o al empezar un trabajo.

> **Ejemplo:** pones una pieza de cartón, activas el bucle con la potencia en `1 %` y mueves el material hasta que el borde visible caiga donde quieres. Luego **No repetir más** y **Iniciar**.

## Láser 2

Para máquinas con dos láseres que se eligen con una orden del firmware:

1. En el perfil, grupo **Ejes y accesorios**, marca **La máquina tiene dos láseres**.
2. En **Dialecto de G-code**, grupo **Segundo láser**, escribe la línea de **Seleccionar el láser 2** y la de **Volver al láser 1** que use **tu** firmware. Estos campos se editan con el tipo de controladora **G-code personalizado**; si dejas vacía la de seleccionar el láser 2, no se emite nada y se avisa.
3. En el editor de cada capa aparece la opción **Láser 2**: las capas que la marquen se ejecutan con el segundo láser.

Si una capa pide el láser 2 y la máquina activa no lo tiene, la capa **sale con el láser 1** y se avisa. Tlaku envía la orden de selección al cambiar de láser entre capas; no hay desplazamiento del cabezal 2 ni láser 2 con Ruida.

## Paquetes de configuración (`.tlakubundle`)

Un **paquete** es un solo archivo para llevar tu configuración a otro equipo o compartirla: tus **máquinas** (con sus macros y posiciones guardadas, y cuál es la activa), tu **biblioteca de materiales**, tus **cámaras** (calibración, alineación y configuraciones), tu **biblioteca de arte propia** y tus **preferencias y atajos**.

**Exportar.** **Archivo > Exportar paquete de configuración…**: marca las secciones que quieras (puedes llevar solo las máquinas, por ejemplo) y guarda.

**Importar.** **Archivo > Importar paquete de configuración…**: elige el archivo. Antes de tocar nada ves una **vista previa** con una casilla por sección y un resumen de cada una:

- **Máquinas:** cuántas son nuevas, cuántas reemplazarían a una con el mismo identificador (te dice cuáles) y cuántas no cambian. Los perfiles inválidos se descartan con el motivo.
- **Materiales:** se **mezclan**, nunca reemplazan lo tuyo: los que ya tienes se conservan y un material distinto con el mismo identificador entra como copia.
- **Cámaras:** la calibración de la lente, la alineación con la cama y la calibración de la cámara del cabezal **reemplazan** a las que tienes (solo hay una de cada; la vista previa te dice cuáles). Si entra una lente distinta y el paquete no trae su alineación, la tuya se quita porque ya no cuadraría. Las **configuraciones de cámara** con nombre se mezclan y nunca pisan las tuyas. **Las URL de las cámaras de red viajan sin usuario ni clave**: esas credenciales se quedan solo en tu equipo; si un archivo ajeno las trae, también se quitan al importar.
- **Biblioteca de arte propia:** se mezcla como los materiales; un dibujo con el mismo nombre y otro contenido entra con un nombre nuevo. La biblioteca incluida en Tlaku no viaja (ya la tiene cada instalación).
- **Preferencias y atajos:** se aplican; las preferencias o los comandos que esta versión no conoce se ignoran o se conservan, y si el paquete trae atajos, reemplazan a los actuales.

No se escribe nada hasta que pulses **Aplicar**. No se puede importar mientras hay un trabajo en curso. Los materiales siguen pudiendo intercambiarse por separado como `.tlakumat` ([Materiales](11-materiales.md)), y las preferencias y atajos como `.tlakuprefs`.

Siguiente: [Print & Cut](09c-print-and-cut.md)
