# Controladoras Ruida: protocolo y viabilidad para Tlaku

Estado: investigación previa (2026-10-13). No hay código en el repositorio todavía.
Método: sala limpia. Solo se leyeron descripciones del protocolo en prosa o tablas; **no se leyó código fuente** de ninguna implementación. Los hechos (bytes, puertos, fórmulas) se resumen con palabras propias y cada uno cita su URL.

## 0. Fuentes y su estado

| Clave | Fuente | Tipo | Uso |
|---|---|---|---|
| [EDU] | https://edutechwiki.unige.ch/en/Ruida | Wiki en prosa y tablas (la más completa) | Leída |
| [FAB] | https://wiki.fablab-nuernberg.de/w/Diskussion:Nova_35 | Wiki de ingeniería inversa (más antigua; Nova 35) | Leída |
| [SCH] | https://stefan.schuermans.info/rdcam/messages.html | Descripción de mensajes (solo parte; la página de scrambling no se obtuvo) | Leída |
| [JNW] | https://github.com/jnweiger/ruida-laser/blob/master/doc/protocol.md | Documento Markdown de protocolo (solo el `.md`, no el código) | Leída |
| [FOR1] | https://forum.lightburnsoftware.com/t/sending-basic-commands-via-udp-to-ruida/166604 | Post de foro: panel remoto, UDP 50207 | Leída |
| [FOR2] | https://forum.lightburnsoftware.com/t/rdc6445g-ec-network-trouble-and-looking-for-more-resources/15399 | Foro: red, UDP, sin reintentos | Leída |
| [FOR3] | https://forum.lightburnsoftware.com/t/communicating-with-ruida-controller-for-user-restriction/59400 | Foro: ofuscación, el controlador ignora Ethernet con un trabajo en marcha | Leída |
| [FOR4] | https://forum.lightburnsoftware.com/t/anyone-know-has-the-ruida-firmware-ever-been-updated-to-fix-their-network-limitation/151129/9 | Foro: limitación de subred, UDP 50200 | Leída |
| [RPA] | https://pypi.org/project/ruida-pa/0.22.0/ y https://github.com/StevenIsaacs/ruida-pa (README, `docs/guides/rpascript-guide.md`, PR 3) | README y guías de un analizador MIT | Solo prosa de README y guías; **código no leído** |
| [FOR5] | https://forum.lightburnsoftware.com/t/ruida-emulator-software/137853 | Foro: no hay emulador del panel | Leída |

Existen pero **no se leyeron** (solo código o código mezclado):
- MeerK40t, su plugin Ruida y su emulador Ruida (GPL). Es la implementación libre más usada; no se toca.
- Crate Rust `ruida` 0.1.1, licencia MIT, https://github.com/lyonlabs/ruida (metadatos en https://crates.io/crates/ruida). Código no leído.
- Código de `ruida-pa` y de `jnweiger/ruida-laser` (más allá del `.md`). Código no leído.
- Un artículo "Network Aware Laser Cutter Security" (autor y URL citados en [EDU]): la URL citada devuelve 404 y no se encontró copia.
- LightBurn y RDWorks (cerrados).

Nota de licencia: los hechos no tienen copyright, pero [EDU] es una wiki con su propia licencia de texto. Aquí no se copian tablas completas ni frases; los vectores de ejemplo son datos.

### Discrepancias entre fuentes (resolver con hardware)

| Tema | Fuente A | Fuente B |
|---|---|---|
| Checksum UDP: ¿sobre bytes antes o después del swizzle? | [EDU]: antes ("preswizzle") | [FAB], [JNW]: suma de los bytes ya swizzleados |
| Byte de ACK/NAK | [EDU]: `0xCC` ok, `0xCF` checksum mal | [FAB], [JNW]: `0xC6` ok, `0x46` error |
| Tamaño máximo del datagrama | [EDU]: 1474 B (en la práctica ~1000) | [FAB], [JNW]: 1472 B incluido el checksum |
| Timeout de ACK | [EDU]: ~4 s | [FAB]: 4 s probado; [JNW]: marcado como TODO |
| Bit "en movimiento" del estado (0x0400) | [EDU]: `IsMove = 0x10` | [RPA guía]: bit 24 = moviéndose; bit 0 = trabajo en curso; bit 1 = pausa |

Las dos primeras tienen explicación probable (ver 1.1 y 1.3): 0xC6 es 0xCC swizzleado con magic 0x88 y 0x46 es 0xCD swizzleado (cálculo propio, sección 9), de modo que [FAB] describe los bytes del cable y [EDU] los bytes ya desswizzleados. El checksum sigue sin resolverse.

## 1. Transporte

### 1.1 UDP 50200: envío del trabajo (el que usa LightBurn)

