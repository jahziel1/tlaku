# Contrato app-motor: Conexión por red (GRBL, grblHAL, FluidNC)

Parte de [el contrato app-motor](README.md). GRBL por TCP y WebSocket.

## Conexión por red y automatización por UDP (ola 4)

### Conexión por red (GRBL, grblHAL, FluidNC)

`DeviceProfile.connection` elige el transporte: `{ "type": "serial" }` (por omisión; el puerto se
escoge al conectar), `{ "type": "tcp", "host", "port" }` o `{ "type": "web_socket", "url" }`
(`udp` es de Ruida). La interfaz entrega al motor, en el mismo argumento `port` de `connect` y
`probe_port`, un identificador por esquema; ningún puerto serie empieza así:

| Identificador | Qué abre |
|---|---|
| `tcp://host[:puerto]` | Telnet o TCP crudo; puerto 23 si falta (grblHAL Ethernet, servidor Telnet de FluidNC). Se rechaza y descarta la negociación Telnet si el servidor la manda |
| `ws://host[:puerto][/ruta]` | WebSocket de FluidNC y ESP3D. `wss://` no está soportado. Los mensajes de sesión de ESP3D (`currentID:`, `activeID:`, `PING:`) se descartan |
| `virtualnet:<modelo>[@<velocidad>]` | Máquina virtual por red: el emulador detrás de un socket TCP local en `127.0.0.1` (puerto libre) al que la app se conecta de verdad. Los modelos y la velocidad son los de `virtual:`. Al soltar la conexión se cierra el socket y se detiene el emulador. `list_ports` los ofrece al final |

Detalles:

- Por red no hay reinicio de la placa: tras conectar solo se espera y descarta un saludo breve (hasta 1,5 s)
  antes de `$I`. El `ok` puede llegar partido en varios paquetes; el lector de líneas trabaja byte a byte.
- Los errores de conexión llegan explicados en español: nombre no encontrado, conexión rechazada,
  tiempo agotado (5 s), equipo no alcanzable, conexión cerrada por la máquina.
- `probe_port` acepta esos identificadores («Probar conexión» del perfil): abre, lee el saludo, hace las
  mismas consultas de solo lectura de siempre y cierra, sin mover nada. Por red espera el saludo 0,8 s
  en lugar de 3,5 s.
- El transporte (`tlaku_grbl::net::{TcpTransport, WebSocketTransport}`) implementa `Transport`, así que el
  `Streamer` y `Connection::start_with` no cambian. El cierre es ordenado (`shutdown` / trama Close).
