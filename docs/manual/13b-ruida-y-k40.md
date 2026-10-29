# Ruida y K40 (experimental)

Dos familias de máquinas que LightBurn trata distinto y que Tlaku soporta **de forma experimental**: las controladoras **Ruida** de los láseres de CO2 con pantalla, y el **K40 con placa M2 Nano**. Esta página dice qué hace cada una, qué no hace y cómo probarlas con cuidado.

> **Ojo, honestidad:** ninguna de las dos se ha probado con una máquina real (otras partes de Tlaku sí se probaron en una Falcon2 Pro con GRBL, pero no Ruida ni K40). Ruida se validó contra un emulador UDP propio y el K40 contra una placa simulada y vectores de prueba de otro proyecto de código abierto. Cuando esta página dice «funciona» significa exactamente eso: funciona contra el emulador. **Una orden equivocada a una controladora de CO2 puede dañar un tubo láser o a quien esté cerca.** Haz la primera prueba con el láser deshabilitado, sin material valioso, con el paro de emergencia al alcance y sin dejar la máquina sola.

## Ruida

### Qué es y qué hace

Ruida es la controladora de la mayoría de los láseres de CO2 de caja metálica con pantalla. Tlaku habla con ella **por red (UDP)** desde la app y desde el programa `tlaku`. En la app puedes:

- **Conectar** con la IP de la máquina. Conectar hace una lectura de solo lectura (modelo, tarjeta, *magic*, cama).
- **Enviar un trabajo.** Tlaku calcula el trabajo, lo convierte al formato `.rd` de Ruida y lo sube con una barra de progreso.
- **Pausar, reanudar y detener.**
- **Mover el cabezal** con el láser apagado y **encuadrar** (siempre con el láser apagado).
- **Leer información** de la controladora (velocidades, aceleración, tiempo encendida, trabajos hechos, recorrido, versión de la placa y funciones) y la **lista de documentos** guardados en su memoria.
- **Elegir el nombre** del documento al guardar, **ejecutar** y **borrar** documentos de la memoria, y **leer y escribir ajustes** de la controladora con respaldo, confirmación y verificación (todo **experimental**, ver «Ajustes y documentos de la controladora»).
- Ensayar todo con la **máquina virtual Ruida (CO2)**, sin riesgo.

### Qué no hace

- **No escribe cualquier cosa en la controladora:** solo una **lista cerrada** de 24 ajustes y dentro de un rango (ver más abajo). No toca la potencia ni la frecuencia del láser, el modo de control, las entradas y salidas ni los bloqueos.
- **No renombra ni «borra todos».** Un documento tiene el nombre que se le puso al guardarlo; borrar es de uno en uno, con su nombre confirmado.
- **No usa G-code:** la consola, los ajustes en vivo de velocidad y potencia, los ajustes `$`, las macros y el disparo de prueba no aplican (se ocultan con el motivo). La velocidad y la potencia van dentro del archivo del trabajo.
- **No encuadra con el láser encendido.**
- **No activa el rotativo.** En las controladoras DSP el rotativo se activa en los ajustes de la propia máquina; Tlaku manda la Y en milímetros de superficie, sin escalarla ni reflejarla, y lo avisa en el registro. Sí puede escribir sus dos ajustes públicos (pulsos y diámetro), como números sin unidad documentada.
- **No se conecta a Trocen ni TopWisdom.** El **láser 2** de una capa sale en el `.rd` (experimental, sin probar en una máquina de dos láseres; las capas de imagen con láser 2 se rechazan). Los archivos `.rd` **sí se abren** como diseño (ver [Importar y exportar](03b-importar-y-exportar.md)), pero solo para recuperar la geometría y los ajustes de cada capa.

### Preparar el perfil