- La controladora escucha en el puerto 50200 y responde **desde** el 40200. Hay que enlazar el socket local a 40200 para recibir las respuestas. [EDU] https://edutechwiki.unige.ch/en/Ruida ; confirmado por hilos del foro de LightBurn (según el resumen del buscador: salida al láser 50200, entrada desde 40200; p. ej. https://forum.lightburnsoftware.com/t/lightburn-firewall-settings/12777) y por [FOR4] para el 50200.
- El contenido es el mismo flujo de comandos que un archivo `.rd` (igual que USB), sin cabecera de sesión ni handshake. Lo que se analiza bien, se ejecuta. [EDU], [FAB], [JNW].
- **Un paquete = un datagrama**: 2 bytes de checksum (big endian) seguidos de datos. La longitud de los datos sale del tamaño del datagrama, por eso el esquema no sirve sobre TCP. [FAB], [JNW], [EDU].
- Checksum: los 16 bits bajos de la suma de los valores de todos los bytes del paquete (datos, sin contar los 2 del propio checksum). [EDU] "The first two bytes of any UDP packet are big endian checksum"; [FAB] y [JNW] igual, pero sobre bytes **ya swizzleados**. **Conflicto abierto** (sección 0).
- Tamaño: máximo 1472 B incluido el checksum según [FAB]/[JNW] (1474 según [EDU]). [EDU] dice que las versiones modernas de LightBurn cortan en ~1000 B **en límite de comando**; [FAB]/[JNW] dicen que las versiones antiguas cortaban por bytes, incluso a mitad de comando. Recomendación nuestra: paquetes de datos de hasta ~1000 B cortados en límite de comando.
- ACK por paquete: la controladora responde un byte por cada paquete. [EDU]: `0xCC` = checksum coincide (siguiente paquete), `0xCF` = checksum falla (reenviar). [FAB]/[JNW]: `0xC6` = ok, `0x46` = error; ante error se reintenta el primer paquete y en los siguientes se aborta la transmisión. Aceptaremos como OK `0xC6` y `0xCC`; como error `0x46`, `0x48`, `0xCD` y `0xCF` (0x48 y 0x46 son 0xCF y 0xCD swizzleados, ver sección 9).
- Reintentos y timeout: UDP puede perder paquetes, así que el ACK es obligatorio tras cada paquete; sin respuesta en ~4 s se considera timeout. [EDU], [FAB]. Un hilo del foro de LightBurn dice que la pila no tiene reintentos y solo la forma más básica de detección de errores [FOR2] https://forum.lightburnsoftware.com/t/rdc6445g-ec-network-trouble-and-looking-for-more-resources/15399 . Por Wi-Fi funciona en modo "ping-pong" envío/ACK limitado a ~600 kb/s [FOR2].
- Red: la controladora solo expone un campo de IP (en RDC6445G no hay máscara ni puerta de enlace). La máscara es fija 255.255.255.0 [JNW]/[FAB]. Debe estar en la misma /24 que el PC; hay quien la aísla tras un router con redirección UDP 50200 y NAT [FOR4]. No hay descubrimiento por broadcast [FOR2]. IP de fábrica citada: 172.16.18.11 [FAB]. Una variación de firmware cambia el modo de red (siguiente punto).
- **Variante TCP**: la RDC8445S (ID de tarjeta 0x90109010) no contesta por UDP; escucha por **TCP 50200**, con el mismo swizzle (magic 0x88) y comandos, **sin** el checksum de 2 bytes, con un ACK por escritura, solo un cliente a la vez (una segunda conexión expulsa a la primera). https://github.com/StevenIsaacs/ruida-pa/pull/3 (resumen en prosa; solo se leyó la descripción del PR).

### 1.2 UDP 50207: panel remoto y control en vivo

- La controladora escucha en 50207 y envía desde 40207. Sin swizzle: los bytes van en claro. [EDU]; confirmado en [FOR1] https://forum.lightburnsoftware.com/t/sending-basic-commands-via-udp-to-ruida/166604 (hay que enlazar el socket local a 40207; el primer intento falló por no hacerlo).
- Handshake: se envía `CC` y se contesta `CC`, seguido de un segundo paquete `A5 68 00 00 3D 04 6D 00 00 24 4F 6F` (coordenadas del cabezal, sin decodificar) [FOR1]. `CE` es un keep-alive periódico (se responde `CE`); `CD` lo manda la app al desconectarse [FOR1].
- Cada pulsación es de 3 bytes: `A5 50 xx` (tecla pulsada) y `A5 51 xx` (tecla soltada). [FOR1], [EDU]. Las respuestas se pueden ignorar y los comandos funcionan igual [FOR1].
- Códigos `xx` (de [EDU], con el mapa de jog confirmado solo parcialmente en [FOR1]): `06` inicio/pausa, `09` parar, `5A` reiniciar, `0F` trazar (trace) on/off, `07` ESC, `12` puerta del láser, `05` pulso (pulse), `08` origen, `00` encuadre (el comando completo es `A5 53 00`), `11` velocidad (alterna jog rápido/lento según "wireless panel speed shift" en la configuración de la máquina [FOR1]); ejes con pulsación `A5 50`/suelta `A5 51`: X+ `02`, X- `01`, Y+ `03`, Y- `04`, Z+ `0A`, Z- `0B`, U+ `0C`, U- `0D`.

### 1.3 USB

- [EDU]: el chip es un FT245R (USB 1.1 full speed) y da como ajustes 19200 baud, 8N1 con control de flujo por hardware. Dos métodos: transferencia directa PC a máquina o memoria USB (almacenamiento).
- [FAB]/[JNW]: el contenido es el mismo `.rd` (USB-serie o USB mass storage), "sin checksum" en USB (el checksum de 2 bytes es solo de red).
- Duda: el FT245R es un FIFO paralelo, no un UART, y [EDU] no dice si LightBurn lo ve como puerto serie virtual. **No hay descripción del framing USB**. Se asume mismo swizzle y mismos comandos que la red, sin checksum, pero es una suposición. En esta investigación se descarta USB para la primera versión; el trabajo en red es lo documentado.

## 2. Swizzle (ofuscación por byte)

Algoritmo [EDU] https://edutechwiki.unige.ch/en/Ruida (sección Swizzling) y descripción equivalente en prosa en [FAB]:

Por cada byte `b` (8 bits), al enviar:
1. Intercambiar el bit 7 y el bit 0 (el wiki lo escribe como tres XOR con desplazamientos de 7: `b ^= b>>7; b ^= b<<7; b ^= b>>7`, siempre enmascarando a 8 bits).
2. `b ^= magic`.
3. `b = (b + 1) mod 256`.

Al recibir: restar 1, `XOR magic`, y repetir el intercambio de bits 0 y 7.
Es una biyección de bytes sin estado: no hay dependencia entre bytes. El checksum se discute en 1.1.

"Magic key" por modelo, [EDU]:

| Modelo | Magic |
|---|---|
| 644XG (por defecto), 644XS, 320, 633X, 654XG | 0x88 |
| 634XG | 0x11 ("altered scrambling": el texto no explica qué más cambia) |
| RDL9635 | 0x38 |

Además hay una tabla **ID de tarjeta -> magic** en [EDU] (el ID se lee de la dirección 0x2FE, ver sección 4.4). Valores citados: 0x6320 -> 0x11, 0x6302 -> 0x83, 0x6301 -> 0x88, 0x6300 -> 0x33, 0x2210 -> 0x16, 0x630B -> 0x33, 0x6501 -> 0x34, 0x6511 -> 0x77, 0x6512 -> 0x75, 0x6513 -> 0x99, 0x6514 -> 0x13, 0x651E -> 0x60, 0x651F -> 0x76, 0x6530 y 0x6532 -> 0x10, 0x6811 -> 0x77, 0x6920 -> 0xA1, 0x690F a 0x6911 -> 0xA1, 0x6912 -> 0x88, 0x6930 y 0x693F -> 0x1A, 0x7230, 0x7235, 0x7236, 0x7430 y 0x7930 -> 0x10. Todo lo no listado usa 0x88. El hecho de que estos IDs aparezcan en una sola fuente los deja con confianza media.

