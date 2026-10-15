# Conectar tu máquina

Cómo darle a Tlaku el perfil de tu láser, abrir el puerto USB (o la conexión de red) y probar todo primero con una máquina virtual. Esta página cubre la conexión; para la red y la automatización ve a [Conexión por red y automatización](08b-red-y-automatizacion.md), y para enviar trabajos a [Enviar trabajos con seguridad](09-enviar-con-seguridad.md).

> **Ojo, honestidad:** Tlaku **todavía no se ha probado con ninguna máquina real**. Conectar, detectar y enviar se validaron con el emulador `tlaku-emu` y con las máquinas virtuales de la app. Haz tu primera prueba real con material de desecho, potencia baja y sin dejar la máquina sola. Qué controladoras funcionan y en qué estado: [Controladoras soportadas](13-controladoras.md).

## Dos conceptos: perfil y puerto

- El **perfil** (la lista **Máquina**) describe tu láser: tamaño de cama, origen, S máxima, velocidades, dialecto de G-code. El motor lo usa para calcular y generar el G-code.
- El **puerto** (la lista **Puerto**) es por donde se habla con la placa por USB. Con una máquina que se conecta **por red** (TCP, WebSocket o Ruida por UDP) no se elige puerto: la dirección es un dato del perfil y la sección **Conexión** la muestra en lugar de la lista de puertos y los baudios ([Conexión por red y automatización](08b-red-y-automatizacion.md)). También hay máquinas virtuales.

Todo esto vive en la pestaña **Máquina** (`Ctrl/Cmd+2`), sección **Conexión**.

## Opción 1: asistente Agregar máquina

La forma más cómoda de crear un perfil. Ábrelo desde el menú **Máquina**, **Agregar máquina…** (no está disponible con un trabajo en curso). Tiene cuatro pasos: **Detectar**, **Modelo**, **Revisar** y **Guardar**.

![Primer paso del asistente: puerto, baudios y botón Detectar](../../apps/desktop/docs/screenshots/26-agregar-maquina.png)

1. **Detectar.** Elige el **Puerto** y los **Baudios** (115200 para casi todos los GRBL; Marlin suele usar 250000) y pulsa **Detectar**. La detección solo escucha el saludo de la placa y le pide su versión y sus ajustes (`$I`, `$$`, `M115`). Nunca mueve la máquina ni cambia su configuración. Abrir el puerto puede reiniciar la placa y tarda unos segundos. Si no tienes la máquina a mano, elige **Saltar la detección y elegir del catálogo**.
2. **Modelo.** Busca tu marca o modelo en el catálogo (por ejemplo `falcon`, `S9`, `xTool`). Si la detección reconoció la familia, se filtran los modelos de esa familia; marca **Mostrar también modelos de otras familias de firmware** para ver todos. Si tu máquina no aparece, elige **Otro / genérico** y el tipo de firmware.
3. **Revisar.** Tlaku compara lo que dice el catálogo con lo que la máquina contestó a `$$` (S máxima, cama, velocidad máxima y aceleración). Donde difieren, tú eliges con **Usar lo de la máquina**; por defecto se propone lo de la máquina. Aquí también te avisa si el modo láser está apagado (`$32=0`) o si el GRBL es anterior a 1.1f.
4. **Guardar.** Ponle un **Nombre de la máquina** y pulsa **Guardar y activar**. El perfil queda como la máquina activa.

El catálogo trae 21 entradas: 18 modelos (Creality Falcon2, Sculpfun, Atomstack, Ortur, xTool, Longer, Neje, Two Trees, K40 con Cohesion3D) y 3 familias genéricas (GRBL, Marlin, Smoothieware). Cada dato dice de dónde sale (**Oficial**, **Tienda o reseña** o **Foro**), y los que ninguna fuente aporta se dejan sin dato para que los leas de tu máquina. Las cifras anunciadas pueden ser distintas en tu unidad: confía en lo que lea la máquina.

