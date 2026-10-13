# GRBL y variantes: referencia de protocolo y contraste con Tlaku

Objetivo: validar el driver GRBL de Tlaku (`crates/tlaku-grbl`, `apps/desktop/src-tauri/src/machine.rs`, `apps/desktop/src/features/machine/`) sin máquina física, contra la documentación oficial, y dejar vectores de prueba para pruebas automáticas o un emulador GRBL propio.

## 0. Método y límites

**Sala limpia.** Solo se leyó documentación (markdown y wikis). No se leyó ni se copió código fuente GPL (`.c`/`.h`). Todo el texto está resumido con palabras propias. Cada dato lleva su URL.

Etiquetas de fiabilidad que se usan en todo el documento:

- **[D]** documentación oficial del proyecto, leída directamente.
- **[T]** documentación de terceros (foros, guías de fabricantes, espejos de wiki). Útil, pero secundaria.
- **[I]** inferencia mía a partir de [D]/[T]. Hay que confirmarla con hardware real o con la fuente oficial antes de darla por buena.
- **[?]** no se pudo verificar.

**Fuentes principales** (todas leídas):

| Alias | URL |
|---|---|
| G-Interface | https://github.com/gnea/grbl/wiki/Grbl-v1.1-Interface y https://raw.githubusercontent.com/gnea/grbl/master/doc/markdown/interface.md |
| G-Commands | https://github.com/gnea/grbl/wiki/Grbl-v1.1-Commands y https://raw.githubusercontent.com/gnea/grbl/master/doc/markdown/commands.md |
| G-Laser | https://github.com/gnea/grbl/wiki/Grbl-v1.1-Laser-Mode y https://raw.githubusercontent.com/gnea/grbl/master/doc/markdown/laser_mode.md |
| G-Jog | https://github.com/gnea/grbl/wiki/Grbl-v1.1-Jogging y https://raw.githubusercontent.com/gnea/grbl/master/doc/markdown/jogging.md |
| G-Config | https://github.com/gnea/grbl/wiki/Grbl-v1.1-Configuration y https://raw.githubusercontent.com/gnea/grbl/master/doc/markdown/settings.md |
| G-Issues | https://github.com/gnea/grbl/wiki/Known-Issues |
| G-FAQ | https://github.com/gnea/grbl/wiki/Frequently-Asked-Questions |
| G-stream.py | https://raw.githubusercontent.com/gnea/grbl/master/doc/script/stream.py (script de host en Python; no es firmware) |
| G-Mega | https://github.com/gnea/grbl-Mega (README) |
| HAL-Report | https://github.com/grblHAL/core/wiki/Report-extensions |
| HAL-Changes | https://github.com/grblHAL/core/wiki/Changes-from-grbl-1.1 |
| HAL-Compat | https://github.com/grblHAL/core/wiki/Compatibility-level |
| HAL-Sender | https://github.com/grblHAL/core/wiki/For-sender-developers |
| HAL-Settings | https://github.com/grblHAL/core/wiki/Additional-or-extended-settings |
| HAL-GM | https://github.com/grblHAL/core/wiki/Additional-G--and-M-codes |
| HAL-Home | https://github.com/grblHAL/core/wiki (página de inicio) |
| First-Run | https://github.com/grblHAL/core/wiki/First-Run-Grbl-Settings |
| HAL-Err | https://svn.io-engineering.com/grblHAL/html/errors_8h_source.html y https://svn.io-engineering.com/grblHAL/html/alarms_8h_source.html (documentación Doxygen de la API pública: solo se tomaron números y nombres de enumeraciones, no código) |
| Sienci | https://resources.sienci.com/view/gs-gsender-grbl-alarm-error-codes/ [T] |
| cnc3d | https://www.cnc3d.com.au/post/grbl-error-alarm-codes-including-extended-code-for-advanced-grbl-controllers [T] |
| FNC-Cmd | https://github.com/bdring/FluidNC/wiki/FluidNC-Commands-and-Settings/ea5af623daa9a360b57d7af3101514b2102c7f35 (versión fijada de la wiki archivada) |
| FNC-Term | https://github-wiki-see.page/m/Longus/FluidNC/wiki/Serial-Terminals [T] (espejo de la wiki de FluidNC) |
| FNC-Forum | https://forum.lightburnsoftware.com/t/move-controls-greyed-out-with-fluidnc/173513 [T] (salida real de FluidNC 3.9.7) |
| ESP-Laser | https://github.com/bdring/Grbl_Esp32/wiki/Laser-Mode |
| ESP-MSG | https://github.com/bdring/Grbl_Esp32/wiki/MSG-Style-Guide |
| ESP-Serial | https://github.com/bdring/Grbl_Esp32/wiki/Serial-Port-Setup-and-Usage |

**Lo que NO se pudo verificar** (anotado para no inventar):

- La wiki oficial de FluidNC (`wiki.fluidnc.com`) rechazó la conexión (`ECONNREFUSED`) en todos los intentos, y la wiki de GitHub de FluidNC está archivada sin contenido. Lo que hay de FluidNC sale de la tabla de comandos fijada, de espejos y de capturas de foros [T]. Los campos exactos del `<...>` de FluidNC, su tamaño de búfer RX y su tabla de errores/alarmas completa quedan **[?]**.
- grblHAL: no se halló tabla de descripciones de errores/alarmas en la wiki; solo los nombres y números de la API (HAL-Err). Tampoco se halló el tamaño de búfer RX por defecto ni la tabla de bits de `$10` más allá de la lista de HAL-Settings.
- grbl-Mega: su README no describe diferencias de protocolo respecto a 1.1; solo dice que funciona únicamente en Arduino Mega2560 (G-Mega). Se asume el mismo protocolo de 1.1 [I].
- Ports de 32 bits (Grbl_ESP32, hoy FluidNC; grbl-LPC; STM32): solo se leyeron las páginas de Grbl_ESP32 de arriba. El resto queda [?].
- Las wikis oficiales de GRBL no dan la longitud máxima de línea (solo `error:11` genérico) ni las reglas finas de "qué `$` es válido en qué estado" (solo el genérico `error:8`).

---

## 1. Protocolo de envío

### 1.1 Respuestas, push y tiempo real

La interfaz distingue tres tipos de mensajes [D] G-Interface:

1. **Respuestas**: `ok` o `error:N`. Son las únicas que cuentan para el control de flujo.
2. **Push**: retroalimentación, estado, ajustes, alarmas y arranque. Se reconocen por corchetes `[...]`, ángulos `<...>`, un `$` inicial o texto específico (`ALARM:N`, banner). No se cuentan.
3. **Comandos en tiempo real**: bytes sueltos que no entran al búfer RX, no necesitan `\n` y normalmente no generan `ok`.

Una línea vacía (solo retorno) se considera válida y también responde `ok` aunque no haga nada [D] G-Interface. Tlaku no envía líneas vacías (las descarta en `prepare`), así que es coherente.

### 1.2 Conteo de caracteres frente a "enviar-y-esperar"

La documentación oficial usa el nombre **Simple Send-Response** (lo que informalmente se llama ping-pong) y **Character-Counting** [D] G-Interface:

- *Send-Response*: se envía una línea, se espera `ok`/`error:`, se envía la siguiente. Es la más robusta, pero la latencia por línea puede dejar vacío el planificador con segmentos cortos y rápidos (movimiento entrecortado).
- *Character-Counting*: el host mantiene la suma de bytes de las líneas enviadas sin respuesta y no envía otra mientras la suma más la nueva línea supere el búfer. Cada `ok`/`error:` libera la línea más antigua. Los bytes de cada línea **incluyen el terminador** (el ejemplo oficial usa líneas de 25, 40, 31, 58 y 20 bytes: las tres primeras suman 96 y se envían; la de 58 espera al primer `ok`; las dos últimas salen juntas tras el segundo).
- Ambos protocolos ignoran los push y no cuentan los bytes de tiempo real.
- Los ajustes (`$x=val`) **no** deben enviarse con conteo de caracteres; solo con enviar-y-esperar, porque la escritura en EEPROM deshabilita la interrupción de recepción serie y pueden perderse caracteres [D] G-Interface, [D] G-Issues, [D] G-FAQ (G10, G28.1 y G30.1 también escriben EEPROM y pausan interrupciones hasta unos 20 ms).
- **Reserva oficial**: si una línea del frente del búfer da error, GRBL sigue ejecutando lo que ya tiene en el búfer; el host no puede detenerlo con el protocolo de conteo. La mitigación sugerida por el proyecto es validar el programa antes con `$C` (modo de verificación) [D] G-Interface, [D] G-Issues.
- Sincronización: un `G4 P0.01` (P en segundos, mayor que 0) fuerza a vaciar el planificador antes de la siguiente tarea [D] G-Interface.
- Velocidad: se recomienda 115200 baud o más; con baudios bajos el comportamiento es impredecible. Las placas con CH340 pueden tener errores de transmisión sin arreglo, que podrían producir movimiento no deseado [D] G-Issues.

### 1.3 Tamaño del búfer RX

- La wiki dice **128 caracteres** y el ejemplo de `$I` muestra `[OPT:...,15,128]`; la wiki nunca menciona 127 [D] G-Interface.
- El script oficial de streaming define `RX_BUFFER_SIZE = 128` pero espera mientras la suma pendiente sea `>= RX_BUFFER_SIZE-1`, es decir, usa **127** efectivos, y cuenta `len(línea_limpia)+1` por línea (el `+1` es el `\n`). La línea se limpia de espacios y comentarios y se pasa a mayúsculas antes de contarla [D] G-stream.py.
  - El script tiene una precedencia dudosa (`|` ata más que `>=` en Python), así que no hay que copiar su condición literalmente; la intención es 127 [I].
- Conclusión práctica para GRBL clásico: **declarado 128, utilizable 127** (el valor que usa Tlaku, `StreamerConfig::rx_buffer = 127`). El tercer campo de `[OPT:...]` informa el tamaño declarado [D].
- `Bf:a,b` (si está habilitado) da bloques libres del planificador y bytes libres del RX; el ejemplo oficial es `Bf:15,128`. **No debe usarse para controlar el flujo** [D] G-Interface.
- grblHAL y FluidNC admiten búferes distintos; ninguna fuente leída da el número (ver sección 9). Para ellos hay que leerlo de `$I` o usar un valor prudente configurable [I].

### 1.4 Qué cuenta en el búfer

- Sí cuenta: cada byte de la línea enviada, incluido el `\n` (y el `\r` si el host lo manda). Tlaku manda solo `\n` y cuenta `len+1`, igual que el script oficial.
- No cuenta: `?`, `!`, `~`, `0x18`, `0x84`, `0x85`, `0x90`–`0x9E`, `0xA0`, `0xA1`, ni ningún byte de tiempo real. GRBL los extrae del flujo al detectarlos [D] G-Interface, [D] G-Commands.
- Los comentarios y espacios se eliminan antes de contar en el script oficial [D]. [I] Si el host los enviara, ocuparían búfer físico hasta que GRBL los analice; por eso conviene quitarlos antes de enviar (Tlaku lo hace en `prepare_line`).
- Códigos de error de longitud: `error:11` = más caracteres por línea de lo permitido; la línea no se ejecuta [D] G-Interface. El límite de línea de GRBL no está en la documentación leída [?]; Tlaku solo valida contra el tamaño del búfer RX, no contra el límite de línea.

