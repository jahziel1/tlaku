# Controladoras Ruida: protocolo y viabilidad para Tlaku

Estado (2026-10-13): `crates/tlaku-ruida` implementa números, swizzle, comandos, archivo `.rd` (vectores y ráster), paquetes UDP, enlace (`UdpLink`/`MemoryLink`), sesión (sondeo, envío, estado, pausa/reanudar/detener, saltos, encuadre) y un emulador en memoria y por UDP; el CLI tiene `tlaku ruida export-rd | send | status`. **Nada se ha probado con una máquina real.**

Método. Las secciones 1 a 11 son la investigación previa (2026-10-13, sala limpia: solo prosa y tablas, sin leer código). Las dudas que quedaron marcadas se resolvieron después **leyendo el código de implementaciones con licencia MIT** (MeerK40t y ruida-pa) y contrastando con archivos y capturas de proyectos GPL usados solo como referencia; el resultado, con fuente y licencia de cada punto, está en la **sección 12**. Donde una sección anterior contradiga a la 12, manda la 12.

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

Fuentes añadidas en la segunda pasada (código leído; licencia comprobada leyendo el `LICENSE` de cada repositorio):

| Clave | Fuente | Licencia | Uso |
|---|---|---|---|
| [MK40] | https://github.com/meerk40t/meerk40t, carpeta `meerk40t/ruida/` (`udp_connection.py`, `ruidasession.py`, `controller.py`, `emulator.py`, `rdjob.py`, `driver.py`) y `test/test_ruida.py` | **MIT** (`Copyright (c) 2021 meerk40t`). La primera pasada la daba por GPL: el `LICENSE` del repositorio dice MIT | Leído; el comportamiento se reescribió, sin copia literal de código |
| [RPAC] | https://github.com/StevenIsaacs/ruida-pa: `protocols/ruida/ruida_protocol.py`, `ruida_parser.py`, `ruidadriver/rd_transport.py`, `rd_status.py`, `rd_gluescript.py`, `rpascript/encoding.py`, `README.md`, `docs/designs/EffectiveMinPower.md` | **MIT** (`Copyright (c) 2025 Steve Isaacs`) | Leído; tablas de comandos y de memoria re-expresadas |
| [JNWC] | https://github.com/jnweiger/ruida-laser: `example-files/` (`Nova35_rect.pcapng`, `square_tri_test_rdworks.rd`, `Hart.rd`, `dolly_page.rd`, `laser_percentages/codes.txt`), `doc/novalokal.pcapng`, `src/` | **GPL-2.0** | **Solo referencia.** Se leyeron y decodificaron con un script propio fuera del repositorio para contrastar hechos (checksum de capturas reales, orden de un `.rd` real de RDWorks, tabla porcentaje-valor). No se copió código ni archivos al repositorio |
| [LYON] | https://crates.io/crates/ruida 0.1.1 (el repositorio `lyonlabs/ruida` no se pudo clonar sin credenciales; se leyó el paquete publicado) | MIT | Es una plantilla PyO3 (`greet`, `ExampleClass`): **no contiene protocolo Ruida** |

Seguimos sin leer: LightBurn y RDWorks (cerrados; ver CONTRIBUTING.md, "Sala limpia"), y el artículo "Network Aware Laser Cutter Security" citado en [EDU] (la URL devuelve 404).

Nota de licencia: los hechos no tienen copyright, pero [EDU] es una wiki con su propia licencia de texto. Aquí no se copian tablas completas ni frases; los vectores de ejemplo son datos. Lo copiado o adaptado de [MK40] y [RPAC] (vectores de prueba y constantes) consta en THIRD_PARTY.md; de [JNWC] no se copió nada.

### Discrepancias entre fuentes: resueltas

La primera pasada dejó cinco discrepancias abiertas. Todas tienen ya una respuesta apoyada en capturas reales y dos implementaciones MIT; el detalle y las demás dudas (12 en total) están en la sección 12.

| Tema | Resultado |
|---|---|
| Checksum: ¿antes o después del swizzle? | **Después** (suma de los bytes del cable). Capturas reales de LightBurn y de Nova 35 + [MK40] + [RPAC] |
| Byte de ACK/NAK | El byte lógico es `CC` ACK, `CF` checksum erróneo, `CD` error, `CE` keep-alive; **viaja swizzleado con el magic de la máquina** (`C6` para 0x88). EduTech daba el lógico y FabLab el del cable: ambos tenían razón |
| Tamaño del datagrama | 1000 B de datos en límite de comando (lo que envían [MK40] y [RPAC]); la máquina acepta hasta 1472 B de datagrama |
| Timeout de ACK | 0,25 a 1 s en las implementaciones actuales; 4 s era de versiones antiguas. Por defecto 1 s |
| Bit "en movimiento" del estado | `0x01000000` en [MK40] y [RPAC] (RDC6442S); `0x1` trabajo en curso; `0x2` pausa/fin de parte |

## 1. Transporte

### 1.1 UDP 50200: envío del trabajo (el que usa LightBurn)

- La controladora escucha en el puerto 50200 y responde **desde** el 40200. Hay que enlazar el socket local a 40200 para recibir las respuestas. [EDU] https://edutechwiki.unige.ch/en/Ruida ; confirmado por hilos del foro de LightBurn (según el resumen del buscador: salida al láser 50200, entrada desde 40200; p. ej. https://forum.lightburnsoftware.com/t/lightburn-firewall-settings/12777) y por [FOR4] para el 50200.
- El contenido es el mismo flujo de comandos que un archivo `.rd` (igual que USB), sin cabecera de sesión ni handshake. Lo que se analiza bien, se ejecuta. [EDU], [FAB], [JNW].
- **Un paquete = un datagrama**: 2 bytes de checksum (big endian) seguidos de datos. La longitud de los datos sale del tamaño del datagrama, por eso el esquema no sirve sobre TCP. [FAB], [JNW], [EDU].
- Checksum: los 16 bits bajos de la suma de los valores de todos los bytes del paquete (datos, sin contar los 2 del propio checksum), big endian, sobre los bytes **ya swizzleados**. **Resuelto** (sección 12.1): [EDU] decía "antes" y estaba equivocado.
- Tamaño: máximo 1472 B incluido el checksum según [FAB]/[JNW] (1474 según [EDU]). [EDU] dice que las versiones modernas de LightBurn cortan en ~1000 B **en límite de comando**; [FAB]/[JNW] dicen que las versiones antiguas cortaban por bytes, incluso a mitad de comando. Recomendación nuestra: paquetes de datos de hasta ~1000 B cortados en límite de comando.
- ACK por paquete: la controladora responde un byte por cada paquete. **Resuelto** (sección 12.2): el byte lógico es `CC` (ACK), `CF` (checksum erróneo: reenviar), `CD` (error del mensaje) o `CE` (keep-alive), y va swizzleado con el magic de la máquina; con 0x88 son `C6`, `48`, `46` y `C8`. [EDU] daba los lógicos y [FAB]/[JNW] los del cable. Ante `46` [FAB] reintenta solo el primer paquete y aborta en los demás.
- Reintentos y timeout: UDP puede perder paquetes, así que el ACK es obligatorio tras cada paquete. [EDU] y [FAB] hablan de ~4 s; las implementaciones actuales esperan 0,25 a 1 s por intento y hasta 15 a 40 s en operaciones que bloquean la comunicación (home) (sección 12.4). Tlaku: 1 s por paquete, **sin reenviar por timeout** (sin números de secuencia un ACK perdido duplicaría comandos); sí se reenvía tras un NAK (el paquete no se aplicó) hasta 2 veces. [EDU], [FAB]. Un hilo del foro de LightBurn dice que la pila no tiene reintentos y solo la forma más básica de detección de errores [FOR2] https://forum.lightburnsoftware.com/t/rdc6445g-ec-network-trouble-and-looking-for-more-resources/15399 . Por Wi-Fi funciona en modo "ping-pong" envío/ACK limitado a ~600 kb/s [FOR2].
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