Cómo detectarla (propuesta nuestra, no documentada): el ID de tarjeta solo se puede leer si ya se habla bien con el controlador. Sondear con una lectura (`DA 00 05 7E`, sin efectos) probando magics candidatos (0x88, luego 0x11, 0x38 y los de la tabla) hasta recibir un ACK y la respuesta esperada. [EDU] dice que la consulta de 05 7E debe responder `0x65006500` pronto en la comunicación ("mission-critical"). No está documentado qué hace el equipo ante un magic equivocado (¿NAK? ¿silencio?), así que el sondeo debe tolerar ambos. Otra opción, que debe ser la predeterminada: que el usuario elija el modelo en el perfil de máquina y usar 0x88.

## 3. Codificación de números

Fuentes: [EDU], [SCH] https://stefan.schuermans.info/rdcam/messages.html , [FAB].

- Estructura del flujo: un byte con el bit 7 en 1 (0x80 a 0xFF) **inicia un comando**; los bytes de datos solo usan 7 bits (0x00 a 0x7F). [EDU], [SCH]. Por eso los números se parten en grupos de 7 bits, el más significativo primero. [FAB]: "most significant first".
- Tipos [EDU]/[SCH]:

| Tipo | Bytes | Unidad |
|---|---|---|
| int7 | 1 | índice de capa/parte (0 a 127) |
| Abscoord | 5 (35 bits) | µm, con signo (complemento a dos de 35 bits) |
| Relcoord | 2 (14 bits) | µm, con signo (complemento a dos de 14 bits) |
| Potencia | 2 (14 bits sin signo) | 100/2^14 % por unidad (0,0061 %) |
| Velocidad | 5 | µm/s |
| Tiempo | 5 | µs (en [EDU]; en el ejemplo de `C6 15`/`C6 16` aparece como "ms") |
| Frecuencia | 5 | Hz |
| Color | 5 (24 bits útiles) | BBGGRR (azul en el byte alto) |
| Cadena | variable | terminada en cero |

- Con signo ([SCH]): Abscoord +1 µm = `0x1`, -1 µm = `0x7FFFFFFFF`, mayor positivo `0x3FFFFFFFF`, mayor negativo `0x400000000`. Relcoord: -1 µm = `0x3FFF`, mayor positivo `0x1FFF` (es decir +-8,191 mm por salto relativo), mayor negativo `0x2000`. [EDU] dice que 32 bits bastarían en la práctica.
- Direcciones de memoria (DA): dos bytes de 7 bits, el alto primero. El wiki [EDU] las muestra combinadas (por ejemplo `0x2FE`), y las guías de `ruida-pa` en bytes crudos (`05 7E`, que es 5*128 + 0x7E = 0x2FE). Ver https://github.com/StevenIsaacs/ruida-pa/blob/main/docs/guides/rpascript-guide.md . Cuidado al leer tablas de distintas fuentes.
- Las unidades que muestra [EDU] en los ejemplos decodificados están en milésimas de pulgada, no en µm (el valor 7504,330709 corresponde a 190 610 µm; ver sección 9, vector V3).

## 4. Comandos

Formato general: `[opcode][subopcode?][parámetros]`. Tabla completa en [EDU] (más de 100 filas). Aquí lo que necesitamos para un trabajo de vectores y para el control, con el nivel de certeza indicado por las fuentes. Longitudes en bytes, incluido el opcode.

### 4.1 Movimiento y corte

| Bytes | Significado | Fuente / certeza |
|---|---|---|
| `88 X(5) Y(5)` | Mover rápido (láser apagado) a X,Y absolutos (11 B) | [EDU], [SCH], [FAB] 99 % |
| `89 dX(2) dY(2)` | Mover relativo láser apagado (5 B) | [EDU], [SCH]; [FAB] 80 % |
| `8A dX(2)` / `8B dY(2)` | Mover relativo solo en X / solo en Y (3 B) | [EDU], [SCH] |
| `A8 X(5) Y(5)` | Cortar a X,Y absolutos con láser (11 B) | [EDU], [SCH], [FAB] 99 % |
| `A9 dX(2) dY(2)` | Cortar relativo (5 B) | [EDU], [SCH], [FAB] 99 % |
| `AA dX(2)` / `AB dY(2)` | Cortar relativo en X / Y (3 B) | [EDU], [SCH] |
| `80 00 X(5)` / `A0 00 Y(5)` / `80 08 Z(5)` / `A0 08 U(5)` | Mover un eje (7 B) | [EDU] |
| `D9 00 opciones X(5)` etc. (D9 00 a D9 03) | Mover eje X/Y/Z/U (8 B) | [EDU]; [FAB]: `D9 00 02` X 99 %, Y/Z/U 50 % |
| `D9 10 opciones` | Home XY (3 B) | [EDU] |

Las coordenadas absolutas son relativas al origen del trabajo [FAB]. Para ir seguros se pueden calcular los relativos a partir de las posiciones absolutas ya redondeadas a µm para no acumular error (idea nuestra).

### 4.2 Potencia y velocidad