> **Consejo:** las máquinas de red (Ruida y similares) no se detectan: Tlaku no busca máquinas en la red. Para una Ruida (experimental) elige el tipo **Ruida** en «Otro / genérico» y escribe su IP ([Ruida y K40](13b-ruida-y-k40.md)). Para GRBL por red, crea el perfil y elige el tipo de conexión en **Gestionar dispositivos…**. El K40 con placa M2 Nano es experimental (probado solo contra el emulador) y va por USB directo, no por puerto serie: elige **K40 con M2 Nano** (en el asistente, que además busca la placa por USB sin enviarle nada) y cierra antes K40 Whisperer, MeerK40t y LaserDRW; ver [Ruida y K40](13b-ruida-y-k40.md). Galvo y fibra no están soportados.

## Opción 2: perfil manual

Para crear o ajustar un perfil a mano: menú **Máquina**, **Gestionar dispositivos…** (o el icono de ajustes junto a la lista **Máquina**). Se abre **Dispositivos**.

- A la izquierda están tus perfiles. **Nuevo**, **Duplicar** y **Borrar** los administran; el que está activo dice "en uso". No se puede borrar la máquina conectada.
- A la derecha se edita el perfil por grupos. Nada se guarda hasta que pulses **Guardar cambios** (**Deshacer cambios** descarta).
- **Usar esta máquina** la deja como activa.

| Dato | Para qué sirve |
|---|---|
| **Tipo de controladora** | Define cómo se escribe el G-code y los valores de partida (S máxima, modo de potencia, baudios). Al cambiarlo, Tlaku te muestra qué se pisaría y puedes elegir **Aplicar los valores del tipo** o **Solo cambiar el tipo** |
| **Tipo de conexión** (grupo **Comunicación**) | **Puerto serie (USB)** de fábrica, **Red · TCP / Telnet** o **Red · WebSocket**, con el botón **Probar conexión**. Ver [Conexión por red y automatización](08b-red-y-automatizacion.md) |
| **Origen de la máquina** | Esquina donde está X0 Y0. Si es incorrecta, el trabajo sale espejado o invertido |
| **Área de trabajo** | Ancho y alto de la cama en mm ($130 y $131). El motor rechaza lo que no cabe |
| **S máxima** y **Modo de potencia** | S máxima debe coincidir con `$30`. **Dinámica (M4, recomendada)** exige GRBL 1.1f o posterior con `$32=1`; **Constante (M3)** sirve para firmware antiguo |
| **Velocidad máxima**, **Aceleración** | Por eje. Sirven para estimar tiempos y calcular el overscan automático |
| **Baudios**, **Búfer de recepción** | Deben coincidir con la placa (127 bytes en GRBL 1.1). Si la máquina se traba, baja el búfer |
| **G-code de inicio** y **de fin** | Líneas que se emiten al empezar y terminar cada trabajo. Conviene dejar `M5` al final |
| **Eje Z**, **Rotativo** | Ver [Rotativo y eje Z](12-rotativo-y-eje-z.md) |
| **Eje U**, **Segundo láser** | Habilitan mover el eje U y elegir el láser 2 por capa. Ver [Control avanzado de la máquina](09b-control-avanzado.md) |
| **Macros** | Botones con G-code guardado en el perfil. Ver [Control avanzado de la máquina](09b-control-avanzado.md) |

Los tipos de controladora que ofrece la lista son: GRBL 1.1, grblHAL, FluidNC, GRBL-LPC / Cohesion3D, GRBL antiguo (sin M4), Smoothieware, Marlin, G-code personalizado y Ruida (CO2, por red; experimental).

![Perfil de una máquina con el dialecto de G-code y su vista previa](../../apps/desktop/docs/screenshots/27-dialecto.png)