Cómo se detecta (implementado en `RuidaSession::probe`, sección 12.13): se envía la lectura `DA 00 05 7E` (sin efectos; es el ping de [RPAC]) con el magic configurado. Como el checksum se suma sobre los bytes del cable, la máquina acepta el paquete aunque el magic sea otro, y **su byte de ACK va swizzleado con su propio magic**: `magic = (ack - 1) ^ swap(0xCC)` (`C6` para 0x88). Un intercambio revela el magic; se confirma repitiendo la lectura con él y comprobando que la respuesta `DA 01 05 7E ...` se decodifica. La respuesta de tarjeta repite el ID de 16 bits en las dos mitades (RDC6442S `0x65106510`, RDC8445S `0x90109010`: [RPAC]); [EDU] decía `0x65006500`. Si la máquina no contesta se prueban los demás magics conocidos. Se comprobó por cálculo que enviar esa consulta con un magic equivocado nunca forma un comando completo de movimiento, corte o proceso en la máquina, para los 16 magics conocidos contra los 255 posibles (prueba `probing_with_the_wrong_magic_never_forms_a_dangerous_command`).

## 3. Codificación de números

Fuentes: [EDU], [SCH] https://stefan.schuermans.info/rdcam/messages.html , [FAB].

- Estructura del flujo: un byte con el bit 7 en 1 (0x80 a 0xFF) **inicia un comando**; los bytes de datos solo usan 7 bits (0x00 a 0x7F). [EDU], [SCH]. Por eso los números se parten en grupos de 7 bits, el más significativo primero. [FAB]: "most significant first".
- Tipos [EDU]/[SCH]:

| Tipo | Bytes | Unidad |
|---|---|---|
| int7 | 1 | índice de capa/parte (0 a 127) |
| Abscoord | 5 (35 bits) | µm, con signo (complemento a dos de 35 bits) |
| Relcoord | 2 (14 bits) | µm, con signo (complemento a dos de 14 bits) |
| Potencia | 2 (14 bits sin signo) | `% * 16383 / 100` truncado; el 100 % es `0x3FFF` (sección 12.14; EduTech decía 100/2^14) |
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
| `C7 P` / `C8 P` | Potencia inmediata 1 / potencia final 1 (3 B); equivalentes `C0, C2, C3` y `C1, C4, C5` para láseres 2 a 4 [EDU]. **`C7` fija la potencia de los cortes siguientes (potencia por píxel de una imagen)**: [RPAC] la marca "verificado en RDC6442S" y la usa en su relleno ráster; `C8` no tiene semántica verificada y Tlaku no la genera (sección 12.8) |
| `C6 10`, `C6 11`, `C6 12`, `C6 13` | Intervalo de láser, retardo añadido, retardo de encendido, retardo de apagado (tiempo, 7 B) [EDU] |
| `C6 60 láser capa frecuencia` | Frecuencia del láser por capa (9 B) [EDU] |
| `C9 02 V(5)` | Velocidad de corte/movimiento (7 B) [EDU], [SCH]; [FAB] 80 % |
| `C9 04 capa V(5)` | Velocidad por capa (8 B) [EDU], [SCH] |
| `C9 03`, `C9 05`, `C9 06` | Velocidad de eje, forzada, de movimiento de eje [EDU] (semántica sin detallar) |

Comportamiento observado (foro): a velocidades por debajo de un umbral ("Start Speed", unos 10 mm/s) la controladora usa solo la potencia mínima, y por encima sube hacia la máxima. https://forum.lightburnsoftware.com/t/why-ruida-doesnt-light-the-laser-below-10mm-s-80w/120353/2 (visto en el resumen del buscador; no se abrió la página completa, confianza media). Esto es el equivalente funcional de nuestro `PowerMode::Dynamic`; `PowerMode::Constant` equivale a mín = máx. Es una interpretación nuestra a validar con hardware. [RPAC] (`docs/designs/EffectiveMinPower.md`) modela lo mismo: a menos velocidad el mínimo efectivo sube hacia el máximo (`efectivo = máx - (v / v_máx) * (máx - mín)`), y recomienda no bajar de ~8 % en CO2; Tlaku sigue tratando `PowerMode::Dynamic` como potencia constante.

### 4.3 Capas y configuración

| Bytes | Significado |
|---|---|
| `CA 01 00` | Fin de capa [EDU] |
| `CA 01 01` a `05`, `55` | **Modos de overscan** del ráster: 01 horizontal bidireccional, 02 horizontal unidireccional, 03 vertical bidireccional, 04 vertical unidireccional, 05 diagonal ([RPAC], verificado en RDC6442S). Tlaku no los usa: el overscan va dentro del trabajo |
| `CA 01 10`/`11` | Dispositivo de la capa 0/1 [EDU] |
| `CA 01 12`/`13` | **Aire apagado / encendido** (por capa) [EDU]; [SCH] describe `CA 01` como flags: 0x00 todo apagado, 0x01 soplador de aire |
| `CA 02 parte índice` | Número de capa actual [EDU]; [SCH]: `CA 02 capa` |
| `CA 05 color(5)` / `CA 06 capa color(5)` | Color de capa / color de parte [EDU]; en [SCH]: `CA 06 capa color` |
| `CA 22 capa` | Número máximo de capa [EDU], [SCH] |
| `CA 41 parte atributos` | `LAYER_ATTRIBUTES` en [RPAC]; 0 en todos los archivos vistos (sección 12.6) |
| `CA 03 0D`, `CA 10 00`, `CA 01 30`/`31` | Inicializaciones del ejemplo (EnLaserTube, EnExIO, EnLaser2Offset) [EDU] |
| `E7 03 X Y` / `E7 07 X Y` | Esquina superior-izquierda / inferior-derecha del proceso (caja del trabajo) [EDU], [SCH] |
| `E7 50`/`E7 51` | Caja del documento (min / max) [EDU]; [FAB] 30 % (dudas) |
| `E7 52`/`53`/`61`/`62 parte X Y` | Caja de cada capa (y versión "Ex") [EDU] |
| `E7 54 n Abscoord` / `E7 55 n Abscoord` | Offset de pluma / de capa [EDU] |
| `F1 03 X Y` | Offset de pantalla [EDU] |
| `F1 00`, `F1 01`, `F2 00`, `F2 01`, `F2 02`, `F2 03`/`04`, `F2 05`/`06`/`07`, `E7 13`/`17`/`23`/`24`/`08`/`06`/`04`/`05`, `EA`, `EB`, `E7 60` | Índices de elemento, nombre de elemento (`F2 02` = 8 caracteres ASCII empaquetados; RDWorks escribe siempre `UNNAMED `), matrices de copias (`E7 04`, `E7 08`, `F2 05` = columnas, filas, paso X, paso Y: los "7 valores" de EduTech son 1x1 con pasos de -90,813 y 73,133 mm), `EA`/`EB` = inicio/fin de matriz. Resuelto en la sección 12.6; [MK40] omite el bloque `F1`/`F2` y funciona |