### 1.5 Respuestas `ok` / `error:N`

- `ok` significa que la línea se interpretó y, en el caso de movimiento, se aceptó en el planificador. No significa que el movimiento terminó [D] por el comportamiento descrito en G-Interface y [I] por el detalle; Tlaku ya lo asume (`wait_idle` espera `Idle` tras el último `ok`).
- `error:N` descarta ese bloque. La recomendación oficial es detener el envío hasta que el usuario corrija el programa [D] G-Interface.
- Los comandos `$` también responden `ok`/`error:`.
- En grblHAL (nivel de compatibilidad 0) un error **persiste**: las líneas siguientes devuelven el mismo error hasta que llegue un reset, una línea vacía o un comando `$` [D] HAL-Changes, [D] HAL-Compat. Es una protección pensada precisamente para streaming con búfer.

### 1.6 Comandos en tiempo real (GRBL 1.1)

Todos son un byte, no ocupan búfer y no necesitan terminador [D] G-Commands:

| Byte | Comando | Cuándo actúa |
|---|---|---|
| `?` (0x3F) | Reporte de estado | Casi siempre; **no** durante homing ni tras una alarma crítica de límites (puede ignorarse) |
| `~` (0x7E) | Cycle start / reanudar | Desde hold, puerta o pausa M0; si no, se ignora |
| `!` (0x21) | Feed hold | En Idle, Run o Jog. No apaga husillo ni refrigerante |
| 0x18 | Soft reset | En cualquier momento. Si hay movimiento: alarma (posición posiblemente perdida). Si está quieto: la posición se conserva |
| 0x84 | Puerta de seguridad | En Idle, Hold, Run, Home o Jog. Apaga husillo y refrigerante |
| 0x85 | Jog cancel | Solo se atiende en estado Jog; si no, se ignora |
| 0x90 | Avance al 100 % | Los overrides de avance no afectan a rápidos ni a jogs |
| 0x91 / 0x92 | Avance +10 % / -10 % | |
| 0x93 / 0x94 | Avance +1 % / -1 % | |
| 0x95 / 0x96 / 0x97 | Rápido 100 % / 50 % / 25 % | |
| 0x99 | Husillo (potencia) al 100 % | |
| 0x9A / 0x9B | Husillo +10 % / -10 % | |
| 0x9C / 0x9D | Husillo +1 % / -1 % | |
| 0x9E | Alternar paro de husillo | Solo en Hold; al reanudar vuelve con un retardo configurable (4 s por defecto) |
| 0xA0 | Alternar refrigerante flood | Idle, Run o Hold |
| 0xA1 | Alternar refrigerante mist | Requiere opción de compilación `ENABLE_M7` (apagada por defecto) |

Las órdenes de override son bytes de 8 bits que no se pueden teclear y no figuran en la ayuda `$` [D] G-Interface.

Tlaku define los 22 bytes en `Realtime` (`streamer.rs`) y coinciden exactamente. La interfaz de usuario solo expone Status, Hold, Resume, Reset y JogCancel (`RealtimeCommand`); los overrides existen en el streamer pero no están conectados a la UI.

### 1.7 Frecuencia de `?`

- GRBL responde en unos 5 a 20 ms. Máximo recomendado: **5 Hz**; 10 Hz "puede funcionar" con rendimientos decrecientes [D] G-Interface.
- Un `?` puede ignorarse si llegan dos antes del primer reporte, justo tras un soft reset, tras una alarma crítica de límites o durante el homing [D] G-Interface.
- FluidNC pide no pasar de 10 Hz [T] FNC-Term.
- Tlaku: 250 ms en reposo (4 Hz) y 200 ms transmitiendo (5 Hz). Dentro de lo recomendado.

---

## 2. Reporte de estado `<...>`

### 2.1 Formato

```
<Estado[:sub]|MPos:x,y,z|campo:valor|campo:valor...>
```

Campos separados por `|`; el estado va siempre primero y la posición siempre segunda [D] G-Interface. Un orden de campos distinto, o campos desconocidos, no debería romper al receptor (grblHAL lo exige explícitamente: no asumir orden, ignorar etiquetas desconocidas) [D] HAL-Sender.

| Campo | Significado | Detalle |
|---|---|---|
| Estado | `Idle`, `Run`, `Hold`, `Jog`, `Alarm`, `Door`, `Check`, `Home`, `Sleep` | Primero siempre. Subestados con `:` (ver sección 8) |
| `MPos:x,y,z` o `WPos:x,y,z` | Posición de máquina o de trabajo | Solo uno de los dos (decide `$10`). Unidades por `$13` (mm/pulgadas) |
| `WCO:x,y,z` | Desplazamiento de coordenadas de trabajo | No va en cada reporte: se envía cada 10 reportes (en movimiento) o 30 (quieto), configurable entre 1 y 255; se envía además al cambiar y tras un reset. `WPos = MPos - WCO` |
| `Bf:a,b` | `a` bloques libres del planificador, `b` bytes libres del RX | Desactivado por defecto; **disponibles**, no "en uso" (cambio respecto a versiones previas) |
| `Ln:n` | Línea en ejecución | Solo si hay números de línea (`N`) y compilación con esa opción |
| `FS:f,s` | Avance en tiempo real y velocidad del husillo | `F:f` en su lugar si `VARIABLE_SPINDLE` está desactivado |
| `Pn:XYZPDHRS` | Pines de entrada activos | `X Y Z` límites, `P` sonda, `D H R S` puerta/hold/reset/cycle-start |
| `Ov:f,r,s` | Overrides activos (%) de avance, rápido y husillo | Se envía cada 10 o 20 reportes, y al cambiar |
| `A:SCFM` | Estado de accesorios | `S` husillo CW, `C` husillo CCW, `F` flood, `M` mist. Solo aparece cuando hay alguno activo y acompaña a `Ov:` |

Fuente de todo el cuadro: [D] G-Interface.

Un reporte real mínimo y por defecto (`$10=1`): `<Idle|MPos:0.000,0.000,0.000|FS:0.0,0>` es el ejemplo de la doc [D]. En 1.1 los números con precisión cero se imprimen sin punto decimal (`FS:500,8000`) [D] G-Interface (cambios 1.1).

### 2.2 `$10` (máscara del reporte)

- GRBL 1.1: bit 0 (valor 1) = `MPos` si está activo, `WPos` si no; bit 1 (valor 2) = incluir `Bf`. Por defecto `$10=1`. `$10=2` da WPos con Bf [D] G-Config. `FS`, `Pn`, `Ov`, `WCO` y `A` no dependen de `$10` en 1.1, solo su frecuencia; la doc dice que `$10` puede alterar qué datos hay y que algunos van de forma intermitente [D] G-Interface.
- grblHAL amplía los bits: 0 posición de máquina, 1 estado de búferes, 2 números de línea, 3 avance/velocidad, 4 estado de pines, 5 WCO, 6 overrides, 7 coordenadas de sonda, 8 sync al cambiar WCO, 9 estado del parser, 10 subestado de alarma, 11 subestado de Run [D] HAL-Settings. Con ciertos niveles de compatibilidad solo se aceptan algunos bits [D] HAL-Compat. Moraleja: **no depender de que un campo esté presente**.

### 2.3 Tlaku frente a esto

`StatusReport::parse` (streamer.rs líneas 181 a 243) acepta MPos o WPos, WCO, Bf, Ln, FS o F, Ov, Pn y A, ignora los campos desconocidos y no depende del orden. Casos a vigilar, tratados en la sección 10: `FS` con tres valores, estados desconocidos que descartan el reporte entero, y WCO tardío tras conectar.

---

## 3. Mensajes push

### 3.1 Banner

`Grbl X.Xx ['$' for help]` al encender y tras cada reset. La letra de versión indica nuevas funciones; las correcciones de errores aparecen como fecha en `$I` (por ejemplo `20161014`) [D] G-Interface.

Variantes (ver sección 9):

- grblHAL: empieza por `GrblHAL ...` (con nivel de compatibilidad 1 o superior dice `Grbl 1.1...`) [D] HAL-Changes, [D] HAL-Compat.
- FluidNC: `Grbl 1.1f [$ for help]` y, en versiones recientes, `Grbl 3.4 [FluidNC v3.4.3 (wifi) '$' for help]`, con líneas `[MSG:...]` antes [T] FNC-Forum y el resultado de búsquedas de issues de FluidNC.

### 3.2 `[MSG:...]` (retroalimentación)

Catálogo oficial de GRBL 1.1 [D] G-Interface:

| Mensaje | Cuándo |
|---|---|
| `[MSG:Reset to continue]` | Error de límite duro o blando: hace falta un reset |
| `[MSG:'$H'\|'$X' to unlock]` | Hay alarma al arrancar; las líneas de arranque no se ejecutan |
| `[MSG:Caution: Unlocked]` | Tras `$X`; la posición puede estar perdida |
| `[MSG:Enabled]` / `[MSG:Disabled]` | Modo de verificación `$C`. Al desactivarlo hay un reset automático |
| `[MSG:Check Door]` | Puerta abierta: hold y estado Door si hay movimiento |
| `[MSG:Check Limits]` | Un límite estaba activo al arrancar con límites duros habilitados |
| `[MSG:Pgm End]` | M2/M30: el estado modal se restablece |
| `[MSG:Restoring defaults]` | Tras `$RST=` |
| `[MSG:Sleeping]` | Tras `$SLP` en Idle o Alarm; para salir hace falta un reset, que entra en alarma |

Los `[MSG:...]` no pertenecen al conteo de caracteres y pueden aparecer en cualquier momento. Un receptor no debe romperse ante ellos (principio explícito en la guía de estilo de Grbl_ESP32: "los remitentes pueden ignorarlos pero deben tolerarlos") [D] ESP-MSG.

### 3.3 Respuestas a consultas

- `$G`: `[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0.0 S0]` seguido de `ok`. Muestra el estado del programa (M0/M2/M30) solo cuando está activo [D] G-Interface.
- `$`: `[HLP:$$ $# $G $I $N $x=val $Nx=line $J=line $C $X $H ~ ! ? ctrl-x]` y `ok`.
- `$#`: `[G54:x,y,z]` ... `[G59:...]`, `[G28:...]`, `[G30:...]`, `[G92:...]`, `[TLO:z]`, `[PRB:x,y,z:s]` y `ok`. El sufijo de `PRB` es un booleano de éxito [D] G-Interface. G92, G43.1 y G38.2 no persisten [D] G-Commands.
- `$I`: `[VER:versión.fecha:cadena_usuario]` y `[OPT:opciones,bloques_planificador,bytes_RX]` y `ok`. Ejemplo oficial de OPT: `[OPT:VL,16,128]`. La cadena de usuario (`$I=texto`, menos de 80 caracteres) puede estar bloqueada por el fabricante [D] G-Interface, [D] G-Commands.
- `$N`: `$N0=línea`, `$N1=línea` y `ok`.
- `$$`: `$0=10` ... `$132=200.000` y `ok`. En 1.1 se quitaron las descripciones para ahorrar flash [D] G-Interface, [D] G-Config. Códigos de ajuste del 0 al 6, 10, 11, 12, 13, 20 a 27, 30, 31, 32, 100 a 102, 110 a 112, 120 a 122, 130 a 132.
- `[echo:...]`: eco de línea si está compilado; sin espacios ni comentarios y en mayúsculas.
- Líneas de arranque: `>G54G20:ok` (el `>` marca una línea de arranque y `:ok` es interno, no cuenta para el flujo). Si falla: `>G54G20:error:X`. Si falla la lectura de EEPROM: `>:error:7` [D] G-Interface.