| Bytes | Significado |
|---|---|
| `C6 01 P(2)` / `C6 02 P(2)` | Láser 1: potencia mínima / máxima (4 B) [EDU], [SCH], [FAB] 99 % |
| `C6 21`/`C6 22` | Láser 2: mínima / máxima [EDU], [FAB] 99 %; láseres 3 y 4: `C6 05`/`06` y `C6 07`/`08` [EDU] |
| `C6 31 capa P` / `C6 32 capa P` | Potencia mín / máx del láser 1 **para una capa** (5 B). Láser 2: `41`/`42`; láser 3: `35`/`36`; láser 4: `37`/`38` [EDU], [SCH] |
| `C7 P` / `C8 P` | Potencia inmediata 1 / potencia final 1 (3 B); equivalentes `C0, C2, C3` y `C1, C4, C5` para láseres 2 a 4. Su semántica **no está descrita**. [EDU] |
| `C6 10`, `C6 11`, `C6 12`, `C6 13` | Intervalo de láser, retardo añadido, retardo de encendido, retardo de apagado (tiempo, 7 B) [EDU] |
| `C6 60 láser capa frecuencia` | Frecuencia del láser por capa (9 B) [EDU] |
| `C9 02 V(5)` | Velocidad de corte/movimiento (7 B) [EDU], [SCH]; [FAB] 80 % |
| `C9 04 capa V(5)` | Velocidad por capa (8 B) [EDU], [SCH] |
| `C9 03`, `C9 05`, `C9 06` | Velocidad de eje, forzada, de movimiento de eje [EDU] (semántica sin detallar) |

Comportamiento observado (foro): a velocidades por debajo de un umbral ("Start Speed", unos 10 mm/s) la controladora usa solo la potencia mínima, y por encima sube hacia la máxima. https://forum.lightburnsoftware.com/t/why-ruida-doesnt-light-the-laser-below-10mm-s-80w/120353/2 (visto en el resumen del buscador; no se abrió la página completa, confianza media). Esto es el equivalente funcional de nuestro `PowerMode::Dynamic`; `PowerMode::Constant` equivale a mín = máx. Es una interpretación nuestra a validar con hardware.

### 4.3 Capas y configuración

| Bytes | Significado |
|---|---|
| `CA 01 00` | Fin de capa [EDU] |
| `CA 01 01` a `04`, `55`, `05` | Modos de trabajo 1..6 (nombres sin significado claro) [EDU] |
| `CA 01 10`/`11` | Dispositivo de la capa 0/1 [EDU] |
| `CA 01 12`/`13` | **Aire apagado / encendido** (por capa) [EDU]; [SCH] describe `CA 01` como flags: 0x00 todo apagado, 0x01 soplador de aire |
| `CA 02 parte índice` | Número de capa actual [EDU]; [SCH]: `CA 02 capa` |
| `CA 05 color(5)` / `CA 06 capa color(5)` | Color de capa / color de parte [EDU]; en [SCH]: `CA 06 capa color` |
| `CA 22 capa` | Número máximo de capa [EDU], [SCH] |
| `CA 41 parte modo` | Modo de trabajo de la parte [EDU] |
| `CA 03 0D`, `CA 10 00`, `CA 01 30`/`31` | Inicializaciones del ejemplo (EnLaserTube, EnExIO, EnLaser2Offset) [EDU] |
| `E7 03 X Y` / `E7 07 X Y` | Esquina superior-izquierda / inferior-derecha del proceso (caja del trabajo) [EDU], [SCH] |
| `E7 50`/`E7 51` | Caja del documento (min / max) [EDU]; [FAB] 30 % (dudas) |
| `E7 52`/`53`/`61`/`62 parte X Y` | Caja de cada capa (y versión "Ex") [EDU] |
| `E7 54 n Abscoord` / `E7 55 n Abscoord` | Offset de pluma / de capa [EDU] |
| `F1 03 X Y` | Offset de pantalla [EDU] |
| `F1 00`, `F1 01`, `F2 00`, `F2 01`, `F2 02`, `F2 03`/`04`, `F2 05`/`06`/`07`, `E7 13`/`17`/`23`/`24`/`08`/`06`/`04`/`05`, `EA`, `EB`, `E7 60` | Índices de elementos, nombre de elemento, matrices (arrays) y repeticiones. [EDU] solo las nombra; el significado de varios campos queda sin explicar (por ejemplo, 7 valores "Array Repeat 1,1,-1,-710,-7808,4,7597"). **Zona de riesgo** |

### 4.4 Archivo y control del proceso

| Bytes | Significado |
|---|---|
| `D8 10` | Modo de punto de referencia (mismas bytes de ref. 0/1/2 en [EDU]; el ejemplo empieza con `D8 10` + `F0`) |
| `F0` | Fijar punto de referencia [EDU] |
| `E6 01` | Absoluto [EDU] |
| `F1 02 n` | Habilitar corte por bloques [EDU] |
| `D8 00` | **Iniciar proceso** (aparece en la cabecera del archivo de ejemplo [EDU]; [FAB]/[JNW] lo omiten) |
| `D8 01` / `D8 02` / `D8 03` | Detener / pausar / reanudar proceso [EDU] |
| `D8 2C` / `D8 2D` / `D8 2E` | Home Z / home U / enfocar Z [EDU] |
| `E7 00` | Fin de bloque [EDU] |
| `D7` | **Fin de archivo** [EDU], [FAB]; el último paquete ya no necesita pausa |
| `E7 01 nombre\0` | Nombre de archivo; `E8 02 E7 01 nombre\0` fija el nombre del archivo del siguiente envío (debe seguir pronto) [FAB] 90 %, [EDU] |
| `E8 00`, `E8 01 n`, `E8 03 n`, `E8 04` | Borrar documento, nombre del documento n, seleccionar documento, calcular tiempo [EDU] |
| `E5 05 suma(5)` | **Fin de trabajo con suma de verificación** del archivo (en capturas de LightBurn). Solo la cita la guía de `ruida-pa` ("END_JOB"). https://github.com/StevenIsaacs/ruida-pa/blob/main/docs/guides/rpascript-guide.md |
| `CC` / `CD` | ACK / error de la máquina a nivel de mensaje [EDU], [FAB], [JNW] |

**Suma de verificación del archivo**: según `ruida-pa`, es una suma corrida de los bytes de los comandos de grabado/corte y de configuración de capa entre el inicio del trabajo y el fin de bloque; se excluyen lectura/escritura de memoria (`DA`) y teclas (`A7`/`A5`). Sus capturas de LightBurn difieren de su cálculo en 220, así que **el conjunto exacto de bytes no está resuelto**. https://pypi.org/project/ruida-pa/0.22.0/ El ejemplo de archivo de [EDU] (hecho con RDWorks) termina con `DA 01 06 20 ...` + `D7` y no muestra `E5 05`; puede ser una diferencia de versión o de programa.