1. **Máquina > Agregar máquina…**, paso **Modelo**, elige **Otro / genérico** y el tipo **Ruida (CO2, por red; experimental)** (o cambia el tipo de un perfil en **Gestionar dispositivos…**). Escribe la IP si la tienes; puedes dejarla vacía y rellenarla después (sin IP solo funciona la máquina virtual). Tlaku **no busca** Ruida en la red.
2. En el perfil, grupo **Controladora Ruida (red)**:
   - **IP de la controladora**, por ejemplo `192.168.1.100`. Tu equipo debe estar en la misma red /24 y **ningún otro programa de láser** (LightBurn, otro Tlaku) puede tener abierto el puerto UDP 40200.
   - **Magic (clave de ofuscación):** **Automático** prueba `0x88` y, si no contesta, los demás conocidos con una lectura de solo lectura; o fíjalo **A mano** si lo sabes (`0x88` en casi todas las RDC644x, `0x11` en la 634XG, `0x38` en la RDL9635).
   - **Ejecutar al subir:** déjalo **apagado** (ver más abajo).
3. En **Origen** elige la esquina donde la máquina hace su *home* (lo más común, arriba a la izquierda). Es una suposición hasta probar con hardware: haz primero un saltito con el láser apagado.
4. Cama: los valores de partida del asistente (600 × 400 mm) son solo eso; confírmalos con **Leer información** al conectar. **Si la cama del perfil no coincide con la que informa la máquina, Tlaku no envía ningún trabajo** hasta que la corrijas, porque el origen del archivo depende de ella.

### Ajustes y documentos de la controladora (experimental)

> **EXPERIMENTAL: sin probar con una controladora real.** Escribir la memoria de una controladora puede dejarla mal configurada. Anota **a mano** todos los valores de su menú antes de empezar.

**Todo lo que escribe está apagado de fábrica.** Para habilitarlo, enciende **Escrituras experimentales en Ruida** en el panel de Ruida: un diálogo explica que las órdenes solo se probaron con un emulador, que las fuentes públicas no coinciden en algunas y que pueden desconfigurar la máquina, y pide escribir **ENTIENDO** o marcar una casilla. Esa preferencia nunca se importa de un paquete `.tlakuprefs`. Apagada, los botones de escribir (ajustes, calibrar, rotativo, restaurar, **Ejecutar**, **Borrar** y **Reemplazar**) quedan deshabilitados y su ayuda dice cómo activarlos, y el motor también los rechaza; **leer** la información, los ajustes y la lista de documentos, y **exportar un respaldo**, funcionan siempre. En la línea de comandos, `tlaku ruida doc <ip> delete|run <número> <nombre>` exige `--experimental-writes`.

En el panel **Controladora Ruida** (con la máquina conectada y sin trabajo en marcha):

- **Ajustes de la controladora…** abre una ventana con los 24 ajustes que Tlaku puede escribir: velocidad de movimiento rápido y de home, y por eje la velocidad máxima y de arranque, la aceleración, la cama, el desplazamiento del home, los botones del panel, el paro de emergencia y el **paso** (X, Y, Z y U), más los dos del **rotativo**. Cada uno muestra su valor actual, su dirección, el nombre público que tiene en otros proyectos y el rango permitido (un límite de seguridad de Tlaku, no de la controladora). Las velocidades se ven en mm/s, las aceleraciones en mm/s² y las longitudes en mm; el paso en µm y el rotativo sin unidad (ninguna fuente pública la dice).
- **Escribir** siempre sigue los mismos pasos: (1) escribes el valor nuevo y pulsas **Revisar los cambios**; (2) ves **cada valor actual y el nuevo** y marcas **«Entiendo que esto cambia la configuración de mi controladora»** (sin la casilla no se escribe nada); (3) Tlaku **guarda un respaldo con la fecha** en la carpeta de configuración (`ruida-backups/`; si no puede guardarlo, no escribe nada), escribe de uno en uno y **vuelve a leer** cada valor para compararlo; (4) si algo no coincide, la ventana lo dice con una advertencia y te ofrece **Restaurar el respaldo previo**.
- **Respaldos.** La lista **Respaldos automáticos** guarda uno antes de cada escritura; elige uno y **Restaurar…** (muestra las diferencias, pide la misma casilla, guarda antes un respaldo del estado actual y verifica). **Exportar respaldo…** guarda los ajustes actuales en un archivo `.tlakuruida` (JSON propio de Tlaku; **no** es el `.lbset` de LightBurn, cuyo formato no tiene una descripción pública) e **Importar respaldo…** lo lee y te enseña las diferencias antes de escribir. Un respaldo de otra tarjeta (otro modelo) se rechaza.
- **Calibrar un eje.** Manda mover el eje una distancia conocida (por ejemplo 100 mm), mide con una regla lo que se movió de verdad y escríbelo. Tlaku propone el paso nuevo (paso actual × medido ÷ pedido), lo carga en la tabla y lo escribes con la revisión de siempre; después repite la medición. Rechaza correcciones de más de ±25 %, que casi siempre son un error de medida. **Si tras escribirlo el error se duplica en lugar de bajar, restaura el respaldo:** el cálculo supone que el ajuste es la longitud de cada paso.
- **Rotativo.** Escribe «Rotativo: pulsos» y «Rotativo: diámetro» con los números que tu controladora ya tiene en su menú; Tlaku no los calcula. El rotativo se activa en el panel de la controladora.