### 3.4 Alarmas

Formato `ALARM:N`. GRBL se bloquea y solo acepta un conjunto pequeño de comandos hasta un reset, `$X` o un homing `$H` [D] G-Interface. Lista de la sección 6.

---

## 4. Modo láser (`$32=1`)

Todo esto sale de [D] G-Laser, [D] G-Config y [D] G-FAQ, salvo lo marcado.

### 4.1 Cómo se activa

- `$32=1` activa; `$32=0` desactiva. Si se vuelve a fresar con husillo, **hay que** poner `$32=0` [D].
- Con `$32=1`, GRBL se mueve de forma continua entre G1/G2/G3 consecutivos aunque cambie S (el PWM se actualiza al instante). Con `$32=0` (por defecto) el movimiento se detiene en cada cambio de S, como en una fresadora [D] G-Config.
- Sin PWM en el pin del husillo, `error:17` ("el modo láser requiere salida PWM") [D].
- En FAQ: en modo láser M3/M4 "solo encienden durante movimientos G1, G2, G3" por seguridad [D] G-FAQ.

### 4.2 M3 y M4

- **M3, potencia constante**: la potencia se mantiene como se programó, se esté moviendo o no, acelerando o parado. Sirve para enfocar y es lo que usa normalmente el CAM láser.
- **M4, potencia dinámica**: la potencia se ajusta a la velocidad real relativa a la programada (supuesto lineal que el propio documento admite que no siempre se cumple). **Cuando no hay movimiento, M4 apaga el láser**; solo enciende mientras la máquina se mueve. Hay que probar en una pieza antes de usarlo en un material nuevo.
- La documentación dice, además, que M4 "no se detiene por nada salvo un cambio de estado del husillo".

### 4.3 S, G0 y S modal

- **G0 y G38.x nunca encienden el láser**: lo desactivan siempre. Aun así, `G0 M3 S1000` sí fija el estado modal (M3 activo, S1000); el siguiente `G1` enciende con esos valores sin necesidad de repetir S.
- **S0 apaga el láser** sin cambiar el estado del husillo (`S0` es G-code válido).
- S es modal. Tras un G0, la S anterior se conserva y el siguiente movimiento con potencia la reutiliza.

### 4.4 Paradas

- En modo láser GRBL **no se detiene por cambios de S** entre movimientos consecutivos.
- Excepción: con M3 y **sin movimiento programado**, un cambio de S sí fuerza una parada (en M4 no).
- GRBL sí se detiene siempre ante cualquier **cambio de estado del husillo**: M3, M4 o M5 [D] G-Laser. Es decir, cambiar entre M3 y M4 entre bloques tiene un costo de parada (Tlaku lo hace solo si cambia el modo de potencia de un bloque).
- Evitar G0/G1 sin palabras de eje en medio de un trabajo en M3 [D].
- El comportamiento exacto de M5 más allá de "cambio de estado = parada" no está desarrollado en la doc leída [?]. Sí: es lo que apaga el láser al final.

### 4.5 Grabado raster: S0 en G1 en lugar de G0

- Recomendación oficial: entre tramos con potencia que no necesiten láser, usar **G0** (apaga automáticamente el láser y conserva la última S) o `S0` en un G1.
- El efecto concreto en el planificador (G0 frente a G1 S0) **no se describe** en la doc leída [?]. Lo único documentado es que los movimientos consecutivos con S distintas son continuos [D]. [I] Un G1 con S0 mantiene el mismo perfil de avance programado (F) y evita cambiar a la velocidad de rápido; un G0 sí cambia de avance. Para mantener velocidad de barrido constante y evitar resoplidos del planificador conviene G1 S0 en el overscan.
- Lead-in/lead-out: se recomiendan movimientos de entrada y salida para que la máquina acelere y frene sin quemar [D].
- Tlaku: el overscan es `G1 ... S0` y los saltos entre trazos son `G0`; ambos son coherentes con la recomendación. Los tramos de potencia 0 dentro de una línea raster salen como `G1 Xn S0`, no como G0.

### 4.6 Ajustes `$30` y `$31`

- `$30`: RPM (aquí, valor de S) que produce el PWM máximo (5 V). Valores mayores se aceptan pero la salida se queda al máximo. El mapeo es lineal en 255 pasos. Defecto 1000.
- `$31`: S que produce el PWM mínimo (0,02 V). Valores menores se aceptan, pero la salida no baja de 0,02 V salvo S=0, que desactiva (0 V). Defecto 0 [D] G-Config.
- El wiki de modo láser no trata `$30`/`$31`; se infiere que `S = $30` es potencia 100 % [I]. Tlaku usa `s_max` = `$30` (`GcodeOptions::s_max`).

### 4.7 M4 sin movimiento

Cuando no hay movimiento M4 apaga el láser [D]; [I] por esa regla no debería quedar encendido en un hold ni en una pausa M0. Con **M3**, en cambio, la potencia sigue encendida estando parado [D]: ver el riesgo del feed hold en la sección 10.

### 4.8 grblHAL y FluidNC en láser

- grblHAL: `$32` pasa de booleano a modo (0 normal, 1 láser, 2 torno); hay `$33` a `$36` para frecuencia y ciclo PWM (apagado, mínimo, máximo); `$63` desactiva el láser durante el hold (la doc es incoherente en el valor por defecto) [D] HAL-Settings.
- FluidNC: no tiene ajuste real `$32`; ofrece `$32` como alias `$FakeLaserMode` por compatibilidad con senders. Dice que "está siempre en modo láser si hay un láser configurado y es el husillo activo" (la frase está truncada en la página) [T] FNC-Cmd.
- Grbl_ESP32: G0 siempre desactiva el láser; G0 es el modo por defecto tras encender o resetear, por lo que hay que pasar a G1/G2/G3 para disparar [D] ESP-Laser.

---

## 5. Jogging (`$J=`)

Todo de [D] G-Jog y [D] G-Commands salvo lo marcado.

- **Sintaxis**: `$J=` seguido de un movimiento tipo G1. Ejemplos oficiales: `$J=X10.0 Y-1.5 F100`; `$J=G91 G20 X0.5 F10`; `$J=G53 Y5.0 F10`.
- **Requeridos**: al menos una palabra de eje y `F`. **F no es modal**: cada jog necesita la suya. F solo se interpreta en G94 (unidades por minuto); un G93 se ignora en jog.
- **Opcionales** (valen solo para ese comando): `G20/G21`, `G90/G91`, `G53`, `N`.
- **Prohibidos**: cualquier otro G, M o palabra de valor (S, T...).
- Espacios y comentarios permitidos (los quita el preprocesador).
- **No altera el estado del parser**: no hace falta restablecer G90/G91 ni F antes de un trabajo.
- **Respuesta**: `ok` cuando se analizó y se puso en cola; `error:` si no es válido. Los números de error exactos los da la tabla general: `error:16` (falta `=` o G-code prohibido), `error:15` (el destino excede el recorrido) [D] G-Interface.
- **Soft limits**: un jog que se salga del recorrido no se ejecuta y devuelve error (no alarma).
- **Estados**: solo se aceptan jogs en Idle o Jog. Mientras corre el jog, el estado es `Jog`. Mezclar G-code con un jog en ejecución devuelve un error de bloqueo [D] G-Jog; el número es probablemente `error:9` (G-code bloqueado durante alarma o jog) [I] a partir de la tabla de G-Interface.
- **Cola**: se pueden encolar varios jogs seguidos.
- **Cancelación**: `0x85` (jog cancel) vacía la cola, detiene con una desaceleración y devuelve a Idle sin soft reset. Solo se atiende en Jog. También cancelan un feed hold o el evento de puerta, pero el wiki desaconseja usar feed hold para cancelar un jog porque, si GRBL ya volvió a Idle, el hold puede suspenderlo sin querer. Para saber cuándo terminó la cancelación, se recomienda enviar `G4P0` después y esperar su `ok`.
- Tlaku: `jog()` envía `$J=G91 G21 X.. Y.. F..` y `Cancel` sin trabajo envía 0x85. Coincide con la doc.

---

## 6. Errores y alarmas

### 6.1 `error:N` de GRBL 1.1 [D] G-Interface

| N | Significado |
|---|---|
| 1 | No se encontró una letra de palabra G-code |
| 2 | Valor numérico no válido o ausente |
| 3 | Comando `$` no reconocido |
| 4 | Valor negativo donde se espera positivo |
| 5 | Homing no habilitado en los ajustes |
| 6 | El pulso de paso mínimo debe ser mayor que 3 µs |
| 7 | Falló la lectura de EEPROM; se restauraron valores por defecto |
| 8 | El comando `$` requiere estado Idle |
| 9 | G-code bloqueado durante alarma o jog |
| 10 | Los límites blandos requieren homing habilitado |
| 11 | Se superó el máximo de caracteres por línea |
| 12 | Un valor `$` supera la velocidad máxima de pasos (opción de compilación) |
| 13 | Puerta de seguridad detectada abierta |
| 14 | La línea de info de compilación o de arranque excede el límite de EEPROM (Grbl-Mega) |
| 15 | El destino del jog excede el recorrido de la máquina; se ignora |
| 16 | Al jog le falta `=` o contiene G-code prohibido |
| 17 | El modo láser requiere salida PWM |
| 20 | G-code no soportado o no válido |
| 21 | Varios comandos del mismo grupo modal en un bloque |
| 22 | Avance no definido |
| 23 | El comando requiere un valor entero |
| 24 | Dos comandos usan palabras de eje a la vez |
| 25 | Palabra G-code repetida |
| 26 | El comando requiere palabras XYZ y no hay |
| 27 | Número de línea `N` fuera de 1 a 9 999 999 |
| 28 | Falta una palabra `P` o `L` requerida |
| 29 | G59.1, G59.2, G59.3 no soportados |
| 30 | G53 requiere G0 o G1 activo |
| 31 | Palabras de eje sin uso con G80 activo |
| 32 | G2/G3 sin palabras de eje del plano |
| 33 | Destino de movimiento no válido (arco imposible, o destino de sonda igual a la posición) |
| 34 | Error matemático en la definición de radio del arco |
| 35 | Falta una palabra IJK en la definición por desplazamiento |
| 36 | Quedan palabras sin usar en el bloque |
| 37 | G43.1 aplicado a un eje no configurado |
| 38 | Número de herramienta mayor que el máximo |

### 6.2 `ALARM:N` de GRBL 1.1

Del 1 al 9 en [D] G-Interface (interface.md); el wiki añade el **10** [D] (wiki G-Interface): fallo de homing porque no se halló el segundo final de carrera de un eje autoescuadrado.