El **Dialecto de G-code** muestra un resumen (por ejemplo "S 0-1000, M4 dinámico, S en cada G1, aire M8/M9") y una vista previa del G-code. Solo se edita con el tipo **G-code personalizado**; en los demás es de consulta. Entre sus datos está el **Tiempo de pausa (G4)**: la unidad con que se escriben las pausas de los ajustes de corte (**Pausa al empezar**, **Pausa al terminar**, **Corte pasante** y **Modo punto**, ver [Capas y ajustes de corte](05-capas-y-ajustes.md)). GRBL lee `G4 P` en segundos; Marlin y Smoothieware (en modo RepRap) lo leen en milisegundos, y Tlaku ya elige la unidad correcta según el tipo de controladora. Una unidad equivocada hace la pausa mil veces más larga o más corta, así que cámbiala solo si tu firmware es raro.

**Leer de la máquina** (con la máquina conectada) envía `$$`, que solo lee, y te propone valores para el perfil. Tú marcas cuáles aplicar; tampoco se guardan hasta **Guardar cambios**.

### Importar un perfil de LightBurn (`.lbdev`)

Si ya tienes la máquina configurada en LightBurn, o el fabricante publica un perfil `.lbdev` (por ejemplo, en su foro), no tienes que teclear todo de nuevo: en **Dispositivos**, abajo de la lista, pulsa **Importar perfil de LightBurn (.lbdev)…** y elige el archivo.

- Se abre un resumen con los perfiles que se van a crear y una lista de **avisos**. Hasta que pulses **Crear N perfil(es)** no se guarda nada. Los perfiles son siempre **nuevos** (con un id propio; si el nombre ya existe se añade «(2)»): tus perfiles actuales no se tocan. Importar nunca envía nada a la máquina.
- Un archivo puede traer varias máquinas; se crea un perfil por cada una.
- Se leen el nombre, el área de trabajo, la esquina de origen (deducida de las marcas de espejo X e Y del archivo: confírmala con un encuadre a baja potencia), el tipo de controladora (GRBL, GRBL-M3, GRBL-LPC, Smoothieware, Marlin, G-code y Ruida), los baudios, la S máxima, el eje Z, el aire por `M7`, el homing al conectar, el envío síncrono y los G-code de inicio y de fin (revísalos: se mandan en cada trabajo).
- **Velocidades y aceleraciones:** el archivo guarda las del *simulador de tiempos* de LightBurn (en mm/s; Tlaku las pasa a mm/min), no las del firmware. Son un punto de partida: con la máquina conectada pulsa **Leer de la máquina**, que lee `$$` y te propone los valores reales (`$110/$111`, `$120/$121`, `$130/$131`, `$30`) para que decidas cuáles aplicar. Tlaku no lo hace solo al conectar.
- Lo que no tiene equivalente (por ejemplo desfases del láser 2 o del proceso, posición final, pluma, ajustes del simulador) sale en los avisos como «no importado». Los datos locales del usuario (puerto, últimas carpetas, cámara) se ignoran. El sentido del eje Z no se importa: pruébalo con un movimiento corto.
- Los controladores que Tlaku no soporta (por ejemplo Trocen o TopWisdom) **no se importan**: se avisa y se omite esa máquina, en lugar de inventar un perfil de G-code que no le corresponde. Para una Ruida, el archivo no trae (que sepamos) la dirección IP: escríbela en el perfil antes de conectar.
- Solo se leen los `.lbdev`. Los paquetes `.lbvendor` y `.lbzip` de LightBurn no se importan todavía (un `.lbzip` es un zip que contiene `.lbdev`: puedes extraerlo y usar ese archivo).

## Ajustes del perfil que conviene conocer

Además de los datos de la tabla, el perfil guarda ajustes que casi nadie toca al principio pero que resuelven problemas muy concretos. Todos están en **Dispositivos** y todos esperan a **Guardar cambios**.

### En el grupo General: perfil predeterminado, orden e ícono