### 4.4 Archivo y control del proceso

| Bytes | Significado |
|---|---|
| `D8 10` / `D8 11` / `D8 12` | Coordenadas respecto al cero de la máquina / al origen del usuario / a la posición actual ([RPAC]: `REF_POINT_MACHINE/ORIGIN/CURRENT`, verificado en RDC6442S). El ejemplo empieza con `D8 10` + `F0`; los `.rd` de RDWorks vistos usan `D8 12` |
| `F0` | Fijar punto de referencia [EDU] |
| `E6 01` | Absoluto [EDU] |
| `F1 02 n` | Habilitar corte por bloques [EDU] |
| `D8 00` | **Iniciar proceso**, siempre al principio del archivo (sección 12.11): [EDU], la captura de Nova 35, los `.rd` de RDWorks, [MK40] y [RPAC]. [FAB]/[JNW] solo omiten la fila en su tabla |
| `D8 01` / `D8 02` / `D8 03` | Detener / pausar / reanudar proceso [EDU] |
| `D8 2A` | **Home XY** ([MK40] y [RPAC], verificado en RDC6442S) |
| `D8 2C` / `D8 2D` / `D8 2E` | Home Z / home U / enfocar Z [EDU] |
| `E7 00` | Fin de bloque [EDU] |
| `D7` | **Fin de archivo** [EDU], [FAB]; el último paquete ya no necesita pausa |
| `E7 01 nombre\0` | Nombre de archivo; `E8 02 E7 01 nombre\0` fija el nombre del archivo del siguiente envío (debe seguir pronto) [FAB] 90 %, [EDU] |
| `E8 00`, `E8 01 n`, `E8 03 n`, `E8 04` | Borrar documento, nombre del documento n, seleccionar documento, calcular tiempo [EDU] |
| `E5 05 suma(5)` | **Fin de trabajo con suma de verificación** del archivo (capturas de LightBurn; `END_JOB` en [RPAC], que lo marca verificado en RDC6442S; `set_file_sum` en [MK40]). Ejemplo real de LightBurn: `E5 05 00 04 53 78 79` = 9 763 961. Ver sección 12.7 |
| `CC` / `CD` | ACK / error de la máquina a nivel de mensaje [EDU], [FAB], [JNW] |

**Suma de verificación del archivo**: ver la sección 12.7. Es la suma de los bytes de todos los comandos del trabajo (excluye las lecturas `DA 00` y las teclas `A5`/`A7`, y el propio `E5 05`) más `0xD7`; con capturas de LightBurn difiere de forma constante en 220 sin explicación. Los `.rd` de RDWorks no la llevan.

Lectura de estado y posición: `DA 00 <dirección 2 B>` consulta y `DA 01 <dirección 2 B> <valor 5 B>` es la respuesta, que llega en un **segundo datagrama tras el ACK**, swizzleada y sin checksum ([MK40], [RPAC]); `DA 01` con dos valores de 5 B es también la forma de escribir memoria (el pie de los `.rd` de RDWorks: `DA 01 06 20 L L`). [EDU], [FAB] 99 %. Direcciones conocidas (bytes crudos entre paréntesis):

| Dirección | Contenido | Fuente |
|---|---|---|
| 0x2FE (`05 7E`) | ID de tarjeta; la RDC6442S devuelve `0x65106510` y la RDC8445S `0x90109010` (el ID de 16 bits repetido); [EDU] decía `0x65006500` | [RPAC], [MK40] |
| 0x2FF (`05 7F`) | Versión de la placa (cadena) | [EDU], guía rpa |
| 0x060 (`00 60`) | Tipo de máquina (0x1155 / 0xAA55) | [EDU] |
| 0x200 (`04 00`) | Estado de la máquina: `0x01000000` en movimiento, `0x1` trabajo en curso, `0x2` pausa (ruida-pa) o fin de parte (MeerK40t); verificado en RDC6442S (sección 12.5) | [RPAC], [MK40] |
| 0x221/0x231/0x241/0x251 (`04 21`, `04 31`, `04 41`, `04 51`) | Posición actual de los ejes X, Y, Z, U en µm, 35 bits con signo, **medida desde el home** (sección 12.10) | [EDU], [RPAC], [MK40] |
| 0x026 / 0x036 (`00 26`, `00 36`) | Tamaño de la cama X / Y | guía rpa |
| 0x205 (`04 05`) | Número total de documentos ("saved job count") | [EDU], [FAB] |
| 0x201/0x202/0x203/0x211 | Tiempo total encendida, tiempo de trabajo, número de trabajos, tiempo de láser | [EDU] |
| 0x390 (`07 10` a `07 74`) | Tiempo del documento | [EDU] |
| 0x591 (`0B 11`) | Bloqueo de la tarjeta (card lock) | [EDU] |
| 0x004, 0x005, 0x00C, 0x010, 0x011-0x013, 0x020/0x030/0x040/0x050, 0x100, 0x102, 0x10A | Habilitación de IO, velocidad G0, velocidad de home, modo de control del sistema (valor de retorno 0x0000 origen / 0x4000 sin retorno / 0x8000 origen absoluto), PWM/mín/máx del láser 1, parámetros de eje, ajustes del sistema, velocidad de giro, aceleración G0 | [EDU] |

La posición es Abscoord en µm ([RPAC] la decodifica como `XFARDIM`/`YFARDIM`; [MK40] como `decode35`). Sigue sin documentarse cómo se consulta la lista de archivos más allá de `E8 01`.

### 4.5 Pausa, reanudar, parar, jog, home, encuadre, aire