| N | Significado | Posición |
|---|---|---|
| 1 | Límite duro activado | Probablemente perdida; se recomienda rehacer homing |
| 2 | El destino del G-code excede el recorrido | Se conserva; se puede desbloquear |
| 3 | Reset durante un movimiento | Perdida; se recomienda homing |
| 4 | Fallo de sonda: no estaba en el estado inicial esperado | |
| 5 | Fallo de sonda: sin contacto dentro del recorrido | |
| 6 | Homing fallido: reset durante el ciclo | |
| 7 | Homing fallido: puerta abierta durante el ciclo | |
| 8 | Homing fallido: el final de carrera no se liberó al retirarse | |
| 9 | Homing fallido: no se halló el final de carrera en la distancia de búsqueda | |
| 10 | Homing fallido: no se halló el segundo final de carrera (autoescuadre) | |

### 6.3 Diferencias de grblHAL

Alarmas (HAL-Err; los números 0 a 9 son los de GRBL clásico, y el comentario de la API dice que el 15 de grblHAL equivale al 10 clásico):

| N | Nombre en la API | Notas |
|---|---|---|
| 10 | EStop | Parada de emergencia activa. [T] Sienci: pulsada; hay que soltarla antes de limpiar. **Choca con el 10 de GRBL clásico** |
| 11 | HomingRequired | Hay que hacer homing (`$H`) [D] HAL-Sender dice que sale `Homing cycle required` y a veces un reset no basta |
| 12 | LimitsEngaged | Límites activos |
| 13 | ProbeProtect | Protección de sonda |
| 14 | Spindle | [T] Sienci: el husillo no alcanzó velocidad a tiempo; [T] cnc3d: fallo de control del husillo (RS485) |
| 15 | HomingFailAutoSquaringApproach | Equivale al 10 clásico |
| 16 | SelftestFailed | |
| 17 | MotorFault | Seguimiento de motor en lazo cerrado |
| 18 | HomingFail | |
| 19 | ModbusException | |
| 20 | ExpanderException | |
| 21 | NVS_Failed | |
| 22 | BufferOverflow | |

Los textos de 11 a 13 y 16 a 22 no están en una página de descripciones que se haya podido leer; son solo nombres [?].

Errores (HAL-Err; 0 a 38 equivalen a GRBL 1.1):

| N | Nombre en la API |
|---|---|
| 18 | Reset (la entrada de reset debe estar desactivada; si no, error 18 y no hay operación [D] HAL-Changes) |
| 19 | NonPositiveValue |
| 39 | GcodeValueOutOfRange |
| 40 | GcodeToolChangePending |
| 41 | GcodeSpindleNotRunning |
| 42 | GcodeIllegalPlane |
| 43 | GcodeMaxFeedRateExceeded |
| 44 | GcodeRPMOutOfRange |
| 45 | LimitsEngaged |
| 46 | HomingRequired |
| 47 | GCodeToolError |
| 48 | ValueWordConflict |
| 49 | SelfTestFailed |
| 50 | EStop |
| 51 | MotorFault |
| 52 | SettingValueOutOfRange |
| 53 | SettingDisabled |
| 54 | GcodeInvalidRetractPosition |
| 55 | IllegalHomingConfiguration |
| 56 | GCodeCoordSystemLocked |
| 57 | UnexpectedDemarcation |
| 58 | AuxiliaryPortUnavailable |
| 60 a 66 | Errores de SD y sistema de ficheros (SDMountError, FileReadError, FsFailedOpenDir, FSDirNotFound, SDNotMounted, FsNotMounted, FsReadOnly) |
| 70 | BTInitError |
| 71 a 76 | Errores de expresiones (operador desconocido, división por cero, argumento fuera de rango, argumento no válido, error de sintaxis, resultado no válido) |
| 77, 78, 79 | AuthenticationRequired, AccessDenied, NotAllowedCriticalEvent |
| 80 a 83 | Control de flujo (macro, sintaxis, desbordamiento de pila, memoria) |
| 84 a 89 | Ficheros y herramienta en husillo (FileOpenFailed, FsFormatFailed, AuxiliaryPortUnusable, ToolInSpindle, NoToolInSpindle, FileDeleteFailed) |
| 90 a 92 | Compensación de cortador |

Estos números superan los que conoce `error_message` de Tlaku (hasta 38). Sin traducción, Tlaku muestra `error:N: error desconocido`, que al menos conserva el número.

### 6.4 Diferencias de FluidNC [T]

La lista de [T] cnc3d (que cubre "controladores GRBL modernos" como FluidNC, sin asignar códigos a firmware) contiene números que **chocan** con los de grblHAL a partir del 60 (por ejemplo, 64 es "archivo vacío" allí y `SDNotMounted` en grblHAL). Por eso **no se debe usar una tabla de descripciones única para `error:>38`**. FluidNC ofrece `$Errors/List` (`$E`, también `$E=N`) y `$Alarms/List` (`$A`) para consultar el texto en la propia máquina [T] FNC-Cmd; aprovecharlo es la forma segura de tener descripciones fiables.

| N (cnc3d) | Descripción |
|---|---|
| 39 | Parámetro P demasiado grande (segundos, no milisegundos) |
| 60 a 69 | Errores de tarjeta SD (no inicia, no lee, no abre directorio, directorio/archivo no encontrado, archivo vacío, ocupada, fallo al borrar) |
| 70, 71 | Bluetooth / WiFi no arrancan |
| 80, 81 | Ajuste fuera de rango o valor no válido |
| 90 | Fallo al enviar mensaje |
| 100, 101 | Fallo al guardar o leer un ajuste |
| 110 | Falló la autenticación |
| 120 | Otra interfaz ocupada |
| alarma 10 | Fallo del control del husillo (típicamente RS485) |

Fuera de esa lista, un foro cita `error:152` (configuración no válida) y `error:19` ("Single axis homing not allowed") en FluidNC [T]. Tabla completa: [?].

---

## 7. Comandos `$` y su seguridad durante un trabajo

### 7.1 Referencia

| Comando | Función | Notas |
|---|---|---|
| `$` | Ayuda `[HLP:...]` | |
| `$$` | Imprime ajustes | `$x=val` guarda un ajuste (EEPROM, persiste); algunos como `$100`-`$102` requieren soft reset para cargarse |
| `$#` | Imprime parámetros (offsets, TLO, PRB) | |
| `$G` | Estado del parser `[GC:...]` | |
| `$I` | Info de compilación; `$I=texto` fija la cadena de usuario (< 80 caracteres) | |
| `$N`, `$Nx=línea` | Ver o guardar líneas de arranque (N0 y N1) | Se ejecutan al encender o resetear, **no** tras alarma ni tras `$X`; con homing activado, tras un homing correcto. Evitar movimiento y comandos que escriben EEPROM (G10, G28.1, G30.1) ahí |
| `$C` | Alterna modo de verificación | Analiza sin mover, ignora pausas, apaga husillo y refrigerante. **Salir provoca un soft reset** |
| `$X` | Desbloquea alarma | Pensado para estado Alarm. Recomiendan moverse con pasos cortos G91 y luego homing o reset |
| `$H` | Homing | Es la única forma de hacerlo. Bloquea G-code con homing activado hasta que se haga |
| `$J=línea` | Jog | Ver sección 5 |
| `$SLP` | Sleep | Apaga husillo, refrigerante y habilitación de motores. Solo se sale con soft reset o apagado; luego entra en Alarm |
| `$RST=$` / `$RST=#` / `$RST=*` | Restaura ajustes / pone a cero offsets y G28/G30 / borra toda la EEPROM | GRBL se reinicia solo después. No aparecen en la ayuda. Los OEM pueden restringirlos |

Fuente: [D] G-Commands y [D] G-Config.

### 7.2 Cuáles son seguros durante un trabajo

La documentación oficial **no** da una tabla por comando. Lo que sí dice:

- `error:8`: "el comando `$` requiere estado Idle" [D] G-Interface. Su aplicación fina (qué `$` sí pasa en Run/Hold) no está documentada [?]. La regla prudente para el host es: **durante un trabajo, todo lo que no sea tiempo real se rechaza**; Tlaku ya lo hace (`stream_request` responde "Hay un trabajo en curso" a `Command`, `Jog` y `Start`).
- Solo tiempo real durante un trabajo: `?`, `!`, `~`, 0x18, 0x84, 0x85 (solo en Jog), overrides, 0x9E (solo en Hold), 0xA0/0xA1.
- `$J=` solo vale en Idle o Jog [D].
- `$X`: pensado para Alarm; no tiene sentido durante un trabajo.
- `$C`: no durante un trabajo; al salir hace soft reset [D].
- `$SLP`, `$RST=*`, `$RST=$`, `$N0=`: modifican estado persistente o desactivan la máquina. Nunca desde el flujo de G-code. Tlaku las trata como `other` (piden confirmación) en `commandSafety.ts`; está bien, aunque `$RST=*` y `$RST=$` merecen un nivel propio, "destructivo" (ver sección 10).
- Streaming de ajustes con conteo de caracteres: prohibido por riesgo de perder bytes [D] G-Interface.

### 7.3 Comportamiento en alarma

Con una alarma, el G-code queda bloqueado (`error:9`). El proyecto dice que sólo funciona un conjunto pequeño de comandos hasta `$X`, `$H` o reset; no enumera cuáles [D] G-Interface. En la práctica se necesitan `?`, `$$`, `$I`, `$X` y `$H` en alarma; Tlaku usa `$$` en alarma para leer `$22` (`preflight.rs`) y `$I` al conectar. Es previsible que funcionen, pero no está confirmado para todas las versiones [I].

---

## 8. Estados y transiciones

### 8.1 Estados y subestados [D] G-Interface

| Estado | Significado | Subestados |
|---|---|---|
| `Idle` | Reposo, listo | |
| `Run` | Ejecutando un ciclo | grblHAL: `Run:1` hold pendiente, `Run:2` sondeando [D] HAL-Report |
| `Hold:0` | Hold completo, listo para reanudar | |
| `Hold:1` | Hold en curso. **Un reset lanzará alarma** | |
| `Jog` | Ejecutando un jog | |
| `Alarm` | Bloqueado (bloquea G-code) | grblHAL: `Alarm:N` con el código [D] HAL-Report |
| `Door:0` | Puerta cerrada, lista para reanudar | |
| `Door:1` | Máquina detenida; puerta aún entreabierta; no se puede reanudar hasta cerrar | |
| `Door:2` | Puerta abierta; hold (o retracción de aparcamiento) en curso; un reset lanzará alarma | |
| `Door:3` | Puerta cerrada y reanudando (restaurando desde aparcamiento); un reset lanzará alarma | |
| `Check` | Modo `$C` | |
| `Home` | Ciclo de homing | |
| `Sleep` | Tras `$SLP` | |

grblHAL añade el estado `Tool` [D] HAL-Report.

### 8.2 Transiciones típicas