**Documentos de la memoria.** En «Documentos en la memoria», **Leer lista** muestra cada documento con **Ejecutar** y **Borrar**. Cada uno abre un diálogo con una casilla de confirmación; **Ejecutar** pasa además por la **misma lista de comprobación de seguridad que Iniciar** (aire, enfoque, material...). Antes de actuar, Tlaku vuelve a leer el nombre del documento y no hace nada si ya no coincide con el que elegiste. Las órdenes de borrar y ejecutar salen de proyectos públicos que **no coinciden entre sí**; la primera vez prueba con **un solo documento de prueba**. Al **guardar** un trabajo, el diálogo **Iniciar** deja elegir el **nombre del documento** (1 a 8 letras, cifras, «_» o «-»; vacío = automático, que nunca pisa otro). Si ya hay un documento con ese nombre, debes marcar **Reemplazar**: Tlaku borra el viejo (con las comprobaciones de arriba) y sube el nuevo; sin marcarlo, falla sin tocar nada.

### Más de 128 capas y velocidades altas

El formato `.rd` admite como máximo **128 capas**, y el límite de velocidad del protocolo es **36 000 mm/min**. Tlaku lo avisa **al calcular**, antes de que intentes enviar:

- **Más de 128 capas.** Un diseño con muchos colores (por ejemplo, un SVG con 129 colores) genera una capa de corte por cada bloque de color. Si hay más de 128, el aviso dice: *Ruida admite 128 capas y el trabajo tiene N: al exportar se fusionarán M capas vecinas de ajustes idénticos (velocidad, potencia, aire) y quedarán K; el orden de corte no cambia*. Solo se fusionan capas **seguidas** y con **exactamente los mismos ajustes** (el color es solo una etiqueta); el orden de corte no cambia. Si aun fusionando quedan más de 128, el aviso dice que **no se podrá exportar ni enviar** y qué hacer: unificar colores o ajustes (velocidad, potencia, aire) de capas seguidas.
- **Tope de 36 000 mm/min.** Si una capa pide más (por ejemplo, 48 000), el archivo `.rd` usa **36 000 mm/min** y el aviso lo dice (*capa …: pide 48000 mm/min y Ruida admite como máximo 36000 mm/min; el archivo .rd usará 36000 mm/min, así que el resultado será más oscuro o más quemado de lo previsto*). Para no depender del recorte, pon en la capa una velocidad dentro del tope.
- **Capa más rápida que la máquina.** Si la velocidad de una capa supera la **Velocidad máxima** del perfil, sale un aviso aparte. Con Ruida ese máximo del perfil es una **suposición** (la controladora tiene el suyo): si tu máquina admite más, sube el valor en el perfil.

Estos avisos no cambian lo que se manda, salvo el recorte a 36 000 mm/min.

### Iniciar desde: coordenadas absolutas, posición actual y origen de usuario

Con Ruida, **Iniciar desde** funciona así:

- **Coordenadas absolutas:** el trabajo sale en el lugar de la cama donde lo ves en el lienzo, sea cual sea la posición del cabezal.
- **Posición actual:** la controladora corta a partir de **donde esté el cabezal cuando arranques el trabajo**, con el punto del trabajo que elegiste en la rejilla de 9 puntos justo ahí. Si lo guardas y lo inicias después desde el panel, vale la posición de ese momento, no la de cuando lo subiste.
- **Origen de usuario:** es **el origen que tiene guardado la controladora** (el que fijas con el botón *Origin* de su panel), no el «Fijar origen aquí» de Tlaku, que con Ruida no se usa. Tlaku no puede leer ese origen, así que no puede dibujar dónde quedará el trabajo ni encuadrar desde él: para encuadrar, lleva el cabezal a ese origen y usa «Posición actual».

Con estos dos últimos modos Tlaku **no puede saber de antemano si el trabajo cabe en la cama**: al subirlo escribe en el registro entre qué coordenadas debe estar el cabezal (o el origen) para que quepa, y avisa si el cabezal está ahora fuera de ese rango. Si el trabajo es más grande que la cama, no se envía.

> **Pendiente de verificar en una máquina real.** Esto se basa en los códigos `D8 11` y `D8 12` del protocolo y en el emulador, pero no se ha probado con una controladora Ruida. Antes de usarlo con material, prueba con el láser deshabilitado o a potencia 0 y comprueba que el recorrido parte donde esperas (ver `docs/hardware/ruida.md`, sección 12.15).

### Por qué solo guarda por defecto

Con **Ejecutar al subir** apagado (de fábrica y lo recomendado), el trabajo se **guarda como documento** en la memoria de la controladora (con un nombre como `TLAKU123`) y **no arranca**: lo inicias tú, desde el panel de la máquina, con la mano en el botón de paro. El diálogo **Iniciar** dice «El trabajo se guardará como documento en la memoria de la controladora y NO arrancará».

Es así porque un trabajo que empieza en cuanto llega puede disparar el láser antes de que estés mirando, con una cama o un origen mal supuestos, y nadie ha probado todavía esta ruta en una máquina real. Con **Ejecutar al subir** encendido, el trabajo empieza a ejecutarse al llegar y el diálogo avisa «el láser puede dispararse»; si la subida falla a medias, Tlaku manda *Detener* y te avisa.

### Primera prueba, paso a paso

1. **Máquina virtual.** Crea el perfil Ruida y conecta a **Ruida (CO2)** (al final de la lista **Puerto**). Envía un trabajo, mira la barra de progreso, el panel **Controladora Ruida** y la lista de documentos. Es el mismo flujo que con la máquina.
2. **Conexión real, sin enviar nada.** Con el láser deshabilitado (o el tubo sin alimentación, según tu máquina), cierra otros programas, pon la IP, conecta y pulsa **Leer información**. Comprueba que el modelo y la cama coinciden con tu máquina.
3. **Movimiento sin láser.** Mueve el cabezal con el pad unos pocos milímetros y comprueba la dirección y el *home*. Prueba **Encuadrar**.
4. **Guardar un trabajo pequeño** (un cuadrado de 20 mm) con **Ejecutar al subir** apagado. En **Documentos en la memoria**, **Leer lista**: debe aparecer.
5. **Ejecutarlo desde el panel de la máquina** con el láser deshabilitado o a potencia 0, y comprueba que el recorrido es el esperado.
6. Solo entonces, potencia baja sobre material de desecho.
7. Los **ajustes** y el **borrar/ejecutar documentos** son lo último: primero un solo cambio inocuo y su restauración (lista en `docs/validacion-hardware.md`, sección 5).

Desde la línea de comandos siguen existiendo `tlaku ruida export-rd` (a archivo `.rd`, sin conexión), `tlaku ruida send` (por UDP, guarda por defecto; `--run` ejecuta) y `tlaku ruida status` (solo lectura).

## K40 con placa M2 Nano

### Qué es

El **K40** es el láser de CO2 chino barato de 40 W con una placa **M2 Nano** (Lihuiyu). **LightBurn no la soporta**; Tlaku sí, de forma experimental, hablando el protocolo de la placa (LHYMICRO-GL) por USB. Si tu K40 tiene la placa cambiada a GRBL-LPC (Cohesion3D, Mini Gerbil) o Smoothie, **no** necesitas esto: funciona como esas familias.