- Vía flujo `.rd`/UDP 50200: pausa `D8 02`, reanudar `D8 03`, parar `D8 01`, iniciar `D8 00`, home XY `D8 2A` (arriba; los cuatro primeros "verificados en RDC6442S" por [RPAC] y los usan [MK40] y [RPAC] también con un trabajo en marcha). Según [FOR3] el controlador parece ignorar el puerto Ethernet de datos mientras corre un trabajo; si pasara, el control pasa por el panel remoto (UDP 50207), que Tlaku no implementa.
- Vía panel remoto (UDP 50207, sección 1.2): `06` inicio/pausa (alterna), `09` parar, `5A` reiniciar, jog con pulsar/soltar, `08` origen, encuadre `A5 53 00`, `0F` trazar.
- Home: `D9 10 <opciones>` (XY), `D8 2C/2D` (Z/U) [EDU]. Qué valor tiene `<opciones>` no se documenta.
- Jog por flujo: `D9 00..03 <opciones> <Abscoord>` (X, Y, Z, U) y `D9 10 <opciones> <X> <Y>` (XY), verificados en RDC6442S ([RPAC], [MK40]). Opciones: `00` coordenada absoluta desde el cero de la máquina, `02` desplazamiento desde la posición actual; el bit 0 (`01`, `03`) enciende el "haz de luz" del láser y **Tlaku nunca lo genera**. Se envía antes `C9 02 velocidad`. No hay un comando de encuadre documentado (`A5 53 00` del panel remoto es `INTERFACE_FRAME`, sin significado conocido): el encuadre se hace con cuatro saltos.
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
- El orden global (referencia -> cajas -> velocidad y potencias de cada capa -> bloque de elementos y arrays -> `CA 01/02` y configuración de capa -> movimientos -> `EB`, `E7 00` -> `D7`) se desprende del ejemplo y coincide con tres `.rd` reales de RDWorks y con la captura de Nova 35 (sección 12.6). **No hay una especificación formal**, pero los nombres de casi todos los campos están ya en [RPAC] (sección 12.6); `F1 04`, `E7 36` y `E7 46` (valor fijo 0x11227766) siguen sin significado, y Tlaku no los genera. Lo seguro sigue siendo reproducir la estructura del ejemplo sustituyendo solo valores conocidos.

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
- Archivo incompleto: `D8 00` (inicio de proceso) va **al principio** del archivo (sección 12.11), así que si la transmisión se corta después, la máquina puede haber empezado a ejecutar. Ninguna implementación lo trata de forma especial; el único "abortar" que documentan es `D8 01` (Detener, verificado en RDC6442S; [MK40] lo usa como `abort`). Lo que hace Tlaku (sección 12.12): modo por defecto "guardar como documento" (`E8 02` + `E7 01 nombre`, como la captura de Nova 35), que no arranca nada; en modo "ejecutar", ante cualquier fallo o Ctrl+C se envía `D8 01` hasta 3 veces y, si la máquina no lo confirma (por ejemplo porque ignora la red con un trabajo en marcha, [FOR3]), se avisa de que puede seguir ejecutando y se manda usar Stop del panel o el paro de emergencia.
- No se describen límites de potencia ni de velocidad en el protocolo: los valores van tal cual (potencia 14 bits, velocidad 35 bits, coordenadas ±17 km en µm); los límites reales los pone el firmware (ancho de cama en 0x26/0x36, velocidades en memoria) y el panel de la máquina. No hay fuente que diga cómo reacciona el firmware a un valor fuera de rango.

Reglas que adoptaremos en `tlaku-ruida` (propias):
1. Nunca enviar `DA 01` (escrituras) ni los comandos de pulso/puerta del láser sin acción explícita.
2. Antes de enviar, comprobar: potencia 0 a 100 % (14 bits), velocidad > 0 y <= tope del perfil, coordenadas dentro de la cama del perfil y, si se pudo leer, dentro de 0x26/0x36.
3. Nada de NaN ni infinitos; todo redondeo a µm con saturación y error (no truncar en silencio).
4. Enviar con la máquina en reposo (estado 0x200) y una sola sesión a la vez; abortar ante NAK repetido y enviar `D8 01` y comunicar al usuario que quizá la máquina ya empezó (hay `D8 00` al inicio).
5. Prueba de trabajo "en seco" siempre disponible (potencia 0 o láser deshabilitado).
6. Subir en una etapa inicial solo trabajos pequeños; documentar que no hay parada de emergencia por red.

## 8. Cómo validar sin hardware

- **Emulador/simulador documentado**: no se encontró ninguno con documentación. MeerK40t incluye un emulador Ruida (GPL; no leído). Un hilo del foro confirma que no hay emulador del panel [FOR5] https://forum.lightburnsoftware.com/t/ruida-emulator-software/137853 . Un desarrollador de MeerK40t dice en los foros que no tiene una controladora y que probar la interfaz sin hardware es el principal obstáculo. https://forum.makerforums.info/t/meerk40t-ruida-support/79891
- **Capturas pcap publicadas**: sí (segunda pasada). [JNWC] (GPL-2.0, solo referencia) trae `Nova35_rect.pcapng` y `novalokal.pcapng` de RDWorks contra una Nova 35: datagramas completos con checksum, byte de ACK `C6` y la secuencia `E8 02`, `E7 01 nombre`, `D8 12 F0`, `F1 02 00`, `D8 00`, ..., `DA 01 06 20`, `EB`, `E7 00`, `D7`, con un paquete de 1472 B cortado a mitad de comando. El README de [RPAC] (MIT) incluye fragmentos de capturas de LightBurn con el checksum `0261` de la consulta `DA 00 05 7E` y un `E5 05` real; esos fragmentos sí se usan como vectores de prueba (THIRD_PARTY.md). Siguen sin publicarse capturas de LightBurn completas con licencia permisiva.
- **Vectores de prueba en documentación**: sí, en [EDU] (Example 1 y 2 con decodificación línea a línea, pero **sin swizzle**) y mediante datos puntuales (`C6 31 00 13 19` = potencia 14,996 %, `C9 04 00 00 00 06 0D 20` = 100 mm/s). [EDU] no trae bytes swizzleados ni paquetes UDP completos; los vectores swizzle de la sección 9 son cálculos propios, ahora contrastados con la captura de LightBurn del README de [RPAC] (`0261 d4890df7`) y con la prueba `test_magic_keys` de [MK40] (ocho magics).

Estrategia de validación sin hardware: construir un decodificador/desensamblador propio (decodificar nuestro flujo y comparar), el ejemplo de [EDU] como prueba de ida y vuelta, y un controlador simulado propio (sección 11).

## 9. Vectores calculados (propios; derivados del algoritmo, no capturados)

Calculados con un script con magic 0x88 y verificados con inversa para los 256 valores y los magics 0x88, 0x11, 0x38.

