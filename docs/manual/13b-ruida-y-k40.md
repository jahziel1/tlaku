# Ruida y K40 (experimental)

Dos familias de máquinas que LightBurn trata distinto y que Tlaku soporta **de forma experimental**: las controladoras **Ruida** de los láseres de CO2 con pantalla, y el **K40 con placa M2 Nano**. Esta página dice qué hace cada una, qué no hace y cómo probarlas con cuidado.

> **Ojo, honestidad:** ninguna de las dos se ha probado con una máquina real. Ruida se validó contra un emulador UDP propio y el K40 contra una placa simulada y vectores de prueba de otro proyecto de código abierto. Cuando esta página dice «funciona» significa exactamente eso: funciona contra el emulador. **Una orden equivocada a una controladora de CO2 puede dañar un tubo láser o a quien esté cerca.** Haz la primera prueba con el láser deshabilitado, sin material valioso, con el paro de emergencia al alcance y sin dejar la máquina sola.

## Ruida

### Qué es y qué hace

Ruida es la controladora de la mayoría de los láseres de CO2 de caja metálica con pantalla. Tlaku habla con ella **por red (UDP)** desde la app y desde el programa `tlaku`. En la app puedes:

- **Conectar** con la IP de la máquina. Conectar hace una lectura de solo lectura (modelo, tarjeta, *magic*, cama).
- **Enviar un trabajo.** Tlaku calcula el trabajo, lo convierte al formato `.rd` de Ruida y lo sube con una barra de progreso.
- **Pausar, reanudar y detener.**
- **Mover el cabezal** con el láser apagado y **encuadrar** (siempre con el láser apagado).
- **Leer información** de la controladora (velocidades, aceleración, tiempo encendida, trabajos hechos, recorrido) y la **lista de documentos** guardados en su memoria.
- Ensayar todo con la **máquina virtual Ruida (CO2)**, sin riesgo.

### Qué no hace

- **No escribe ajustes de la controladora.** La información y la lista de documentos son de solo lectura. Escribir la memoria de la controladora sin una máquina de pruebas puede dejarla inutilizable, así que se dejó pendiente a propósito.
- **No borra ni ejecuta documentos** de su memoria (hazlo desde el panel de la máquina). Con un índice equivocado se borraría o ejecutaría otro documento.
- **No usa G-code:** la consola, los ajustes en vivo de velocidad y potencia, los ajustes `$`, las macros y el disparo de prueba no aplican (se ocultan con el motivo). La velocidad y la potencia van dentro del archivo del trabajo.
- **No encuadra con el láser encendido.**
- **No configura el rotativo.** En las controladoras DSP el rotativo se activa en los ajustes de la propia máquina; Tlaku manda la Y en milímetros de superficie, sin escalarla ni reflejarla, y lo avisa en el registro.
- **No abre archivos `.rd`**, no soporta láser 2 con Ruida y tampoco se conecta a Trocen ni TopWisdom.

### Preparar el perfil

1. **Máquina > Agregar máquina…**, paso **Modelo**, elige **Otro / genérico** y el tipo **Ruida (CO2, por red; experimental)** (o cambia el tipo de un perfil en **Gestionar dispositivos…**). Escribe la IP si la tienes; puedes dejarla vacía y rellenarla después (sin IP solo funciona la máquina virtual). Tlaku **no busca** Ruida en la red.
2. En el perfil, grupo **Controladora Ruida (red)**:
   - **IP de la controladora**, por ejemplo `192.168.1.100`. Tu equipo debe estar en la misma red /24 y **ningún otro programa de láser** (LightBurn, otro Tlaku) puede tener abierto el puerto UDP 40200.
   - **Magic (clave de ofuscación):** **Automático** prueba `0x88` y, si no contesta, los demás conocidos con una lectura de solo lectura; o fíjalo **A mano** si lo sabes (`0x88` en casi todas las RDC644x, `0x11` en la 634XG, `0x38` en la RDL9635).
   - **Ejecutar al subir:** déjalo **apagado** (ver más abajo).
3. En **Origen** elige la esquina donde la máquina hace su *home* (lo más común, arriba a la izquierda). Es una suposición hasta probar con hardware: haz primero un saltito con el láser apagado.
4. Cama: los valores de partida del asistente (600 × 400 mm) son solo eso; confírmalos con **Leer información** al conectar. **Si la cama del perfil no coincide con la que informa la máquina, Tlaku no envía ningún trabajo** hasta que la corrijas, porque el origen del archivo depende de ella.

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

Siguiente: [Venir desde LightBurn](14-desde-lightburn.md)