- **Perfil predeterminado.** Marca **Activar este perfil al abrir Tlaku** en el que uses casi siempre: al arrancar la aplicación queda como máquina activa y en la lista se ve la insignia **Predeterminado**. **Solo uno puede serlo**: al marcar otro se quita la marca del anterior. Un proyecto guardado con otra máquina sigue abriéndose con la suya.
- **Orden.** **Subir en la lista** y **Bajar en la lista** (junto a la lista de perfiles) ordenan tus perfiles: ponle arriba el que más uses. Con un solo perfil no hacen nada.
- **Ícono.** Elige uno de diez (**Controladora**, **Rayo**, **Llama (CO2)**, **Sol (fibra o galvo)**, **Tijeras (corte)**, **Impresora**, **Llave**, **Caja**, **Capas**, **Indicador**) para reconocer el perfil de un vistazo en la lista.

### En el grupo Cama

- **Ejes cruzados: Intercambiar X e Y en la máquina.** Para máquinas cuyo eje X recorre el **alto** del diseño y su Y el **ancho**. El **Área de trabajo** de arriba sigue en la orientación del documento; el G-code, el encuadre y el movimiento manual (las flechas de **Mover cabezal**) cruzan los ejes por ti. No se combina con el rotativo.
- **Desfase del puntero rojo** (**Puntero X** y **Puntero Y**, en mm; X a la derecha, Y arriba). Dónde cae el puntero rojo respecto del rayo. Con el desfase puesto, el **encuadre con el láser apagado** y **Posicionar el láser** con la casilla **Llevar el puntero rojo al punto** hacen que sea el **puntero** —y no el rayo— el que recorra el trabajo o llegue al punto. Mídelo sobre un material de prueba: pon el puntero sobre un punto, marca ahí a baja potencia y mide la distancia entre la marca y el punto. 0 y 0 significa que el puntero cae sobre el rayo. Ver [Enviar trabajos con seguridad](09-enviar-con-seguridad.md).
- **Unidad de velocidad: mm/min o mm/s.** Solo cambia **cómo se muestran y se escriben** las velocidades del perfil (la velocidad máxima de cada eje y la de desplazamiento rápido) y las del diálogo de **Iniciar**. Por dentro, y en el G-code, todo sigue en mm/min; las velocidades de las capas se siguen escribiendo en mm/min. Útil si tu láser o tu fabricante da las velocidades en mm/s (60 mm/s son 3600 mm/min).

### En el grupo Comunicación (solo controladoras de la familia GRBL)

- **Homing al conectar: Ofrecer el homing ($H) al conectar.** Con finales de carrera, la máquina suele perder la posición al apagarse. Marcado, al conectar se abre la confirmación del homing (despeja el área, ¿hacer home?). **Nunca se envía solo.**
- **No reiniciar la placa al abrir el puerto: Abrir el puerto serie sin subir DTR** (solo con USB serie). Muchas placas Arduino se reinician cuando el programa abre el puerto, y con eso pierden su posición y su estado: si la tuya lo hace cada vez que conectas, márcalo para que conserve la posición. Depende del sistema: en Linux el controlador sube DTR siempre, y algunos adaptadores USB también, así que a veces no tendrá efecto.
- **Envío síncrono: Mandar una línea y esperar su «ok».** Tlaku envía G-code llenando el búfer de la placa (conteo de caracteres), que es lo más rápido. Algunas placas **pierden datos con el búfer lleno**: se salen líneas, hay `error:` sueltos o movimientos que no corresponden. El envío síncrono manda **una línea y espera su respuesta antes de mandar la siguiente**: es más lento y el movimiento puede **entrecortarse en trazos finos**. Úsalo solo si tu placa pierde líneas aun bajando el **Búfer de recepción**; si no, déjalo apagado.

### En el grupo Ejes y accesorios

- **Desplazamiento U de los trabajos** (con **La máquina tiene un eje U** marcado). Cuánto se mueve el eje U **al empezar cada trabajo**; al terminar se deshace (con movimientos relativos y el láser apagado). Sirve, por ejemplo, para avanzar una mesa o una cinta entre trabajos sin escribir una macro. 0 deja a U sin tocar.

## Conectar