**Estado: experimental, probado solo contra el emulador (no hay placa real probada).** Está disponible **en la app** (tipo de máquina «K40 con M2 Nano») y también desde la **línea de comandos** (`tlaku k40`).

### K40 en la app

- **Conexión por USB directo** (chip CH341, `1A86:5512`), **no** por puerto serie. Cierra antes K40 Whisperer, MeerK40t y LaserDRW: solo un programa a la vez puede tener la placa. En Linux hace falta una regla udev y en Windows el controlador WinUSB (ver «Permisos del USB»). En macOS la app lleva el permiso (entitlement) de USB, así que no hay que instalar nada.
- **Agregar la máquina:** el asistente «Agregar máquina» busca placas por USB sin enviarles nada y ofrece «K40 con M2 Nano». El modelo de placa es la M2 salvo que la tuya lleve otra letra impresa. Si después **cambias el perfil** (placa, cama, origen o velocidad máxima), **hay que reconectar** para que se aplique.
- **Puertos:** cada placa USB aparece como `usb:k40:<n>` (en la lista se muestra «K40 · placa USB n», desde 1) y la **máquina virtual** como `virtual:k40`: la placa simulada, para practicar sin hardware.
- **Home propio:** el K40 no informa su posición. Tlaku la cuenta desde el *home* (la orden `IPP`, los finales de carrera). Usa «Hacer home del K40…» del panel Máquina. Si envías un trabajo **sin posición conocida, la app hace home primero**.
- **La posición se pierde al detener:** si detienes o falla un trabajo, la cuenta se pierde y hay que hacer home otra vez.
- **Si detener no se confirma:** la app destaca un aviso de **paro de emergencia** («La placa no confirmó la parada»): el cabezal puede seguir en movimiento o el láser encendido. Usa el paro de emergencia de la máquina o abre la tapa.
- **Potencia:** se ajusta con la **perilla** de la máquina. El diálogo de inicio lo avisa **antes de enviar**, junto con los demás avisos del codificador (aire, Z, pausas omitidas). Los **grises se binarizan**.
- **Lo que no existe en el K40:** eje Z, eje U, control del aire, consola, ajustes en vivo (overrides), ajustes `$` y disparo de prueba. El encuadre se hace siempre con el láser apagado.
- **Moshiboard:** esa placa comparte el USB de la M2 Nano pero **no es compatible**; si al conectar la app lo detecta, lo dirá.

### Lo más importante: la potencia va con la perilla

> **Atención:** la M2 Nano **no controla la potencia por software**. La potencia de las capas **se ignora** (Tlaku lo avisa para cada capa: «la potencia de 20 % se ignora; la M2 Nano no la controla por software»). La potencia real la fija **la perilla (potenciómetro) o el panel de tu máquina**. Antes de cualquier prueba, **baja la perilla al mínimo** y súbela de poco en poco.

Otras cosas que la placa no hace y Tlaku avisa: no controla la **asistencia de aire**, no tiene **eje Z**, no hace **pausas ni pulsos en el sitio** (se omiten), y los **grises** se graban en blanco y negro (cualquier potencia mayor que 0 quema): para simular grises usa un tramado. Y la placa **no informa su posición**: Tlaku la cuenta desde el *home*, por eso empieza llevando el cabezal al *home* (los finales de carrera).

### Comandos

Con la placa conectada por USB (no hay que instalar nada en macOS; ver más abajo para Linux y Windows):

| Comando | Qué hace |
|---|---|
| `tlaku k40 list` | Lista las placas CH341 (USB `1A86:5512`) conectadas |
| `tlaku k40 status` | **Solo lectura**: modelo del chip y código de estado. No mueve nada ni enciende el láser. Avisa si la placa es una Moshiboard (otra placa que comparte USB y que Tlaku no soporta). `--repeat 20 --interval-ms 500` repite la consulta para ver cómo cambia el estado al usar los botones de la máquina |
| `tlaku k40 send diseño.svg` | Muestra qué va a hacer, **pide confirmación**, lleva el cabezal al *home* y graba. `Ctrl+C` detiene la máquina y apaga el láser |
| `tlaku k40 send diseño.svg --dry-run` | Prepara y revisa el trabajo **sin conectar** con la máquina |
| `tlaku k40 send diseño.svg --emu` | Todo contra una **placa simulada**, sin USB |
| `tlaku k40 export-egv diseño.svg salida.egv` | Escribe el trabajo como archivo `.egv` sin conectar |

