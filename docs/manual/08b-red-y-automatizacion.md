# Conexión por red y automatización

Cómo conectar tu máquina por Wi-Fi o Ethernet en lugar de un cable USB, cómo probar la conexión antes de usarla, cómo ensayar sin máquina con una máquina virtual por red y cómo dejar que otros programas de tu propio equipo manejen Tlaku con comandos UDP.

> **Ojo, honestidad:** la conexión por red **no se ha probado con una máquina real**. Se validó con el emulador de `tlaku-emu` detrás de sockets de verdad (TCP y WebSocket) y con pruebas automáticas, igual que el resto de la conexión. Haz tu primera prueba con el láser a potencia baja y sin dejar la máquina sola. Qué controladoras funcionan y en qué estado: [Controladoras soportadas](13-controladoras.md).

## Qué máquinas se conectan por red

| Máquina | Tipo de conexión en el perfil | Dirección típica |
|---|---|---|
| grblHAL con Ethernet | **Red · TCP / Telnet** | La IP de la máquina y el puerto 23 |
| FluidNC (servidor Telnet) | **Red · TCP / Telnet** | La IP o el nombre (por ejemplo `fluidnc.local`) y el puerto 23 |
| FluidNC o ESP3D (WebSocket) | **Red · WebSocket** | `ws://192.168.1.50:81`; el puerto suele ser el 81 |
| Controladora Ruida (CO2) | Propia, por UDP | Ver [Ruida y K40](13b-ruida-y-k40.md) |

Lo que **no** existe: conexiones cifradas (`wss://`), la red propietaria de xTool o de Glowforge, Trocen y TopWisdom, ni búsqueda automática de máquinas en la red (la dirección se escribe a mano).

## Configurar una máquina por red

La conexión es un dato del **perfil**, no del puerto. Ejemplo: un FluidNC que tiene la IP `192.168.1.50` y escucha por Telnet.

1. Abre el menú **Máquina**, **Gestionar dispositivos…**, y elige o crea el perfil (por ejemplo, con el tipo de controladora **FluidNC**).
2. En el grupo **Comunicación**, abre **Tipo de conexión** y elige **Red · TCP / Telnet**.
3. En **Dirección de la máquina** escribe `192.168.1.50` (sin `tcp://`, sin espacios ni barras) y deja el **Puerto** en `23`.
4. Pulsa **Probar conexión** (más abajo) y, si responde, **Guardar cambios**.
5. Ve a la pestaña **Máquina**. En la sección **Conexión** ya no aparecen el puerto serie ni los baudios: ves la **Dirección** (`192.168.1.50:23`) y una nota que dice cómo cambiarla. Pulsa **Conectar**.

Si tu máquina usa WebSocket, elige **Red · WebSocket** y escribe la **Dirección WebSocket** completa, por ejemplo `ws://192.168.1.50:81`. Cambiar entre TCP y WebSocket conserva el nombre de la máquina (`192.168.1.50`), solo hay que revisar el puerto.

La pantalla te corrige antes de guardar:

- **Dirección vacía o con `tcp://`, espacios o barras:** «Escribe la dirección de la máquina, sin «tcp://», espacios ni barras».
- **Puerto fuera de 1 a 65535:** «El puerto debe ser un número entero de 1 a 65535».
- **WebSocket que no empieza por `ws://`:** te pide escribirla así; con `wss://` te dice que las conexiones cifradas no están soportadas.

> **Consejo:** si tu router no resuelve nombres como `fluidnc.local`, usa la IP. Conviene reservarle una IP fija a la máquina en el router para que no cambie.

## Probar la conexión

**Probar conexión** (en el mismo grupo **Comunicación**) abre la dirección, lee el saludo de la máquina, hace las mismas consultas de solo lectura de la detección y cierra. **No mueve nada ni cambia ningún ajuste**, así que es seguro hacerlo con la máquina encendida y la tapa cerrada.