```
power-on / reset --(homing habilitado $22=1)--> Alarm   (hay que $H o $X) [I: inferido de $22 y de [MSG:'$H'|'$X' to unlock]]
                 --(sin homing)--------------> Idle
Idle --G-code con movimiento--> Run --fin del último bloque--> Idle
Run/Idle/Jog --'!'--> Hold:1 --(desacelera)--> Hold:0 --'~'--> Run (o Idle)
Run --0x84 o puerta--> Door:2 --> Door:1 --(cierra)--> Door:0 --'~'--> Door:3 --> Run
Idle --$J=--> Jog --fin--> Idle ;  Jog --0x85--> (flush) --> Idle
Idle/Alarm --$H--> Home --(ok)--> Idle ;  Home --0x18--> Alarm:6
Idle --$C--> Check --$C o 0x18--> (soft reset) Idle
Idle/Alarm --$SLP--> Sleep --0x18--> Alarm
Run --(límite duro)--> Alarm:1  (hace falta reset)
Run --(destino fuera del recorrido, límites blandos)--> hold + Alarm:2 (posición conservada)
Alarm --$X--> Idle  (con [MSG:Caution: Unlocked])
Alarm --$H correcto--> Idle
```

Reglas que importan al driver:

- **Soft reset durante Run (o durante la desaceleración de un hold, `Hold:1`)**: la máquina se detiene de golpe, la **posición se considera perdida** y GRBL lanza **ALARM:3** ("reset durante un movimiento; posición no garantizada; se recomienda homing"). Si el reset llega con la máquina **quieta**, la posición se conserva y no hay alarma [D] G-Commands, [D] G-Interface.
- Tras un soft reset, GRBL imprime el banner y vuelve a su estado inicial (el WCO se reenvía).
- El reset también limpia el planificador y el búfer RX: lo pendiente se pierde.
- `?` durante homing o tras una alarma crítica de límites puede no recibir respuesta [D].

---

## 9. grblHAL y FluidNC: lo que rompe suposiciones

### 9.1 grblHAL

Hecho [D] salvo indicación:

- Es una reescritura de GRBL 1.1 para procesadores de 32 bits con capa de abstracción de hardware (HAL) [D] HAL-Home. `COMPATIBILITY_LEVEL` en `config.h` devuelve el comportamiento de GRBL 1.1: nivel 1 desactiva extensiones (como el cambio manual de herramienta) y el banner vuelve a decir `Grbl 1.1`; nivel 2 además oculta los ajustes nuevos en `$$`; nivel 10 desactiva todas las extensiones [D] HAL-Changes, HAL-Compat.
- **Banner** `GrblHAL ...` en nivel 0.
- **Estado y subestados**: añade `Tool`; `Run:1`, `Run:2`; `Alarm:N`.
- **Campos nuevos en `<...>`**: `WCS:G54`, `Pn` con más letras (`O` sonda desconectada, `A B C U V W` límites, `E` paro, `L T Q M F`), `A:` con `T` (cambio de herramienta), `MPG:0|1`, `SD:...`, `H:0|1` (homing), `D:`, `Sc:`, `TLR:`, `FW:grblHAL`, `In:`, y `FS:f,s_programado,s_real` (**tres valores**) [D] HAL-Report. Los remitentes deben ignorar lo que no conozcan.
- **Comandos en tiempo real extra**: `0x80`/`0x81`/`0x82` como alternativas de `?`, `~`, `!` con el bit alto (evitan chocar con caracteres imprimibles en mensajes); `0x83` estado del parser; `0x87` reporte completo (incluye campos que normalmente salen solo al cambiar; sirve para detectar grblHAL: se envía y se espera ~250 ms respuesta); `0x19` parada sin warm reset; `0x88`, `0x89`, `0x8C`, `0x8A`, `0x8B`, `0xA2`, `0xA3`, `0xA4`. `RT+`/`RT-` en `[NEWOPT:...]` indica si hay soporte [D] HAL-Sender.
- **Terminadores**: CRLF y LFCR cuentan como uno; GRBL clásico los cuenta como dos (se recomienda un terminador de un solo carácter) [D] HAL-Sender. Tlaku manda `\n`: correcto en ambos.
- **Errores persistentes** hasta reset, línea vacía o comando `$` (nivel 0) [D] HAL-Changes, HAL-Compat. Con un streamer en modo conteo, tras el primer error llegarán más `error:N` iguales: el streamer debe tratarlos como el mismo fallo, no como errores independientes.
- **Homing**: informa el estado `Home` durante el homing y registra qué ejes están referenciados; si se exige homing al arrancar y falla, el mensaje de homing requerido se repite [D] HAL-Changes. Una alarma de bloqueo (códigos 1, 2, 10) solo responde a peticiones de reporte; sugerencia: soft reset [D] HAL-Sender.
- **Handshake** recomendado [D] HAL-Sender: esperar ~400 a 500 ms tras abrir; si llega un reporte en tiempo real, hay otro remitente (MPG) activo; si llega el banner, se acaba de resetear; enviar `0x87` y esperar ~250 ms: la respuesta indica un controlador extendido; `$I+` pide el reporte extendido de capacidades.
- **Ajustes**: muchas antiguas `#define` son ahora `$xx`, lo que puede romper remitentes que no lean `$$` dinámicamente [D] HAL-Changes. `$32` es modo (0/1/2), no booleano [D] HAL-Settings. Los controles por defecto son normalmente cerrados (NC): con la entrada de reset o paro abierta, la máquina entra en alarma al arrancar [D] HAL-Changes, [D] First-Run.
- **Búfer RX**: mayor que 128 en muchas plataformas, pero no hay cifra en la documentación leída [?]. Léelo del tercer campo de `[OPT:]` y confirma con una prueba de saturación.
- **Mensajes extra**: `[MSG:...]` (también se usa `Homing cycle required` al arrancar con homing obligatorio [D] HAL-Sender), `[GC:...]` empujado tras cambios si `$10` lo activa (bit 9) [D] HAL-Report/Settings. Los remitentes deben tolerar un `[GC:...]` sin haber enviado `$G`.
- **`$I`**: puede traer `[NEWOPT:...]` y `[OPT:...]` con número de herramientas; con nivel de compatibilidad alto se omiten [D] HAL-Compat.

### 9.2 FluidNC

Hecho [T] salvo indicación (la wiki oficial no pudo leerse):

- Firmware para ESP32 que mantiene compatibilidad con GRBL para el trabajo diario con un sender (envío de G-code y respuestas), pero la configuración va en un YAML, no en `$` [T].
- **Banner/`$I` real capturado** (FluidNC 3.9.7) [T] FNC-Forum:

```
Grbl 1.1f [$ for help]
[VER:3.9 FluidNC v3.9.7:]
[OPT:MPHS]
[MSG:Machine: TMC2209 IO Test]
```

  En versiones más recientes el banner es `Grbl 3.4 [FluidNC v3.4.3 (wifi) '$' for help]` con líneas `[MSG:INFO: ...]` antes.
- `[OPT:...]` no tiene el formato `letras,bloques,bytes` de GRBL (`MPHS` solo trae letras) [T]: **no sirve para leer el búfer RX**.
- Comandos solo en reposo: "se pueden usar solo en modo idle" para los `$` propios; los `$` de Grbl se soportan [T] FNC-Cmd. Incluye `$I`, `$H`, `$HX` (un eje), `$X`, `$C`, `$G`, `$#`, `$J`, `$$` (lista de ajustes), `$SLP`, `$RST=`, `$N`, `$E`/`$A`, `$T` (estado), `$Bye` (reinicia el procesador) y comandos de SD/WiFi (`$SD/Run`...).
- **Flujo**: se debe esperar el `ok` antes de enviar la línea siguiente; un movimiento largo puede devolver `ok` pronto porque FluidNC acumula varios comandos. `?` y Ctrl-X se atienden de inmediato, fuera del búfer de línea, y no devuelven `ok` [T] FNC-Term. Terminador recomendado: CR. Tamaño de búfer: [?].
- **Auto-reporte**: ajuste `$Report/Interval` hace que FluidNC emita `<...>` por su cuenta (cada N ms en movimiento o al cambiar de estado), y también empuja `[GC:...]` [T]. Un receptor que asuma "un `<...>` por cada `?`" fallará.
- `<...>`: FluidNC usa el reporte de estado de GRBL 1.1, `?` para consultar, `$10` 1 y 2 para MPos/WPos [T]. WCO aparece al cambiar y periódicamente [T].
- **Mensajes**: formato `[MSG:...]` (y en versiones recientes `[MSG:INFO:...]`, `[MSG:ERR:...]`, `[MSG:WARN:...]` [I]); en la captura de 3.9.7 hay una línea mal formada `MSG:Mode=STA:...` sin corchete inicial [T] FNC-Forum. **El receptor debe tolerar líneas desconocidas sin romperse** (Tlaku ya registra como `Message` todo lo no reconocido).
- Códigos: ver 6.4.
- Laser: ver 4.8.

### 9.3 grbl-Mega y ports de 32 bits

- grbl-Mega: mismo protocolo que 1.1, solo cambia la plataforma (Mega2560) [D] G-Mega y [I]; menciona `error:14` (línea de info/arranque > EEPROM) [D] G-Interface. Búfer RX: no documentado distinto [?].
- Grbl_ESP32: precursor de FluidNC; baudios 115200 8N1; algunos terminales reinician el ESP32 al abrir el puerto (señales RTS/DTR); PlatformIO suele hacerlo, Arduino IDE no [D] ESP-Serial.

---

## 10. Diferencias con Tlaku

Gravedad: **rompe** (el comportamiento documentado hace fallar al driver con una entrada legítima), **riesgo** (puede ocurrir en condiciones reales o es de seguridad), **menor** (cosmético o poco probable).

Referencias a líneas: `streamer.rs` = `crates/tlaku-grbl/src/streamer.rs`, `machine.rs` = `apps/desktop/src-tauri/src/machine.rs`, etc.

