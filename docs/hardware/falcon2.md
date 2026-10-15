# Creality Falcon2: evidencia pública y comparación con Tlaku

Fecha de la investigación: 2026-10-13. Método de sala limpia: solo documentación oficial de Creality, foros (LightBurn, Creality), fichas de tienda y salidas de consola que publicó la gente. No se leyó código fuente GPL (MeerK40t, LaserGRBL, etc.) ni se copiaron textos largos; todo está resumido con palabras propias y con la URL de cada dato.

**Leyenda de confianza**

- **OFICIAL**: página o manual de Creality / Creality Falcon.
- **VARIAS**: dos o más fuentes de la comunidad que coinciden.
- **UNA**: una sola fuente (puede ser un usuario con ajustes modificados; no tomar como regla).
- **NO HALLADO**: se buscó y no hay dato público.

**Aviso importante**: la Falcon2 clásica (22 W / 40 W), la Falcon2 Pro (22 / 40 / 60 W) y la Pro S son placas y firmwares distintos. Casi todas las salidas de consola públicas vienen de la **Pro** (y algunas de un modelo sin identificar) (maestra "CV50-Pro-MASTER" + módulo láser "CV50-LASER-22W"). La Falcon2 clásica del dueño probablemente se comporte igual en GRBL, pero **no hay una salida de consola publicada de la clásica con banner completo**; todo lo marcado como "Pro" hay que confirmar en cuanto se pueda conectar la máquina real (ver `$I` y `$$` al final).

---

## 1. Especificaciones

| Dato | Valor | Confianza | Fuente |
|---|---|---|---|
| Área de trabajo 22 W y 40 W | 400 x 415 mm | OFICIAL | https://crealityfalcon.com/blogs/product-tutorials-and-support/the-creality-falcon2-series-comparison |
| Área de trabajo Pro 22 W (anuncios de tiendas) | 400 x 415 mm (15,7 x 16,3 in) | VARIAS (tiendas) | https://slickdeals.net/f/18970864-creality-falcon-2-pro-22w-laser-engraver-and-cutter-fully-enclosed-diode-laser-15-7-x-16-3-in-work-area-camera-alignment-air-assist-with-code-cffam-719-1 |
| Área Pro S (con autoenfoque) | 355 x 390 mm (13,98 x 15,35 in) | UNA (ficha de tienda) | https://www.adorama.com/cr1005010221.html |
| Velocidad máxima de grabado 22 W / 40 W | 25 000 mm/min | VARIAS (tiendas); coincide con `$110=25000` en un volcado | https://www.adorama.com/cr1005010047.html |
| Aceleración | Sin cifra oficial. `$120=1000` (volcado 22 W/40 W) o `$120=5000` (volcado Pro); el perfil de LightBurn de Creality simula 3000 mm/s² | UNA cada una; contradictorias | ver sección 2 |
| Láser | Diodo azul 455 ± 5 nm; punto 0,08 x 0,1 mm (22 W) y 0,08 x 0,12 mm (40 W) | OFICIAL | URL del área de trabajo (comparación de series) |
| Placa / firmware | GRBL 1.1f modificado por Creality (compilación 2022-08-10), sobre ESP32-S3 | VARIAS (4 hilos) | https://forum.lightburnsoftware.com/t/falcon-2-pro-stuck-in-startup-loop/192185 , https://forum.creality.com/t/not-functioning-correctly/41237 , https://forum.lightburnsoftware.com/t/lightburn-no-longer-works-with-creality-falcon2-pro-laser-after-v2-0-05-update/185797 |
| Firmware en dos partes | "Maestra" (marco) `CV50-Pro-MASTER-Release V1.0.x` y módulo láser `CV50-LASER-22W/40W-Release V3.0.5`; el .bin es distinto por vatios | VARIAS | https://forum.creality.com/t/falcon-2-22w-firmware-update-clarifying/44631 , https://www.creality.com/support/creality-falcon2-22w |
| Baudios | 115 200 (perfil `.lbdev` de Creality) | OFICIAL (Creality publica el perfil) | https://forum.creality.com/t/profile-settings/36308 |
| Chip USB-serie | CH340 (Windows lo muestra como "USB-SERIAL CH340"); con los drivers CH341SER. En la Pro con USB-C se reporta que un cable C-a-C no funciona: hace falta A-a-C | UNA para el chip (Falcon2 40 W); VARIAS (2) para el cable | https://forum.lightburnsoftware.com/t/lightburn-falcon2-controller-firmware-is-stuck/190904 , https://3dpadvisor.com/blog/creality-falcon-2-troubleshooting/ |
| Conexión | USB-C + tarjeta TF (modo sin conexión: lee el último archivo de la TF al pulsar Frame). Software soportado oficialmente: LightBurn y LaserGRBL | OFICIAL / tiendas | https://www.3dtechvalley.com/creality-falcon-2-22w-laser-engraver-review/ |
| WiFi | NO HALLADO: ninguna fuente menciona WiFi en la Falcon2 | NO HALLADO | (búsquedas sin resultado) |
| Eje Z motorizado | No en 22 W / 40 W (perfil de Creality: `EnableZ:false`). La **Pro S** sí tiene autoenfoque motorizado (se mueve el carro completo); comando `$HZ1` reportado por un usuario | UNA para `$HZ1`; OFICIAL (perfil) para clásica | https://forum.creality.com/t/profile-settings/36308 , https://forum.creality.com/t/auto-focus/51926 |
| Asistencia de aire | Bomba integrada ("Integrated Air Assist" en la ficha de la 22 W); perilla de caudal en el lateral izquierdo | OFICIAL (ficha) / tabla oficial de errores | https://au.crealityfalcon.com/products/falcon2-pro-40w-and-22w-enclosed-laser-engraver-cutter , https://3dpadvisor.com/blog/creality-falcon-2-troubleshooting/ |
| Modo del módulo 40 W | Mantener Reset 2 s al encender alterna Normal 40 W / Preciso 20 W | OFICIAL | https://au.crealityfalcon.com/blogs/news/troubleshooting-common-lssues-with-creality-falcon2-tips-and-solutions-2 |

---