| Id | Entrada | Resultado | Origen |
|---|---|---|---|
| V1 | swizzle(`CC`) | `C6` | Coincide con el ACK `0xC6` de [FAB]: comprobación cruzada independiente del algoritmo de [EDU] |
| V2 | swizzle(`CD`) | `46` | Coincide con el error `0x46` de [FAB] |
| V2b | swizzle(`CF`) | `48` | Posible NAK de [EDU] visto en cable |
| V3 | velocidad 100 mm/s = 100 000 µm/s | `00 00 06 0D 20` | Coincide con `c904 00 000006 0d20` de [EDU] (capa 00 + 5 B) |
| V4 | potencia 2457 (15 % en la escala 16383; EduTech la leía como 14,996 % con 16384) | `13 19` | Coincide con `c631 00 1319` |
| V5 | 190 610 µm | `00 00 0B 51 12` | Coincide con la caja `e703 00000b5112 ...` de [EDU] |
| V6 | -1 µm (35 bits) | `7F 7F 7F 7F 7F` | Coincide con [SCH] |
| V7 | `DA 00 05 7E` | swizzle `D4 89 0D F7` | Consulta del ID de tarjeta. Checksum sin swizzle `0x015D`, con swizzle `0x0261`. **La captura real de LightBurn es `0261 d4890df7`: el checksum va sobre los bytes del cable** (sección 12.1) |
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
| `power_pct` constante en el bloque | mín = máx = `trunc(p * 16383 / 100)` (el 100 % es 0x3FFF; sección 12.14) |
| `PowerMode::Constant` | mín = máx |
| `PowerMode::Dynamic` | mín/máx distintos sujetos a la velocidad de inicio: **a validar**; hasta entonces tratar como constante |
| `Travel` | `88` (absoluto) o `89`/`8A`/`8B` (relativo) |
| `Cut` | `A8`, `A9`/`AA`/`AB` |
| `Cut` con potencia variable (ráster, 0 para overscan) | Capa con mín 0 y máx = la mayor potencia del bloque; `C7 P` antes de cada corte cuya potencia cambia; los cortes a potencia 0 salen como saltos con el láser apagado (sección 12.8). Medio riesgo: `C7` está verificado en RDC6442S por [RPAC], pero no se sabe si su escala es absoluta |
| `z_mm`, rotativo | Fuera de la primera etapa |

### 10.3 Arquitectura propuesta: `crates/tlaku-ruida` (implementada; ver la sección 13)

Dependencias: `tlaku-core` y `std` (sin `unsafe`, sin crates de protocolo de terceros; ningún código GPL). Estructura:

- `num.rs`: codificar/decodificar int7, Relcoord (14 bits con signo), Abscoord (35 bits con signo), potencia, velocidad, tiempo, color, cadenas. Saturación explícita con error.
- `swizzle.rs`: `Magic(u8)`, `swizzle`, `unswizzle`, tabla ID-de-tarjeta -> magic y sondeo.
- `cmd.rs`: `enum Command` con las variantes necesarias (mover, cortar, potencias, velocidad, capa, color, aire, cajas, `D8 xx`, `DA 00`/`DA 01`, `D7`, `E5 05`), con `encode(&self, &mut Vec<u8>)` y `decode(&[u8])`. El decodificador es la herramienta de prueba (un desensamblador).
- `rd.rs`: `RdWriter` desde `Toolpath` + `RuidaJobOptions` (magic, perfil de máquina, origen, tipo de archivo de cabecera) a `Vec<u8>` sin swizzle, con cálculo de cajas envolventes por capa, y la suma `E5 05` marcada como experimental. También `to_swizzled`.
- `packet.rs`: partir el flujo en paquetes en límite de comando (tope configurable, ~1000 B), añadir el checksum (sobre los bytes del cable; la propuesta original lo dejaba configurable), interpretar ACK/NAK.
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
| Checksum pre o post swizzle | **Resuelto** (post; capturas reales). Riesgo residual bajo |
| Bytes de ACK/NAK | **Resuelto** (lógicos `CC/CF/CD/CE` swizzleados con el magic de la máquina) |
| Estructura de cabecera y elementos/arrays | Casi resuelto: nombres de campo de [RPAC], valores iguales a los `.rd` reales de RDWorks. Riesgo residual: la máquina rechaza algún archivo "casi igual" |
| Suma de verificación del archivo (`E5 05`) | Sin resolver: diferencia constante de 220 con LightBurn. Tlaku no la emite por defecto |
| Semántica de potencia mín/máx y velocidad de inicio | Solo por foros y el modelo de [RPAC] (`EffectiveMinPower.md`); afecta a la calidad del corte |
| Potencia por movimiento (ráster, overscan) | `C7` verificado en RDC6442S ([RPAC]); falta confirmar la escala y la calidad del resultado |
| Sistema de coordenadas (origen, signo de Y, punto de referencia) | Las coordenadas son distancias desde el home hacia dentro de la cama; el home depende de la máquina: perfil `HomeCorner` (sección 12.10), por confirmar con la prueba 2 |
| Estado y posición (0x200, 0x221...) | Resuelto con dos implementaciones: bits `0x1`, `0x2`, `0x01000000`; posición en µm con signo |
| Control en vivo (panel 50207) | No implementado; pausa/reanudar/detener/home/saltos van por 50200 (D8/D9) |
| Modelos 634XG, 8445S y otros | 8445S es TCP (fuera de alcance); 634XG solo cambia el magic según [MK40] (sin probar) |
| USB | Sin framing documentado |
| Seguridad física | Sin parada de emergencia por red; ejecución parcial posible si se interrumpe la subida: modo "guardar" por defecto y `D8 01` automático en el modo "ejecutar" (sección 12.12) |

### 10.6 Prueba mínima con una máquina real antes de publicarlo

Con láser deshabilitado (tubo apagado, interruptor del láser abierto, o potencia 0) hasta el paso 5. Cada paso dice qué comando de Tlaku usar y qué se espera. Detalle en la sección 14.

1. Solo lectura: `tlaku ruida status <ip>` (con `--magic` si se conoce). Se espera modelo, ID de tarjeta (`0x65106510` en una RDC6442S), cama y estado en reposo. Anotar el magic detectado y si el ACK llegó como `C6`.
2. Ejes y origen: `tlaku ruida status <ip>` con la cabeza en un punto conocido; comparar con lo que muestra el panel y fijar `--home`. Después mover 10 mm (saltos con láser apagado) y devolverlo; confirmar signo de X e Y.
3. Documento: `tlaku ruida send <ip> cuadrado.svg --dry-run` y luego sin `--dry-run` (modo guardar): la máquina debe listar el documento y, al iniciarlo desde el panel, trazar el cuadrado sin láser. Cortar la subida a medias (desenchufar el cable) para ver que no arranca nada.
4. Ejecutar: `--run` con el láser deshabilitado; probar `D8 02`/`D8 03`/`D8 01` desde la sesión y el comportamiento al cortar la subida a medias (¿empieza?, ¿responde a `D8 01`?).
5. Con láser real a baja potencia sobre material de desecho: cuadrado 20 mm, comprobar tamaño (+-0,1 mm), dos capas con velocidades distintas, aire encendido/apagado; después un ráster pequeño para ver la escala de `C7`.
6. Contribuir una captura de LightBurn del mismo trabajo para comparar byte a byte, sobre todo `E5 05` (hace falta una persona con la máquina y LightBurn).
Dispositivo recomendado: una RDC6442S/G o 6445G (magic 0x88; la 6442S es la que probaron [MK40] y [RPAC]). Hay que pedir probadores a la comunidad.

