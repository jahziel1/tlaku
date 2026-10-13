# Dialectos de G-code para láser: Marlin, Smoothieware, grblHAL, FluidNC, GRBL-LPC y GRBL-M3

Estado: investigación de sala limpia (solo documentación, foros y artículos; no se leyó ni copió código fuente de ningún firmware).
Fecha de consulta: 2026-10-13. GRBL 1.1 puro lo cubre `docs/hardware/grbl.md`; aquí solo lo que **difiere**.

Convención de confianza en cada dato:

- **[DOC]** lo dice la documentación oficial del firmware (URL citada).
- **[LB]** lo dice la documentación de LightBurn (fuente de comportamiento observable, no del firmware).
- **[FORO]** informe de un usuario o desarrollador en un foro; útil pero anecdótico.
- **[CALC]** cálculo propio a partir de datos citados.
- **[VERIFICAR]** no se pudo confirmar en una fuente leída; hay que probarlo en hardware antes de fiarse.

Los firmwares G-code que LightBurn soporta (GRBL, GRBL-M3, Marlin, Smoothieware y "Custom GCode") salen de la página de ajustes de dispositivo de LightBurn [LB: https://docs.lightburnsoftware.com/UI/DeviceSettings.html]. Ruida, Trocen, TopWisdom y galvos no son G-code y quedan fuera.

---

## 1. Resumen ejecutivo

1. Todos estos firmwares hablan el mismo G-code de movimiento (`G0/G1`, `G90`, `G21`, `F` en mm/min). Lo que cambia es **cómo se enciende el láser y cómo se escala `S`**, **cómo se controla el flujo de la línea serie** y **cómo se pausa, consulta y cancela**.
2. El único dialecto con flujo por conteo de caracteres, `?` en tiempo real y `!`/`~` es la familia GRBL (GRBL 1.1, grblHAL, FluidNC, GRBL-LPC). Marlin y Smoothie son "ping-pong" al estilo RepRap: no hay `?` ni `!` (salvo `?` en Smoothie).
3. Tres trampas que rompen trabajos y que el generador debe evitar:
   - **Marlin sin inline power** frena el planificador en cada cambio de `M3/M4 S` (vale para rasterizado: manchas y vibración).
   - **Smoothie y G0**: apaga el láser en G0, pero hay placas que "queman" en el sobrescaneo si se hace con G1 S0 (LightBurn lo resuelve con "Use G0 moves for overscan").
   - **GRBL ≤1.1e y GRBL-M3**: no hay modo láser (`$32`), así que G0 no apaga el rayo y cada cambio de `S` es una pausa.
4. La propuesta (sección 8) es una estructura `Dialect` con dos mitades, `GcodeStyle` (cómo se escribe) y `LinkProfile` (cómo se transmite), de modo que el generador actual (`crates/tlaku-grbl/src/gcode.rs`) y el streamer (`streamer.rs`) sirvan a todos cambiando parámetros. Hacen falta unos 35 parámetros; la lista exacta está en la sección 8.2.

---

## 2. Marlin 2.x con función láser

### 2.1 Encendido y potencia

| Tema | Dato | Fuente |
|---|---|---|
| Activación | Las funciones de láser se activan con `LASER_FEATURE` en `Configuration_adv.h`. Sin PWM (`SPINDLE_LASER_USE_PWM`), cualquier valor no cero enciende al máximo y cero apaga. | [DOC] https://marlinfw.org/docs/configuration/2.0.9/laser_spindle.html |
| Sintaxis | `M3 [I<modo>] [O<potencia>] [S<potencia>]`; igual para `M4`. `O` es PWM 0-255 siempre; `S` está en la unidad que fije `CUTTER_POWER_UNIT`. | [DOC] https://marlinfw.org/docs/gcode/M003.html y https://marlinfw.org/docs/gcode/M004.html |
| Unidad de `S` | `CUTTER_POWER_UNIT`: PWM (por defecto, 0-255), porcentaje (0-100) o RPM. Ejemplos de la doc: `M3 S50` = 50 % con unidad PERCENT; `M3 O128` = 50 % en PWM; `M3 S204 I` = 80 % en PWM. | [DOC] M003 |
| Sin parámetros | `M3` solo enciende a la potencia `SPEED_POWER_STARTUP`. | [DOC] M003 |
| Modo estándar | `M3/M4/M5` **sincronizan el planificador** (parada momentánea) antes de cambiar la potencia. | [DOC] laser_spindle 2.0.9 |
| Modo inline continuo | `M3 I` entra en "Continuous Inline Mode". La `S` se aplica **en el siguiente** `G1/G2/G3/G5`, no de inmediato; el valor viaja en el bloque del planificador, así que potencia y movimiento van sincronizados. Ejemplo oficial: `G1 X10 Y10 S100`. | [DOC] laser_spindle 2.0.9 |
| Modo inline dinámico | `M4 I`: escala la potencia con la velocidad del paso a paso. La propia doc lo llama **experimental**. `M5 I` vuelve al modo estándar. | [DOC] laser_spindle 2.0.9 |
| `G1 ... S` | Desde Marlin 2.1.1 el parámetro `S` existe en `G0/G1` bajo `LASER_FEATURE`: "Laser power for the move". La página no da su rango. | [DOC] https://marlinfw.org/docs/gcode/G000-G001.html |
| `LASER_POWER_INLINE` / `LASER_MOVE_POWER` | Con ambos activados, `M3/M4` no hacen falta: la potencia va dentro de los `G1..G5`. Versiones recientes parecen haber quitado los `#define` de inline (siempre activo con láser); los foros no se ponen de acuerdo. | [DOC] M003/M004; [FORO] https://forum.v1e.com/t/marlin-515-not-turning-on-the-laser/34782 [VERIFICAR] |
| `LASER_POWER_SYNC` | Permite que los `M3/M4 S` se encolen y los procese la ISR sin pausar. | [DOC] laser_spindle 2.0.9 |
| `LASER_POWER_TRAP` | Ajusta la potencia durante aceleración/frenado para evitar puntos calientes; en CO2 puede requerir `SPEED_POWER_MIN` para no caer bajo el umbral de disparo. | [DOC] laser_spindle 2.0.9 |
| Apagado | `M5` espera a que acaben los movimientos y apaga láser y PWM. **Los `G0` y `G28` también apagan el láser**, no hace falta `M5` antes de ellos. | [DOC] https://marlinfw.org/docs/gcode/M005.html |
| Láser en pin de ventilador | `LASER_SYNCHRONOUS_M106_M107`: la potencia sale por `M106 P1 S128` (≈50 %) / `M107`. La doc dice que inline es más rápido para rasterizado. | [DOC] laser_spindle 2.0.9; `M106`: `S` 0-255, `P` = índice de ventilador, https://marlinfw.org/docs/gcode/M106.html |
| Máquinas viejas | La práctica habitual de LightBurn en Marlin es láser en salida de ventilador PWM con `M106 Sxxx`/`M107`, o `M03/M05` con velocidad de husillo; el dispositivo permite elegir sabor y el índice de herramienta. | [FORO] LightBurn, https://forum.lightburnsoftware.com/t/m106-m107-g-codes-for-laser-inline-control-and-air-assist-setting/174900 |
| Prusa | Su firmware sube el ventilador al 100 % un instante tras `M106` si estaba parado; el apagado seguro es `M106 S1`, no `M107`. | [FORO] https://forum.lightburnsoftware.com/t/prusa-specific-requirements-on-m106-m107-and-incorrect-m114-reading/176841 |

Resumen: hay **tres sabores** de Marlin que el generador debe poder emitir:

1. **marlin-inline** (recomendado): `M3 S0 I` al inicio y `G1 ... S<n>` con la potencia dentro del movimiento.
2. **marlin-standard**: `M3 S<n>` en línea aparte; funciona, pero cada cambio de `S` frena. Aceptable para corte vectorial a potencia constante; no para imagen.
3. **marlin-m106** (heredado): `M106 S<n>`/`M107`, y como G0 **no** apaga en este modo hay que apagar explícitamente antes de cada desplazamiento.

El perfil "Marlin" de LightBurn emite `M03 S<n>` en línea aparte antes de cada `G1` (hilo [FORO]: https://forum.lightburnsoftware.com/t/laser-power-inline/15493) y es la causa del tirón en el grabado; la solución que dan los usuarios es el modo inline [FORO]: https://forum.lightburnsoftware.com/t/laser-fahrt-bei-der-gravur-nicht-durch-sondern-bremst-immer-wieder-laser-does-not-drive-through-during-engraving-but-brakes-again-and-again/61231. El perfil "Snapmaker Marlin" no emite `M3/M4`: pone la potencia con `I` en el `G1` (p. ej. `G1 Y153 F6000 I S255`) y deja `M106/M107` ligado al checkbox de aire [FORO] https://forum.lightburnsoftware.com/t/m106-m107-g-codes-for-laser-inline-control-and-air-assist-setting/174900.

### 2.2 Protocolo

| Tema | Dato | Fuente |
|---|---|---|
| `ok` | Un `ok` por línea. Las órdenes de movimiento (G0-G3, G28-G32) se confirman al entrar en el búfer circular de comandos; si está lleno, el `ok` se retrasa y eso es el control de flujo. Otras órdenes G/M/T se confirman solo cuando el búfer se vacía y se ejecutan (el host espera). | [DOC-comunidad] https://reprap.org/wiki/G-code |
| Modelo de flujo | **No hay conteo de caracteres.** Es ping-pong con la cola del firmware como colchón: tras cada `ok` se manda la siguiente línea. | derivado de lo anterior |
| Tamaño de búfer | `BUFSIZE` por defecto 4 comandos y `MAX_CMD_SIZE` 96 bytes por línea (valores leídos en un diff de un mirror de Marlin; el nombre y los valores pueden cambiar entre versiones). `RX_BUFFER_SIZE`: 32 bytes bastan sin XON/XOFF. | [FORO/mirror] https://git.xythobuz.de/thomas/marlin/commit/526ad82d20608d5262b5333684bcf7e85d18e431 [VERIFICAR contra la doc oficial] |
| Baudios | Por defecto 115200; la doc recomienda **250000** y bajar solo si aparecen errores de "line number" o "checksum". Valores permitidos: 2400…250000; `M575` permite cambiarlos hasta 1 000 000 (`BAUD_RATE_GCODE`). LightBurn usa 250000 por defecto en Marlin. | [DOC] https://marlinfw.org/docs/configuration/configuration.html, https://marlinfw.org/docs/setting/serial.html, https://marlinfw.org/docs/gcode/M575.html; [LB] DeviceSettings |
| Consulta de posición | `M114` (opciones `D`, `E`, `R`; `R` = posición real, requiere `M114_REALTIME`). Respuesta: `X:0.00 Y:127.00 Z:145.00 E:0.00 Count X: 0 Y:10160 Z:116000` y luego `ok`. Sin `R` es la posición *proyectada* (fin de la cola). | [DOC] https://marlinfw.org/docs/gcode/M114.html |
| Autoinforme | `M154 S<seg>` envía la posición solo; la doc dice que es preferible a sondear con `M114`, que "podía causar tirones". Se anuncia con `EXTENDED_CAPABILITIES_REPORT`. | [DOC] https://marlinfw.org/docs/gcode/M154.html |
| Identificación | `M115` devuelve `FIRMWARE_NAME:Marlin 2.x.x (...)`, `SOURCE_CODE_URL`, `PROTOCOL_VERSION`, `MACHINE_TYPE`, etc.; con `EXTENDED_CAPABILITIES_REPORT` añade líneas `Cap:NOMBRE:0/1` (p. ej. `EMERGENCY_PARSER`, `AUTOREPORT_POS`). | [DOC] https://marlinfw.org/docs/gcode/M115.html |
| Espera de fin | `M400` pausa el procesamiento hasta que acaben todos los movimientos del planificador; su `ok` llega entonces. | [DOC] https://marlinfw.org/docs/gcode/M400.html |
| Pausa | `M0 [P<ms>] [S<s>] [msg]` ("M1" es alias obsoleto): pausa tras el último movimiento. **Sin LCD y sin `EMERGENCY_PARSER` se ignora.** Es una orden *encolada*, no actúa antes de las líneas ya enviadas. | [DOC] https://marlinfw.org/docs/gcode/M000-M001.html |
| Reanudar | `M108` rompe una espera (la de `M0` incluida) como el botón del LCD; con `EMERGENCY_PARSER` se procesa aunque la cola esté llena. | [DOC] https://marlinfw.org/docs/gcode/M108.html |
| Cancelar | `M410` parada rápida: detiene los motores **sin deceleración** (se pierde posición); sin `EMERGENCY_PARSER` se retrasa. `M112` parada total (hay que reiniciar); con `EMERGENCY_PARSER` actúa al instante. `M999` reinicia tras un STOP y por defecto vacía el búfer pidiendo reenvío desde el último `N`. | [DOC] https://marlinfw.org/docs/gcode/M410.html, https://marlinfw.org/docs/gcode/M112.html, https://marlinfw.org/docs/gcode/M999.html |
| Líneas numeradas | `N<n>` al inicio y `*<checksum>` al final, **ambos o ninguno**. Checksum = XOR de los bytes de la línea (incluido `N…`) hasta antes de `*`, en decimal (el ejemplo de la wiki RepRap es `*71`). Si falla, el firmware pide reenvío. `M110 N<n>` fija el número de línea. | [DOC-comunidad] https://reprap.org/wiki/G-code ; [DOC] https://marlinfw.org/docs/gcode/M110.html |

Consecuencia para Tlaku: **no se puede reutilizar** el esquema "`?` cada 200 ms + `!`/`~`" del streamer actual. En Marlin el progreso sale del conteo de `ok`; la pausa es "dejar de enviar" (y, si hay `EMERGENCY_PARSER`, mandar `M0`/`M108`); y la cancelación es `M410` + `M5` (hay que apagar el láser a mano porque no hay reset en caliente).

### 2.3 Unidades, posición, homing

- `F` es **mm/min** en `G0/G1` ("current units per minute") [DOC: https://marlinfw.org/docs/gcode/G000-G001.html]. Los topes de velocidad se configuran en unidades **por segundo** (`M203 X100 Y100` = 100 unidades/s) [DOC: https://marlinfw.org/docs/gcode/M203.html]; si `F` pide más, Marlin recorta al máximo configurado sin avisar [CALC/VERIFICAR]. No hay que convertir nada en el G-code, pero conviene avisar al usuario si `F/60 > M203`.
- `G0` y `G1` se tratan igual (feedrate común) [LB: DeviceSettings, "Fast Whitespace Scan"]. La doc recomienda seguir usando `G0` para movimientos sin impresión [DOC: G000-G001]. Esto obliga a emitir `F` también en el `G0` (ver sección 8).
- `G21` mm / `G20` pulgadas (`INCH_MODE_SUPPORT`) [DOC: https://marlinfw.org/docs/gcode/G020.html]. `G90/G91` absoluto/relativo.
- Homing: `G28 [X] [Y] [Z]`; sin parámetros, todos los ejes [DOC: https://marlinfw.org/docs/gcode/G028.html]. Muchas máquinas láser Marlin no tienen finales de carrera.
- `G92 X0 Y0` fija la posición actual sin mover [DOC: https://marlinfw.org/docs/gcode/G092.html]. Con `CNC_COORDINATE_SYSTEMS` existen `G54-G59`/`G53` y `G92.1`. Origen de usuario = `G92` (no hace falta `G54`; el proyecto ya evita `G54..G59`).

### 2.4 Aire y salidas

- `AIR_ASSIST` habilita `M8`/`M9` (pin `AIR_ASSIST_PIN`, nivel `AIR_ASSIST_ACTIVE`); `AIR_EVACUATION` habilita `M10`/`M11`; `COOLANT_CONTROL` habilita `M7`/`M8`/`M9` (`COOLANT_MIST`, `COOLANT_FLOOD`, con `_INVERT`) [DOC: laser_spindle 2.0.9].
- Alternativas reales: `M106 P<n> S255`/`M107 P<n>` (ventilador), `M42 P<pin> S<0-255>` (pin digital/PWM) [DOC: https://marlinfw.org/docs/gcode/M042.html].
- En LightBurn: el perfil Marlin "vainilla" emite `M8` al iniciar y `M05` para apagar; el perfil Snapmaker liga el checkbox de aire a `M106/M107` [FORO: hilo 174900]. Un desarrollador de Marlin advirtió que `M9` apaga mist y flood a la vez [FORO: https://forum.lightburnsoftware.com/t/can-i-set-up-air-assist-with-marlin/25821].

### 2.5 Particularidades que rompen trabajos

1. **Frenado en cada cambio de `S` sin inline** (ver 2.1). Mitigación: forzar `marlin-inline` para trabajos con imagen; avisar si la prueba de conexión (`M115`) no revela capacidad inline (no hay `Cap:` para ello; hay que probar con un `M3 S0 I`+`G1`) [VERIFICAR].
2. **Potencia en G0**: la doc dice que `G0` apaga el láser; algunos usuarios ven el láser encendido en saltos con `G1` + `S0` [FORO: hilo 15493]. Defensa: `S0` explícito en el `G0` cuando la `S` anterior no era 0 (`travel_s0`).
3. **Overscan**: el mismo hilo observa que durante el sobrescaneo inverso queda la última potencia [FORO]. El generador debe emitir `S0` real en el `G1` de overscan (ya lo hace).
4. **Líneas largas**: `MAX_CMD_SIZE` ≈ 96 bytes (ver 2.2). Líneas de raster con muchos parámetros no deberían acercarse a 80.
5. **N/checksum**: solo útil con enlaces ruidosos (UART real, cables largos). Por USB-CDC aumenta el tamaño de línea (~+10 bytes) y reduce el rendimiento sin beneficio. Debe ser opción apagada por defecto.
6. **Quitar espacios**: `prepare_line` elimina *todo* el espacio en blanco (`M3 S0 I` → `M3S0I`). En GRBL es inocuo; en Marlin y Smoothie hay que conservar los espacios (la doc de Smoothie advierte que Pronterface puede truncar `G28.2` [DOC: https://smoothieware.org/grbl-mode]). Parámetro `strip_spaces`.
7. **Sondear `M114` durante el trabajo** puede provocar tirones (doc de `M154`, arriba).
8. **Unidades de `S`**: depende de `CUTTER_POWER_UNIT`; el usuario debe indicarlo (PWM 255 / porcentaje 100 / RPM). Un `S255` en una máquina PERCENT satura (se corta a 100 o se rechaza) [VERIFICAR].
9. **Errores**: Marlin usa `Error:...`, `echo:...`, `Resend: N`; el streamer actual solo entiende `error:N` numérico y `ok` exacto (ver sección 9).

### 2.6 Raster

- Marlin no tiene conteo de caracteres ni buffer grande: con ping-pong, el techo lo marcan el ancho de banda serie y la latencia por línea (en Windows/macOS el controlador USB-serie puede esperar varios milisegundos por lectura, los de Linux no [FORO: https://forum.makerforums.info/t/problem-with-lag-in-communications/81213]).
- Cálculo propio [CALC]: una línea típica `G1 X12.3 S128` ocupa ~15 bytes con salto de línea. A 250000 baud (≈25 kB/s) son como mucho ~1700 segmentos/s; con píxel de 0,1 mm eso es ~170 mm/s *teórico*, y a 115200 baud ~770 segmentos/s (~77 mm/s). La latencia de ida y vuelta lo reduce mucho más.
- Recomendaciones:
  - Fundir píxeles contiguos de igual potencia (ya se hace por S modal) y cuantizar la potencia a pocos niveles.
  - Limitar la velocidad máxima de raster a `0,5 × (baud/10) / (bytes_por_segmento × segmentos_por_mm)` y avisar en preflight.
  - Usar `marlin-inline` (obligatorio para imagen).
  - Si el firmware lo permite, enviar el archivo a la SD y ejecutarlo desde ahí (el firmware no depende del host); no se investigó el flujo SD de Marlin en esta pasada.

---

## 3. Smoothieware

### 3.1 Encendido y potencia

| Tema | Dato | Fuente |
|---|---|---|
| Módulo | `laser_module_enable` (si es `false` se ignora toda la configuración de láser). PWM en un pin de hardware PWM (2.0-2.5, 1.18, 1.20, 1.21, 1.23, 1.24, 1.26, 3.25, 3.26; `!` invierte). | [DOC] https://smoothieware.github.io/Webif-pack/documentation/web/html/laser.html y https://smoothieware.org/laser-options |
| Rango de `S` | `S` de 0 (0 %) a 1 (100 %) por defecto: `G1 X20 F300 S0.2`. La opción **Maximum S-value** cambia el rango: 1.0 (S0.0-S1.0), 100.0 o 255.0. El valor se divide por ese máximo y luego se escala por `laser_module_max_power`; no cambia la potencia real, solo cómo se lee el G-code. | [DOC] https://smoothieware.org/laser (y laser-options) |
| `max_power` | Duty máximo (0-1); limita aunque el G-code pida 100 %. | [DOC] laser-options |
| `minimum_power` ("tickle") | Duty de base para desplazamientos; con valor distinto de cero el láser queda "activo" sin quemar. | [DOC] laser-options |
| Potencia proporcional a la velocidad | `proportional_power` (por defecto **true**): `potencia = pedida × (velocidad actual / nominal)`. Equivale al `M4` de GRBL. Se puede desactivar en caliente con `M221 P1` (no se guarda con `M500`). | [DOC] laser-options, https://smoothieware.org/laser |
| `M3/M5` | Soportados como encender/apagar husillo-láser (`M3 S12000`, `M5`). Sin `S`, `M3/M4` usa la "default power" (típicamente 0,8). El módulo láser **solo dispara durante el movimiento `G1/G2/G3`**. | [DOC] https://smoothieware.org/supported-g-codes y laser-options |
| `G0` | "El láser se mantiene apagado en G0, y en G1 está encendido solo durante el movimiento." | [DOC] https://smoothieware.org/laser |
| Escala | `M221 S75` escala la potencia de los `G1` al 75 %; `M221 R<Hz>` fija la frecuencia PWM (¡frecuencia, no periodo!). | [DOC] https://smoothieware.org/laser |
| Prueba | `fire nnn` (0-100) y `fire off`; con `@` en Pronterface o `M1000`. | [DOC] https://smoothieware.org/laser |
| LightBurn | Para Smoothie el "S-Value Max" de LightBurn aplica igual que en GRBL; "Use G0 moves for overscan" existe para placas que queman en el sobrescaneo; "Emit S value with every G1" para láseres que no se reenciendan tras un desplazamiento. | [LB] DeviceSettings |
| Cohesion3D | Las placas Cohesion3D se venden con Smoothieware o GRBL-LPC (ver sección 6). | [FORO] ver sección 6 |

### 3.2 Protocolo

| Tema | Dato | Fuente |
|---|---|---|
| Flujo | **Ping-pong RepRap**: "DEBES esperar el `ok` antes de enviar cada línea". El `ok` solo confirma recepción, no ejecución ni planificación. | [DOC] https://smoothieware.org/communication |
| Cuándo llega el `ok` | Los `G1` se confirman **inmediatamente al recibirlos**, antes de entrar en la cola de planificación; la mayoría de los demás G-codes, al entrar en el búfer. Si el búfer está lleno el `ok` se retiene. | [DOC] communication |
| Fin de movimientos | `M400` devuelve `ok` solo cuando la cola está vacía y acabó el último movimiento. | [DOC] communication |
| `$J` | Bypassa el retardo de espera y mueve de inmediato (útil para mandos). | [DOC] communication |
| Búfer | La doc consultada no da tamaño de línea ni de RX. La cola de planificación es corta (un foro cita 32 entradas como máximo). | [FORO] https://forum.makerforums.info/t/hey-all-im-really-struggling-with-laser-engraving-on-my-smoothiboard/72257 |
| USB | Serie sobre USB (ACM); la velocidad en baudios se ignora. En Ethernet usa "TCP streaming mode" para no ir lento. | [DOC] https://smoothieware.org/pronterface |
| Consulta de estado | `?` es un carácter en tiempo real "que devuelve de inmediato el estado, posiciones, avances y temperaturas". La doc consultada no muestra el formato de la respuesta. También `M114` (posición), `M27` (progreso de SD), `M105`. | [DOC] https://smoothieware.org/console-commands, https://smoothieware.org/supported-g-codes; formato [VERIFICAR] |
| Pausa / reanudar | `M600` suspende (espera a que la cola esté vacía, guarda estado, apaga calentadores) y `M601` reanuda; en consola `suspend`/`resume`. `M25` pausa e `M24` reanuda **solo trabajos de SD**. `!` y `~` en tiempo real en "modo grbl" figuran como "trabajo en curso". | [DOC] https://smoothieware.org/player, supported-g-codes, https://smoothieware.org/grbl-mode |
| Cancelar | `M26` aborta una impresión SD (consola `abort`); `M112` parada de emergencia (se pierde posición) y `M999` limpia el estado halt. | [DOC] supported-g-codes, console-commands |
| Identificación | LightBurn: la consola de Smoothie muestra solo "Smoothie" al conectar. `M115` devuelve `FIRMWARE_NAME:Smoothieware...`, pero se añadió en enero de 2018 y firmwares anteriores devuelven error. | [LB] https://docs.lightburnsoftware.com/CommonGrblSetups.html; [FORO] https://community.octoprint.org/t/octoprint-vs-smoothieware-m115-error/36984 |
| Modo | `grbl_mode true` hace que Smoothie interprete G-code "CNC" (`G28` va a la posición de aparcado, `$H` hace homing). El software láser suele hablarle en modo 3D/RepRap. | [DOC] https://smoothieware.org/grbl-mode |

### 3.3 Unidades, posición, homing

- `F` en **mm/min**; `default_feed_rate`, `max_speed` y `max_rate` también en mm/min; la aceleración en mm/s² [DOC: https://smoothieware.org/motion-control]. Hay una tasa por defecto separada para `G0` [DOC: motion-control], así que un `G0` sin `F` usa la velocidad de búsqueda configurada.
- Homing: `G28` homea (en modo 3D) o aparca (en modo CNC, donde se usa `$H`) [DOC: supported-g-codes]. `G92 X0 Y0 Z0` fija coordenadas globales de trabajo [DOC: supported-g-codes]. `G21`, `G90` son los valores por defecto.

### 3.4 Aire y salidas

- El módulo láser ofrece un **pin "fire"** digital que sube cuando el láser dispara, usado "para aire asistido, extractores o interbloqueos" [DOC: https://smoothieware.org/laser-options]. Es la forma más limpia de ligar el aire al láser sin G-code, pero no es controlable desde el trabajo.
- Para aire controlable por G-code: módulo `switch` con `input_on_command`/`input_off_command` (cualquier comando que empiece por esa cadena coincide), con salida digital o PWM (S 0-255 en `pwm`, 0-100 en `hwpwm`) [DOC: https://smoothieware.org/switch]. `M7/M8` no están documentados como nativos [DOC: supported-g-codes, que sí documenta `M9`]; el usuario debe mapearlos en `config` (p. ej. `M8`/`M9`).

### 3.5 Particularidades que rompen trabajos

1. **Potencia en G0 / sobrescaneo**: G0 apaga el láser; con `minimum_power` > 0 o en ciertas placas el sobrescaneo con `G1 S0` "quema" y LightBurn recomienda sobrescanear con `G0` [LB]. Parámetro `overscan_as_g0`: un `Cut` con potencia 0 se emite como `G0` (la velocidad deja de importar).
2. **`ok` inmediato para `G1`** hace que el host crea que hay hueco, pero la cola de planificación es corta: con segmentos diminutos el firmware limita la velocidad para poder frenar ante un cambio de dirección [FORO: hilo 72257]. Un usuario pasó de 0,1 a 0,2 mm de píxel y el grabado fue "considerablemente más suave".
3. **Rendimiento con imágenes**: un hilo de LightBurn reporta que Smoothie estándar pierde pasos al grabar imágenes tramadas porque no procesa los comandos a tiempo, y que un build más nuevo sube la velocidad segura de ~80 a ~240 mm/s [FORO: https://forum.lightburnsoftware.com/t/lightburn-slowing-down-stuttering-during-pass/28488]. Otra regla empírica: bajar a 80 mm/s a 254 dpi [FORO] — no es un límite documentado.
4. **Rango de `S` ambiguo**: según `Maximum S-value` la `S` es 0-1, 0-100 o 0-255. El dialecto necesita `s_max` configurable y `s_decimals` ≥ 3 si es 1.0.
5. **Unidades**: mm/min en G-code y en `max_speed`; mm/s² solo en aceleración. Sin riesgo de confusión si se emite siempre `F` en mm/min.
6. **Líneas largas**: no hay límite documentado en las páginas consultadas [VERIFICAR].
7. **Pausar a mitad de un trabajo en streaming**: `M25/M24/M26` aplican a SD, no a streaming por serie; `M600/M601` suspenden calentadores, no el láser. Lo robusto es "dejar de enviar" y confiar en que el módulo láser solo dispara durante `G1`.
8. **Respuesta a comandos de consola**: no llevan `ok`; no mezclar con G-code [FORO: https://forum.makerforums.info/t/problem-with-lag-in-communications/81213].

### 3.6 Raster

El hecho de que el `ok` sea inmediato en `G1` permite tener varias líneas en vuelo en la práctica, pero la doc oficial dice "MUST wait". Recomendación conservadora: ping-pong estricto (una línea por `ok`), píxel ≥ 0,2 mm, velocidad de raster limitada (80-240 mm/s según build, ver arriba) y, para imágenes grandes, ejecutar desde la SD (`play`/`M24`; el envío de archivos por serie a la SD se menciona en https://smoothieware.org/pronterface). Si el cliente dispone de GRBL-LPC para la misma placa, es la ruta recomendada para grabado [FORO: https://forum.lightburnsoftware.com/t/connect-to-cohesion3d-mini-over-ethernet/1544 resumido en la búsqueda; no leído completo].

---

## 4. grblHAL

grblHAL es una reescritura de GRBL 1.1 para microcontroladores de 32 bits [DOC: https://github.com/grblHAL/core/wiki]. Para el generador es **idéntico a GRBL 1.1** (M3/M4/M5, `S` modal, `$32=1`); las diferencias son del enlace y la configuración.

| Tema | Diferencia respecto a GRBL | Fuente |
|---|---|---|
| Modo láser | `$32`: 0 normal, 1 láser, **2 torno** ("changed from original boolean"). | [DOC] https://github.com/grblHAL/core/wiki/Additional-or-extended-settings |
| PWM | `$33` frecuencia (Hz), `$34` valor de apagado (%), `$35` mínimo (%), `$36` máximo (%). | [DOC] extended-settings |
| Láser en pausa | `$63` desactiva el láser durante el feed hold (por defecto 0 = el láser se apaga). `$61` permite mover el cabezal con la puerta abierta y solo apaga el husillo/láser. | [DOC] extended-settings |
| `$39` | Controla si los caracteres de tiempo real *imprimibles* (`?`, `!`, `~`) se tratan como tiempo real. | [DOC] extended-settings |
| Tiempo real extra | Equivalentes con el bit alto: `0x80` = `?`, `0x81` = `~`, `0x82` = `!`; `0x87` informe completo; `0x19` stop (como reset sin reinicio cálido); `0x83` parser state; `0x88/0x89` parada opcional/paso virtual; `0x8C` autoinforme. | [DOC] https://github.com/grblHAL/core/wiki/For-sender-developers |
| Detección | Esperar 400-500 ms en la conexión; enviar `0x87` y esperar ~250 ms: una respuesta indica un protocolo GRBL extendido; `$I+` devuelve capacidades (`NEWOPT` con `RT+`/`RT-`). Con `COMPATIBILITY_LEVEL` 1 el saludo es "Grbl 1.1…" en vez de "grblHAL 1.1…". | [DOC] For-sender-developers y https://github.com/grblHAL/core/wiki/Changes-from-grbl-1.1 |
| Fin de línea | CRLF y LFCR cuentan como **un** terminador (GRBL clásico los cuenta como dos). Usar un único `\n`, como ya hace el streamer. | [DOC] For-sender-developers |
| Errores persistentes | **Tras un error de G-code, el error persiste para todo el G-code posterior hasta un reset, una línea vacía o un comando `$`.** Con `ErrorPolicy::Continue` esto convertiría un error en una cascada. | [DOC] Changes-from-grbl-1.1 |
| Homing | Informa estado `Home` durante el homing y rastrea qué ejes están referenciados. | [DOC] Changes-from-grbl-1.1 |
| Salidas | M62-M65 (salidas digitales), M66 (esperar entrada), M67/M68 (analógicas), M6/M61 con tabla de herramientas, G65 macros; "poco probados". `M7/M8/M9` estándar. | [DOC] https://github-wiki-see.page/m/grblHAL/core/wiki/Additional-G--and-M-codes |
| Búfer RX | Las páginas leídas **no** dan el tamaño del búfer ni el límite de línea [VERIFICAR]. El comentario del streamer actual afirma "grblHAL admite bastante más" que 127 bytes; no está confirmado por la doc leída. |  |

Particularidades que rompen trabajos: la persistencia del error (arriba); `$32=2` (modo torno) hace que M3/M4 se comporten distinto [DOC]; y todo lo ya conocido de GRBL. Raster: igual que GRBL (conteo de caracteres); un búfer RX mayor ayudaría pero hay que leerlo de `$I`/`Bf:` en la conexión en vez de suponerlo.

---

## 5. FluidNC

| Tema | Dato | Fuente |
|---|---|---|
| Láser como husillo | "Laser is considered a spindle because gcode does not have laser specific codes"; usa el valor de RPM como potencia y "debe apagarse cuando la máquina está parada o en un movimiento rápido". Se declara con una sección `Laser:` del YAML: `pwm_hz` (5000 en el ejemplo; algunos diodos prefieren ~1500), `output_pin`, `enable_pin`, `disable_with_s0`, `s0_with_disable`, `tool_num`, `speed_map`. | [DOC-espejo] https://github.com/Longus/FluidNC/wiki/Fluid-Spindle-Setup (la wiki oficial migró a wiki.fluidnc.com, que no respondió en esta consulta) |
| Escala de `S` | `speed_map: 0=0.000% 255=100.000%`: el último valor "puede ser el que quieras, normalmente 255 o 1000", y el programa CAM debe usar el mismo máximo. | [DOC-espejo] ídem |
| `M4` | FluidNC **acepta `M4` solo si el YAML tiene sección `Laser:`**; si no, devuelve `error:20` en esa línea y el trabajo se detiene. Con LightBurn: usar el perfil GRBL (no GRBL-M3). | [FORO] https://forum.lightburnsoftware.com/t/error-20-job-halted-custom-machine-with-fluidnc/179747 |
| `disable_with_s0` / `s0_with_disable` | Nombres presentes en los ejemplos; la documentación leída **no explica** su efecto. | [VERIFICAR] |
| M-codes | Heredados de Grbl_Esp32: `M0, M2, M30`, `M3/M4/M5`, `M6`, `M7/M8/M9`, `M56`, `M62-M65`, `M67`; G: `G0-G4, G10 L2/L20, G53-G59, G92, G92.1, G28/G30 (+.1), G17-G21, G38.x, G40, G43.1, G49, G90/G91/G91.1, G93/G94`. (v3.6.7 añade `M7.1`/`M8.1` para apagar mist/flood por separado.) | [DOC, antigua 2020] https://github.com/bdring/Grbl_Esp32/wiki/Supported-GCodes ; [FORO] https://newreleases.io/project/github/bdring/FluidNC/release/v3.6.7 |
| Protocolo | Compatible con GRBL a nivel de remitente: el banner es del estilo `Grbl 3.2 [FluidNC v3.2.4 (wifi) '$' for help]` (el "Grbl 3.x" existe para que los remitentes lo reconozcan); `$Grbl/Show` o `$GS` repite un banner estilo Grbl; `$I` responde con `[VER:3.9 FluidNC v3.9.7:]` (un ejemplo de foro). | [FORO/notas de versión] https://newreleases.io/project/github/bdring/FluidNC/release/v3.9.4 ; https://forum.lightburnsoftware.com/t/move-controls-greyed-out-with-fluidnc/173513 |
| Configuración que usa el desarrollador con LightBurn | Perfil GRBL, DTR activado, `$J` activado, consulta regular de posición (`?`) activada, modo "text based", **Buffered**, **tamaño de búfer objetivo 127**. | [FORO] hilo 173513 |
| Autoinforme | `$report/interval=200` permite estado sin sondear con `?`. | [FORO] hilo 173513 |
| Aire | `M8` (flood) / `M7` (mist) según qué pin se asigne en el YAML; `M8.1`/`M7.1` apagan individualmente (v3.6.7). | [FORO] |
| Parada en feed hold | Una corrección evita paradas bruscas al entrar en feed hold con muchas lecturas de jog o arcos diminutos. | [FORO/notas] v3.9.4 |
| Bytes de tiempo real | `?`, `!`, `~`, `0x18` y override: no confirmados en las páginas leídas, pero el desarrollador usa el perfil GRBL de LightBurn con conteo de caracteres de 127 bytes. | [VERIFICAR] |

Particularidades: (1) el `M4` sin sección `Laser:` (error 20 y fin del trabajo); (2) `S` máx. = último valor de `speed_map`, hay que leerlo de la configuración (`$CD` o similar) o pedírselo al usuario [VERIFICAR el comando]; (3) con un perfil sin `Laser:` y `M3`, el comportamiento en G0 depende de la configuración del husillo; (4) un usuario con CO2 sobre FluidNC reportó que LightBurn no emite `S0` entre perfiles y el láser queda encendido [FORO: https://forum.lightburnsoftware.com/t/laser-not-shutting-off-between-profiles/110814] — Tlaku ya emite `S0` y el modo M4 apaga con G0.

---

## 6. GRBL-LPC y Cohesion3D

GRBL-LPC es un port de GRBL 1.1 al LPC1769 (Smoothieboard, Cohesion3D Remix y Mini, MKS SBase, Azteeg X5) [DOC: https://github.com/cprezzi/grbl-LPC].

| Tema | Dato | Fuente |
|---|---|---|
| Modo láser | Activado por defecto (`$32`). Probado solo con láseres de PWM. | [DOC] README grbl-LPC |
| Escala de `S` | `$30` = S máximo, **por defecto 1.0**; `$31` mínimo 0.0; `$33` frecuencia PWM; `$34/$35/$36` duty de apagado/mínimo/máximo en %. Cambiando `$30=1000` se vuelve compatible con el valor por defecto de LightBurn (1000). | [DOC] README grbl-LPC; [LB] CommonGrblSetups |
| Carencias | Sin finales de carrera ni entradas de control (cycle start, puerta), sin control de husillo no-PWM, la SD no se pasa al host. | [DOC] README |
| Protocolo | "GRBL 1.1 portado al LPC1769"; el README no confirma compatibilidad total del protocolo ni da la cadena de versión [VERIFICAR]. Se espera conteo de caracteres, `?`, `!`, `~` y `0x18`. | [DOC] README |
| Por qué existe | Smoothieware carece de protocolo de streaming en tiempo real adecuado para grabado rápido; GRBL-LPC se recomienda para grabado. | [FORO] https://forum.lightburnsoftware.com/t/diode-laser-recommended-hardware-firmware/4405 (resumen de búsqueda; no leído completo) |
| Identificación | GRBL: banner con versión; Smoothie: solo "Smoothie". | [LB] CommonGrblSetups |
| Aire | El README no menciona refrigerante; en GRBL 1.1 upstream `M8` está siempre, `M7` solo con `ENABLE_M7` en compilación. Para el port: [VERIFICAR]. | [DOC] https://github.com/gnea/grbl/wiki/Grbl-v1.1-Commands (`0xA1` solo con `ENABLE_M7`) |

Para el generador GRBL-LPC es **igual que GRBL 1.1** con `s_max` leído de `$30` (1.0 por defecto, con decimales). Particularidad principal: el defecto 1.0 es un riesgo; si Tlaku asume 1000 y la placa tiene `$30=1.0`, todas las `S` quedan saturadas al 100 % (S500 ≫ 1.0). El preflight debe leer `$30` (como ya lee `$22`) para todo el grupo GRBL.

Cohesion3D: la documentación propia del fabricante (freshdesk) devolvió 404; solo hay fuentes de la comunidad y LightBurn. La placa se entrega con Smoothieware o con el firmware de la serie GRBL-LPC; ambas se distinguen por el saludo.

---

## 7. "GRBL-M3": GRBL 0.9 y 1.0 sin `M4`

LightBurn exige el dispositivo **GRBL-M3** para "GRBL 1.1e o anterior (1.0, 0.9, etc.)" y el estándar solo para 1.1f o posterior, que admite `M4` [LB: https://docs.lightburnsoftware.com/2.0/Guides/GRBLConfiguration/ y https://docs.lightburnsoftware.com/CommonGrblSetups.html]. Las X-Carve anteriores a 2018 llevan GRBL 1.0c y también necesitan GRBL-M3 [LB: CommonGrblSetups].

| Tema | Dato | Fuente |
|---|---|---|
| Orden de potencia | GRBL-M3 emite `M3`; "no hay opción M4 en el perfil GRBL-M3". El perfil estándar emite `M4` una sola vez al inicio (y `M3` solo si la capa pide "Constant Power"). | [FORO] https://forum.lightburnsoftware.com/t/gcodes-with-m4-instead-of-m3/37162 |
| Sin modo láser | "Laser Mode elimina las pausas que ocurren al cambiar la potencia": en versiones anteriores a 1.1f cada cambio de `S` pausa, con "manchas de exceso" al grabar imágenes. | [LB] GRBLConfiguration |
| Sin G0 apaga | La garantía de "G0 siempre apaga el láser" es de **laser mode** de 1.1 [DOC: https://github.com/gnea/grbl/wiki/Grbl-v1.1-Laser-Mode]. En versiones anteriores un `M3 S…` activo sigue encendido durante un `G0`; los usuarios ven líneas quemadas entre objetos y la recomendación de LightBurn es el perfil GRBL-M3 y revisar S-Value Max. | [FORO] https://forum.lightburnsoftware.com/t/how-to-not-print-toolpath-between-objects/1771/2 ; derivado del documento anterior [VERIFICAR para 0.9] |
| Pausa | Con `M4` y modo láser, parar equivale a potencia 0; en versiones sin esto, "pausar corre el riesgo de dejar el rayo encendido". | [LB] GRBLConfiguration |
| Escala de `S` | "Las versiones actuales de GRBL usan 1000 por defecto; las antiguas 255". | [LB] DeviceSettings |
| Aire | `M7` o `M8` elegible en LightBurn. | [LB] DeviceSettings |
| Protocolo | La documentación de interfaz de 0.9 no se pudo leer (la página de la wiki devolvió una pantalla de "crear página"). Lo conocido de 1.1: conteo de caracteres, `ok`/`error:N`; v1.1 cambió: mensajes numéricos en vez de texto, quitó XON/XOFF, un solo vector de posición, `Bf:` ahora son bytes *libres*, y `$J` solo existe desde 1.1. | [DOC] https://github.com/gnea/grbl/wiki/Grbl-v1.1-Interface ; [LB] DeviceSettings ($J ≥ 1.1) |
| Informe de estado en 0.9 | Formato `<Idle,MPos:...,WPos:...>` (con comas, ambas posiciones). No está en ninguna fuente que se haya leído con éxito. | [VERIFICAR] |

Comportamiento a emitir: `M3 S0` al inicio (nunca `M4`); `S0` explícito en cada `G0` si la `S` anterior no era 0 (porque el rayo no se apaga solo); `M5` al final. Para trabajos de imagen, **advertir** que los cambios de `S` hacen pausa por línea. Hay que contar que en 0.9 el ritmo de líneas se ve limitado (el planificador sincroniza) y que el estado `?` debe analizarse con otro parser (`StatusReport::parse` actual espera `|`).

---

## 8. Diseño de "dialecto"

### 8.1 Idea

Separar *qué* se quiere hacer del *cómo* lo escribe cada firmware. El `Toolpath` (bloques con `speed_mm_min`, `power_mode`, `air_assist`, `Move::{Travel, Cut}`) no cambia. El generador consulta una estructura `Dialect` de datos puros, sin código por firmware. El streamer consulta otra mitad del mismo `Dialect` para el enlace. Un tercer dialecto `Custom` deja al usuario editar las plantillas (equivale al "Custom GCode" de LightBurn).

```rust
pub struct Dialect {
    pub id: &'static str,        // "grbl11", "grbl-m3", "grblhal", "fluidnc", "grbl-lpc",
                                 // "smoothie", "marlin-inline", "marlin-standard", "marlin-m106", "custom"
    pub style: GcodeStyle,
    pub link: LinkProfile,
}

pub struct GcodeStyle {
    // --- Potencia ---
    pub s_max: f64,                 // valor de S para el 100 %
    pub s_decimals: u8,             // 0 (Marlin/GRBL-M3), 3 (Smoothie/GRBL-LPC con 1.0)
    pub s_max_source: SMaxSource,   // Fixed | FromDollar30 (GRBL, grblHAL, GRBL-LPC) | FromUser
    pub on_constant: String,        // "M3 S0" | "M3 S0 I"
    pub on_dynamic: Option<String>, // "M4 S0"; None => se usa on_constant (Marlin, Smoothie, GRBL-M3)
    pub off: String,                // "M5"
    pub s_placement: SPlacement,    // InlineG1 | SeparateLine { template: "M3 S{s}" | "M106 S{s}" }
    pub s_on_every_g1: bool,        // LightBurn: "Emit S value with every G1"
    pub g0_turns_laser_off: bool,   // true: GRBL 1.1 laser mode, Smoothie, Marlin; false: GRBL-M3, M106
    pub travel_s0: bool,            // emitir "S0" en el G0 si la S anterior != 0
    pub off_before_travel: Option<String>, // "M107" para marlin-m106
    pub overscan_as_g0: bool,       // Cut con 0 % => G0 (Smoothie)
    // --- Movimiento ---
    pub g0_feed_mm_min: Option<f64>,// Marlin: G0 = G1, F compartido; Smoothie/GRBL: None
    pub preamble: Vec<String>,      // ["G90", "G21"]
    pub origin: OriginSetup,        // None | SetHere("G92 X0 Y0") | Home("G28 X Y") | DollarH
    pub strip_spaces: bool,         // true GRBL; false Marlin/Smoothie
    pub decimals: usize,            // 3
    pub max_line_len: usize,        // incl. salto de línea; 127 GRBL, 96 Marlin
    // --- Salidas auxiliares ---
    pub air_on: Option<String>,     // "M8" | "M106 P1 S255" | "M42 P44 S255"
    pub air_off: Option<String>,    // "M9"  | "M107 P1"
}

pub struct LinkProfile {
    pub flow: FlowControl,          // CharCounting{rx_buffer} | Window{max_lines,max_bytes} (PingPong = Window{1,..})
    pub ack_prefix: &'static str,   // "ok"  (prefijo, no igualdad: Marlin puede añadir texto)
    pub error_prefixes: Vec<&'static str>, // ["error:"] GRBL; ["Error:", "error:"] Marlin
    pub resend_prefix: Option<&'static str>,// "Resend:" (Marlin)
    pub line_numbering: Option<LineNumbering>, // Some{checksum:true} => "N{n} {line}*{xor}"
    pub status: StatusQuery,        // Realtime(b'?') | GcodeLine("M114") | AutoReport("M154 S1") | None
    pub status_parser: StatusParser,// Grbl11 | Grbl09 | MarlinM114 | SmoothieQ
    pub pause: ControlAction,       // Realtime(b'!') | HostSide | Line("M0")
    pub resume: ControlAction,      // Realtime(b'~') | HostSide | Line("M108")
    pub cancel: Vec<ControlAction>, // GRBL: [Realtime(b'!'), Realtime(0x18)];
                                    // Marlin: [HostSide, Line("M410"), Line("M5")]
    pub finish: FinishDetection,    // StatusIdle | SyncCommand("M400")
    pub probe: Vec<ProbeStep>,      // "$I"/"?" para GRBL; "M115" para Marlin/Smoothie
    pub default_baud: u32,          // 115200 GRBL, 250000 Marlin, ignorada en Smoothie USB
    pub dtr_reset: bool,
    pub homing: Option<String>,     // "$H" | "G28"
    pub jog: JogStyle,              // DollarJ | G91G0G90 (no cancelable)
}
```

### 8.2 Lista exacta de parámetros que hacen falta

(Los pedidos explícitamente están marcados con ★.)

| # | Parámetro | Tipo | Para qué |
|---|---|---|---|
| 1★ | `s_max` | f64 | escala de S: 1000 (GRBL 1.1), 255 (0.9 / PWM de Marlin), 100 (Marlin PERCENT), 1.0 (Smoothie y GRBL-LPC por defecto) |
| 2 | `s_decimals` | u8 | Marlin y GRBL-M3 enteros; Smoothie/GRBL-LPC con S0.5 |
| 3 | `s_max_source` | enum | leer `$30` de la placa en vez de suponerlo |
| 4★ | `on_constant` | String | comando de encendido a potencia constante (`M3 S0`, `M3 S0 I`) |
| 5★ | `on_dynamic` | Option<String> | `M4 S0` (GRBL 1.1+); `None` si el firmware no tiene M4 o es experimental (Marlin) |
| 6 | `off` | String | `M5` |
| 7★ | `s_placement` | enum | S dentro de `G1` o en línea aparte (Marlin estándar, M106) |
| 8 | `s_on_every_g1` | bool | láseres que no se reenciendan tras desplazarse |
| 9 | `g0_turns_laser_off` | bool | GRBL 1.1 y Smoothie y Marlin sí; GRBL ≤1.1e y M106 no |
| 10 | `travel_s0` | bool | `S0` defensivo en el G0 (Marlin, GRBL-M3) |
| 11 | `off_before_travel` | Option<String> | apagado explícito (`M107`) cuando G0 no apaga |
| 12 | `overscan_as_g0` | bool | Smoothie |
| 13 | `g0_feed_mm_min` | Option<f64> | Marlin: `F` en G0 |
| 14 | `preamble` | Vec<String> | `G90`, `G21` (y quizá `M3 I`) |
| 15 | `origin` | enum | `G92`, `G28`, `$H` (tiene que ver con el modo de origen del trabajo) |
| 16 | `strip_spaces` | bool | solo GRBL puede juntar todo |
| 17 | `decimals` | usize | ya existe |
| 18 | `max_line_len` | usize | validación previa (ya existe como `rx_buffer`) |
| 19★ | `air_on` / `air_off` | Option<String> | `M8`/`M9`, `M106 P1 S255`/`M107 P1`, `M42 …` |
| 20★ | `flow` | enum | conteo de caracteres (`rx_buffer`) o ventana de líneas (`max_lines`; ping-pong = 1) |
| 21★ | tamaño de búfer | usize | `rx_buffer` en GRBL (127); `max_line_len` en Marlin |
| 22 | `ack_prefix` | &str | `ok` (prefijo) |
| 23 | `error_prefixes` / `resend_prefix` | listas | `error:N`, `Error:…`, `Resend: N` |
| 24★ | `line_numbering` | Option | `N` + checksum XOR; apagado por defecto |
| 25★ | `status` | enum | `?` en tiempo real, `M114`, `M154` autoinforme o ninguno |
| 26 | `status_parser` | enum | formatos distintos (GRBL 0.9 con comas, GRBL 1.1 con `|`, Marlin `X:… Count …`) |
| 27★ | `pause` / `resume` | ControlAction | `!`/`~`, o dejar de enviar (+ `M0`/`M108`) |
| 28★ | `cancel` | Vec<ControlAction> | `!`+`0x18` (GRBL), `M410`+`M5` (Marlin) |
| 29 | `finish` | enum | `Idle` por `?` o `M400` |
| 30 | `probe` | Vec | `$I`/`0x87`, `M115` |
| 31 | `default_baud` | u32 | 115200 / 250000 |
| 32 | `dtr_reset` | bool | placas AVR (LightBurn "Enable DTR") |
| 33 | `homing` | Option | `$H` o `G28` |
| 34 | `jog` | enum | `$J=` frente a `G91 G0 … G90` |
| 35 | `rapid_override_policy` | enum | si el firmware escribe `F` distinto en G0 vs G1 (Marlin comparte) |

### 8.3 Tabla de valores por firmware

Leyenda: GRBL 1.1 = referencia actual del proyecto. "=" significa igual a GRBL 1.1.

| Parámetro | GRBL 1.1 | GRBL-M3 (0.9/1.0) | grblHAL | FluidNC | GRBL-LPC | Smoothie | Marlin inline | Marlin estándar | Marlin M106 |
|---|---|---|---|---|---|---|---|---|---|
| `s_max` | 1000 (`$30`) | 255 (0.9) / `$30` (1.0) | `$30` (def. 1000 upstream) | último valor de `speed_map` (255 o 1000) | `$30`, def. **1.0** | `Maximum S-value`: 1.0 / 100 / 255 | 255 (PWM), 100 (PERCENT), RPM | = | 255 |
| `s_decimals` | 3 | 0 | 3 | 0-3 | 3 | 3 | 0 | 0 | 0 |
| `on_constant` | `M3 S0` | `M3 S0` | = | `M3 S0` | = | `M3 S0` [VERIFICAR] | `M3 S0 I` | `M3 S0` | (ninguno) |
| `on_dynamic` | `M4 S0` | `None` | `M4 S0` | `M4 S0` (requiere `Laser:`) | `M4 S0` | `None` (potencia proporcional por defecto) | `None` (`M4 S0 I` es experimental) | `M4 S0` (sin sync fino) | `None` |
| `off` | `M5` | `M5` | `M5` | `M5` | `M5` | `M5` | `M5` | `M5` | `M107` |
| `s_placement` | InlineG1 | InlineG1 | InlineG1 | InlineG1 | InlineG1 | InlineG1 | InlineG1 | SeparateLine `M3 S{s}` | SeparateLine `M106 S{s}` |
| `s_on_every_g1` | no | no | no | no | no | opcional | no | no | no |
| `g0_turns_laser_off` | sí (`$32=1`) | **no** | sí | sí (según YAML) | sí | sí | sí | sí | **no** |
| `travel_s0` | no | **sí** | no | no | no | no | sí (defensivo) | no | no |
| `off_before_travel` | — | — | — | — | — | — | — | — | `M107` |
| `overscan_as_g0` | no | no | no | no | no | **sí** | no | no | no |
| `g0_feed_mm_min` | None | None | None | None | None | None (tasa de G0 propia) | `Some(rápido)` | `Some(rápido)` | `Some(rápido)` |
| `strip_spaces` | sí | sí | sí | sí | sí | no | no | no | no |
| `max_line_len` | 127 | 127 | ver `$I` [VERIFICAR] | 127 (LightBurn) | 127 | sin dato [VERIFICAR] | ~96 [VERIFICAR] | ~96 | ~96 |
| `air_on`/`air_off` | `M8`/`M9` | `M8`/`M9` (o `M7`) | `M8`/`M9` | `M8`/`M9` | `M8`/`M9` [VERIFICAR] | `M8`/`M9` si el usuario mapea `switch`; si no, ninguno | `M8`/`M9` (`AIR_ASSIST`) | = | `M106 P1 S255`/`M107 P1` o `M42` |
| `flow` | CharCounting(127) | CharCounting(127) | CharCounting(≥127; leer `Bf:`) | CharCounting(127) | CharCounting(127) | Window{1} (ping-pong) | Window{1} | Window{1} | Window{1} |
| `ack_prefix` | `ok` | `ok` | `ok` | `ok` | `ok` | `ok` | `ok` | `ok` | `ok` |
| `error_prefixes` | `error:` | `error:` o texto "error: …" (0.9) | `error:` | `error:` | `error:` | `Error`/texto [VERIFICAR] | `Error:` | = | = |
| `line_numbering` | no | no | no | no | no | no | opcional | opcional | opcional |
| `status` | `?` (200 ms) | `?` | `?` (o `0x80`) | `?` o `$report/interval` | `?` | `?` (formato [VERIFICAR]); `M114` | solo en reposo: `M114`; durante el trabajo: conteo de `ok` o `M154` | = | = |
| `pause` | `!` | `!` | `!` | `!` | `!` | HostSide | HostSide (+`M0` si `EMERGENCY_PARSER`) | = | = |
| `resume` | `~` | `~` | `~` | `~` | `~` | HostSide | HostSide (+`M108`) | = | = |
| `cancel` | `!`, 0x18 | `!`, 0x18 | `!`, 0x18 | `!`, 0x18 | `!`, 0x18 | HostSide, `M112`+`M999` (o `abort` si es SD) | HostSide, `M410`, `M5` | = | HostSide, `M410`, `M107` |
| `finish` | StatusIdle | StatusIdle | StatusIdle | StatusIdle | StatusIdle | `M400` | `M400` | `M400` | `M400` |
| `probe` | banner `Grbl 1.1x` + `$I` | banner `Grbl 0.9x/1.0x` | `0x87`/`$I+`, banner `GrblHAL` (o `Grbl 1.1` en nivel 1) | `Grbl 3.x [FluidNC …]`, `$I` | banner `Grbl 1.1` [VERIFICAR] | `M115` (`FIRMWARE_NAME:Smoothieware`) o banner "Smoothie" | `M115` (`FIRMWARE_NAME:Marlin`) | = | = |
| `default_baud` | 115200 | 115200 (algunos 9600) | 115200 | 115200 | 115200 | ignorada (USB) | 250000 | 250000 | 250000 |
| `homing` | `$H` | `$H` | `$H` | `$H` | `$H` (sin finales) | `G28` (modo 3D) / `$H` (modo CNC) | `G28` | `G28` | `G28` |
| `origin` | `G92`/`G54` según modo | idem | idem | idem | idem | `G92 X0 Y0` | `G92 X0 Y0` | `G92 X0 Y0` | `G92 X0 Y0` |
| `jog` | `$J=` | `G91 G0…G90` (sin `$J` antes de 1.1) | `$J=` | `$J=` | `$J=` | `G91 G0 … G90` | `G91 G0 … G90` | = | = |
| `dtr_reset` | sí (AVR) | sí | no (32 bits) | sí | no | no | opcional | opcional | opcional |

---

## 9. Qué habría que cambiar en el código actual (solo análisis, sin modificar)

Resumen de lo leído en `crates/tlaku-grbl/src/`:

- `gcode.rs`: `GcodeOptions` tiene `s_max`, `return_to_origin`, `start_gcode`, `end_gcode`, `decimals`, `y_scale`. `mode_cmd` fija `M4 S0`/`M3 S0`. `generate` emite `; Generado por Tlaku`, `G90`, `G21`, `start_gcode`, la orden de modo, y por bloque comentario + `M8`/`M9` + `G0 Z…` + movimientos; cierra con `M9` (si hay aire), `M5` y `G0 X0 Y0`. `S` y `F` son modales y se omiten coordenadas repetidas. `s_value` ya conserva decimales "porque algunos firmwares (Smoothieware) usan S de 0 a 1".
- `streamer.rs`: conteo de caracteres con `rx_buffer` (127), `ok`/`error:N`/`ALARM:`/`<…>`/`grbl…` como reinicio, sondeo `?` cada 200 ms, `!`/`~`/`0x18` fijos, `wait_idle` por estado `Idle`.
- `preflight.rs`: exige respuesta a `?` y `Idle`, lee `$22`; `validate_gcode(text, rx_buffer)` valida longitud de línea.

Cambios necesarios, en orden de riesgo creciente:

1. **`GcodeOptions` → `GcodeOptions { dialect: Dialect, … }`**; `s_max` pasa a `style.s_max`/`s_decimals` (en `s_value`, el `3` fijo pasa a `style.s_decimals`).
2. **`mode_cmd`** lee `on_constant`/`on_dynamic` del dialecto; `Dynamic` cae en `on_constant` si `on_dynamic` es `None`. Además `generate` debe emitir `off` (no `M5` literal) y `air_on/air_off` (no `M8/M9` literales; `None` = ignorar con aviso).
3. **`Writer::motion` y `s_placement`**: añadir el modo `SeparateLine` (línea de potencia antes del `G1` cuando cambia la `S`, y apagado explícito antes del `G0` si `g0_turns_laser_off == false`), el `S0` defensivo en `G0` (`travel_s0`) y `overscan_as_g0` (un `Cut` a 0 % se emite como `G0` y no actualiza `F`).
4. **`F` compartido entre `G0` y `G1`** (Marlin): `motion("G0", …, feed: Some(g0_feed))` y, al volver a `G1`, volver a emitir `F` (el estado `w.f` ya lo hace si se actualiza en G0; hoy los `G0` no tocan `f`).
5. **`prepare_line`** (`streamer.rs`): hoy borra todo espacio y rechaza `?`, `!`, `~`. Hace falta `strip_spaces: bool` y, en Marlin/Smoothie, no rechazar `?` ni `!` fuera de strings (no hay tiempo real); al menos debe validar que no existan en las órdenes generadas (nunca se emiten).
6. **`StreamerConfig`** pasa de `rx_buffer` a `flow: FlowControl` (en `tick`, la condición `st.used + len > rx_buffer` se vuelve `st.pending.len() >= max_lines` para ventana). `handle_line` compara `line == "ok"`: debe ser prefijo (`ok` con texto, p. ej. con `ADVANCED_OK`, y las líneas `echo:`/`busy:` no deben contarse) y no debe confundir `Error:` de Marlin con `error:N`. Con `line_numbering` se añade `N{n}` y checksum al construir `batch`, y se maneja `Resend:` retrocediendo `st.next`.
7. **Pausa/reanudar/cancelar** (`handle_command`): sustituir `Realtime::FeedHold`/`CycleStart`/`SoftReset` por `ControlAction` del dialecto. En Marlin la pausa es dejar de enviar (el campo `st.paused` ya lo hace, solo falta no enviar `!`); la cancelación necesita `M410` + `M5`, y como esas líneas son encoladas deben salir inmediatamente tras vaciar el flujo (`M410` acompaña mejor con `EMERGENCY_PARSER`).
8. **Consulta de estado y fin** (`tick`, `preflight.rs`): para dialectos sin `?` el sondeo periódico se desactiva (`status_interval = None`, ya soportado) y `wait_idle` se sustituye por enviar `M400` y esperar su `ok`. `query_status` en `preflight.rs` y `StatusReport::parse` necesitan variantes: Marlin (`X:… Count …`), Smoothie y GRBL 0.9 (formato con comas).
9. **Preflight de capacidades**: `probe` por dialecto (leer `$30` para `s_max`; `M115` para Marlin/Smoothie; `$I` para grblHAL/FluidNC).
10. **Preflight de raster**: estimar bytes/s del trabajo con los segmentos/s previstos (sección 2.6) y avisar si supera ~50 % del ancho de banda del enlace del dialecto.
11. **Reconocimiento del reinicio**: `handle_line` termina el trabajo si ve una línea que empieza por `grbl`; con FluidNC el banner también empieza por `Grbl`, con grblHAL por `GrblHAL`, con Smoothie/Marlin no hay (hay `start`/`Smoothie`). Parametrizar con `reset_banners: Vec<&str>`.

Todo esto cabe en el mismo `generate()`: no hace falta un generador por firmware.

---

## 10. Vectores de prueba

**Trabajo común**: un `Toolpath` con un único bloque `"Cuadrado"`: `speed_mm_min = 1000`, `power_mode = Dynamic` (por defecto), `air_assist = true`, `z_mm = None`. Movimientos: `Travel(0,0)`, `Cut(10,0, 50 %)`, `Cut(10,10, 50 %)`, `Cut(0,10, 50 %)`, `Cut(0,0, 50 %)`. Opciones: `return_to_origin = true`, `decimals = 3`, `y_scale = 1`, sin `start_gcode` ni `end_gcode`.

Estado de cada vector: **GRBL 1.1** es la salida exacta que produce hoy `generate()` (derivada de leer el código y comprobada contra sus pruebas, p. ej. `closed_path_returns_to_start_and_s_f_are_modal`). Los demás son **hechos a mano** según la documentación citada y marcados con lo que falta confirmar. Se escriben con `\n`; los comentarios `;` no se transmiten (el streamer los quita).

### V1. GRBL 1.1 (referencia; `s_max = 1000`)

```gcode
; Generado por Tlaku
G90
G21
M4 S0
; Cuadrado
M8
G0 X0 Y0
G1 X10 S500 F1000
G1 Y10
G1 X0
G1 Y0
M9
M5
G0 X0 Y0
```

### V2. grblHAL

Igual que V1 byte a byte. Diferencias solo de enlace: banner `GrblHAL …`, sondeo opcional con `0x87`; con `ErrorPolicy::Continue` un `error:N` envenena las líneas siguientes hasta una línea vacía o un `$`, así que usar `Stop`.

### V3. FluidNC (`speed_map: 0=0% 1000=100%`)

Igual que V1. Requiere sección `Laser:` en el YAML; si el máximo del mapa es 255 el mismo trabajo sale con `s_max = 255`: `G1 X10 S127.5 F1000` (o `S128` con `s_decimals = 0`).

### V4. GRBL-LPC (`$30 = 1.0`, valor por defecto)

```gcode
; Generado por Tlaku
G90
G21
M4 S0
; Cuadrado
M8
G0 X0 Y0
G1 X10 S0.5 F1000
G1 Y10
G1 X0
G1 Y0
M9
M5
G0 X0 Y0
```

Con `$30 = 1000` sale como V1. `M8` no está confirmado en el port [VERIFICAR].

### V5. GRBL-M3 (GRBL 0.9/1.0, `S` máx. 255, sin `M4`)

```gcode
; Generado por Tlaku
G90
G21
M3 S0
; Cuadrado
M8
G0 X0 Y0
G1 X10 S128 F1000
G1 Y10
G1 X0
G1 Y0
M9
M5
G0 X0 Y0
```

`S128` = 50,2 % (127,5 redondeado). Aquí no aparece `travel_s0` porque el único `G0` previo ocurre con `S0`; en un trabajo con más de una forma cada `G0` posterior llevaría `S0` (`G0 X20 Y0 S0`) y cada `S` nuevo causaría una pausa. [Derivado; la guía de 0.9 no se pudo leer.]

### V6. Smoothieware (`laser_module`, `Maximum S-value = 1.0`)

```gcode
; Generado por Tlaku
G90
G21
M3 S0
; Cuadrado
M8
G0 X0 Y0
G1 X10 S0.5 F1000
G1 Y10
G1 X0
G1 Y0
M9
M5
G0 X0 Y0
```

Notas: el láser solo dispara durante `G1` y la potencia es proporcional a la velocidad por defecto (`proportional_power = true`) [DOC]; `M3 S0`/`M5` son inocuos con el módulo láser y necesarios con la configuración por `switch` de LaserGRBL [DOC: https://github.com/arkypita/LaserGRBL/wiki/Smoothie-firmware]; `M8`/`M9` requieren un `switch` configurado por el usuario (si no, quitar `air_on/air_off`). Hay que transmitir ping-pong: una línea por `ok`. Con `Maximum S-value = 100` la línea sería `S50`.

### V7. Marlin 2.x, modo inline (`CUTTER_POWER_UNIT = PWM`, `S` 0-255, `G0` con `F` compartido)

```gcode
; Generado por Tlaku
G90
G21
M3 S0 I
; Cuadrado
M8
G0 X0 Y0 F6000
G1 X10 S128 F1000
G1 Y10
G1 X0
G1 Y0
M9
M5
G0 X0 Y0 F6000
```

Notas: `M3 S0 I` entra en "Continuous Inline Mode" (el `I` va en la misma línea; la doc muestra `M3 S204 I`); la `S` de `G1` se aplica a ese `G1`; `Dynamic` no tiene equivalente estable (`M4 I` es experimental) y por eso se usa `M3`. `F6000` es la velocidad de desplazamiento (`g0_feed_mm_min`, valor de ejemplo); como `F` es compartido, el primer `G1` repite `F1000` y el `G0` final reescribe `F6000` (el estado modal de `F` lo gestiona el generador). Con `CUTTER_POWER_UNIT = PERCENT` sale `S50`. `M8/M9` necesitan `AIR_ASSIST` o `COOLANT_CONTROL`. Con `strip_spaces = false`. Marlin no tiene `?`: avance por conteo de `ok`, fin con `M400`.

### V8. Marlin 2.x, modo estándar (sin inline; solo vectorial a potencia constante)

```gcode
; Generado por Tlaku
G90
G21
; Cuadrado
M8
G0 X0 Y0 F6000
M3 S128
G1 X10 F1000
G1 Y10
G1 X0
G1 Y0
M9
M5
G0 X0 Y0 F6000
```

Notas: `M3 S128` sincroniza el planificador (parada momentánea) [DOC]; si hubiera varias formas, cada `G0` apaga el láser (`G0` y `G28` apagan [DOC: M005]) y habría que repetir `M3 S128` antes de cada trozo con otra `S`. No usar para imágenes. Compárese con V7.

### V9. Marlin heredado con láser en salida de ventilador (`LASER_SYNCHRONOUS_M106_M107`)

```gcode
; Generado por Tlaku
G90
G21
; Cuadrado
M8
G0 X0 Y0 F6000
M106 P1 S128
G1 X10 F1000
G1 Y10
G1 X0
G1 Y0
M107 P1
M9
G0 X0 Y0 F6000
```

Notas: aquí el `G0` no apaga, de ahí el `M107 P1` antes de desplazarse (y al final); `P1` es el índice de ventilador de la doc de `M106` (`M106 P1 S128` ≈ 50 %) [DOC: laser_spindle 2.0.9]. En Prusa usar `M106 P1 S1` en vez de `M107` [FORO].

### V10. Marlin con aire por salida de ventilador (variante del aire de V7)

Sustituir en V7 `M8` por `M106 P2 S255` y `M9` por `M107 P2`, con `P2` según el pin del usuario. Es la variante que LightBurn usa en el perfil Snapmaker.

### Comprobaciones automáticas sugeridas (pruebas de unidad futuras)

1. V1 debe coincidir byte a byte con la salida actual de `generate()` (prueba de no regresión).
2. Para cada dialecto: ninguna línea supera `max_line_len`, `S` no supera `s_max`, y en `g0_turns_laser_off == false` ningún `G0` va precedido de un láser encendido.
3. Marlin: con `strip_spaces = false` conserva `M3 S0 I`.
4. Smoothie: un `Cut` a 0 % se emite como `G0` con `overscan_as_g0`.
5. Marlin con `line_numbering`: comprobar el checksum con la regla XOR (el ejemplo de la wiki RepRap sirve de oráculo: https://reprap.org/wiki/G-code).

---

## 11. Pendientes y lo que no se pudo confirmar

Antes de implementar cada dialecto, probar en hardware o leer la doc primaria (marcados [VERIFICAR]):

1. Marlin: orden exacta de `M3 S0 I` (¿`S0` antes o después de `I`?); si `G0` con `S` está soportado en tu build; comportamiento de `LASER_MOVE_POWER`; rango real de `G1 S` según `CUTTER_POWER_UNIT`; valores por defecto de `BUFSIZE`/`MAX_CMD_SIZE` en la doc oficial (aquí salen de un mirror).
2. Smoothie: formato de la respuesta a `?`; límite de longitud de línea; si `M3/M5` interactúan con `laser_module` o solo con `switch`; significado exacto de `M221 P1`; nombres de las claves entre versiones (`laser_module_pin` frente a `[laser] pwm_pin`).
3. grblHAL: tamaño del búfer RX y longitud máxima de línea; valor por defecto de `$30` (la doc de ajustes leída no lo lista); la página oficial "Additional G and M codes" se leyó a través de un espejo.
4. FluidNC: la wiki oficial (wiki.fluidnc.com) no respondió y las páginas de la wiki antigua en GitHub redirigen; se usaron espejos y foros. Falta: significado de `disable_with_s0`/`s0_with_disable`, bytes de tiempo real confirmados, cómo obtener `speed_map` máximo por consola.
5. GRBL-LPC / Cohesion3D: la doc del fabricante no pudo leerse (404); falta cadena de versión, soporte de `M7/M8`, y límite de línea.
6. GRBL 0.9/1.0: la página de interfaz v0.9 no se pudo leer; falta formato de estado y comportamiento de `M3 S0` (¿apaga el husillo?).
7. Cohesion3D y MKS SBase pueden llevar Smoothie o GRBL-LPC: la detección debe basarse en el saludo/`M115`, no en el nombre de la placa.

---

## 12. Fuentes consultadas

Marlin (documentación oficial, GPLv3 solo de lectura): https://marlinfw.org/docs/gcode/M003.html · /M004.html · /M005.html · /M106.html · /M042.html · /M110.html · /M112.html · /M114.html · /M115.html · /M154.html · /M203.html · /M400.html · /M410.html · /M575.html · /M999.html · /M108.html · /M000-M001.html · /G000-G001.html · /G020.html · /G028.html · /G092.html · https://marlinfw.org/docs/configuration/2.0.9/laser_spindle.html · https://marlinfw.org/docs/configuration/configuration.html · https://marlinfw.org/docs/setting/serial.html · https://reprap.org/wiki/G-code · https://reprap.org/wiki/M115_Keywords

Smoothieware: https://smoothieware.org/laser · https://smoothieware.org/laser-options · https://smoothieware.org/supported-g-codes · https://smoothieware.org/communication · https://smoothieware.org/console-commands · https://smoothieware.org/grbl-mode · https://smoothieware.org/player · https://smoothieware.org/switch · https://smoothieware.org/motion-control · https://smoothieware.org/pronterface · https://smoothieware.github.io/Webif-pack/documentation/web/html/laser.html · https://github.com/arkypita/LaserGRBL/wiki/Smoothie-firmware

GRBL / grblHAL / GRBL-LPC / FluidNC: https://github.com/gnea/grbl/wiki/Grbl-v1.1-Interface · .../Grbl-v1.1-Laser-Mode · .../Grbl-v1.1-Commands · https://github.com/grblHAL/core/wiki (+ `Additional-or-extended-settings`, `Changes-from-grbl-1.1`, `For-sender-developers`) · https://github-wiki-see.page/m/grblHAL/core/wiki/Additional-G--and-M-codes · https://github.com/cprezzi/grbl-LPC · https://github.com/Longus/FluidNC/wiki/Fluid-Spindle-Setup · https://github.com/bdring/Grbl_Esp32/wiki/Supported-GCodes · https://newreleases.io/project/github/bdring/FluidNC/release/v3.6.7 · .../v3.9.4

LightBurn (comportamiento observable de cada perfil): https://docs.lightburnsoftware.com/UI/DeviceSettings.html · https://docs.lightburnsoftware.com/CommonGrblSetups.html · https://docs.lightburnsoftware.com/2.0/Guides/GRBLConfiguration/ · foros: https://forum.lightburnsoftware.com/t/laser-power-inline/15493 · /t/m106-m107-g-codes-for-laser-inline-control-and-air-assist-setting/174900 · /t/gcodes-with-m4-instead-of-m3/37162 · /t/error-20-job-halted-custom-machine-with-fluidnc/179747 · /t/move-controls-greyed-out-with-fluidnc/173513 · /t/laser-not-shutting-off-between-profiles/110814 · /t/how-to-not-print-toolpath-between-objects/1771/2 · /t/lightburn-slowing-down-stuttering-during-pass/28488 · /t/prusa-specific-requirements-on-m106-m107-and-incorrect-m114-reading/176841 · /t/can-i-set-up-air-assist-with-marlin/25821

Foros adicionales: https://forum.makerforums.info/t/problem-with-lag-in-communications/81213 · https://forum.makerforums.info/t/hey-all-im-really-struggling-with-laser-engraving-on-my-smoothiboard/72257 · https://community.octoprint.org/t/octoprint-vs-smoothieware-m115-error/36984 · https://forum.v1e.com/t/marlin-515-not-turning-on-the-laser/34782 · https://git.xythobuz.de/thomas/marlin/commit/526ad82d20608d5262b5333684bcf7e85d18e431 (solo para dos valores numéricos por defecto, marcados arriba; no se copió código).