Lectura de estado y posición: `DA 00 <dirección 2 B>` consulta y `DA 01 <dirección 2 B> <valor 5 B>` es la respuesta (y también la forma de escribir; en [EDU] la sintaxis incluye una segunda copia del valor de 5 B cuyo propósito no se explica). [EDU], [FAB] 99 %. Direcciones conocidas (bytes crudos entre paréntesis):

| Dirección | Contenido | Fuente |
|---|---|---|
| 0x2FE (`05 7E`) | ID de tarjeta; debe devolver `0x65006500` (condición para que la comunicación se considere correcta) | [EDU] |
| 0x2FF (`05 7F`) | Versión de la placa (cadena) | [EDU], guía rpa |
| 0x060 (`00 60`) | Tipo de máquina (0x1155 / 0xAA55) | [EDU] |
| 0x200 (`04 00`) | Estado de la máquina; bits relevantes 0b00110111; ver discrepancia en sección 0 | [EDU], guía rpa |
| 0x221/0x231/0x241/0x251 (`04 21`, `04 31`, `04 41`, `04 51`) | Posición de los ejes X, Y, Z, U ("Axis Preferred Position" en [EDU]; "posición actual" en la guía de rpa, que escribe `0x0421` etc. en bytes crudos) | [EDU], guía rpa |
| 0x026 / 0x036 (`00 26`, `00 36`) | Tamaño de la cama X / Y | guía rpa |
| 0x205 (`04 05`) | Número total de documentos ("saved job count") | [EDU], [FAB] |
| 0x201/0x202/0x203/0x211 | Tiempo total encendida, tiempo de trabajo, número de trabajos, tiempo de láser | [EDU] |
| 0x390 (`07 10` a `07 74`) | Tiempo del documento | [EDU] |
| 0x591 (`0B 11`) | Bloqueo de la tarjeta (card lock) | [EDU] |
| 0x004, 0x005, 0x00C, 0x010, 0x011-0x013, 0x020/0x030/0x040/0x050, 0x100, 0x102, 0x10A | Habilitación de IO, velocidad G0, velocidad de home, modo de control del sistema (valor de retorno 0x0000 origen / 0x4000 sin retorno / 0x8000 origen absoluto), PWM/mín/máx del láser 1, parámetros de eje, ajustes del sistema, velocidad de giro, aceleración G0 | [EDU] |

No se encontró documentado cómo se codifica la posición exacta (se asume Abscoord en µm), ni cómo se consulta la lista de archivos más allá de `E8 01`.

### 4.5 Pausa, reanudar, parar, jog, home, encuadre, aire

- Vía flujo `.rd`/UDP 50200: pausa `D8 02`, reanudar `D8 03`, parar `D8 01`, iniciar `D8 00` (arriba). Con un trabajo en marcha, según [FOR3] el controlador parece ignorar el puerto Ethernet de datos; entonces el control en vivo pasa por el panel remoto (UDP 50207).
- Vía panel remoto (UDP 50207, sección 1.2): `06` inicio/pausa (alterna), `09` parar, `5A` reiniciar, jog con pulsar/soltar, `08` origen, encuadre `A5 53 00`, `0F` trazar.
- Home: `D9 10 <opciones>` (XY), `D8 2C/2D` (Z/U) [EDU]. Qué valor tiene `<opciones>` no se documenta.
- Jog por flujo: `D9 00..03 <opciones> <Abscoord>`; las opciones no se documentan.
- Aire: dentro del archivo, `CA 01 12`/`13` por capa [EDU]. No hay comando documentado de aire inmediato fuera de un trabajo.
- Láser (laser gate/pulso de prueba): `A5 50 12` y `A5 50 05`/`A5 51 05` en el panel [EDU]. **Disparan el láser: no se usan nunca** en Tlaku salvo acción explícita del usuario.

## 5. Archivo `.rd`

Lo documentado ([EDU], sección File Structure, y su Example 1):

- Es el mismo flujo de comandos que viaja por UDP/USB, y se swizzlea igual (el wiki dice que el contenido por UDP es "an RD file ... using the same swizzling as USB"). [EDU], [FAB]. Sin checksum de 2 bytes en el archivo ni en USB.
- Cabecera principal: punto de origen y cajas envolventes de todo el proyecto. Cabeceras de capa: una por capa (velocidad, potencia mín y otras potencias, color, frecuencia), equivalentes a los ajustes "Work" de RDWorks. Cuerpo: comandos de pre-arranque y configuración, luego los datos de cada capa, cerrando con `D7`.
- Ejemplo 1 de [EDU] (un rectángulo, una capa), en orden (los números son los que decodifica el wiki; los hex son bytes **sin swizzle**):