Opciones útiles: `--bed 300x200` (la cama; de fábrica 300 × 200 mm), `--home top-left` (la esquina de los finales de carrera: `top-left`, `top-right`, `bottom-left`, `bottom-right`), `--board M2` (la placa: A, B, B1, B2, M, M1, M2 —la de serie— o M3), `--max-speed 140` (mm/s; una capa más rápida es un error), `--no-home` (si el cabezal ya está en el *home*), `--device N` (si hay varias placas) y `--yes` (no pedir confirmación; no lo uses en tus primeras pruebas). Las velocidades de las capas se escriben en mm/min como siempre: `--layer "#ff0000:speed=1000"`.

### Plan de prueba

1. **Sin hardware:** `tlaku k40 send diseño.svg --emu --bed 300x200 --yes`. Lee el resumen, los avisos de la placa y el final: «La placa confirmó el final del trabajo».
2. **Preparación de la máquina:** agua circulando por el tubo **antes** de cualquier prueba con el láser encendido, tapa cerrada, gafas adecuadas para CO2, **perilla de potencia al mínimo**, y el paro de emergencia al alcance. Si tu máquina tiene un interruptor para deshabilitar el láser, déjalo apagado en las primeras pruebas.
3. **Detectar:** conecta el USB y ejecuta `tlaku k40 list`. Debe salir una placa `1A86:5512`. Si no sale, mira [Solución de problemas](15-solucion-de-problemas.md).
4. **Leer el estado:** `tlaku k40 status`. Debe decir «Estado: lista». Con `--repeat` pulsa los botones de la máquina y mira que el código cambie.
5. **Movimiento con el láser deshabilitado:** envía un cuadrado de 20 mm (`tlaku k40 send cuadrado.svg`). Confirma el *home*, que el cabezal sube a la esquina correcta (si va al revés, corrige `--home`) y que el cuadrado tiene el tamaño correcto.
6. **Láser a la mínima potencia** sobre cartón o papel de desecho, subiendo la perilla de poco en poco. Mide un cuadrado de 100 mm y compáralo con el diseño.
7. **Cuenta lo que pasó:** copia la salida del comando y usa **Ayuda > Reportar un problema…**. Un resultado real, bueno o malo, es lo más valioso que puedes aportar.

Con la app, el plan es el mismo; los pasos 3 a 6 se hacen desde el perfil «K40 con M2 Nano»: elige la placa `usb:k40:<n>` en el panel Máquina, conecta, haz home y revisa el aviso del diálogo de inicio antes de enviar. Para el paso 1 usa la máquina virtual `virtual:k40`.

### Permisos del USB

- **Linux:** hace falta una regla de udev para que tu usuario abra la placa. Crea `/etc/udev/rules.d/99-k40.rules` con una línea:

  ```
  SUBSYSTEM=="usb", ATTR{idVendor}=="1a86", ATTR{idProduct}=="5512", MODE="0666"
  ```

  y ejecuta `sudo udevadm control --reload-rules && sudo udevadm trigger`; desconecta y vuelve a conectar la placa. (`MODE="0666"` deja la placa abierta a todos los usuarios del equipo; si prefieres restringirla, sustituye `MODE="0666"` por `TAG+="uaccess"`.) Tlaku desacopla por sí solo el controlador del kernel si hace falta.
- **macOS:** no necesita controlador. Si tienes instalado el controlador del fabricante del chip (WCH), quítalo.
- **Windows:** la placa debe estar asociada al controlador **WinUSB** (por ejemplo con la herramienta Zadig). Con el controlador original de WCH no se puede abrir.

Cierra antes cualquier otro programa que use la placa (K40 Whisperer, MeerK40t, LaserDRW): solo uno a la vez puede tenerla abierta.

Siguiente: [Galvo (experimental)](13c-galvo.md)