- Si todo va bien lees «La máquina respondió: Grbl 1.1h» (o el firmware que informe). Si responde pero no reconoce el firmware, lo dice.
- Si falla, el mensaje dice qué pasó y qué revisar (ver la tabla siguiente).
- Está desactivado mientras la dirección no sea válida y cuando ya estás conectado a esa misma dirección (desconecta antes).
- Cambiar la dirección borra el resultado anterior: la prueba vale solo para la dirección que ves.

## Si la conexión falla

Los errores llegan explicados con claridad, con la dirección que se intentó:

| Mensaje | Qué significa y qué hacer |
|---|---|
| «No se encontró 'X' en la red» | El nombre no se resuelve. Revisa cómo está escrito o usa la IP |
| «La máquina en X rechazó la conexión» | El puerto está cerrado o el servicio de red de la controladora está apagado. Telnet suele usar el 23; revisa el puerto y la configuración de la placa |
| «X no contestó a tiempo» | Hay un tiempo de espera de 5 segundos. La máquina está apagada, en otra red o la IP es otra |
| «No se puede llegar a X» | El equipo no es alcanzable desde esta red. Revisa el Wi-Fi o el cable y la IP |
| «X cerró la conexión al abrirla: ¿hay otro programa conectado a ella?» | Muchas placas aceptan un solo cliente. Cierra el otro programa (el navegador con la interfaz web, otro software de láser) |
| «X cerró la conexión de red» (durante el trabajo) | Se perdió la red. La app lo avisa como una desconexión; mira [Solución de problemas](15-solucion-de-problemas.md) |

## Qué cambia respecto al USB

- **No hay reinicio de la placa al conectar**, así que conecta más rápido; Tlaku solo descarta un saludo breve (hasta 1,5 segundos) antes de preguntar el firmware.
- **No hay baudios.** El selector desaparece.
- Todo lo demás es igual: la consola, el encuadre, el trabajo, la pausa y los ajustes en vivo usan la misma ruta que con el cable. Ver [Enviar trabajos con seguridad](09-enviar-con-seguridad.md).
- Una conexión de red puede cortarse sin que nadie toque un cable (el Wi-Fi se cae). Si pasa en pleno trabajo, la máquina puede seguir ejecutando lo que ya tenía en su búfer: ten el paro de emergencia a mano.

## Máquina virtual por red

Para aprender el flujo sin tener la máquina, la lista **Puerto** de un perfil con conexión **Puerto serie (USB)** ofrece, además de las máquinas virtuales de siempre, las **Máquina virtual por red · Falcon2 Pro**, **GRBL 1.1**, **grblHAL**, **FluidNC**, **Marlin** y **Smoothieware**.

A diferencia de la máquina virtual normal, esta es un emulador que escucha en un puerto TCP local de tu propio equipo (`127.0.0.1`) y la app se conecta a él **de verdad, por un socket**, como lo haría con una máquina de red. Sirve para ver cómo se comporta la conexión por red (saludo, trabajo, desconexión) sin hardware. El láser sigue siendo simulado: no se graba nada. Al desconectar se cierra el socket y se detiene el emulador. Tiene las mismas velocidades (`×1`, `×5`, `×20`) y la sección **Simular falla** de la máquina virtual normal (ver [Conectar tu máquina](08-conectar-maquina.md)).

## Automatización por UDP

Tlaku puede **escuchar comandos de texto** que le manden otros programas o scripts **de tu propio equipo**: abrir un archivo, preguntar el estado de la máquina, detener un trabajo o pedir que empiece. Sirve, por ejemplo, para que un script de tu taller (o un programa de producción) deje un diseño cargado, un botón físico detenga el láser o una hoja de cálculo consulte si la máquina está libre.

**Está apagada de fábrica.** Para activarla: **Editar > Preferencias…** (`Ctrl/Cmd+,`), sección **Automatización**.

1. Marca **Activar la automatización por UDP**. Aparece el **Puerto UDP** (47801 de fábrica; de 1024 a 65535) y un estado: «Escuchando en 127.0.0.1:47801». Si el puerto está ocupado, dice «No se pudo activar» y el motivo; cambia el puerto.
2. Deja **Permitir iniciar desde automatización** apagada, salvo que sepas muy bien lo que haces (ver más abajo).