| # | Qué hacemos | Qué dice la documentación | Gravedad | Cambio sugerido |
|---|---|---|---|---|
| 1 | `handle_line` (streamer.rs:810) detecta el saludo con `line.len() >= 4 && line[..4].eq_ignore_ascii_case("grbl")`. `LineReader` convierte con `from_utf8_lossy` (serial.rs:156), y cada byte inválido se vuelve `U+FFFD` de 3 bytes. Verificado con un programa de prueba: una línea como `ab\xff\xfe` hace **panic** (índice 4 dentro de un carácter). | Los CH340 y los cables con ruido producen errores de transmisión (G-Issues); además se espera tolerar cualquier línea desconocida (HAL-Sender, ESP-MSG). | **rompe** (panic del hilo de envío: pierde el puerto y no se puede cancelar mientras la máquina sigue quemando) | Usar `line.as_bytes().get(..4).is_some_and(\|b\| b.eq_ignore_ascii_case(b"grbl"))` o `line.get(..4)`. Añadir prueba con bytes inválidos (vector L-3). |
| 2 | Con `ErrorPolicy::Stop` (valor por defecto), tras `error:N` el streamer termina con `LineError` y `machine.rs::finish` (líneas 978 a 999) solo registra el error y vuelve a `Idle`. No se manda `!` ni `0x18`. | Tras un error GRBL **sigue ejecutando todo lo que ya tiene en el búfer** y el host no puede pararlo; la línea con error se descarta, así que la S, F o M3/M4 que traía no se aplica (G-Interface, G-Issues). En grblHAL además repite el error en las líneas siguientes (HAL-Changes). | **riesgo** (seguridad: el láser puede seguir con un estado modal equivocado) | En `LineError`, cancelar como `Cancel`: `!`, esperar `Hold:0`, `0x18`. Avisar de que el trabajo quedó cortado. No ofrecer `Continue` para G-code real. |
| 3 | Cancelar = `!`, `sleep(50 ms)`, `0x18` (streamer.rs:727 a 731, 742 a 747). | `Hold:1` significa "hold en curso; un reset lanzará alarma". Un reset en movimiento da **ALARM:3** y posición perdida; con la máquina quieta la posición se conserva (G-Interface, G-Commands). Una desaceleración desde velocidades de grabado supera 50 ms con facilidad. | **riesgo** (cada cancelación a velocidad deja ALARM:3 y obliga a `$X`/homing; pierde la posición) | Tras `!`, sondear `?` hasta ver `Hold:0` (con plazo, p. ej. 2 s) y solo entonces enviar `0x18`; si vence el plazo, resetear igual. Vectores E-8, E-9 y Q-2. |
| 4 | `pump` (machine.rs:558) y el streamer (streamer.rs:708) declaran la conexión perdida si pasan 5 s sin recibir ningún byte. `$H` espera hasta 60 s (`home_timeout`). | `?` puede no contestarse **durante el homing** ni **tras una alarma crítica de límites** (G-Interface, G-Commands). Por tanto un `$H` de más de 5 s, o un `ALARM:1` seguido de silencio, se interpretaría como cable desconectado. | **rompe** (falso "se perdió la conexión" y cierre del trabajador; hay que confirmarlo en hardware: la doc es ambigua, porque también lista el estado `Home` en los reportes) | Suspender `silence_timeout` mientras haya un `$H` en curso (usar `home_timeout`) y tras recibir `ALARM:1`/`ALARM:N` crítico hasta que llegue un banner o un `ok`; dejar de sondear `?` en esos casos. |
| 5 | `Event::Alarm` termina el trabajo (`Outcome::Alarm`), pero la conexión sigue en `Idle` sin esperar el banner; `[MSG:Reset to continue]` solo se registra. | Tras límite duro/blando hace falta un reset (`[MSG:Reset to continue]`) (G-Interface). | **menor** | Marcar el estado de la UI como "necesita reset" al recibir ese mensaje; ofrecer el botón de reset. |
| 6 | `Realtime::SoftReset` en `Command::Realtime` y `reset_device` llaman a `clear_input` tras 50 ms; se pierde el banner `Grbl ...` y cualquier `ALARM:N` que lo acompañe. | El banner indica reinicio completo y la alarma explica la causa. | **menor** | Leer y registrar lo que llega tras el reset en lugar de descartarlo; así el usuario ve `ALARM:3` si ocurrió. |
| 7 | `parse_state` (streamer.rs:159) devuelve `None` para cualquier estado desconocido (p. ej. `Tool` de grblHAL), y entonces todo el reporte se trata como `Message`. | grblHAL añade el estado `Tool`; los remitentes deben ignorar lo desconocido (HAL-Report, HAL-Sender). | **riesgo** con grblHAL (la UI se queda congelada en el último estado) | Añadir `MachineState::Other(String)`; no descartar el reporte. |
| 8 | `FS:` solo se acepta con exactamente 2 valores (streamer.rs:220 a 225). | grblHAL envía `FS:f,s,real` con tres. | **menor** (se pierde el avance y la potencia en la UI) | Aceptar 2 o 3 y quedarse con los dos primeros (y guardar el tercero si se quiere). |
| 9 | `error_message` y `ALARM_CODES`/`ERROR_CODES` cubren solo 1 a 38 y 1 a 9 (streamer.rs:250 a 306, grblCodes.ts). | GRBL 1.1 define también `ALARM:10`; grblHAL define alarmas 10 a 22 (con 10 = EStop, que choca con el 10 de GRBL clásico) y errores 18, 19, 39 a 92; FluidNC define otros a partir de 39 y 60. | **menor** | Cuando el código no esté en la tabla, mostrar `error:N` con el texto "ver documentación del firmware" y, si el firmware es FluidNC, consultar `$E=N`/`$A=N`. Distinguir firmware (sección 9) antes de interpretar `ALARM:10`. |
| 10 | `alarm_advice` (preflight.rs) y `alarmAdvice` (grblCodes.ts) tratan `ALARM:10` como "otro": sugieren `$H`/`$X`. | En grblHAL `ALARM:10` es EStop: `$X` no sirve hasta soltar el botón; `ALARM:11` es "homing requerido". | **menor** | Casos propios para grblHAL: 10 -> "suelta el paro de emergencia y luego desbloquea"; 11 -> `$H`. |
| 11 | `firmware_name` (machine.rs:318) siempre antepone `Grbl` al valor de `[VER:...]`; tampoco lee `[OPT:...]`. | grblHAL y FluidNC tienen `[VER:...]` distintos y un banner propio (`GrblHAL`, `FluidNC`). `[OPT:letras,bloques,rx]` de GRBL trae el tamaño de RX; el de FluidNC (`[OPT:MPHS]`) no. | **menor** | Detectar la familia con banner + `[VER:]` (`FluidNC`, `GrblHAL`), extraer RX de `[OPT:]` solo cuando tenga 3 campos y proponer `rx_buffer = rx-1` en GRBL clásico (doc: declara 128, el script oficial usa 127). Para el resto, usar el perfil. |
| 12 | `rx_buffer` fijo en 127 por defecto en el perfil (`device.ts:25`) y en `StreamerConfig`. | 128 declarado, 127 usable en GRBL clásico (G-Interface, stream.py); grblHAL/FluidNC pueden diferir ([?]). | **menor** (127 es seguro para GRBL clásico; para FluidNC puede ser demasiado) | Permitir "modo prudente" (enviar y esperar `ok`, 1 línea) por perfil para FluidNC hasta medir su búfer. |
| 13 | El estado inicial por defecto de `Bf` es 15/127 (`empty_status`, machine.rs:416). | El ejemplo oficial es `Bf:15,128`; `Bf` está desactivado por defecto y no sirve para flujo. | **menor** | Mostrar `Bf` solo si el reporte lo trae; no usarlo para decidir nada. |
| 14 | `on_status` (machine.rs:508) convierte MPos a WPos con el último WCO conocido, que empieza en `[0,0]`. | WCO no llega en cada reporte: se envía cada 10 o 30 reportes y tras un reset; en la conexión (sin reset) puede tardar hasta 7,5 s a 250 ms. | **menor** (la posición de trabajo mostrada es incorrecta al principio) | Al conectar, pedir `$#` (o `$G`) para leer G54/G92 y calcular WCO; o activar `$10` para WPos si la máquina lo permite. |
| 15 | El saludo y el `[MSG:'$H'\|'$X' to unlock]` iniciales se descartan en `Grbl::open` (serial.rs:187 a 194, sleep de 2 s + `clear_input`). | El mensaje indica que arrancó en alarma y que no se ejecutan las líneas de arranque. | **menor** | Leer y registrar el arranque; usarlo para el diagnóstico. El `\r\n\r\n` inicial provoca dos `ok` extra que el `clear_input` elimina, correcto. |
| 16 | `exchange` (machine.rs:690 a 724) termina al primer `ok`, `error` o `ALARM`. | Un `ALARM:N` es un push asíncrono, no la respuesta del comando; la respuesta real (`ok` o `error:N`) puede llegar después. | **menor** | Tratar `ALARM:N` como evento y seguir esperando `ok`/`error:` hasta el plazo; o descartar el `ok` sobrante en la siguiente petición (ya se hace con `reader.clear()`). |
| 17 | `jog()` manda `$J=G91 G21 X.. Y.. F..` y `Cancel` sin trabajo manda `0x85`. | Correcto. Pero el wiki recomienda `G4P0` tras el cancel para saber cuándo termina, y sugiere no usar `!` para cancelar un jog. | **menor** | Tras `0x85`, enviar `G4P0` y esperar su `ok` antes de aceptar otro jog. |
| 18 | El generador no emite `G94` ni `G54`/`G17` (solo `G90` y `G21`, gcode.rs:213 a 214). Tampoco hay un reset antes del trabajo. | El jog no altera el parser, pero la consola sí puede (`G91`, `G20`, `G93`...). En G93 la F se interpreta como tiempo inverso. | **menor** | Añadir `G94` (y opcionalmente `G17 G54`) al preámbulo, o enviar `$G` en el preflight y rechazar si hay G91/G93/G20. |
| 19 | El preflight (preflight.rs) no lee `$32`; la interfaz solo advierte si `$32=0` (`ConnectionSection.tsx:112`) y no bloquea el envío. | Con `$32=0`, M3/M4 con S>0 se comportan como husillo normal: el láser queda **encendido durante los G0** y el movimiento se detiene en cada cambio de S. | **riesgo** (seguridad: quema entre trazos) | Leer `$32` con `$$` en el preflight (ya se usa `$$` para `$22`) y bloquear el trabajo cuando sea 0 (o pedir confirmación explícita). Con grblHAL `$32=2` es modo torno: también bloquear. |
| 20 | Para `PowerMode::Constant` se emite `M3 S0` y se pausa con `!`. | `!` no apaga husillo ni refrigerante (G-Commands) y M3 mantiene la potencia aunque no haya movimiento (G-Laser). El efecto exacto de `!` sobre el láser en modo láser no está documentado [?]. | **riesgo** (a verificar: posible quemadura al pausar en M3) | Verificar en hardware/emulador oficial. Si el láser queda encendido, tras `Hold:0` enviar `0x9E` (paro de husillo, solo en Hold) y volver a enviarlo antes de `~`. Documentar que M4 es más seguro en pausa. |
| 21 | El generador usa `G1 ... S0` para el overscan y los huecos, y `G0` para saltos. | Recomendación oficial: G0 (apaga el láser) o S0 en G1 (G-Laser). | Coincide | Ninguno. Dejar un vector de regresión. |
| 22 | `prepare_line` descarta comentarios y espacios y el streamer cuenta `len+1`. | Igual que el script oficial. | Coincide | Ninguno. |
| 23 | `prepare_line` no filtra G10, G28.1, G30.1 ni `$x=` dentro del trabajo, y el `start_gcode`/`end_gcode` del usuario se envía por conteo. | Las líneas que escriben EEPROM (G10, G28.1, G30.1, `$x=val`) pueden perder bytes si se envían con conteo; `$` durante Run da `error:8` (G-Issues, G-FAQ, G-Interface). | **menor** | Rechazar o avisar de esas líneas en `validate_gcode`; ejecutarlas por el camino de "enviar y esperar" antes de arrancar el trabajo. |
| 24 | Comentario de `MachineState::Door` (streamer.rs:89): "0 = cerrada y lista, 1 = abierta, 2 = reanudando, 3 = ya cerrada". | `Door:1` detenida con puerta entreabierta; `Door:2` hold o retracción en curso; `Door:3` puerta cerrada y reanudando (G-Interface). | **menor** (comentario) | Corregir el comentario. |
| 25 | `commandSafety.ts::classify` pone `$SLP`, `$RST=`, `$C` en `other`. | `$RST=*` borra toda la EEPROM; `$SLP` deshabilita la máquina hasta reset; `$C` hace reset al salir (G-Commands). | **menor** | Nueva clase `destructive` con aviso propio para `$RST`/`$SLP`/`$N`. |
| 26 | `LineTooLong` se calcula contra el tamaño del búfer RX (127). | `error:11` se produce con el máximo de caracteres por línea, valor que la doc leída no da [?]. | **menor** | Averiguar el límite (80 es lo que recuerdo del firmware; **no verificado en la documentación leída**) y comprobarlo en el preflight, con un máximo configurable. |
| 27 | La consola de la app pasa el texto a mayúsculas (`commandSafety.ts::parseCommand`) y quita espacios. | `$I=texto` guarda la cadena tal cual; los espacios se eliminan por el preprocesador de GRBL en G-code pero no en la cadena de `$I=` [I]. | **menor** | No modificar el argumento de `$I=`. |
| 28 | Sin soporte de overrides ni de puerta en la UI (solo en el streamer). | Disponibles en 1.1. | Informativo | Priorizar override de avance/potencia para trabajos largos. |