## 11. Veredicto y plan

**Viabilidad**: viable como función **experimental**. Tras la segunda pasada (sección 12), la confianza actual es: **alta** (~85 %) en el transporte (checksum, ACK, magic: capturas reales y dos implementaciones MIT probadas en una RDC6442S); **media-alta** (~70 %) en un trabajo de vectores correcto, porque la cabecera coincide con la de archivos reales de RDWorks pero Tlaku no ha movido una máquina; **media** (~55 %) en el ráster (`C7` verificado, escala sin confirmar); **media** (~70 %) en estado, pausa, reanudar, detener y saltos (dos implementaciones los usan). Lo que sigue limitando: `E5 05` (diferencia de 220 con LightBurn), el home/origen de cada máquina y que ninguna prueba ha usado hardware.

Plan por etapas:
1. **Núcleo verificable (sin hardware)**: `num`, `swizzle`, `cmd` con decodificador, pruebas V1-V9 y el ejemplo de [EDU] decodificado. **Hecho.**
2. **Generación de archivo**: `rd.rs` desde `Toolpath` (vectores y ráster), validado con ida y vuelta y con el emulador propio; CLI `tlaku ruida export-rd`. **Hecho.**
3. **Transporte**: `packet.rs`, `link.rs` (`UdpLink`, `MemoryLink`), `session.rs` (sondeo, envío con progreso, estado, pausa/reanudar/detener, saltos, encuadre), emulador en memoria y por UDP, CLI `send` y `status`. **Hecho.**
4. **Prueba de hardware 1 a 3** (sección 10.6) con un probador; congelar un "perfil probado" por modelo.
5. **Hardware 4 y 5**: corte real, ajustes de potencia/velocidad, comparar con captura de LightBurn; decidir `E5 05`.
6. **Integración en la app** (sección 14).
7. **Después**: Z/rotativo, USB, variante TCP 8445S, panel remoto (50207), más modelos.
No publicar como estable antes de la etapa 5; hasta entonces, marcar la función como experimental en la app y exigir confirmación.

## 12. Resolución de las dudas (segunda pasada, 2026-10-13)

Cada punto indica qué dice cada fuente, qué adopta Tlaku y por qué. Rutas relativas al repositorio indicado; licencias comprobadas en su `LICENSE`.

### 12.1 Checksum: ¿antes o después del swizzle?

- [EDU]: antes del swizzle. [FAB]/[JNW]: suma de los bytes swizzleados.
- [RPAC] `ruidadriver/rd_transport.py` (`_package`): `payload = swizzle(data); chk = sum(payload) & 0xFFFF` y se antepone big endian. [MK40] `meerk40t/ruida/udp_connection.py` (`_package`) y `emulator.py` (`checksum_write`: suma de `sent_data[2:1472]`, o sea los bytes del cable). Ambas MIT.
- Prueba directa: el README de [RPAC] reproduce una captura de LightBurn, datagrama `0261 d4890df7` para `DA 00 05 7E`; `0xD4+0x89+0x0D+0xF7 = 0x0261` (con los datos en claro sería `0x015D`). Las capturas de Nova 35 de [JNWC] (GPL, referencia) dan el mismo resultado en los 40 paquetes de las dos capturas (todos con ACK `c6`), incluido uno de 1472 B.
- **Adoptado: después del swizzle.** Se eliminó `ChecksumMode`. Confianza alta.

### 12.2 Bytes de ACK y NAK

- [EDU]: `CC` ok, `CF` checksum mal. [FAB]/[JNW]: `C6` ok, `46` error.
- [RPAC] `protocols/ruida/ruida_protocol.py`: `ACK=0xCC`, `ERR=0xCD`, `ENQ=0xCE`, `NAK=0xCF`; `rd_transport.py` des-swizzlea el byte recibido y lo compara con `ACK`. [MK40] `rdjob.py`: mismas constantes, `udp_connection.py` reenvía el paquete ante `NAK`. Las capturas de [JNWC] muestran `c6` en el cable.
- **Adoptado**: los cuatro son bytes lógicos que viajan swizzleados con el magic de la máquina. Con 0x88: `C6` ACK, `48` NAK, `46` ERR, `C8` ENQ (así `C6`/`46` de FabLab y `CC`/`CF` de EduTech son la misma cosa vista en dos capas). NAK: se reenvía el mismo paquete (hasta 2 veces); ERR: se reenvía una sola vez el primer paquete (FabLab) y se aborta en los demás. Confianza alta.

### 12.3 Tamaño del datagrama

- [EDU] 1474 (en la práctica ~1000), [FAB]/[JNW] 1472.
- [MK40] `controller.py` (`divide_data_into_queue`) corta a ~1000 B en límite de comando; [RPAC] usa `chunk_size = 1024`; el emulador de [MK40] acepta datos hasta el byte 1472.
- **Adoptado**: 1000 B de datos por defecto, tope 1470 (datagrama de 1472), siempre en límite de comando. Confianza alta.

### 12.4 Timeout y reintentos

- [EDU]/[FAB]: 4 s. [MK40] `udp_connection.py`: socket de 0,25 s y 4 intentos (1 s) y 40 s "gross" para el home; [RPAC] `rd_transport.py`: 0,5 a 1 s y 15 s "gross". Ninguna reenvía por timeout (sí [MK40] ante `NAK`).
- **Adoptado**: 1 s por ACK, 15 s en operaciones largas, NAK reenvía, timeout **no** reenvía los paquetes del trabajo (sin números de secuencia no se distingue un paquete perdido de un ACK perdido y duplicar comandos corrompería el trabajo); las lecturas sí se reintentan (son idempotentes). Configurable en `SessionConfig`. Confianza media-alta.

### 12.5 Estado de la máquina (bit "en movimiento")

- [EDU]: `IsMove = 0x10`. [RPAC] `ruida_protocol.py`: `MACHINE_STATUS_MOVING 0x01000000`, `PAUSED 0x2`, `JOB_RUNNING 0x1` ("verificado RDC6442S" para `MEM_MACHINE_STATUS`). [MK40] `rdjob.py`: mismos valores, pero llama a `0x2` "Part end".
- **Adoptado**: movimiento `0x01000000`, trabajo en curso `0x1`, `0x2` = pausa o fin de parte (se muestra como "en pausa" con trabajo en marcha). En reposo = ni trabajo ni movimiento. La posición son las lecturas `0x221`/`0x231` en µm. Confianza media-alta; el bit `0x2` es ambiguo.