```
d810 | f0 | e601 | f10200 | d800
e70600000000000000000000                 feed repeat
e70300000b51120000021258                 process top-left
e707000011164c0000064e02                 process bottom-right
e75000000b51120000021258                 doc min
e751000011164c0000064e02                 doc max
e7040001000100000000000000000000         process repeat
e70500                                   array direction
c904000000060d20                         velocidad 100 mm/s (capa 0)
c631001319 / c632001828                  potencia 1 mín / máx (capa 0)
c641002632 / c642002632                  potencia 2 mín / máx (capa 0)
c635002632 / c636002632 / c637002632 / c638002632
ca06000000000000                         color capa 0 = #000000
ca410000                                 modo de trabajo 0
e7520000000b51120000021258 / e75300000011164c0000064e02      caja de capa
e7610000000b51120000021258 / e76200000011164c0000064e02      caja de capa "Ex"
ca2200                                   capa máxima = 0
e754000000030650 / e754010000030650      offset de pluma 0/1
e755000000000000 / e755010000000000      offset de capa 0/1
f10300000306500000030650                 offset de pantalla
f10000 / f10100 / f20000 / f20100        índices de elemento
f202052a391c41046a150820                 nombre de elemento (cadena codificada)
f20300000b51120000021255 / f20400001116500000064e03          matriz mín / máx
f20600000000000000000000 / f20700
f205000100017f7f7a3a430000043b2d         matriz de elemento (repetición)
ea00 | e76000 | e71300000b51120000021255 | e71700001116500000064e03
e72300000000000000000000 | e72400
e708000100017f7f7a3a430000043b2d         matriz de repetición
ca0100 | ca0200 | ca0130 | ca0110 | ca0113      fin de capa, capa 0, láser 2 offset 0, dispositivo 0, AIRE ON
c9020000060d20                           velocidad 100 mm/s
c6150000000000 / c6160000000000
c6011319 / c6021828 / c6212632 / c6222632 / c6052632 / c6062632 / c6072632 / c6082632
ca030d | ca1000
8800001116500000021256                   mover (abs) a la esquina inicial
a800000b51120000021256                   cortar abs
a800000b51120000064e03                   cortar abs
a800001116500000064e03                   cortar abs
a800001116500000021256                   cortar abs (cierra el rectángulo)
eb | e700
da01062000000002470000000247             escribir parámetro 06 20
d7                                       fin de archivo
```

  (Reproducido como datos de ejemplo. Fuente: https://edutechwiki.unige.ch/en/Ruida , sección Example Files. El ejemplo 2 añade una segunda capa de 80 mm/s en rojo, `ca0601000000017f`.)
- El orden global (referencia -> cajas -> velocidad y potencias de cada capa -> bloque de elementos y arrays -> `CA 01/02` y configuración de capa -> movimientos -> `EB`, `E7 00` -> `D7`) se desprende del ejemplo. **No hay una especificación formal**: varios campos (matrices, índices de elemento, nombre de elemento, `F1 04`, `E7 36`, `E7 46` con valor fijo 0x11227766) tienen significado incierto. Lo seguro es reproducir la estructura del ejemplo sustituyendo solo valores conocidos.

## 6. Variaciones por modelo y firmware

- Magic distinto por modelo y por ID de tarjeta (sección 2).
- Modelos 644XS, 320, 633X, 654XG: en [EDU] una respuesta 0 a la consulta que el wiki escribe "DA0004" (dirección sin aclarar) provoca un error de comunicación (nota del wiki, sin más detalle).
- 634XG: "altered scrambling" (más cambios que el magic, sin documentar).
- RDC8445S: TCP 50200, sin checksum de 2 bytes, un cliente a la vez, ACK por escritura (PR de ruida-pa).
- RDC6445G/EC: solo campo de IP (sin máscara ni puerta de enlace) [FOR2]; la pila de red impone /24 y limita redes grandes [FOR4].
- RDC6445S, RDC6442G, RDC6442S-B: citadas en foros como conectables por Ethernet con LightBurn; no hay diferencias de protocolo documentadas. https://forum.lightburnsoftware.com/t/connecting-and-setting-up-ruida-rdc6445s/47901 (solo listada por el buscador).
- Funciones opcionales según firmware/IO: capa 2 láser, eje Z/U, enfoque, puerta (en 0x30F bits de "foco" y "aire", guía rpa).
- No hay documento por modelo (RDC6442S/G, RDC6445G, RDC6332G): solo el 6442S aparece como ID en un ejemplo de `ruida-pa`. La RDC6332G no aparece en ninguna fuente leída.

## 7. Seguridad

Hechos documentados:
- **Sin autenticación ni sesión**: lo que se analiza bien, se ejecuta [EDU]. La ofuscación del swizzle no es seguridad [FOR3 menciona "algo de ofuscación"]. Hay bloqueos de PC/tarjeta en memoria (0x181-0x187, 0x591) pero no se describe su uso en red.
- Sin reintentos ni protocolo de errores robusto en UDP [FOR2], [FOR3]. La controladora ignora el puerto de red con un trabajo en marcha (observado con Wireshark) [FOR3] y se reportó un cuelgue ante ciertos trabajos pequeños de ráster al abrir el panel de archivos [FOR3].
- Escritura de memoria (`DA 01`): puede cambiar parámetros de ejes y del sistema. `ruida-pa` la bloquea por defecto ("modo protegido"). https://pypi.org/project/ruida-pa/0.22.0/ No hay documento sobre qué ocurre con una escritura malformada o un valor fuera de rango.
- Archivo malformado: lo que muestra el ejemplo es que `D8 00` (inicio de proceso) va **al principio** del archivo; si la transmisión se corta después, la máquina podría haber empezado a ejecutar. No hay fuente que lo aclare (inferencia nuestra).
- No se describen límites de potencia ni de velocidad en el protocolo: los valores van tal cual (potencia 14 bits, velocidad 35 bits, coordenadas ±17 km en µm); los límites reales los pone el firmware (ancho de cama en 0x26/0x36, velocidades en memoria) y el panel de la máquina. No hay fuente que diga cómo reacciona el firmware a un valor fuera de rango.

Reglas que adoptaremos en `tlaku-ruida` (propias):
1. Nunca enviar `DA 01` (escrituras) ni los comandos de pulso/puerta del láser sin acción explícita.
2. Antes de enviar, comprobar: potencia 0 a 100 % (14 bits), velocidad > 0 y <= tope del perfil, coordenadas dentro de la cama del perfil y, si se pudo leer, dentro de 0x26/0x36.
3. Nada de NaN ni infinitos; todo redondeo a µm con saturación y error (no truncar en silencio).
4. Enviar con la máquina en reposo (estado 0x200) y una sola sesión a la vez; abortar ante NAK repetido y comunicar al usuario que quizá la máquina ya empezó (si hay `D8 00` al inicio).
5. Prueba de trabajo "en seco" siempre disponible (potencia 0 o láser deshabilitado).
6. Subir en una etapa inicial solo trabajos pequeños; documentar que no hay parada de emergencia por red.

## 8. Cómo validar sin hardware

- **Emulador/simulador documentado**: no se encontró ninguno con documentación. MeerK40t incluye un emulador Ruida (GPL; no leído). Un hilo del foro confirma que no hay emulador del panel [FOR5] https://forum.lightburnsoftware.com/t/ruida-emulator-software/137853 . Un desarrollador de MeerK40t dice en los foros que no tiene una controladora y que probar la interfaz sin hardware es el principal obstáculo. https://forum.makerforums.info/t/meerk40t-ruida-support/79891
- **Capturas pcap publicadas**: no se encontró ninguna con descripción. `ruida-pa` (MIT) explica cómo capturar con tshark/tcpdump y decodificar; tiene un directorio `tests/` en el repositorio que no se examinó. https://github.com/StevenIsaacs/ruida-pa . En los foros hay usuarios que capturaron tráfico con Wireshark pero sin publicarlo.
- **Vectores de prueba en documentación**: sí, en [EDU] (Example 1 y 2 con decodificación línea a línea, pero **sin swizzle**) y mediante datos puntuales (`C6 31 00 13 19` = potencia 14,996 %, `C9 04 00 00 00 06 0D 20` = 100 mm/s). No hay ejemplos de bytes swizzleados ni de paquetes UDP con checksum completos; los únicos datos swizzleados son los bytes de respuesta `0xC6` y `0x46` de [FAB]. Por eso los vectores swizzle de la sección 9 son cálculos propios con el algoritmo.

Estrategia de validación sin hardware: construir un decodificador/desensamblador propio (decodificar nuestro flujo y comparar), el ejemplo de [EDU] como prueba de ida y vuelta, y un controlador simulado propio (sección 11).

## 9. Vectores calculados (propios; derivados del algoritmo, no capturados)

Calculados con un script con magic 0x88 y verificados con inversa para los 256 valores y los magics 0x88, 0x11, 0x38.

| Id | Entrada | Resultado | Origen |
|---|---|---|---|
| V1 | swizzle(`CC`) | `C6` | Coincide con el ACK `0xC6` de [FAB]: comprobación cruzada independiente del algoritmo de [EDU] |
| V2 | swizzle(`CD`) | `46` | Coincide con el error `0x46` de [FAB] |
| V2b | swizzle(`CF`) | `48` | Posible NAK de [EDU] visto en cable |
| V3 | velocidad 100 mm/s = 100 000 µm/s | `00 00 06 0D 20` | Coincide con `c904 00 000006 0d20` de [EDU] (capa 00 + 5 B) |
| V4 | potencia 2457 (14,996 %) | `13 19` | Coincide con `c631 00 1319` |
| V5 | 190 610 µm | `00 00 0B 51 12` | Coincide con la caja `e703 00000b5112 ...` de [EDU] |
| V6 | -1 µm (35 bits) | `7F 7F 7F 7F 7F` | Coincide con [SCH] |
| V7 | `DA 00 05 7E` | swizzle `D4 89 0D F7` | Consulta del ID de tarjeta. Checksum sin swizzle `0x015D`, con swizzle `0x0261` (muestra el conflicto) |
| V8 | `D8 10 F0 E6 01 F1 02 00 D8 00` | `D2 99 FA F0 09 7A 8B 89 D2 89` | Cabecera del ejemplo 1. Checksum sin swizzle `0x048A`, con swizzle `0x0647` |
| V9 | `D7` | `60` | Fin de archivo swizzleado |

## 10. Viabilidad para Tlaku

### 10.1 Qué hay en el código

- IR (`crates/tlaku-core/src/toolpath.rs`): `Toolpath { blocks }`, cada `ToolpathBlock` con `speed_mm_min`, `air_assist`, `power_mode`, `z_mm` y `moves` de tipo `Travel { to }` o `Cut { to, power_pct }`. Coordenadas en mm (f64).
- Driver de referencia (`crates/tlaku-grbl/src/`): `gcode.rs` (IR a texto, opciones, recorte de S), `serial.rs` (trait `Transport` de bytes con `MemoryTransport` para pruebas), `streamer.rs` (envío con conteo de caracteres, pausa/cancelación, estados), `preflight.rs` (solo se transmite en reposo), `frame.rs` (encuadre).
- GRBL es un flujo de líneas con control de flujo por caracteres; Ruida es un flujo de **datagramas con ACK por paquete**, así que el trait `Transport` de bytes no encaja tal cual: hace falta un trait de paquetes.

### 10.2 Correspondencia IR -> Ruida

| IR | Ruida |
|---|---|
| `ToolpathBlock` | Una capa Ruida (índice 0 a 127), con `CA 02`, velocidad `C9 02`/`C9 04`, potencias `C6 01/02` (y `C6 31/32`), color `CA 06`, aire `CA 01 12/13` |
| `speed_mm_min` | µm/s = `speed * 1000 / 60` |
| `power_pct` constante en el bloque | mín = máx = `round(p * 16384 / 100)`, saturado a 0x3FFF |
| `PowerMode::Constant` | mín = máx |
| `PowerMode::Dynamic` | mín/máx distintos sujetos a la velocidad de inicio: **a validar**; hasta entonces tratar como constante |
| `Travel` | `88` (absoluto) o `89`/`8A`/`8B` (relativo) |
| `Cut` | `A8`, `A9`/`AA`/`AB` |
| `Cut` con potencia variable (ráster, 0 para overscan) | Hay `C7`/`C8` (potencia inmediata/final) pero sin semántica documentada: **riesgo alto**. Solución inicial: dividir en capas por nivel de potencia cuantizado, o rechazar |
| `z_mm`, rotativo | Fuera de la primera etapa |

### 10.3 Arquitectura propuesta: `crates/tlaku-ruida`

Dependencias: `tlaku-core` y `std` (sin `unsafe`, sin crates de protocolo de terceros; ningún código GPL). Estructura:

- `num.rs`: codificar/decodificar int7, Relcoord (14 bits con signo), Abscoord (35 bits con signo), potencia, velocidad, tiempo, color, cadenas. Saturación explícita con error.
- `swizzle.rs`: `Magic(u8)`, `swizzle`, `unswizzle`, tabla ID-de-tarjeta -> magic y sondeo.
- `cmd.rs`: `enum Command` con las variantes necesarias (mover, cortar, potencias, velocidad, capa, color, aire, cajas, `D8 xx`, `DA 00`/`DA 01`, `D7`, `E5 05`), con `encode(&self, &mut Vec<u8>)` y `decode(&[u8])`. El decodificador es la herramienta de prueba (un desensamblador).
- `rd.rs`: `RdWriter` desde `Toolpath` + `RuidaJobOptions` (magic, perfil de máquina, origen, tipo de archivo de cabecera) a `Vec<u8>` sin swizzle, con cálculo de cajas envolventes por capa, y la suma `E5 05` marcada como experimental. También `to_swizzled`.
- `packet.rs`: partir el flujo en paquetes en límite de comando (tope configurable, ~1000 B), añadir el checksum según `ChecksumMode::{PreSwizzle, PostSwizzle}` (configurable hasta aclararlo), interpretar ACK/NAK.
- `link.rs`: trait `PacketLink { send(&[u8]); recv(timeout) }`, con `UdpLink` (socket enlazado a 40200, destino IP:50200) y `MemoryLink` para pruebas; mismo diseño que `Transport`/`MemoryTransport` de `tlaku-grbl`.
- `session.rs`: `probe` (sondeo de magic con `DA 00 05 7E`), `upload` (con reintentos y abortar), `status` (lectura 0x200 y posición), `control` (UDP 50207: pausa, parar, jog, origen, encuadre).
- `safety.rs`: validación previa y límites del perfil; sin escritura de memoria.
- `emu.rs` (solo pruebas, detrás de `#[cfg(test)]` o una feature): controlador simulado propio que valida checksum, responde ACK, interpreta el flujo y reconstruye un `Toolpath` para compararlo con el original.

Integración: un nuevo tipo de driver/perfil en `tlaku-doc`/`tlaku-pipeline`, junto al G-code, y un subcomando `tlaku ruida ...` en `tlaku-cli`. La app Tauri lo consumiría igual que GRBL, vía el hilo dedicado de conexión.

### 10.4 Qué se puede verificar con certeza sin hardware

- Codificación de números (V3 a V6), coincide con tres fuentes.
- Swizzle y su inversa (biyección probada); cruce V1/V2 con los bytes de [FAB].
- Decodificación de los dos ejemplos de [EDU] (ida y vuelta de comandos individuales: bytes conocidos -> `Command` -> bytes).
- Particionado en paquetes, checksum aritmético (con ambos modos), orden de comandos del archivo siguiendo el ejemplo, límites de seguridad.
- Cálculo de cajas envolventes, conversión de unidades, relativos sin deriva.

### 10.5 Qué es riesgoso

| Riesgo | Por qué |
|---|---|
| Checksum pre o post swizzle | Fuentes en conflicto; cualquier error hace que no se acepte ni el primer paquete |
| Bytes de ACK/NAK | Resuelto en parte (0xC6/0x46), pero falta confirmación |
| Estructura de cabecera y elementos/arrays | Campos sin significado documentado; la máquina puede rechazar o ejecutar mal un archivo "casi igual" |
| Suma de verificación del archivo (`E5 05`) | Conjunto exacto de bytes desconocido (diferencia 220 en `ruida-pa`) |
| Semántica de potencia mín/máx y velocidad de inicio | Solo por foros; afecta a la calidad del corte |
| Potencia por movimiento (ráster, overscan) | `C7`/`C8` sin descripción |
| Sistema de coordenadas (origen, signo de Y, punto de referencia) | No documentado salvo "relativo al origen del trabajo" |
| Estado y posición (0x200, 0x221...) | Bits y formato en conflicto entre fuentes |
| Control en vivo (panel 50207) | Solo teclas; la pausa alterna estado y no hay confirmación fiable |
| Modelos 634XG, 8445S y otros | Variaciones parcialmente documentadas |
| USB | Sin framing documentado |
| Seguridad física | Sin parada de emergencia por red; ejecución parcial posible si se interrumpe la subida |

### 10.6 Prueba mínima con una máquina real antes de publicarlo

Con láser deshabilitado (tubo apagado, interruptor del láser abierto, o potencia 0) hasta la etapa 4:
1. Solo lectura: enviar `DA 00 05 7E` con magic 0x88 y probar ambos modos de checksum; ver cuál recibe ACK y si la respuesta es `0x65006500`. Leer también 0x200, la posición y el tamaño de cama.
2. Mover el cabezal 10 mm con `88` (sin láser) y devolverlo; confirmar signo de ejes y origen.
3. Enviar un `.rd` mínimo (un cuadrado de 20 mm, una capa, sin láser) y confirmar que la máquina lo acepta, traza bien, y reporta fin; probar parar y pausar desde el panel remoto; cortar la subida a medias para ver si empieza.
4. Con láser real a baja potencia sobre material de desecho: cuadrado 20 mm, comprobar tamaño (+-0,1 mm), dos capas con velocidades distintas, aire encendido/apagado, repetición.
5. Contribuir una captura de LightBurn del mismo trabajo para diferenciar bytes (hace falta una persona con la máquina y LightBurn).
Dispositivo recomendado: una RDC6442G/6445G (magic 0x88, la más documentada). Hay que pedir probadores a la comunidad.

## 11. Veredicto y plan

**Viabilidad**: viable como función **experimental**, con confianza **media** (~65 %) de lograr un trabajo de vectores correcto sin leer código; **baja** (~30 %) para ráster y para lectura de estado sin ayuda de un probador con hardware. Lo que sostiene la viabilidad es que el wiki [EDU] describe casi todo el flujo, incluido un ejemplo completo decodificado. Lo que la limita: el conflicto del checksum, los campos de cabecera sin significado, y que no hay capturas ni emulador públicos.

Plan por etapas:
1. **Núcleo verificable (sin hardware)**: `num`, `swizzle`, `cmd` con decodificador, pruebas V1-V9 y el ejemplo de [EDU] decodificado. Entregable: biblioteca con pruebas.
2. **Generación de archivo**: `rd.rs` desde `Toolpath` (vectores, una capa por bloque, potencia constante), validado con ida y vuelta y con el controlador simulado propio; CLI `tlaku ruida export-rd`.
3. **Transporte**: `packet.rs`, `link.rs`, `session.rs` solo con `MemoryLink`/`emu` (reintentos, timeout, NAK), más el modo sondeo de solo lectura.
4. **Prueba de hardware 1 a 3** (sección 10.6) con un probador; resolver checksum, ACK, cabecera; congelar un "perfil probado" por modelo.
5. **Hardware 4 y 5**: corte real, ajustes de potencia/velocidad, compara con captura de LightBurn; añadir control en vivo (UDP 50207) y estado.
6. **Después**: ráster (potencia por movimiento), Z/rotativo, USB, variante TCP 8445S, más modelos.
No publicar como estable antes de la etapa 5; hasta entonces, marcar la función como experimental en la app y exigir confirmación.