## 2. Ajustes `$$` publicados

Dos volcados completos hallados. Ambos son de máquinas de usuarios (pueden estar modificados).

| Ajuste | Volcado A (22 W/40 W, módulo V3.0.5; usuario con rodillo giratorio) | Volcado B (Falcon2 Pro 22 W, maestra V1.0.1) | Comentario |
|---|---|---|---|
| `$0` pulso de paso | 10 | 10 | |
| `$1` retardo de reposo | 250 | 250 | |
| `$2` / `$3` máscara de paso / dirección | 0 / 4 | 0 / 4 | |
| `$10` máscara de informe | 3 | 3 | MPos + Bf. Un tutorial de LaserWeb pide `$10=0` |
| `$13` informe en pulgadas | 0 | 0 | |
| `$20` límites blandos | 0 | 0 | |
| `$21` límites duros | **1** | **1** | |
| `$22` homing | **1** | **1** | |
| `$23` dirección de homing | **3** | **3** | Un usuario también la cita como 3 por defecto |
| `$24` / `$25` avance de localización / búsqueda | 2000 / 4000 | 2000 / 4000 | |
| `$26` rebote / `$27` retroceso | 20 / 5,000 | 20 / 5,000 | |
| `$30` S máx | **1000** | **1000** | |
| `$31` S mín | 0 | 0 | |
| `$32` modo láser | **1** | **1** | |
| `$100` pasos/mm X | 80 | 80 | La Pro necesita 80 (un usuario tenía 53,333 y fallaba) |
| `$101` pasos/mm Y | 60,375 (ajustado por el usuario para su rotativo) | 80 | 80 es el valor de fábrica |
| `$102` Z | no aparece | no aparece | |
| `$110` velocidad máx X (mm/min) | 25 000 | 50 000 | Difiere por versión |
| `$111` velocidad máx Y (mm/min) | 8000 | 50 000 | Un moderador sospecha que 8000 es un error |
| `$120` / `$121` aceleración X / Y (mm/s²) | 1000 / 1000 | 5000 / 5000 | Difiere |
| `$130` / `$131` recorrido X / Y (mm) | 400 / 415 | 400 / 400 | Difiere en Y |
| `$132` recorrido Z | no aparece | no aparece | |
| `$150` / `$151` bomba de aire apagado / encendido | 1 / 4 | 36 / 100 | Y otro tutorial: 30 / 100 |
| `$152` retardo de apagado del aire | 30 | 30 | |
| `$153` alarma de poco aire | 0 | 0 | |
| `$154` alarma de llama | 1 | 1 | |
| `$155` alarma de lente sucia | 1 | 1 | |
| `$156`, `$157`, `$158`, `$160`, `$161` | 0, 1, 1, 1, 13,00 | 0, 1, 1, 1, 13,00 | Significado desconocido |

Fuentes: volcado A en https://forum.lightburnsoftware.com/t/creality-falcon2-s-value-variable-power-issue/137714 ; volcado B en https://forum.lightburnsoftware.com/t/falcon-2-pro-stuck-in-startup-loop/192185 (el propio autor dice que su lista salió incompleta y cambió según el momento). El tutorial de LaserWeb cita `$150=30` / `$151=100` (https://forum.makerforums.info/t/laserweb-for-creality-falcon-2-how-to/92345, UNA) y dice que no coinciden con el manual de Creality. El significado de `$153` (poco aire), `$154` (llama) y `$155` (lente sucia) sale de un texto de ayuda sin marca visible, que por su contenido (luces AIR/FIRE/LENS) parece de Creality: https://forum.lightburnsoftware.com/uploads/short-url/yEYRKeuA7b4N9mR9TOgUCNkWWbW.txt (UNA); lo confirman citas de la tabla oficial de errores (`$154=0` para llama, `$155=0` para lente) en https://forum.creality.com/t/falcon-2-40w-alarm-wont-shut-off/22013 y https://3dpadvisor.com/blog/creality-falcon-2-troubleshooting/. La página del wiki de Creality de códigos de error (https://wiki.creality.com/en/laser-engraver/creality-falcon2-series-error-code) devolvió el cuerpo vacío al leerla.

**Qué aporta**

- `$21=1` y `$22=1`: **hay finales de carrera y homing**. LightBurn lo usa con `HomeOnStartup:true` en el perfil de Creality. Confianza: VARIAS.
- `$32=1` modo láser, `$30=1000`: confirmado en los dos volcados. VARIAS.
- Los ajustes de velocidad/aceleración **no son consistentes entre versiones** (25 000 vs 50 000; 1000 vs 5000). Hay que leerlos de la máquina, no suponerlos.
- Los ajustes `$150`-`$161` son propios de Creality (aire, alarmas) y no existen en GRBL 1.1 estándar.
- Hubo un caso donde el volcado de LaserWeb avisó de que `$#` mostraba `-nan` en G54 (ver sección 3).
- Dato de cambio de firmware sobre `$30`: ninguna fuente dice que el firmware lo cambie solo. El problema real publicado es el contrario: tras pasar a GRBL 1.1f, un usuario veía potencia muy baja (~1 W de un 40 W) hasta que "S-value max" de LightBurn y `$30` coincidían (1000); subir S máx a 10 000 era solo un apaño. UNA fuente con respuesta de un moderador. https://forum.lightburnsoftware.com/t/creality-falcon2-s-value-variable-power-issue/137714

---

## 3. Comportamiento observado

### Banner al conectar (texto exacto publicado, Falcon2 Pro 22 W)

```
usb connect succeed
[OK]system start-up CV50-Pro-MASTER-Release V1.0.1 20240325-1042
[OK] ILmp_app_init OK
[OK] gas_init OK
[MSG:Using cpu_map…esp32-s3]
Grbl 1.1f ['$' for help]
[OK] ILmp Link connect ok
```