### 12.6 Campos de la cabecera `.rd`

- Nombres ([RPAC] `ruida_protocol.py`): `E7 03/07` cajas del trabajo, `E7 50/51` cajas del documento, `E7 04` `JOB_COPIES`, `E7 08` `ARRAY_COPIES`, `F2 05` `ELEMENT_COPIES` (columnas, filas, paso X, paso Y), `E7 54/55` offsets de pluma y de capa, `F1 03` `DISPLAY_OFFSET`, `EA`/`EB` inicio/fin de matriz (`EB` verificado en RDC6442S), `CA 41` `LAYER_ATTRIBUTES`, `CA 22` última capa (verificada), `F2 02` `ELEMENT_NAME` ("cadena de 8 caracteres en dos valores de 35 bits").
- Valores: el `F2 02` del ejemplo de [EDU] decodifica a `UNNA` + `MED ` = **`UNNAMED `**, igual en todos los `.rd` reales de RDWorks revisados ([JNWC]); los "siete valores" `1,1,-1,-710,-7808,4,7597` son 1 columna, 1 fila, paso X -90,813 mm y paso Y 73,133 mm (el tamaño del dibujo más 3 µm; en otros archivos son ±el tamaño, con signos distintos: con 1x1 no se usan). Los offsets de pluma, capa y pantalla del ejemplo (50 000 µm) son 0 en los demás archivos y en [MK40].
- [MK40] `rdjob.py` (`write_header`) funciona en una RDC6442S **sin** el bloque `F1`/`F2` y con pasos arbitrarios en `E7 08`.
- Otras diferencias con el ejemplo 1, resueltas por la mayoría: retardos del láser `C6 12/13` (los `.rd` de RDWorks y [MK40]) en vez de `C6 15/16` (solo EduTech; [RPAC] los llama `LASER_ON/OFF_DELAY2`); `CA 03 01` (tubos activos; [MK40], verificado) en vez de `0D`/`0F` (RDWorks según su configuración), ya que los láseres 2 a 4 se escriben a 0.
- **Adoptado**: nombre `UNNAMED `, copias 1x1 con pasos 0, offsets 0, `C6 12/13`, `CA 03 01` (única opción que queda: `laser_tube_start`). Se mantiene el bloque `F1`/`F2` porque es lo que escriben los archivos reales. El orden de las dos primeras instrucciones (`F0` antes o después de `E6 01`) difiere entre EduTech y [MK40]; se conserva el de EduTech/RDWorks. Confianza media-alta; lo no verificable sin hardware es si la máquina acepta los pasos 0.

### 12.7 La suma `E5 05`

- [RPAC] `README.md` y `rpascript/encoding.py`: suma de los bytes de todos los comandos salvo lecturas/escrituras `DA` (las escrituras `DA 01` sí cuentan en `ruida_parser.py`), teclas `A7` y el propio `E5 05`, más el `D7` final. Con capturas de LightBurn el resultado difiere en 220 de forma constante; "qué bytes falta es desconocido". Ejemplo real: `E5 05 00 04 53 78 79` = 9 763 961.
- [MK40] `rdjob.py` (`file_sum`, `write_tail`): suma de todos los bytes de todos los comandos del trabajo + `0xD7`, y lo envía en cada trabajo en una RDC6442S.
- Los `.rd` de RDWorks ([EDU], [JNWC]) no llevan `E5 05`.
- **Adoptado**: fórmula de [MK40] como opción (`FileChecksum::SumAll`), apagada por defecto, porque la máquina acepta sin ella los archivos del programa del fabricante, no sabemos si valida la suma (según [RPAC] "puede mostrar un mensaje") y la fórmula no reproduce a LightBurn. Confianza media. A decidir con la captura del paso 6 de 10.6.

### 12.8 `C7`/`C8` y la potencia variable (ráster)

- [RPAC] `ruida_protocol.py`: `C7` = `IMD_POWER_1` ("verificado RDC6442S"), `C2` = `IMD_POWER_3` ("verificado"), `C8` = `END_POWER_1`; `rd_gluescript.py` (`scan_rows`, `_emit_imd_power`) rellena imágenes escribiendo `IMD_POWER_1`/`3` con la potencia de cada tramo (cambia solo cuando varía), después `CUT_*` para los tramos encendidos y `MOVE_*` para los de potencia 0; `rpa_plotter.py` interpreta `IMD_POWER` como "potencia por píxel de una imagen en grises".
- [MK40] `driver.py` hace otra cosa: reescribe `C6 01/02` (mín y máx) antes de cada cambio de potencia y convierte `on <= 0` en salto; su README exige overscan 0 en las capas ráster.
- **Adoptado**: el método de [RPAC] (`C7 P` + corte; potencia 0 como salto con el láser apagado), con la capa en mín 0 y máx = la mayor potencia del bloque. `ZeroPower::Cut` (`C7 0` + corte, recorrido a la velocidad de la capa) queda como alternativa sin verificar. `C8` no se genera. Quedan sin confirmar la escala (¿porcentaje absoluto?, [RPAC] pasa el porcentaje tal cual) y cómo se comporta el salto rápido de los extremos (la aceleración del cabezal es la de la máquina). Confianza media. El overscan de la capa de la máquina (`CA 01 01..05`) no se usa: el overscan va como tramos dentro del trabajo.

### 12.9 Orden de varias capas

- Los `.rd` reales (3 capas en la captura de Nova 35; dos en el ejemplo 2 de [EDU]; [MK40] `write_header`/`write_settings`) escriben **primero las cabeceras de todas las capas** (`C9 04`, `C6 31..`, `CA 06`, `CA 41`, cajas) y **después el cuerpo de cada capa en orden ascendente** (`CA 01 00`, `CA 02 n`, ..., movimientos). `CA 22` lleva el índice de la última capa.
- **Adoptado**: capas numeradas 0, 1, 2... en el orden de los bloques del `Toolpath`; el cuerpo se ejecuta en ese orden. Confianza alta (la máquina ejecuta el cuerpo en el orden del archivo).

### 12.10 Origen y signo de Y

- [EDU]: "relativas al origen del trabajo". [RPAC] nombra `E7 03` `JOB_TOP_RIGHT` y `E7 07` `JOB_BOTTOM_LEFT` (los valores mínimos y máximos), o sea que su máquina tiene el origen arriba a la derecha; [MK40] necesita "Flip X" en una RDC6442S (README) y convierte la posición con `ancho_cama - x` pero usa Y tal cual con Y hacia abajo (`controller.py`, `update_x`/`update_y`). Con `D8 10` las coordenadas son absolutas desde el cero de la máquina.
- **Adoptado**: las coordenadas del archivo son distancias desde el home hacia dentro de la cama (X e Y positivas). Como el home depende de la máquina, se añade `HomeCorner` (arriba izquierda por defecto, `--home` en el CLI) que traduce el sistema de Tlaku (abajo-izquierda, Y arriba): con el home arriba, `y_archivo = alto - y`; con el home a la derecha, `x_archivo = ancho - x`. Se usa tanto para generar el archivo como para mostrar la posición leída. Ya no existe `flip_y`. **El valor por defecto es una suposición**: la prueba 2 de 10.6 lo confirma. Confianza media-baja hasta probar.