### 10.1 Cosas que ya están bien alineadas con la documentación

- Bytes de tiempo real (coinciden todos) y su exclusión del conteo.
- `\n` como único terminador; `len+1` por línea; 127 efectivos.
- `error:N` libera la línea más antigua (se cuenta igual que `ok`).
- `Hold:0`/`Hold:1`, `Door:0..3` se parsean con subcódigo opcional.
- El trabajo solo arranca en `Idle` (preflight) y no se envía nada más durante el trabajo salvo tiempo real.
- `?` a 4 a 5 Hz.
- `$C`, `Sleep`, `Alarm` y `Check`, con textos de recuperación correctos.
- Se tolera cualquier línea no reconocida (la registra como `Message`).
- Tlaku nunca ejecuta `$X`, `$H` ni `~` por su cuenta.

---

## 11. Vectores de prueba

Notación: `TX` = lo que envía Tlaku, `RX` = lo que responde la máquina simulada, `#` = comentario. `\r\n` se omite salvo que importe. Los bytes en hexadecimal van como `<0x..>`. La etiqueta [D]/[T]/[I] indica el origen del formato; las líneas [I] son construidas a partir de la descripción de campos y deben contrastarse con una máquina real.

### 11.1 Banners y arranque

```
V-1  GRBL 1.1h (Arduino Uno)                                              [D formato]
RX  Grbl 1.1h ['$' for help]

V-2  GRBL 1.1 con alarma al arrancar (homing activado, $22=1)             [D]
RX  Grbl 1.1h ['$' for help]
RX  [MSG:'$H'|'$X' to unlock]
TX  ?
RX  <Alarm|MPos:0.000,0.000,0.000|FS:0,0>

V-3  grblHAL                                                              [D: empieza por GrblHAL]
RX  GrblHAL 1.1f ['$' or '$HELP' for help]     # texto exacto del paréntesis: [?]
# Debe disparar Reset/banner igual que "Grbl".

V-4  FluidNC 3.9.7 (captura real)                                         [T]
RX  Grbl 1.1f [$ for help]
RX  [VER:3.9 FluidNC v3.9.7:]
RX  [OPT:MPHS]
RX  [MSG:Machine: TMC2209 IO Test]

V-5  FluidNC 3.4 (banner con corchete interno)                            [T]
RX  [MSG:INFO: FluidNC v3.4.3]
RX  Grbl 3.4 [FluidNC v3.4.3 (wifi) '$' for help]

V-5b Línea de arranque ejecutada                                          [D]
RX  >G54G20:ok
RX  >G54G20:error:20
RX  >:error:7
```

### 11.2 Reportes de estado

```
S-1  Reposo por defecto ($10=1)                                           [D]
RX  <Idle|MPos:0.000,0.000,0.000|FS:0.0,0>
# Esperado: Idle, mpos(0,0,0), feed 0, spindle 0.

S-2  Trabajando con Bf y FS ($10=3)                                       [D formato, valores I]
RX  <Run|MPos:12.500,3.250,0.000|Bf:14,120|FS:3000,500>
# Esperado: Run, buffer{planner 14, rx 120}, feed 3000, spindle 500.

S-3  Con WCO y Ov y A                                                     [D formato, valores I]
RX  <Run|MPos:1.000,2.000,0.000|FS:1500,255|WCO:10.000,20.000,0.000|Ov:100,100,100|A:S>
# WPos = MPos - WCO = (-9,-18). A:S = husillo CW (M3). Ov feed/rápido/husillo.

S-4  Con WPos ($10=0 o 2)                                                 [D]
RX  <Idle|WPos:3.000,4.000,0.000|FS:0,0|WCO:1.000,1.000,0.000>
# MPos derivada = WPos + WCO = (4,5).

S-5  Con número de línea y pines                                          [D]
RX  <Run|MPos:0.000,0.000,0.000|Ln:99999|FS:100,0|Pn:PXY>
# Ln=99999, pins="PXY".

S-6  Subestados                                                           [D]
RX  <Hold:0|MPos:0.000,0.000,0.000|FS:0,0>      # hold completo, se puede reanudar
RX  <Hold:1|MPos:5.200,0.000,0.000|FS:0,0>      # deteniéndose; un reset lanza alarma
RX  <Door:0|MPos:0.000,0.000,0.000|FS:0,0>      # cerrada, lista
RX  <Door:1|MPos:0.000,0.000,0.000|FS:0,0>      # detenida, puerta entreabierta
RX  <Door:2|MPos:0.000,0.000,0.000|FS:0,0>      # hold o retracción en curso
RX  <Door:3|MPos:0.000,0.000,0.000|FS:0,0>      # reanudando

S-7  Otros estados                                                        [D nombres]
RX  <Jog|MPos:10.000,0.000,0.000|FS:1000,0>
RX  <Home|MPos:0.000,0.000,0.000|FS:0,0>
RX  <Check|MPos:0.000,0.000,0.000|FS:0,0>
RX  <Sleep|MPos:0.000,0.000,0.000|FS:0,0>
RX  <Alarm|MPos:0.000,0.000,0.000|FS:0,0>

S-8  grblHAL con subestado de alarma y FS de tres valores                 [D HAL-Report, valores I]
RX  <Alarm:1|MPos:0.000,0.000,0.000|FS:0,0,0|Pn:X>
RX  <Run|MPos:1.000,1.000,0.000|FS:1500,255,240>
# Tlaku actual: FS de tres valores se descarta (diferencia #8). Esperado tras la corrección: feed 1500, spindle 255.

S-9  grblHAL con estado Tool y campos extra                               [D HAL-Report, valores I]
RX  <Tool|MPos:0.000,0.000,0.000|FS:0,0,0|WCS:G54|TLR:0|H:1>
# Esperado: no romper; estado "otro", campos desconocidos ignorados (diferencia #7).

S-10 Cuatro ejes / dos ejes                                               [I]
RX  <Idle|MPos:1.000,2.000,3.000,4.000|FS:0,0>      # extra=[4]
RX  <Idle|MPos:1.500,2.500|FS:0,0>                   # z=0

S-11 Campo malformado dentro de un reporte válido                         [I]
RX  <Run|MPos:abc|Zz:1|Bf:1|FS:5,6>
# Esperado: Run, mpos None, buffer None, FS (5,6). (ya cubierto en streamer.rs::status_edge_cases)

S-12 FluidNC con WCO y varios campos                                      [T formato GRBL]
RX  <Idle|MPos:0.000,0.000,0.000|FS:0,0|WCO:0.000,0.000,0.000>   # forma habitual de GRBL 1.1; campos extra de FluidNC: [?]
```

### 11.3 Mensajes `[MSG:...]` y resto de push

```
M-1  Todos deben registrarse como Message sin afectar al conteo           [D]
RX  [MSG:Reset to continue]
RX  [MSG:'$H'|'$X' to unlock]
RX  [MSG:Caution: Unlocked]
RX  [MSG:Enabled]
RX  [MSG:Disabled]
RX  [MSG:Check Door]
RX  [MSG:Check Limits]
RX  [MSG:Pgm End]
RX  [MSG:Restoring defaults]
RX  [MSG:Sleeping]

M-2  Ruido y mensajes inesperados                                         [T/I]
RX  MSG:Mode=STA:SSID=Casa:Status=Connected:IP=192.168.4.56:MAC=2C-BC-BB-28-2C-58]
RX  [GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0 S0]    # push no solicitado (grblHAL/FluidNC)
RX  [echo:G1X1F100]
```

### 11.4 Errores y alarmas

```
E-1  Error de G-code en medio de un trabajo (conteo de caracteres)
TX  G90\nG21\nM4S0\nG1X10F1000S500\nXYZZY\n...
RX  ok
RX  ok
RX  ok
RX  ok
RX  error:20
# Esperado: Event::Error{code 20, source_line=5, text="XYZZY"}, y con política Stop
# Tlaku debe además enviar '!' y luego 0x18 (diferencia #2). La máquina simulada sigue
# contestando "ok" a las líneas ya en búfer hasta que llegue 0x18.            [D reserva]

E-2  Comando $ durante un trabajo
TX  $$\n          # con la máquina en Run
RX  error:8                                                                [D]

E-3  G-code con alarma activa
TX  G0X1\n
RX  error:9                                                                [D]

E-4  Jog fuera de recorrido (límites blandos)
TX  $J=G91 G21 X999 F1000\n
RX  error:15                                                               [D]

E-5  Jog sin '='  o con G-code prohibido
TX  $JG91X1F100\n
RX  error:16                                                               [D]
TX  $J=G1 X1 F100\n         # G1 no se admite dentro del jog
RX  error:16                                                               [D: "contiene G-code prohibido"]

E-6  Jog sin F        # código exacto no documentado; probable error:22      [I]
TX  $J=G91 G21 X1\n
RX  error:22

E-7  Alarma de límite duro en pleno movimiento
RX  ALARM:1
RX  [MSG:Reset to continue]
# Esperado: Outcome::Alarm(1). A partir de aquí la máquina deja de responder a '?'
# hasta recibir 0x18 (diferencia #4).                                       [D]

E-8  Reset durante movimiento (cancelar sin esperar Hold:0)
TX  !
RX  <Hold:1|MPos:10.000,0.000,0.000|FS:0,0>
TX  <0x18>
RX  ALARM:3                                   # el orden respecto al banner no está documentado
RX  Grbl 1.1h ['$' for help]
TX  ?
RX  <Alarm|MPos:...>

E-9  Reset con la máquina quieta (hold completo)
TX  !
RX  <Hold:1|...>
RX  <Hold:0|...>
TX  <0x18>
RX  Grbl 1.1h ['$' for help]
TX  ?
RX  <Idle|...>                                # sin alarma, posición conservada     [D]

E-10 Homing con y sin éxito                                                   [D]
TX  $H\n
RX  <Home|...>            # (si responde a '?')
RX  ok
# fallo:
TX  $H\n
RX  ALARM:9
# fallo por reset:  RX ALARM:6    # fallo por puerta: ALARM:7   # pull-off: ALARM:8

E-11 grblHAL: error persistente                                               [D HAL-Compat]
TX  G1X1F100\n      RX error:20
TX  G1X2\n          RX error:20       # mismo error repetido en el buffer
TX  \n              RX ok             # línea vacía lo limpia
```