- Hay líneas propias de Creality **antes y después** del saludo de GRBL (`usb connect succeed`, `[OK] ...`). Ninguna es exactamente `ok` (empiezan por corchete o por texto). Fuente: https://forum.lightburnsoftware.com/t/falcon-2-pro-stuck-in-startup-loop/192185 (UNA con el bloque completo; el saludo `Grbl 1.1f ['$' for help]` y `[MSG:Using cpu_map…esp32-s3]` aparecen además en https://forum.lightburnsoftware.com/t/alarm-3-ligthburn-version-2-0-05/189654 y https://forum.creality.com/t/not-functioning-correctly/41237: VARIAS).
- Tras un reinicio de la controladora a mitad de trabajo se imprime ese saludo otra vez, justo después de `ALARM:3`.
- Se vio también `[MSG:Restoring defaults]` / `[MSG:Restoring spindle]` en mitad de la sesión. UNA.

### `$I` y banner de versión

```
[VER:1.1f.20220810:]
[OPT:VHL,127,65536]
[software versions: CV50-Pro-MASTER-Release V1.0.0 20231129-1024]
inquire_info->power:22W ,CV50-LASER-22W-Release V3.0.5 20231129-1001
ok
```

- `[OPT:VHL,127,65536]` y `Bf:127,65535` en cada informe de estado indican: bloques de planificador 127 y **búfer de recepción de 65 536 bytes**, no 128. VARIAS (4 hilos): https://forum.lightburnsoftware.com/t/lightburn-no-longer-works-with-creality-falcon2-pro-laser-after-v2-0-05-update/185797 , https://forum.creality.com/t/not-functioning-correctly/41237
- En otro hilo `$I` devolvió solo `ok`, sin líneas de versión. UNA. https://forum.lightburnsoftware.com/t/falcon-2-pro-stuck-in-startup-loop/192185
- Firmware de una "Falcon 2" en LaserWeb: `CV50-Pro-MASTER-Release V1.0.0 20231129-1024` (https://forum.makerforums.info/t/laserweb-for-creality-falcon-2-how-to/92345) y módulo 40 W viejo `CV50-LASER-40W-Debug V3.0.2 20230823-1610` (https://forum.creality.com/t/falcon-2-40w-alarm-wont-shut-off/22013).

### Informes de estado reales

```
<Idle|MPos:0.000,0.000,0.000|Bf:127,65535|FS:0,0|Pn:XY|WCO:0.000,0.000,0.000>
<Home|MPos:-0.363,-0.363,0.000|Bf:127,65535|FS:5657,0|WCO:0.000,0.000,0.000>
<Run|MPos:17.225,17.862,0.000|Bf:126,65498|FS:3000,0>
```

- Estado `Home` durante `$H`; `Pn:XY` en reposo en la esquina de homing (los finales de carrera de X e Y están pulsados). Tras el homing la posición de máquina es ~0 y **crece en positivo hacia dentro de la cama** (en GRBL estándar sería negativa). Fuente: https://forum.lightburnsoftware.com/t/falcon-2-pro-stuck-in-startup-loop/192185 (UNA, Pro). Cuidado: un hilo de ALARM:3 muestra `MPos:-248.712,-233.062` pero no dice que sea una Falcon (https://forum.lightburnsoftware.com/t/alarm-3-ligthburn-version-2-0-05/189654), así que no sirve.
- No aparece `Ln:` ni `Ov:`; `FS:` sí.

### Origen, homing y límites

- Homing real con finales de carrera, `$22=1`, `$21=1`, `$23=3`. La máquina "debe hacer homing en la esquina de los interruptores" (respuesta de un usuario veterano). Perfil de Creality: `HomeOnStartup:true`, `MirrorX/Y:false`, 400 x 415, sin campo de origen (LightBurn asume esquina inferior izquierda). VARIAS. https://forum.creality.com/t/profile-settings/36308 , https://forum.lightburnsoftware.com/t/changing-default-home-location-for-creality-falcon-2-pro/184864
- Un usuario de la Pro dice que por defecto hace homing a la **esquina inferior izquierda** de LightBurn; al forzar el origen arriba a la izquierda con `$23=1`/`5` o `G10 L2 P1 X-0 Y-415`, no se mantuvo tras reiniciar. UNA. https://forum.lightburnsoftware.com/t/changing-default-home-location-for-creality-falcon-2-pro/184864
- Mover el cabezal con la mano hace perder la posición (no hay realimentación). VARIAS. https://forum.lightburnsoftware.com/t/creality-falcon-2-pro-22-w-issues-with-lightburn/191022
- **Los desplazamientos de trabajo G54-G59 están dañados**: `$#` o `[G54:-nan,1.579,1.619]` muestran `-nan`. Un usuario de LaserWeb dice que el firmware de Creality no soporta bien G54-G59 y trabaja con otra vía; un usuario pone `$RST=#` para restablecerlos. Fuentes: https://forum.makerforums.info/t/laserweb-for-creality-falcon-2-how-to/92345 , https://forum.lightburnsoftware.com/t/falcon-2-pro-stuck-in-startup-loop/192185 , https://forum.lightburnsoftware.com/t/creality-falcon-2-pro-22-w-issues-with-lightburn/191022. VARIAS (3 fuentes, todas Pro). Tlaku ya evita G54 (comentario en `gcode.rs`).
- `[G92:0.000,0.000,0.000]` aparece en el informe de parámetros, así que G92 existe. UNA.
- Una nota de LightBurn: `Error: 9` aparece si se activa auto-home en una máquina sin finales de carrera (no aplica a la Falcon2). https://docs.lightburnsoftware.com/2.0/Reference/DeviceSettings/BasicSettings/

### Alarmas propias de Creality: llama, inclinación, lente, aire, tapa

Creality NO usa solo códigos GRBL. Tiene una tabla propia de códigos que llega por serie como `ERROR:NN` (mayúsculas, dos dígitos). La tabla oficial (resumida):

| Código | Significado | Confianza |
|---|---|---|
| ERROR:01 | Ángulo anormal (sensor de inclinación / vibración); se detiene | OFICIAL |
| ERROR:02 | No hay archivo G-code en la raíz de la TF (.gcode/.gc/.nc) | OFICIAL |
| ERROR:04 | Sin flujo de aire (bomba / perilla / tubo) | OFICIAL |
| ERROR:05 | Llama detectada; se detiene. Reset y Start para seguir. Se desactiva con `$154=0` | OFICIAL |
| ERROR:06 | Lente contaminada. Reset, limpiar, Reset. Se desactiva con `$155=0` | OFICIAL |
| ERROR:07 | Módulo láser demasiado caliente | OFICIAL |
| ERROR:08 a 12 | Fallos de sensores (presión de aire, llama, lente, temperatura LD, cortocircuito LD): reiniciar | OFICIAL |
| ERROR:13 / 14 | Temperatura alta de la CPU del módulo / del anfitrión | OFICIAL |
| ERROR:24 | "Máquina no sellada": cierra la tapa y el cajón y pulsa Start (modelos con carcasa) | OFICIAL |
| ERROR:25 | Bomba de aire anormal | OFICIAL |

Fuentes: https://3dpadvisor.com/blog/creality-falcon-2-troubleshooting/ (transcribe la tabla oficial), https://uk.crealityfalcon.com/blogs/news/creality-falcon-2-troubleshooting y https://wiki.creality.com/en/laser-engraver/creality-falcon2-series-error-code (el wiki oficial: la página no se pudo extraer completa; un buscador cita los textos de ERROR:01, :04, :05).

Formato en el cable:

- Un usuario publicó la línea `ERROR:06.` seguida de un texto en inglés (lente contaminada) tras encender la máquina. UNA. https://forum.creality.com/t/falcon-2-40w-alarm-wont-shut-off/22013 . **No hay prueba pública** de si estos `ERROR:NN` van acompañados de un `ALARM:N` de GRBL, de un cambio de estado a `Alarm`, ni de si dejan de enviar `ok`.
- Distinto: `error:1` (minúsculas, estándar) apareció tras `[MSG:Restoring defaults]` en un hilo, y `error:2` ("Numeric value format...") por un "SS" olvidado en el G-code de inicio de LightBurn (error de configuración de usuario, ya resuelto). https://forum.lightburnsoftware.com/t/error-2-on-my-falcon2-22w/119233
- **ALARM:3** ("Reset while in motion") es lo más frecuente en foros de la Falcon2: reinicio de la placa durante un movimiento. Causas citadas: sensores (inclinación, llama), paro de emergencia, alimentación débil, USB, módulo de 40 W a más del 5-10 % de potencia. Sin causa confirmada. VARIAS (4 hilos): https://forum.lightburnsoftware.com/t/alarm-3-ligthburn-version-2-0-05/189654 , https://forum.lightburnsoftware.com/t/falcon-2-pro-40w-alarm-3/187925 , https://forum.creality.com/t/not-functioning-correctly/41237 , https://forum.lightburnsoftware.com/t/lightburn-no-longer-works-with-creality-falcon2-pro-laser-after-v2-0-05-update/185797
- Tapa / sensor: se ve `ERROR:24` en la tabla oficial. No hay evidencia de que use el estado `Door` de GRBL. NO HALLADO.

### Resto de comportamiento

| Pregunta | Respuesta | Confianza | Fuente |
|---|---|---|---|
| ¿Responde `ok` normal? | Sí, `ok` estándar tras cada línea y tras `$I`, `$#`, `$H` | VARIAS | los mismos hilos |
| M4 dinámico | Sí. Se recomienda el dispositivo "GRBL" de LightBurn (M4) y no "GRBL-M3": con M3 hay grabado más oscuro en esquinas / lados | VARIAS (2) | https://forum.lightburnsoftware.com/t/creality-falcon-2-40w/137856 , https://docs.lightburnsoftware.com/2.0/Guides/GRBLConfiguration/ |
| Asistencia de aire | `M8` encendido y `M9` apagado (el generador de LightBurn los usa y funciona). `AirAssistM7:false` en el perfil de Creality. El caudal de la bomba lo fijan `$150`/`$151`. Tras `M9` el aire sigue unos `$152` segundos (30 de fábrica; con `$152=0` para de inmediato) | VARIAS para M8/M9; UNA para `$152` | https://forum.creality.com/t/falcon2-40w-air-assist/40384 , https://forum.makerforums.info/t/laserweb-for-creality-falcon-2-how-to/92345 , https://forum.lightburnsoftware.com/t/lb-2-0-01-air-assist-stops-during-cut/175296?page=3 |
| Aire y alarma | Si el aire está encendido y no se detecta flujo, la alarma de aire detiene la máquina (`ERROR:04`, controlada por `$153`) | OFICIAL | tabla oficial arriba |
| Tamaño del búfer | `Bf:127,65535` y `[OPT:VHL,127,65536]` | VARIAS | sección anterior |
| Comandos propios | `$150`-`$161`, `$HZ1` (Pro S, no verificado); los botones del panel envían macros distintos del `$H` de la consola ("macro0, macro1" en la consola) | UNA cada uno | https://forum.lightburnsoftware.com/t/lightburn-no-longer-works-with-creality-falcon2-pro-laser-after-v2-0-05-update/185797 , https://forum.lightburnsoftware.com/t/changing-default-home-location-for-creality-falcon-2-pro/184864 |
| ¿Puerto serie o app? | Las dos vías oficiales son LightBurn/LaserGRBL por serie USB y la TF. Existe "Falcon Design Space" de Creality pero no hay datos de su protocolo | NO HALLADO para el protocolo | https://au.crealityfalcon.com/products/falcon2-pro-40w-and-22w-enclosed-laser-engraver-cutter |

---

## 4. Accesorios

- **Rotativo Creality (Rotary Kit Pro con mandril, y Rotary Roller con rodillos)**: se conecta **en el puerto del eje Y** (el usuario lo confirma). Reemplaza al motor Y. UNA. https://forum.creality.com/t/difficulty-setting-up-rotary-kit-pro/48503
- Valor oficial de Creality para "mm por vuelta": **40** en modo mandril. Con `$101=80`, 40 mm corresponde a 3200 pasos por vuelta, que cuadra con la fórmula de la comunidad: `$101 = 3200 / circunferencia de la pieza (mm)`. VARIAS (2 fuentes coherentes entre sí). https://forum.creality.com/t/difficulty-setting-up-rotary-kit-pro/48503 , https://forum.lightburnsoftware.com/t/creality-falcon-2-40w-laser-creality-rotary-pro-and-lightburn-not-working-together/187395
- Problema: con LightBurn el valor de mm por vuelta "no hace nada" (el mandril da ~5 vueltas con cualquier número entre 5 y 1000). La solución que funcionó fue cambiar `$101` por pieza y volver a 80 para trabajos normales. UNA (2 hilos del mismo problema). Es decir, el valor se aplica en la controladora, no en LightBurn.
- Diámetros: rodillo Falcon2 Pro de 5 a 120 mm de pieza, carga 10 kg; kit con elevadores de 1 a 110 mm. Tiendas. https://www.3djake.ch/de-CH/creality-3d-drucker-ersatzteile/rotary-roller-fuer-lasergravierer , https://3dstisk.com/produkt/creality-rotary-roller-kit-with-risers-for-laser-creality-falcon. **No se halló** el diámetro del rodillo motriz ni los "mm por vuelta" del Rotary Roller.
- **Eje Z**: sección 1.

---

## 5. Problemas conocidos de la comunidad

| Problema | Qué se sabe | Confianza | Fuente |
|---|---|---|---|
| Desfase bidireccional | Falcon 2 60 W: líneas ida/vuelta desplazadas ~0,3 mm; el usuario lo resolvió desactivando el relleno bidireccional. Otro usuario de Falcon2 mejoró con "scanning offset" (no publica valores) y 300 líneas/pulgada. **No hay tabla pública de scan offset** de la Falcon2 | UNA / NO HALLADO | https://forum.creality.com/t/falcon-2-60w-out-of-focus/29723 , https://forum.creality.com/t/fill-and-dimensional-accuracy-lightburn-cleaner-cuts-highly-recommended/47110 |
| Dobles líneas | En una 40 W: rueda excéntrica muy apretada | UNA | https://forum.creality.com/t/falcon-2-40w-laser-making-double-lines/35336 |
| Pérdida de pasos | Aparece junto a ALARM:3 (el mensaje lo dice). Correa floja, rieles sucios, cables atascados. `ERROR:01` a veces al llegar al límite X | UNA | https://forum.lightburnsoftware.com/t/error-01-with-creality-falcon-2/150260 |
| `$30` | Ver sección 2: no hay evidencia de que el firmware lo cambie, pero hay desajuste S máx / `$30` que da poca potencia | UNA | hilo 137714 |
| M4 | Funciona; M3 da peores esquinas. Perfil GRBL en LightBurn, no GRBL-M3 | VARIAS | sección 3 |
| Desconexiones USB | Cable solo de carga o C-a-C; falta del driver CH340; otro programa con el puerto abierto; autodetección de puerto falla (elegir el COM a mano); el módulo 40 W con ALARM:3 al subir la potencia; firmware del láser colgado a medias | VARIAS | https://forum.creality.com/t/connection-problems-with-creality-falcon-2-22w/36882 , https://3dpadvisor.com/blog/creality-falcon-2-troubleshooting/ , https://forum.lightburnsoftware.com/t/lightburn-falcon2-controller-firmware-is-stuck/190904 |
| Actualización 2.0.05 de LightBurn | Dejó de funcionar con una Pro tras actualizar; sin resolución en el hilo | UNA | https://forum.lightburnsoftware.com/t/lightburn-no-longer-works-with-creality-falcon2-pro-laser-after-v2-0-05-update/185797 |
| G-code de inicio equivocado | `SS` en el G-code de inicio de LightBurn provoca `error:2`; limpiar inicio/fin lo arregla | UNA | https://forum.lightburnsoftware.com/t/error-2-on-my-falcon2-22w/119233 |

---

## 6. Diferencias con Tlaku

Código revisado: `crates/tlaku-doc/src/device.rs`, `apps/desktop/src/api/mock/sample.ts`, `crates/tlaku-grbl/src/{gcode,streamer,preflight,serial}.rs`, `apps/desktop/src/features/machine/grblCodes.ts`, `apps/desktop/src-tauri/src/machine.rs`, `crates/tlaku-pipeline/src/rotary.rs`.

Gravedad: **rompe** = trabajo o seguridad mal manejados; **riesgo** = puede dar resultados malos o confusos; **menor** = cosmético o de ajuste.

| # | Qué asumimos (dónde) | Qué dice la evidencia | Gravedad | Cambio sugerido | Confianza de la evidencia |
|---|---|---|---|---|---|
| 1 | Los errores son `error:N` en minúsculas. `streamer.rs` (`strip_prefix("error:")`, sensible a mayúsculas) y `serial.rs` / `machine.rs` (`starts_with("error")`) los detectan así. Cualquier otra línea es `Event::Message`. | Creality manda `ERROR:NN` (mayúsculas, dos dígitos; p. ej. `ERROR:05` llama, `ERROR:01` inclinación, `ERROR:06` lente). Con el streamer actual esa línea se ignora como mensaje: el trabajo sigue "esperando ok", no se avisa al usuario ni se cancela, y se queda colgado hasta el tiempo de silencio (que no salta porque `?` sigue respondiendo). | **rompe** (seguridad: llama / inclinación) | **Resuelto:** `parse_creality_error` y `creality_error_message` en `crates/tlaku-grbl/src/streamer/firmware.rs` reconocen `ERROR:NN` (mayúsculas, con variantes `ERROR:06.`) y el streamer lo trata como falla de máquina (`CREALITY_RECOVERY` da el consejo). Reconocer `^ERROR:(\d+)` como falla de la máquina (no como línea rechazada): nuevo `Outcome::MachineFault(code)` que detiene el envío, muestra el texto de la tabla oficial y sugiere Reset del panel (física), sin enviar `$X`. Añadir la tabla de la sección 3. | Tabla: OFICIAL. Formato `ERROR:NN` en el cable: UNA (hilo 22013) |
| 2 | `grblCodes.ts` usa `/^\s*error:\s*(\d+)/i` (insensible a mayúsculas). | `ERROR:05` (llama) se leería como GRBL `error:5` ("ajuste desactivado") y `ERROR:04` como `error:4` ("valor negativo"). Mensajes engañosos. | **riesgo** | **Resuelto:** la interfaz tiene su propio diccionario de códigos de Creality (`features/machine/grblCodes.ts`), separado del de `error:N`. Separar `parseErrorLine` (solo minúscula) de `parseCrealityError` y su propio diccionario i18n. | Igual que arriba |
| 3 | `rx_buffer` por defecto 127 bytes (`device.rs`, `StreamerConfig`) tomado como "búfer RX de GRBL 1.1". | La Falcon2 informa `[OPT:VHL,127,65536]` y `Bf:127,65535`: 127 son **bloques del planificador** y el RX es 65 536 bytes. 127 bytes sigue siendo válido (conservador) pero mantiene solo unas 8-10 líneas en vuelo; para trozos cortos a 25 000 mm/min el planificador puede quedar corto. | **menor** (rendimiento, no seguridad) | Conservar 127 por defecto, leer `[OPT:` / `Bf:` al conectar y mostrar el valor; permitir un valor mayor en el perfil de la Falcon2 tras probar con hardware. | VARIAS (4 hilos) |
| 4 | `sample.ts` Falcon2 22 W: `accel [3000,3000]`, `max_rate [30000,30000]`. | Máximo anunciado y en `$110`: 25 000 mm/min. Aceleración en firmware: 1000 (volcado A) o 5000 (Pro); 3000 solo en la simulación del perfil de LightBurn. 30 000 supera el techo; el estimador de tiempo saldría optimista. Un volcado tiene `$111=8000` (Y). | **riesgo** (tiempos y planificación) | `max_rate [25000,25000]`; aceleración conservadora 1000 hasta leer `$120`/`$121`; al conectar, leer `$110`-`$121` y avisar de diferencias con el perfil. | `max_rate`: OFICIAL (tienda) + UNA; aceleración: UNA cada una |
| 5 | `rapid_rate` 12 000 mm/min (sample) / 12 000 (defecto). | GRBL mueve los G0 a `$110`/`$111` (25 000 y en un volcado Y 8000). | **menor** | Usar `min($110,$111)` cuando se lea; si no, 25 000. | UNA |
| 6 | `RotarySettings::default()` y `DEFAULT_ROTARY`: `mm_per_rotation = 360`. | Creality documenta 40 mm por vuelta en modo mandril (3200 pasos/vuelta a `$101=80`). Con 360, y_scale saldría 9 veces mayor y el rotativo daría ~9 vueltas por vuelta de pieza si `$101` es 80. | **riesgo** (un rotativo mal calibrado puede chocar o dar un diseño ilegible) | Perfil Falcon2 con `rotary.kind = chuck`, `mm_per_rotation = 40`; no cambiar el default genérico pero avisar en la UI de que depende del `$101` y de los pasos por vuelta del accesorio. Documentar que Tlaku escala en software, así que `$101` debe estar en 80 (no ajustar `$101` por pieza como se hace en LightBurn). | VARIAS (2), coherentes |
| 7 | `gcode.rs` no usa G54-G59, y emite G0 X0 Y0 al final (`return_to_origin`). | Coincide: los G54-G59 de Creality dan `-nan`. El origen de trabajo debe ser el de la máquina tras homing (o G92). | **menor** (ya correcto) | Mantener. Añadir al preflight una lectura opcional de `$#` y avisar si hay `nan` y que se recomiende `$RST=#` solo si el usuario lo decide. Probar G92 en hardware. | VARIAS (3, todas Pro) |
| 8 | `StatusReport::parse` convierte cadenas con `parse::<f64>()`. | En Rust `"-nan"` se lee como `NaN`. Si una línea de estado trae `nan` (WCO) llegaría `NaN` al UI y a restar MPos - WCO. | **riesgo** | **Resuelto:** `crates/tlaku-grbl/src/streamer/status.rs` descarta valores no finitos (`nan`, `-nan`, `inf`). Rechazar no finitos en `parse_position`. | UNA (en `$#`, no en `?` aún) |
| 9 | Perfil: origen `bottom_left`, cama 400 x 415. | Coincide con el perfil de Creality y el comportamiento descrito (homing abajo a la izquierda, MPos positivo hacia dentro). | **menor** (correcto) | Sin cambio. Confirmar con `?` tras `$H` en la máquina real. | VARIAS |
| 10 | `Grbl::open`: envía `\r\n\r\n`, espera 2 s y descarta el saludo; `command("$I")` espera `ok`. | La Falcon2 imprime líneas propias antes y después del saludo (hasta 7 líneas). Un hilo muestra `$I` devolviendo solo `ok`. Con la espera de 2 s las líneas del saludo se descartan, lo cual es seguro; lo importante es que `firmware_name` y la identificación toleren la respuesta vacía o con `[software versions: ...]` extra. | **menor** | Prueba de regresión con el volcado real (sección 8). Mostrar `[software versions:]` y `inquire_info` en el registro. | UNA (banner) / VARIAS (VER/OPT) |
| 11 | `alarm_advice` y `check_ready` suponen alarma GRBL estándar, `Door` para la tapa. | La Falcon2 usa `ERROR:24` (tapa/cajón) y `ERROR:01/05/06` y se recupera con **Reset físico + Start en el panel**. Con `$22=1` el consejo `$H` después de `ALARM:3` es correcto. | **riesgo** (consejos inútiles ante alarma de Creality) | **Resuelto en parte:** `alarm_advice` ya usa `CREALITY_RECOVERY` ante un `ERROR:NN` (`crates/tlaku-grbl/src/preflight.rs`). Añadir consejos para los `ERROR:NN` (ver arriba); mantener `$H` tras ALARM:3. | OFICIAL |
| 12 | `s_max = 1000` y M4 por defecto. | Correcto (`$30=1000`, `S_Scale 1000`, GRBL con M4). Pero si LightBurn/otro lleva S máx distinto de `$30` la potencia se escala mal. | **riesgo** (potencia) | Al conectar leer `$30` y `$32`; avisar si `$30 != s_max` o `$32 != 1`. | VARIAS |
| 13 | G-code usa `M8`/`M9` para el aire y termina `M9`, `M5`. | Coincide con lo que genera LightBurn para la Falcon2. Detalles: el aire sigue `$152` s tras `M9` (30 s); sin flujo de aire se detiene con `ERROR:04` si `$153=1`. | **menor** | Documentar y mostrar aviso si una capa pide aire. No enviar `$150`-`$153` por cuenta propia. | VARIAS (M8/M9), UNA (`$152`) |
| 14 | `scan_offsets: []` por defecto en la Falcon2. | No hay valores públicos; el único dato es ~0,3 mm de desfase en una 60 W. | **menor** | Dejar vacío y ofrecer la calibración; documentar en la UI. | UNA |
| 15 | Cabezal puede llegar al borde de la cama con overscan. | Un usuario ve `ERROR:01` (inclinación) al llegar al límite X y apagar el overscan no lo evitó. | **menor** | Avisar si objeto + overscan queda a menos de ~2 mm del borde. | UNA |
| 16 | `z_axis: false` en el perfil. | Correcto para 22 W / 40 W; la Pro S tiene autoenfoque (distinto, 355 x 390). | **menor** | Perfil Pro S aparte. | OFICIAL / UNA |
| 17 | Ruta de conexión: baud 115 200 desde `device.rs`. | Coincide con el perfil de Creality. Faltan consejos para el cable (A-a-C) y el driver CH340. | **menor** | Mensaje de ayuda cuando no se abre el puerto. | VARIAS |

---

## 7. Perfil recomendado de la Falcon2

Valores para un `DeviceProfile` "Creality Falcon2 22 W / 40 W". Todo lo que dice "leer de la máquina" se debe comprobar con `$$` en cuanto haya hardware.

| Campo | Valor | Origen / confianza |
|---|---|---|
| `kind` | `grbl` | OFICIAL |
| `bed` | `[400, 415]` | OFICIAL |
| `origin` | `bottom_left` | VARIAS (perfil de Creality + hilo de homing); confirmar con `?` tras `$H` |
| `s_max` | `1000` | VARIAS (`$30=1000`, `S_Scale 1000`) |
| `power_mode` | `dynamic` (M4) | VARIAS |
| `accel` | `[1000, 1000]` hasta leer `$120`/`$121` (en la Pro llega a 5000) | UNA cada cifra; contradictorias |
| `max_rate` | `[25000, 25000]` (la Pro llega a 50 000; Y en un volcado 8000) | OFICIAL (tienda) |
| `rapid_rate` | `25000` | UNA; leer `$110`/`$111` |
| `baud` | `115200` | OFICIAL |
| `rx_buffer` | `127` (seguro). El hardware dice 65 536 B de RX y 127 bloques; probar con hardware | VARIAS |
| `start_gcode` | vacío | OFICIAL (perfil de Creality con inicio vacío) |
| `end_gcode` | vacío (Tlaku ya emite `M9`, `M5`) | OFICIAL |
| `return_to_origin` | `true` (con homing al inicio, X0 Y0 es la esquina de los interruptores) | VARIAS |
| `z_axis` | `false` (Pro S: `true`, cama 355 x 390) | OFICIAL / UNA |
| `rotary` | `kind: chuck`, `mm_per_rotation: 40`, `enabled: false`; `$101` debe quedar en 80 | VARIAS (2) |
| `scan_offsets` | vacío hasta calibrar con la máquina | NO HALLADO |
| Ajustes a leer al conectar | `$30=1000`, `$32=1`, `$21=1`, `$22=1`, `$23=3`, `$100=$101=80`, `$130=400`, `$131=415`, `$154/$155`, `$152` | VARIAS |

---

## 8. Casos de prueba automatizables sin máquina

Todos se pueden escribir con `memory_pair` / `MemoryTransport` y el `Streamer` existentes (ya hay pruebas de este estilo en `serial.rs`, `streamer.rs` y `machine/tests.rs`).

1. **Banner completo de la Pro.** El extremo simulado envía las 7 líneas del banner (sección 3) al abrir. Esperado: ninguna línea que no sea `ok` termina un comando; `Grbl::open` no falla; `firmware_name` devuelve "Grbl 1.1f" a partir de `[VER:1.1f.20220810:]` aunque haya `[OPT:VHL,127,65536]` y `[software versions: ...]`.
2. **`$I` con solo `ok`.** `command("$I")` devuelve lista vacía; la conexión debe seguir adelante (nombre genérico "GRBL").
3. **Informes de estado reales.** Analizar las tres líneas de la sección 3: `Home` con MPos negativo pequeño, `Pn:XY`, `Bf:127,65535` con `rx_bytes` 65 535, `Run` con `Bf:126,65498`. Esperado: estado y campos correctos, sin pánico.
4. **`ERROR:05`, `ERROR:06`, `ERROR:24` durante el envío.** El extremo simulado responde `ERROR:05` en vez de `ok`. Esperado tras el cambio: el `Streamer` termina con una falla de máquina y no queda colgado; hoy el test fallaría y documenta el bug 1. Además, un test de `grblCodes.ts`: `parseErrorLine("ERROR:05")` debe ser `null` (no `5`).
5. **`ALARM:3` seguido del saludo.** Tras `ALARM:3` el simulador manda el banner; esperado `Outcome::Alarm(3)`; el consejo del preflight con `$22=1` incluye `$H` y menciona "posición perdida".
6. **Lectura de `$$` real.** Alimentar los dos volcados de la sección 2 a `read_homing_setting` (esperado `Some(true)`) y, cuando se implemente, a una comprobación de perfil que avise de `$30 != s_max`, `$130/$131 != bed`, `$32 != 1`. Incluir las líneas `$150`-`$161` y `$161=13.00`.
7. **G54 con `-nan`.** La línea `[G54:-nan,1.579,1.619]` en la respuesta de `$#` no debe provocar `NaN` en el UI; y un `WCO:nan,...` en un informe de estado debe ignorarse (arreglo del punto 8).
8. **Búfer de caracteres.** Con un extremo que responde `Bf:127,65535` y `ok` con retardo, el `Streamer` nunca debe tener más de `rx_buffer` bytes en vuelo, y ninguna línea del G-code de la plantilla de ejemplo con el perfil Falcon2 debe pasar de 126 caracteres.
9. **G-code dorado del perfil Falcon2.** Generar el documento de ejemplo y comprobar: sin `G54`-`G59`/`G10`/`G92`; `M4 S0` al empezar; ningún `S` mayor que 1000; ningún `F` mayor que 25 000; `M8`/`M9` emparejados; termina en `M9`, `M5`, `G0 X0 Y0`.
10. **Rotativo Falcon2.** Con mandril, 40 mm/vuelta y una pieza de 236 mm de circunferencia (75,1 mm de diámetro), `y_scale` debe ser 40/236 = 0,1695 y la velocidad sobre la superficie se mantiene; comprobar la equivalencia con la fórmula de la comunidad (`$101 = 3200 / 236` = 13,56 pasos/mm).
11. **Desconexión.** Soltar el extremo simulado a mitad de trabajo produce `Outcome::Disconnected`; si antes llegó un `ALARM:3` y un saludo, el desenlace es la alarma y no la desconexión.

Pendientes que solo se resuelven con la máquina física: `$I` real de la clásica, si `ERROR:NN` viene con `ALARM:N`, si el RX de 65 536 B es utilizable, valores reales de `$110`-`$121`, valores de scan offset (no existen datos públicos) y si G92 se conserva.

---

## 9. Fuentes principales

- Comparación oficial de series: https://crealityfalcon.com/blogs/product-tutorials-and-support/the-creality-falcon2-series-comparison
- Perfil `.lbdev` de Creality en el foro de Creality: https://forum.creality.com/t/profile-settings/36308
- Volcado `$$` A y hilo de potencia: https://forum.lightburnsoftware.com/t/creality-falcon2-s-value-variable-power-issue/137714
- Volcado `$$` B, banner, `$I`, informes de estado: https://forum.lightburnsoftware.com/t/falcon-2-pro-stuck-in-startup-loop/192185
- ALARM:3, Pro 22 W: https://forum.creality.com/t/not-functioning-correctly/41237
- Tabla `ERROR:NN`: https://3dpadvisor.com/blog/creality-falcon-2-troubleshooting/ , https://uk.crealityfalcon.com/blogs/news/creality-falcon-2-troubleshooting , https://wiki.creality.com/en/laser-engraver/creality-falcon2-series-error-code
- Alarmas del módulo (`$153`/`$154`/`$155`): https://forum.lightburnsoftware.com/uploads/short-url/yEYRKeuA7b4N9mR9TOgUCNkWWbW.txt
- Firmwares: https://www.creality.com/support/creality-falcon2-22w , https://forum.creality.com/t/falcon-2-22w-firmware-update-clarifying/44631
- LaserWeb (`$I`, `$150`-`$152`, G54): https://forum.makerforums.info/t/laserweb-for-creality-falcon-2-how-to/92345
- Homing y origen: https://forum.lightburnsoftware.com/t/changing-default-home-location-for-creality-falcon-2-pro/184864 , https://forum.lightburnsoftware.com/t/creality-falcon-2-pro-22-w-issues-with-lightburn/191022
- Rotativo: https://forum.creality.com/t/difficulty-setting-up-rotary-kit-pro/48503 , https://forum.lightburnsoftware.com/t/creality-falcon-2-40w-laser-creality-rotary-pro-and-lightburn-not-working-together/187395
- Aire: https://forum.creality.com/t/falcon2-40w-air-assist/40384
- M3/M4: https://forum.lightburnsoftware.com/t/creality-falcon-2-40w/137856 , https://docs.lightburnsoftware.com/2.0/Guides/GRBLConfiguration/
- Conexión USB: https://forum.lightburnsoftware.com/t/lightburn-falcon2-controller-firmware-is-stuck/190904 , https://forum.creality.com/t/connection-problems-with-creality-falcon-2-22w/36882

## 10. Primera sesión con una Falcon2 real (2026-10-14, macOS)

Primera prueba con hardware de Tlaku, niveles N1 de [validacion-hardware.md](../validacion-hardware.md)
(solo lectura, sin mover ni encender el láser). Máquina del autor, Falcon2 22 W, conectada a la Mac.

- **Puerto:** aparece como `/dev/cu.usbmodem1234561`, dispositivo USB «Espressif Device»
  (VID `0x303A`, PID `0x4001`): es el USB nativo del ESP32-S3, **no** un CH340, así que en macOS no hace
  falta driver. Se desconectó y volvió a aparecer sola una vez al conectar la cámara al mismo hub: mejor
  conectarla directa a la computadora.
- **Cámara:** «Creality Falcon Camera» (VID `0x1BCF`, PID `0x0B19`), UVC estándar; la reconoce el sistema.
- **`$I`:** `[VER:1.1f.20220810:]`, `[OPT:VHL,127,65536]` y
  `[software versions: CV50-Pro-MASTER-Release V1.0.0 20231129-1024]`: la clásica usa el firmware de la
  familia Pro.
- **`$$`:** coincide con lo esperado (`$30=1000`, `$32=1`, `$22=1`, `$21=1`, `$23=3`, `$100=$101=80`,
  `$130=400`, `$131=415`, `$150` a `$161` presentes). Además: `$3=4` (Y invertido), **`$110=25000` y
  `$111=8000`** (la Y es mucho más lenta que la X), `$120=$121=1000`, `$11=0.010`, `$27=5.000`.
- **`?` en reposo:** `<Idle|MPos:0.000,0.000,0.000|Bf:127,65535|FS:0,0|Ov:100,100,100|A:S>`; los
  siguientes, sin `Ov` ni `A`. El primero trae `A:S` (accesorio «husillo») aunque `$G` dice `M5`.
- **`$G`:** `[GC:G1 G54 G17 G21 G90 G94 M5 M9 T0 F600 S0.000]`.
- **`$#`:** las coordenadas de trabajo traen **`-nan`** en X (`[G54:-nan,0.008,0.000]`, igual G55 a G59,
  G28 y G30). Tlaku descarta los valores no finitos (`finite_list`), así que no producen posiciones
  rotas; conviene no depender de G54 en esta máquina.
