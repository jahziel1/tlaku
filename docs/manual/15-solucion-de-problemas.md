# Solución de problemas

Qué significan las alarmas y errores que muestra Tlaku y qué hacer cuando no hay puerto, el láser no dispara o el tamaño no sale bien.

> **Ojo:** estas soluciones salen del código de Tlaku, de la documentación pública del firmware GRBL y de reportes de usuarios. Nada se ha probado con una máquina real todavía. Si algo no coincide con lo que ves, la máquina manda: ante la duda, detén el trabajo y apaga el láser.

## Dónde mirar primero

- Si hay una alarma, la app muestra un banner con el código, una explicación y botones **Desbloquear ($X)…** y **Hacer home ($H)…**. Cada botón pide confirmación; Tlaku nunca los envía por su cuenta.
- La pestaña **Consola** guarda todo lo enviado y lo recibido. El botón de copiar deja el registro en el portapapeles, útil para pedir ayuda (menú **Ayuda > Reportar un problema…**).
- Para ver los ajustes de la máquina, envía `$$` desde la consola (hay un botón de consulta rápida).

## Alarmas `ALARM:N`

Una alarma bloquea la máquina hasta que la desbloqueas o haces homing. Estos son los códigos con texto propio en Tlaku.

| Código | Significado | Qué hacer |
|---|---|---|
| `ALARM:1` | Se activó un final de carrera. | Aparta el cabezal del límite a mano y desbloquea. |
| `ALARM:2` | El movimiento pedido sale del recorrido de la máquina. | Igual; revisa que el diseño quepa en la cama. |
| `ALARM:3` | Reinicio durante un movimiento: se perdió la posición. | Haz homing si hay finales de carrera; si no, fija el origen a mano. Ver la sección de Falcon2. |
| `ALARM:4`, `ALARM:5` | Fallo de sonda. | Desbloquea. No aplica a un láser normal. |
| `ALARM:6` a `ALARM:9` | Fallo de homing (reinicio, puerta, final de carrera no liberado o no encontrado). | Revisa puerta, finales de carrera y cableado; repite `$H`. |
| `ALARM:10` | Depende del firmware: autoescuadre sin segundo final de carrera (GRBL 1.1), paro de emergencia (grblHAL) o fallo del husillo (FluidNC). | Si la app conoce tu firmware, te dice cuál. Con paro de emergencia, suéltalo antes de desbloquear. |
| `ALARM:11` a `ALARM:22` | Solo grblHAL (por ejemplo: 11 pide homing, 12 límites activados). | Lee el texto del banner. |

Cuándo usar cada botón:

- **Con finales de carrera y homing activado (`$22=1`):** lo normal es **Hacer home ($H)**. El cabezal viaja hasta los finales de carrera, así que despeja el área antes.
- **Sin finales de carrera (`$22=0`):** lo normal es **Desbloquear ($X)**. Sin homing, la máquina no sabe dónde está: vuelve a fijar el origen.
- `$X` no comprueba nada. Úsalo solo si sabes por qué saltó la alarma.

## Errores `error:N`

Salen cuando la máquina rechaza una línea de G-code o un comando.

| Código | Significado | Causa frecuente |
|---|---|---|
| `error:1` | Falta la letra del comando. | Línea mal escrita en la consola o en el G-code de inicio. |
| `error:2` | Formato de número incorrecto. | Un `S` sin valor en el G-code de inicio o de fin del perfil. |
| `error:3` | Comando `$` no válido. | Comando de ajuste mal escrito. |
| `error:5` | Ajuste desactivado. | `$H` en una máquina sin homing activado. |
| `error:8` | La máquina no está en reposo. | Comando enviado durante un movimiento. |
| `error:9` | G-code bloqueado por alarma. | Hay una alarma sin resolver. |
| `error:20` | Comando G-code no soportado. | El firmware no entiende alguna orden del dialecto (revisa el tipo de controladora). |
| `error:22` | Velocidad de avance sin definir. | Falta `F` en un movimiento. |
| `error:33` | Destino del movimiento no válido. | Coordenadas fuera de lo que acepta el firmware. |

Los números **18, 19 y desde el 39** significan cosas distintas en grblHAL y en FluidNC. Si la app no conoce tu firmware, solo dice "error extendido"; en FluidNC puedes consultarlo con `$E=N`.

> **Ojo:** `error:5` (minúsculas, de GRBL) no es lo mismo que `ERROR:05` (mayúsculas, de Creality). Tlaku los distingue; tú también al leer la consola.