Cobertura de códigos: la lista completa de las secciones 6.1 y 6.2 debe generar `Event::Error`/`Event::Alarm` con su texto. Las pruebas pueden iterar por la tabla y comprobar que `error_message(N)` no devuelve "error desconocido" para N en 1 a 17 y 20 a 38, ni `alarm_message(N)` para N en 1 a 9, y que para N=10 (alarma) y N>38 (error) se obtiene un texto genérico que contiene el número.

### 11.5 `$I`, `$$`, `$G`, `$#`, `$N`, `$`

```
I-1  $I en GRBL 1.1h                                                      [D formato]
TX  $I\n
RX  [VER:1.1h.20190825:]
RX  [OPT:V,15,128]
RX  ok
# firmware_name -> "Grbl 1.1h"; OPT: planner 15, rx 128 (usable 127).

I-2  $I con cadena de usuario y variante con ruta larga                    [D]
RX  [VER:1.1f.20170801:Ejemplo]        # cadena de usuario inventada para la prueba
RX  [OPT:VL,16,128]
RX  ok

I-3  $I en FluidNC                                                         [T]
TX  $I\n
RX  [VER:3.9 FluidNC v3.9.7:]
RX  [OPT:MPHS]
RX  ok
# OPT no tiene 3 campos: no usarlo para el búfer.

I-4  $I en grblHAL                                                         [I]
RX  [VER:1.1f.20240101:]
RX  [OPT:VNM,35,1024,0]      # forma y valores: [?]; el parser debe tolerar campos extra
RX  [NEWOPT:ENUMS,RT+]       # [NEWOPT] y RT+ existen según HAL-Sender; contenido exacto [?]
RX  ok

P-1  $$ (ajustes típicos de un láser de diodo)                            [D nombres de ajuste, valores I]
TX  $$\n
RX  $0=10
RX  $1=25
RX  $2=0
RX  $3=0
RX  $4=0
RX  $5=0
RX  $6=0
RX  $10=1
RX  $11=0.010
RX  $12=0.002
RX  $13=0
RX  $20=0
RX  $21=0
RX  $22=0
RX  $23=0
RX  $24=25.000
RX  $25=500.000
RX  $26=250
RX  $27=1.000
RX  $30=1000
RX  $31=0
RX  $32=1
RX  $100=80.000
RX  $101=80.000
RX  $102=250.000
RX  $110=6000.000
RX  $111=6000.000
RX  $112=500.000
RX  $120=1000.000
RX  $121=1000.000
RX  $122=10.000
RX  $130=400.000
RX  $131=400.000
RX  $132=200.000
RX  ok
# proposeFromSettings: s_max=1000, laserMode=true, bed=(400,400).
# Variante laserMode=false: $32=0 -> la interfaz advierte (y debería bloquear el envío, diferencia #19).
# Variante grblHAL: "$32=2" -> modo torno; "$33=5000" y "$34..$36" presentes.

G-1  $G                                                                   [D]
TX  $G\n
RX  [GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0.0 S0]
RX  ok

G-2  $G tras M4 S500 y G1 F1000                                           [I]
RX  [GC:G1 G54 G17 G21 G90 G94 M4 M9 T0 F1000 S500]
RX  ok

H-1  $#                                                                   [D formato]
TX  $#\n
RX  [G54:0.000,0.000,0.000]
RX  [G55:0.000,0.000,0.000]
RX  [G56:0.000,0.000,0.000]
RX  [G57:0.000,0.000,0.000]
RX  [G58:0.000,0.000,0.000]
RX  [G59:0.000,0.000,0.000]
RX  [G28:0.000,0.000,0.000]
RX  [G30:0.000,0.000,0.000]
RX  [G92:0.000,0.000,0.000]
RX  [TLO:0.000]
RX  [PRB:0.000,0.000,0.000:0]
RX  ok

N-1  $N                                                                   [D]
TX  $N\n
RX  $N0=
RX  $N1=
RX  ok

U-1  $ (ayuda)                                                            [D]
TX  $\n
RX  [HLP:$$ $# $G $I $N $x=val $Nx=line $J=line $C $X $H ~ ! ? ctrl-x]
RX  ok

X-1  $X en alarma                                                         [D]
TX  $X\n
RX  [MSG:Caution: Unlocked]
RX  ok

C-1  $C y salida                                                          [D]
TX  $C\n
RX  [MSG:Enabled]
RX  ok
TX  $C\n
RX  [MSG:Disabled]          # hace soft reset automático -> banner
RX  Grbl 1.1h ['$' for help]
```

### 11.6 Flujo de caracteres (emulador)

Reglas que debe imponer el emulador con tamaño de búfer RX = 128 (127 usables para el host, igual que el script oficial):

1. Un hilo de recepción cuenta los bytes que le han llegado y que aún no ha respondido. Si llegan más de 128 bytes sin consumir, descarta el exceso (así se detecta un host que no respeta el conteo).
2. Para cada línea completa (`\n`) responde `ok` o `error:N` en orden FIFO.
3. Los bytes `? ! ~ 0x18 0x84 0x85 0x90..0x9E 0xA0 0xA1` se extraen del flujo en cuanto llegan, no cuentan, y `?` responde al instante con un `<...>`.
4. Una línea vacía responde `ok`.
5. Un `$` mientras el estado es `Run` o `Hold` responde `error:8`.
6. Una línea con G-code mientras el estado es `Alarm` o `Jog` responde `error:9`.

Escenario del ejemplo oficial (longitudes incluyen `\n`): líneas de 25, 40, 31, 58 y 20 bytes; el host debe enviar las tres primeras (96 bytes), esperar al primer `ok` (quedan 71 pendientes; 71 + 58 = 129 > 127, así que no envía), enviar la cuarta solo tras el segundo `ok` (quedan 31; 31 + 58 = 89) y la quinta justo después (89 + 20 = 109). Con esas cifras se puede verificar que `buffer_used` nunca supera 127 y que cada `ok` reduce el pendiente exactamente en la longitud de la línea más antigua.

Variante de larga duración: 2000 líneas de 20 a 60 bytes con `ok` atrasados aleatoriamente; invariante: `Σ pendientes <= 127` en todo momento.

### 11.7 Modo láser (para un emulador que modele potencia)

Modelo mínimo para asertar la semántica oficial [D]. El estado del láser `on` depende del modo de husillo (`M3`/`M4`/`M5`), la S modal, el modo de movimiento (`G0`/`G1`) y si hay movimiento real.

| Entrada (con `$32=1`) | Resultado esperado |
|---|---|
| `M4 S500` (sin mover) | láser **apagado** (M4 sin movimiento) |
| `M4 S500` + `G1 X10 F1000` | encendido durante el movimiento; potencia escalada con la velocidad real |
| `M3 S500` (sin mover) | láser **encendido** (potencia constante) |
| `M3 S500` y luego `G0 X10` | apagado durante el G0 |
| `G0 M3 S1000` | no enciende; fija M3 y S1000 modales; el siguiente `G1 X5` enciende con S1000 |
| `M4 S500`, `G1 X10 S0`, `G1 X20 S800` | sin parada entre los dos G1; el primero con láser apagado |
| `M3 S500`, `S800` (sin movimiento) | parada forzada (M3 sin movimiento) |
| `M4 S500`, `S800` (sin movimiento) | sin parada |
| `M3` -> `M4` o `M5` | parada del planificador (cambio de estado del husillo) |
| `M5` | láser apagado |
| `$32=0` + `M3 S500` + `G0 X10` | láser **encendido** durante el rápido (modo fresadora): debe disparar el bloqueo del preflight de Tlaku (diferencia #19) |

Salida esperada del generador de Tlaku para un raster pequeño (regresión; ya cubierta en `gcode.rs::raster_line_omits_unchanged_y_and_varies_s`):

```
; Generado por Tlaku
G90
G21
M4 S0
; Raster
G0 X10 Y5
G1 X12.3 S500 F6000
G1 X15
G1 X20 S0
G1 X25 S1000
G0 Y5.1
G1 X10 S250
M5
G0 X0 Y0
```

Se verifica: el hueco de potencia cero va como `G1 ... S0` (no `G0`); `F` y `S` modales solo cuando cambian; M5 al final.

### 11.8 Pausa, reanudación, cancelación (secuencias de emulador)

```
Q-1  Pausa y reanudación                                                  [D]
TX  !
RX  <Hold:1|...>   RX <Hold:0|...>
TX  ~
RX  <Run|...>

Q-2  Cancelación correcta (propuesta)                                     [D + sugerencia]
TX  !                         # t0
RX  <Hold:1|...>
RX  <Hold:0|...>              # el host espera este estado (plazo máx. ~2 s)
TX  <0x18>
RX  Grbl 1.1h ['$' for help]
TX  ?
RX  <Idle|...>                # sin ALARM:3

Q-3  Cancelación actual de Tlaku (50 ms)                                  [D]
TX  !
TX  <0x18>  (a los 50 ms, con el hold aún en curso)
RX  ALARM:3 + banner
# Expectativa del emulador: tras reset en Hold:1 el estado es Alarm y la posición se declara perdida.

Q-4  Banner a mitad de trabajo                                            [D]
RX  Grbl 1.1h ['$' for help]
# Esperado: Outcome::Reset (el trabajo queda incompleto).
```

### 11.9 Robustez del lector de líneas

```
L-1  Terminadores:  "ok\r\n", "ok\n", "ok\r", "ok\n\r"    -> una línea "ok" cada vez, sin líneas vacías.
L-2  Fragmentos:    "<Idle|MPo" + "s:0,0,0>\r\n"           -> un único reporte.
L-3  Bytes inválidos: b"ab\xff\xfe\r\nok\r\n"              -> NO panic; luego "ok".   (diferencia #1)
L-4  Línea larguísima sin fin: 70 000 bytes 'x' + "ok\n"    -> descarta basura; luego "ok". (ya cubierto)
L-5  Respuesta con espacios: "  ok  "                         -> "ok".
L-6  grblHAL: "ok" repetidos por línea vacía                   -> tolerados.
```

---

## 12. Lista de comprobaciones antes de dar el driver por validado

1. Corregir #1 (panic) y #4 (silencio durante `$H` y alarmas críticas): son los que pueden romper una sesión con una entrada legítima.
2. Corregir #2, #3 y #19 (seguridad del láser).
3. Verificar #20 (qué hace `!` con M3 en modo láser) en una máquina real o en un emulador que reproduzca el firmware oficial.
4. Confirmar con hardware real: respuesta a `?` durante `$H`, orden de `ALARM:3` y banner tras un reset en movimiento, y el límite de caracteres por línea.
5. Añadir las tablas de grblHAL y la detección de firmware (#9, #10, #11).
6. Integrar los vectores de la sección 11 en `crates/tlaku-grbl/tests/` y en el `Fake` de `machine/tests.rs` (hoy ese emulador solo contesta `$I`, `$$` y los reportes en Idle).