1. En la pestaña **Máquina**, elige tu perfil en **Máquina**.
2. Elige el **Puerto** (si el perfil es de red, en su lugar ves la **Dirección** y este paso no hace falta). Si la lista está vacía, conecta el cable USB, instala el driver de la placa (CH340 o FTDI) y pulsa el botón de buscar puertos de nuevo.
3. Elige los **Baudios** (el perfil ya trae los suyos; por red no existen).
4. Pulsa **Conectar**. Por USB tarda unos 2 segundos porque la placa se reinicia; por red conecta más rápido.

Si todo va bien ves "Conectada en ..." y la versión del firmware (por ejemplo `Grbl 1.1h`). Mientras estés conectado no puedes cambiar de perfil, puerto ni baudios: pulsa **Desconectar** primero.

Avisos que puede darte la sección:

- Firmware anterior a GRBL 1.1f: no soporta bien el modo láser (M4); usa potencia constante (M3) en el perfil.
- Modo láser desactivado (`$32=0`): el grabado saldría mal. El botón **Activar modo láser ($32=1)…** te pide confirmación antes de enviarlo.
- Si la conexión se pierde (cable desconectado, puerto que desaparece, máquina apagada), la app lo avisa.
- Si la placa **deja de contestar** unos segundos (pasa con algunas placas USB, como la Falcon2 detrás de un concentrador), Tlaku **no desconecta**: espera, igual que LightBurn. A los 2 segundos aparece un aviso discreto «La máquina no contesta desde hace N s…» con **Detener** y **Desconectar**; se quita solo cuando la placa vuelve a contestar y el trabajo sigue sin que hagas nada. Si en pleno trabajo pasa más de un minuto, el aviso se vuelve más serio («Lleva más de un minuto sin contestar»): revisa la máquina y, si sigue moviéndose y no responde, usa el paro de emergencia.

Si no encuentra el puerto o falla la conexión, mira [Solución de problemas](15-solucion-de-problemas.md).

Ya conectado, la pestaña **Máquina** ofrece también **Mover cabezal** (con pasos configurables, **Posicionar el láser** con un clic en el lienzo, **Posición final**, ejes Z y U, posiciones guardadas y disparo de prueba), las **Macros**, los botones de encuadre y los **Ajustes en vivo** de velocidad y potencia durante un trabajo. Todo eso está explicado en [Enviar trabajos con seguridad](09-enviar-con-seguridad.md) y en [Control avanzado de la máquina](09b-control-avanzado.md). Los ajustes en vivo solo existen con controladoras de la familia GRBL (no con Marlin ni Smoothieware).

## Máquina virtual

Al final de la lista **Puerto** hay seis máquinas virtuales, que son un emulador dentro de la app: **Falcon2 Pro**, **GRBL 1.1**, **grblHAL**, **FluidNC**, **Marlin** y **Smoothieware**. Sirven para aprender el flujo completo sin riesgo: no hay láser ni hardware, no se graba nada. Cada una existe también como **Máquina virtual por red**, que escucha en un puerto TCP local y a la que la app se conecta de verdad por un socket, para ver la conexión por red sin tener la máquina; y con un perfil Ruida hay una **Ruida (CO2)** ([Conexión por red y automatización](08b-red-y-automatizacion.md), [Ruida y K40](13b-ruida-y-k40.md)).

![Conectado a la máquina virtual Falcon2 Pro, con la sección para simular fallas](../../apps/desktop/docs/screenshots/25-maquina-virtual.png)

- Arrancan referenciadas y sin alarma, listas para trabajar.
- **Velocidad del tiempo** (`×1`, `×5`, `×20`) acelera el trabajo simulado; con `×1` dura lo mismo que uno real.
- La insignia **VIRTUAL** aparece en el panel **Máquina** y en la barra de estado.
- La sección **Máquina virtual** tiene **Simular falla**: **Llama detectada (ERROR:05)**, **Inclinación anormal (ERROR:01)**, **Alarma 3 (reinicio en movimiento)** y **Cable desconectado**. Elige una y pulsa **Provocar** para ver cómo reaccionan el banner de alarma, la consola y el trabajo. Esa sección no existe con una máquina real.

Siguiente: [Conexión por red y automatización](08b-red-y-automatizacion.md)