## Creality Falcon2

La Falcon2 tiene códigos propios, que llegan como `ERROR:NN` (mayúsculas y dos dígitos). Tlaku los muestra en un banner aparte, **Falla de la máquina (Creality)**.

| Código | Significado |
|---|---|
| `ERROR:01` | Ángulo anormal: se activó el sensor de inclinación o vibración. |
| `ERROR:02` | No hay archivo G-code en la raíz de la tarjeta TF. |
| `ERROR:04` | Sin flujo de aire: revisa bomba, perilla de caudal y tubo. |
| `ERROR:05` | Llama detectada: la máquina se detuvo. |
| `ERROR:06` | Lente contaminada: límpiala. |
| `ERROR:07` | Módulo láser demasiado caliente. |
| `ERROR:08` a `ERROR:12` | Fallo de un sensor del módulo: reinicia la máquina. |
| `ERROR:13`, `ERROR:14` | Temperatura alta de la CPU del módulo o del anfitrión. |
| `ERROR:24` | Máquina no sellada: cierra la tapa y el cajón. |
| `ERROR:25` | Bomba de aire anormal. |

Para recuperarte, corrige la causa y reinicia con el botón **Reset** de la máquina (y Start en su panel si lo pide). Tlaku no la desbloquea por su cuenta.

Otros puntos de la Falcon2, según reportes públicos (sin probar aquí):

- **`ALARM:3` es lo más común.** Las causas que citan los usuarios son fuente de alimentación débil, cable USB, un sensor que reinicia la placa o subir demasiado la potencia en el módulo de 40 W. Prueba otra fuente y otro cable.
- **Cable USB-C:** un cable de USB-C a USB-C puede no funcionar; usa uno de USB-A a USB-C. Necesita el driver CH340.
- **Cama y velocidades:** distintos firmwares traen valores distintos. Lee los de tu máquina con `$$` y compáralos con tu perfil.
- **Desplazamientos G54 a G59:** pueden mostrar `-nan`. Tlaku no los usa.
- Si el grabado sale más oscuro en las esquinas, el perfil debe usar `M4` (tipo **GRBL 1.1**), no el tipo antiguo con `M3`.

Puedes ver cómo se muestran estas fallas sin riesgo en la máquina virtual (**Simular falla**: llama, inclinación, alarma 3, cable desconectado). Ver [Conectar tu máquina](08-conectar-maquina.md).

## La app no encuentra el puerto

> Si tu máquina se conecta **por red** (TCP, WebSocket o Ruida) no hay puerto serie que buscar: mira más abajo las secciones **La conexión por red falla** y **Ruida no responde**.

1. Revisa que el cable transmita **datos** (algunos solo cargan) y que la máquina esté encendida.
2. Pulsa **Buscar puertos de nuevo**.
3. Cierra cualquier otro programa que tenga el puerto abierto (otro software de láser o un monitor serie). Solo uno a la vez puede usarlo.
4. Instala el driver de la placa si hace falta (ver abajo) y vuelve a conectar.

**Windows.** Abre el Administrador de dispositivos y mira **Puertos (COM y LPT)**. Si tu máquina sale como "dispositivo desconocido" o con una advertencia, falta el driver del chip, normalmente **CH340** (a veces FTDI). Instálalo, reconecta el cable y elige el puerto `COMn` que aparezca.

**macOS.** Algunas máquinas con chip CH340 necesitan el driver del fabricante del chip. Tras instalarlo, el puerto aparece con un nombre como `/dev/cu.usbserial-...`. Si macOS pide aprobar una extensión del sistema, hazlo en Ajustes del Sistema y reinicia. Verifica en tu versión de macOS si hace falta driver.

**Linux.** Tu usuario debe pertenecer al grupo `dialout` (en Arch y derivadas, `uucp`):

```sh
sudo usermod -aG dialout "$USER"
```

Cierra la sesión y vuelve a entrar para que surta efecto. Si la máquina aparece y desaparece, suele ser `ModemManager` o `brltty` quedándose con el puerto: desactiva `ModemManager` (`sudo systemctl disable --now ModemManager`) o desinstala `brltty`.

## Conecta pero no responde

- **Baudios:** la mayoría de los GRBL usan 115200; Marlin suele usar 250000; algunos modelos Neje piden 38400.
- **Espera unos segundos.** Abrir el puerto reinicia muchas placas; Marlin tarda más.
- Si la detección dice "No se reconoció", revisa el cable, prueba otros baudios o elige **Otro / genérico** en el asistente.
- Si la máquina no devuelve ajustes, comprueba que sea una controladora GRBL.