### 12.11 `D8 00` al inicio

- [FAB]/[JNW] lo omiten en su tabla; [EDU] lo incluye. Está en la captura de Nova 35 ([JNWC]), en los tres `.rd` reales de RDWorks revisados, en el flujo de [MK40] (`write_header`) y [RPAC] lo marca `START_JOB`, verificado en RDC6442S.
- **Adoptado**: siempre se emite justo tras `F1 02 00` (se eliminó la opción de omitirlo y `--no-process-start`). Consecuencia: ver 12.12. Confianza alta en que debe estar.

### 12.12 Qué hacer si la subida falla a medias

- Ninguna fuente documenta un tratamiento: [RPAC] solo descarta el resto del script y reintenta; [MK40] aborta con `D8 01` (`controller.abort`). No hay otro comando de abortar. La captura de Nova 35 muestra que RDWorks guarda el archivo con `E8 02` + `E7 01 nombre` y el mismo `D8 00` dentro, sin ejecutarlo (se inicia desde el panel o con `E8 03`); LightBurn en modo "Iniciar" manda el flujo directo.
- **Adoptado** (`RuidaSession::send_job`): (a) valida el flujo (termina en `D7`, dentro de la cama, sin `DA 01`) y la máquina en reposo antes del primer byte; (b) modo `Store` (predeterminado en el CLI): guarda como documento, no arranca nada; un fallo deja un documento incompleto que se borra desde el panel; (c) modo `Run` (`--run`): ante timeout, NAK/ERR repetido, error de red o Ctrl+C se envía `D8 01` (3 intentos) y se informa si se confirmó; si no, el mensaje dice que puede seguir ejecutando y manda usar Stop o el paro de emergencia; (d) sin reenvío por timeout. Confianza media: el comportamiento de la máquina ante un flujo cortado es el punto 4 de la prueba de hardware.

### 12.13 Sondeo: magic y checksum

- Checksum: ya no se sondea (12.1). Magic: ver la sección 2; se basa en que el ACK va swizzleado con el magic de la máquina. La prueba `test_magic_keys` de [MK40] da la consulta swizzleada con ocho magics (copiada como vectores, THIRD_PARTY.md); [MK40] la usa para identificar al emisor en su emulador.
- Tarjeta: 0x65106510 (RDC6442S) y 0x90109010 (RDC8445S, que habla **TCP**, sin checksum: no soportada). La tabla de magics por ID de [EDU] usa el ID de 16 bits y sigue siendo de una sola fuente.

### 12.14 Otros hechos confirmados o corregidos

- **Escala de la potencia**: el valor es `trunc(p * 16383 / 100)`, no `round(p * 16384 / 100)`. Se comprobó con 72 pares porcentaje-valor de RDWorks ([JNWC] `example-files/laser_percentages/codes.txt`, solo como referencia: 0 fallos con truncar sobre 16383, 38 con truncar sobre 16384, 35 con redondear sobre 16383); [MK40] también usa 16383. El 100 % es `7F 7F`. Cambió `power_from_pct`.
- **`CA 01 xx` (1 a 5)** no son "modos de trabajo": son los modos de overscan del ráster (12.8).
- **`D8 12`** (posición actual) es el modo de referencia de los `.rd` de RDWorks; Tlaku usa `D8 10` (cero de la máquina) como el ejemplo de EduTech y [MK40].
- **634XG** ("altered scrambling"): [MK40] solo le asigna el magic 0x11; sin más datos se mantiene y sigue sin probarse.
- El pie `DA 01 06 20 L L` de RDWorks es una escritura de memoria (longitud de corte); [MK40] no lo emite. Sigue siendo opcional y apagado, y `send_job` rechaza cualquier `DA 01`.

## 13. Qué hace la etapa 3

| Módulo | Contenido |
|---|---|
| `packet.rs` | Datagramas (checksum sobre el cable), partición en límite de comando, interpretación del byte de ACK según el magic |
| `link.rs` | `PacketLink` (envío/recepción de datagramas con timeout), `UdpLink` (escucha en 40200, envía al 50200, socket conectado al interlocutor), `MemoryLink` (con respondedor opcional) |
| `session.rs` | `RuidaSession`: `probe`, `read_mem`, `status`, `send_job` (progreso, cancelación, parada segura), `pause`, `resume`, `stop`, `home`, `jog_to`, `jog_by`, `frame`. Todo antes de enviar pasa por `safety` |
| `emu.rs` | Emulador: en memoria (`Emulator::handle`, `into_link`) y por UDP en `127.0.0.1` con puerto efímero (`UdpEmulator`); responde a lecturas con `DA 01` tras el ACK, modela reposo/trabajo/pausa/posición, documentos guardados y fallos a propósito (paquete perdido, ACK perdido, NAK, ERR, red ignorada) |
| CLI | `tlaku ruida export-rd`, `send <ip> diseño.svg` (sondeo, resumen, confirmación, progreso; `--run`, `--name`, `--dry-run`, `--yes`, `--magic`, `--home`) y `status <ip>` (solo lectura) |

El panel remoto (UDP 50207) no está implementado: pausa, reanudar, detener, home y saltos van por el flujo 50200 como hacen [MK40] y [RPAC].

## 14. Integración en la app y plan de prueba con una máquina real

Falta para integrar (no hecho; fuera de la zona de este trabajo):
- `DeviceKind::Ruida` y un perfil de máquina con IP, magic ("auto" por defecto), `HomeCorner`, tamaño de cama y velocidad de salto.
- En `src-tauri/machine`: un hilo de conexión que posea `RuidaSession<UdpLink>`, con comandos de conectar (sondeo), estado periódico (la lectura de 3 direcciones cada 0,5 a 1 s; no consultar durante un envío), enviar (con `Progress` hacia la interfaz y el indicador de cancelación), pausa/reanudar/detener, saltos y encuadre. La confirmación previa y el aviso de modo experimental son obligatorios, igual que en el CLI.
- Mapear `ComputedJob` a `RuidaJobOptions` como en `export_rd` de `crates/tlaku-cli/src/ruida.rs` (colores de capa, home, cama).
- Interfaz: elegir "guardar documento" (predeterminado) o "ejecutar", y mostrar el resultado del `D8 01` si falla un envío.

Plan de prueba con una máquina real: la sección 10.6, en el orden indicado. Criterios para pasar a "perfil probado" de un modelo: pasos 1 a 3 correctos, el paso 4 sin comportamientos inesperados, y la captura del paso 6 comparada.