### Seguridad

- **Solo escucha en `127.0.0.1`**: ningún otro equipo de tu red puede mandarle nada. Pero **cualquier programa que corra en tu equipo** sí puede, así que no la actives en un equipo compartido con software que no controles.
- **`START` no dispara el láser por sí solo.** Con la casilla apagada (de fábrica), `START` solo abre el diálogo de iniciar de siempre, con su lista de comprobación, y **decides tú**. La respuesta es `PENDING`.
- Con **Permitir iniciar desde automatización** encendida, `START` empieza el trabajo sin preguntar: cualquier programa de este equipo puede arrancar el láser. Aun así, si la lista **Antes de enviar** del diálogo de iniciar la tienes como obligatoria, se pregunta en el diálogo. Y siempre se exige que la máquina esté conectada y en reposo, y que haya algo que enviar.
- **`STOP` siempre se atiende** si hay una máquina conectada: detener nunca es peligroso.
- **`LOAD` solo abre archivos que existen**, con ruta absoluta, con el mismo importador de siempre. No hay forma de leer ni escribir archivos desde aquí.

### El protocolo

Un comando por datagrama UDP, en una sola línea de texto UTF-8 (hasta 4096 bytes). Las mayúsculas no importan. La respuesta es un datagrama de una línea, enviado a quien preguntó.

| Comando | Respuesta | Qué hace |
|---|---|---|
| `PING` | `PONG` | Comprueba que Tlaku escucha |
| `LOAD /ruta/absoluta/pieza.svg` | `OK loading` | Abre un proyecto `.tlaku` o importa el diseño, como si lo soltaras sobre la ventana. La ruta puede llevar espacios |
| `START` | `OK starting` o `PENDING confirm_in_app: ...` | Pide iniciar el trabajo (ver Seguridad) |
| `STATUS` | `STATUS connected=1 state=idle x=12.500 y=30.000` | Estado de la máquina y su posición de trabajo en mm; sin máquina, `STATUS connected=0` |
| `STOP` | `OK` | Detiene el trabajo (pausa de avance y reinicio suave); sin trabajo, cancela el movimiento |

Los errores llegan como `ERR <código>: <texto en el idioma de la app>`; los códigos son `not_connected`, `busy`, `not_found`, `bad_argument`, `bad_text`, `too_long`, `empty`, `unknown_command` y `failed`.

> **Ojo:** este protocolo es propio de Tlaku. **No es el de LightBurn**: los scripts hechos para LightBurn no funcionan sin cambios.

### Ejemplos

Desde una terminal en macOS o Linux (con `nc`):

```sh
echo PING | nc -u -w1 127.0.0.1 47801
echo 'LOAD /Users/ana/disenos/llavero.svg' | nc -u -w1 127.0.0.1 47801
echo STATUS | nc -u -w1 127.0.0.1 47801
```

Desde Python, un script que carga un diseño y espera a que la máquina esté libre antes de pedir que empiece:

```python
import socket, time

def tlaku(cmd, port=47801):
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.settimeout(2)
    s.sendto(cmd.encode(), ("127.0.0.1", port))
    return s.recvfrom(4096)[0].decode().strip()

print(tlaku("LOAD /Users/ana/disenos/llavero.svg"))   # OK loading
while "state=idle" not in tlaku("STATUS"):
    time.sleep(1)
print(tlaku("START"))   # PENDING confirm_in_app: ... -> confirmas en la ventana
```

Con la casilla apagada, ese `START` deja el diálogo de iniciar abierto en la ventana y tú confirmas con la vista previa delante. Es la forma recomendada de trabajar.

El detalle técnico está en `docs/app-api/automatizacion.md`, sección «Automatización por UDP».

Siguiente: [Enviar trabajos con seguridad](09-enviar-con-seguridad.md)