## El láser no dispara

Revisa en este orden:

1. **Modo láser apagado.** Si la app avisa de `$32=0`, actívalo con `$32=1` (hay un botón en el panel Máquina). Sin eso, `M4` no sigue la velocidad y el grabado sale mal.
2. **Potencia.** La capa tiene potencia mayor que 0 y la **S máxima** del perfil coincide con `$30` de la máquina (1000 en GRBL actual, 255 en versiones antiguas, 1 en GRBL-LPC y Smoothie).
3. **Salida de la capa.** La capa tiene la salida activa (no está apagada ni es de guías: las guías nunca se envían) y tiene objetos visibles de su modo.
4. **Tipo de controladora.** Un tipo que no corresponde a tu firmware cambia el G-code (por ejemplo `M4` donde solo existe `M3`). Revisa el perfil.
5. **Seguridad de la máquina.** Tapa, llave, paro de emergencia o sensores (en la Falcon2, `ERROR:24` o `ERROR:01`). Mira la consola.
6. Si usas la máquina virtual, recuerda que **no hay láser real**: no se graba nada.

## El tamaño no sale como esperabas

- **Mide en el lienzo primero.** Selecciona el objeto y mira Ancho y Alto en la barra numérica. Las imágenes se importan a 96 píxeles por pulgada: una foto de muchos píxeles puede salir grande o pequeña. Corrige en la barra numérica.
- **Máquina que se mueve de más o de menos.** Si el cuadro sale a otra medida, `$100` y `$101` (pasos por mm de X e Y) pueden estar mal. Mide lo que hizo la máquina y ajusta: nuevo valor = valor actual por (medida pedida / medida real). Hazlo desde la consola y con cuidado.
- **Rotativo activado.** Con un rotativo, la escala de Y cambia. Si no lo usas, desactívalo en el perfil.
- **Cama del perfil.** El área de trabajo del perfil debe coincidir con `$130` y `$131`.
- **Se sale de la cama.** La sección **Iniciar desde** del panel Máquina avisa "Se sale de la cama" e indica por qué lado.
- **Sale espejado o invertido.** El **Origen de la máquina** del perfil no coincide con la esquina real de tu X0 Y0.
- **Sale desplazado.** Revisa **Iniciar desde** y el punto del trabajo que se ancla: ver [Enviar trabajos con seguridad](09-enviar-con-seguridad.md).

## La conexión por red falla

Para GRBL, grblHAL y FluidNC por TCP o WebSocket (ver [Conexión por red y automatización](08b-red-y-automatizacion.md)). Empieza siempre por **Probar conexión** en el perfil: no mueve nada y el mensaje dice qué falló. Los mensajes más comunes:

| Mensaje | Qué revisar |
|---|---|
| «No se encontró 'X' en la red» | El nombre no se resuelve (`fluidnc.local` no funciona en todas las redes). Usa la dirección IP |
| «La máquina en X rechazó la conexión» | El puerto está cerrado o el servicio de red de la placa está apagado. Telnet suele ser el 23; los WebSocket de FluidNC y ESP3D, el 81. Mira la configuración de red de la placa |
| «X no contestó a tiempo» | Pasaron 5 segundos sin respuesta. ¿Está encendida? ¿Tu equipo y la máquina están en la **misma red**? ¿La IP cambió (el router la reasignó)? Reserva una IP fija |
| «No se puede llegar a X» | No hay ruta hasta ese equipo: Wi-Fi o cable desconectado, o una red de invitados que aísla los equipos |
| «X cerró la conexión al abrirla: ¿hay otro programa conectado a ella?» | Muchas placas aceptan un solo cliente a la vez. Cierra la interfaz web de la máquina en el navegador, otra copia de Tlaku u otro programa de láser |
| «Las conexiones cifradas (wss://) no están soportadas» | Usa una dirección `ws://` |
| «La máquina respondió, pero no se reconoció el firmware» | Hay conexión, pero el saludo no es de GRBL. Revisa que el puerto sea el de la consola de la controladora y no el de otro servicio |

Otras ideas:

- **Se conecta pero se corta a mitad del trabajo.** Es casi siempre el Wi-Fi: acerca la máquina al router o usa un cable. Una conexión que se cae puede dejar la máquina ejecutando lo que ya tenía en su búfer; ten el paro de emergencia a mano.
- **No puedes cambiar la dirección en la pestaña Máquina.** Está a propósito: la dirección es del perfil. Abre **Gestionar dispositivos…** y desconéctate primero.
- **Probar la app sin la red de la máquina:** usa la **Máquina virtual por red**. Si esa conecta, el problema está en la máquina o en tu red, no en Tlaku.
- **La automatización por UDP no contesta** (`PING` sin respuesta): comprueba que esté activada en **Preferencias > Automatización**, que el estado diga «Escuchando en 127.0.0.1:…» (si el puerto está ocupado lo dice) y que mandes el datagrama a `127.0.0.1`, no a la IP de tu red: solo escucha en tu propio equipo.

## Ruida no responde

La conexión con Ruida es experimental (ver [Ruida y K40](13b-ruida-y-k40.md)). Revisa en este orden:

1. **La IP.** La del perfil debe ser la de la controladora (en su pantalla, en el menú de red). Tu equipo debe estar en la **misma red /24** (por ejemplo, ambos en `192.168.1.x`).
2. **Otro programa con el puerto.** Si ves «no se pudo abrir el puerto UDP local 40200 (¿hay otro programa de láser abierto?)», cierra LightBurn, otra copia de Tlaku o cualquier otro software de láser: solo uno a la vez puede usar ese puerto UDP.
3. **El firewall del equipo** puede bloquear UDP. Permite a Tlaku en la red privada.
4. **El magic.** En **Automático** Tlaku prueba `0x88` y luego los demás conocidos; si tu controladora usa otro, fíjalo **A mano** (`0x88` en casi todas las RDC644x, `0x11` en la 634XG, `0x38` en la RDL9635).
5. **«Sin respuesta de la máquina»** al conectar o al leer información: la máquina no contestó. Prueba con un cable directo al PC o al router, sin repetidores Wi-Fi. UDP no tiene reintentos y las controladoras Ruida son poco tolerantes a una red lenta.
6. **La cama no coincide.** Si el aviso dice que la cama del perfil no coincide con la de la máquina, no se enviará ningún trabajo: corrige el tamaño de la cama del perfil con lo que informa la máquina (**Leer información**).
7. **Durante un trabajo** algunas controladoras ignoran la red; Tlaku no lo toma por una conexión perdida mientras hay un trabajo en marcha.
8. **«La máquina rechazó el paquete N»** al subir: el trabajo no llegó entero. Tlaku manda *Detener*; revisa la red y vuelve a intentarlo.
9. **Descartar a Tlaku:** conecta con la máquina virtual **Ruida (CO2)**. Si esa funciona, el problema está en la red o en la controladora.

## El K40 no aparece por USB

El soporte del K40 con M2 Nano es experimental y por ahora funciona desde la línea de comandos (`tlaku k40`) <!-- K40-APP -->; ver [Ruida y K40](13b-ruida-y-k40.md). Si `tlaku k40 list` dice «No hay placas CH341 (USB 1A86:5512) conectadas»:

1. **Cable y energía.** Usa el cable USB que trae la máquina, que la máquina esté encendida, y prueba otro puerto USB del equipo (mejor directo, sin concentrador).
2. **Otro programa con la placa.** K40 Whisperer, MeerK40t o LaserDRW no pueden tenerla abierta a la vez. Ciérralos.
3. **Linux: faltan permisos.** Tu usuario necesita una regla de udev. Crea `/etc/udev/rules.d/99-k40.rules` con esta línea:

   ```
   SUBSYSTEM=="usb", ATTR{idVendor}=="1a86", ATTR{idProduct}=="5512", MODE="0666"
   ```

   recarga con `sudo udevadm control --reload-rules && sudo udevadm trigger` y desconecta y vuelve a conectar la placa. Para comprobar que el sistema la ve: `lsusb | grep 1a86:5512`. (`MODE="0666"` abre la placa a todos los usuarios; para restringirla usa `TAG+="uaccess"` en su lugar.)
4. **Windows:** la placa debe usar el controlador **WinUSB** (por ejemplo, con Zadig). Con el controlador original del fabricante del chip (WCH) Tlaku no puede abrirla.
5. **macOS:** no necesita controlador. Si tienes instalado el de WCH, desinstálalo.
6. **Aparece pero `tlaku k40 status` falla o dice Moshiboard.** Una Moshiboard comparte el mismo USB, pero no es una M2 Nano y Tlaku no la soporta. Si es una M2 Nano y no responde, apaga y enciende la máquina y vuelve a probar.
7. **Se queda esperando el final del trabajo.** Si tu placa no informa el estado «terminó», prueba `--no-wait-finish`.

Y recuerda: la M2 Nano **no controla la potencia por software**; si el láser no marca, sube la perilla de la máquina, no la potencia de la capa.

## Macros, ajustes `$$` y disparo de prueba

- **«La macro … se detuvo en …: la máquina contestó …».** La macro envía las líneas una a una y se detiene en la primera que la máquina rechaza. Mira el `error:N` en la sección **Errores `error:N`** de esta página y corrige esa línea en el perfil.
- **«La macro … no es válida».** Tiene un nombre vacío, ninguna línea, más de 50 líneas, o una línea con `?`, `!` o `~`. El editor de macros marca cuál.
- **Una macro no hace nada durante un trabajo.** Es a propósito: las macros no se envían mientras hay un trabajo en curso.
- **Escribí un ajuste `$` y «al releer algunos valores no coinciden».** La controladora dijo `ok` pero guardó otro valor, o solo lo aplica al reiniciar. Reinicia la controladora y pulsa **Releer**. Si te equivocaste, **Restaurar desde archivo…** con el respaldo `.tlakuset` que hiciste antes.
- **El botón Disparo de prueba está gris.** Necesita la máquina conectada, sin trabajo en curso y en reposo (sin alarma ni pausa). Pasa el cursor para ver el motivo.
- **«No se pudo confirmar que el láser se apagó».** Reinicia la máquina o corta la energía y revísala antes de seguir. Puede ser una conexión caída justo durante el disparo.
- **Encuadrar con el láser encendido está deshabilitado.** El dialecto de tu máquina no lleva la potencia dentro de cada movimiento, y entonces no hay forma de apagar el láser en el traslado: solo se puede encuadrar con el láser apagado.

## Problemas al importar o exportar

- **Importé un `.lbrn` o `.lbrn2` y faltan ajustes de corte.** El aviso de importación lista lo que no pudo traer: perforado, modo de puntos, relleno inundado, sobrecorte, rampa y corte pasante tienen su equivalente en Tlaku, pero el importador todavía no los traduce; vuelve a encenderlos en **Corte avanzado** o en el selector **Relleno** de la capa. Los puentes manuales de LightBurn se reparten solos. Ver [Venir desde LightBurn](14-desde-lightburn.md).
- **Un PDF o AI llega sin texto o sin fotos.** Solo se importan trayectos: convierte el texto a curvas en el programa de origen y vuelve a exportar. El aviso de importación lo dice. Si el archivo tiene varias páginas, importa la primera; elige otra en **Importar con opciones…**.
- **"Es un AI antiguo (PostScript)".** Ábrelo en Illustrator y guárdalo con **Crear archivo compatible con PDF**, o expórtalo a PDF o SVG.
- **Un HPGL o un SVG sale a otro tamaño.** El HPGL no declara su escala (se supone 40 unidades por mm) y un SVG sin unidades usa 96 ppp. Usa **Escala (%)** y **Resolución (ppp)** en **Archivo > Importar con opciones…**; Illustrator suele necesitar 72.
- **Un DXF sale unas 25 veces más grande o más chico.** Es un DXF sin unidades en pulgadas o milímetros: elige la **Unidad de un DXF sin unidades** en las opciones de importación.
- **Importé un G-code y quedó en una sola capa (o en muchas).** De fábrica se hace una capa por nivel de potencia; elige **Una sola capa** en las opciones.
- **"No hay objetos que exportar".** Exportar necesita al menos un objeto visible; revisa que no estén ocultos ni sus capas apagadas.
- **El PDF o la impresión sale a otra escala.** Imprime desde el visor con **tamaño real (100 %)** y sin **ajustar a la página**.
- **El código QR no se lee.** Súbele el tamaño o la **Corrección de errores**, ajusta el grabado de la capa para que el contraste sea claro, y evita **Invertir** si tu lector no lo admite. Prueba siempre con un lector real.
- **Se cerró Tlaku y perdí cambios.** Al volver a abrirlo debería ofrecerte recuperar la copia (ver [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md)). Si no aparece, revisa que **Guardar copias de recuperación automáticamente** estuviera activo.
- **Un atajo no hace nada.** Puede que lo hayas cambiado: mira **Editar > Editar atajos de teclado…**. Las teclas sueltas no funcionan mientras escribes en un campo.

## Si nada de esto sirve

Copia el registro de la consola y usa **Ayuda > Reportar un problema…**. Indica tu firmware (versión que devuelve `$I`), el tipo de controladora del perfil y si lo probaste en la máquina virtual.

Siguiente: [Glosario](16-glosario.md)
