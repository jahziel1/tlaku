# 03 - Conexión/control de máquinas y funciones avanzadas (inventario funcional)

Alcance: área "máquinas y funciones avanzadas" del reemplazo open source de LightBurn en Rust.
Fecha de investigación: 2026-10-12. Versiones de referencia: documentación oficial 2.1 (con cotejo puntual 2.0/1.7), foro oficial, repos open source.

## 0. Reglas de sala limpia y convenciones

- Todo el comportamiento está descrito con palabras propias a partir de documentación pública y hilos del foro. No se copió texto ni capturas. No se consultó ni descompiló código de LightBurn.
- Cada función cita su URL fuente. Prefijo de las URL oficiales: `D21 = https://docs.lightburnsoftware.com/2.1/`. Foro: `https://forum.lightburnsoftware.com/t/...`.
- "(no verificado)" = no pude confirmarlo en una fuente durante esta sesión. No se debe asumir cierto.
- Complejidad: B = baja (días), M = media (1-3 semanas), A = alta (1-2 meses), MA = muy alta / riesgo de investigación (meses o incierto).
- Prioridad: P0 = MVP, P1 = importante, P2 = avanzado.
- Los datos de crates (versiones/fechas) se consultaron en la API de crates.io el 2026-10-12.

Aviso sobre la documentación: la página de la versión 2.0 de "New Features" devolvió 404 y la de 1.7 de calibración de cámara también; lo que se cita de 2.0 viene de otras páginas.

---

## 1. Tabla resumen de funciones

| # | Función | Qué hace (resumen propio) | Prio | Compl. | Referencia open source |
|---|---|---|---|---|---|
| 1 | Asistente "Find My Laser" (autodetección USB) | Escanea puertos USB, lista dispositivos reconocidos, crea el perfil. No detecta lasers de red. Marlin no es compatible con la detección (reinicia muy lento). | P1 | M | LaserGRBL (detección de puerto), MeerK40t (autodetección por dispositivo) |
| 2 | Creación manual de dispositivo (wizard de 6 pasos) | Tipo de controladora, tipo de conexión (Serial/USB, Ethernet-WiFi/TCP, Packet/USB, Bridge), IP, nombre y área, origen y homing, resumen. Incluye "No Machine" para trabajar sin láser. | P0 | M | Rayforge (perfiles de máquina), MeerK40t |
| 3 | Gestor de dispositivos y perfiles | Lista, importar (.lbdev/.lbzip/.lbvendor), exportar (en 2.x como bundle .lbzip), por defecto, duplicar, renombrar, icono, ordenar. | P0 | B | Rayforge (paquetes de perfil importables) |
| 4 | Bundles de usuario (.lbzip) | Un solo archivo con ajustes, atajos, dispositivos, cámaras, librerías de materiales y arte, para migrar entre equipos. Los dispositivos en conflicto se duplican, no se sobrescriben. | P1 | M | - |
| 5 | Device Settings: área, origen, unidades | Tamaño de trabajo, origen (esquina), offset del láser (puntero rojo), unidades de dispositivo vs control (diseño en pulgadas, velocidad en mm/s o mm/min), enlace de unidades. | P0 | M | Rayforge, MeerK40t |
| 6 | Device Settings: GRBL/G-code básicos | Auto-home al inicio, swap X/Y, S-value máx, baud, puerto de red (23 por defecto, 8080 alternativo), DTR, modo de transferencia (Buffered/Synchronous), $J jogging, fast whitespace, return to finish position, aire M7/M8, clustering. | P0 | M | LaserGRBL, Rayforge |
| 7 | Modos M3/M4 y dispositivo "GRBL" vs "GRBL-M3" | GRBL 1.1f+ usa M4 (potencia dinámica con $32=1). GRBL 1.1e o anterior usa M3 (potencia constante). | P0 | B | LaserGRBL, Rayforge |
| 8 | Custom G-code (flavor, plantillas, variables) | Define dialecto, plantillas por evento (mover, tool on/off, aire, dwell, home, abort, pausa, estado...), variables de sustitución, macros de modo relativo/absoluto, bytes de control. | P1 | A | Rayforge (dialectos editables), MeerK40t |
| 9 | Start / End scripts G-code | Texto G-code libre al inicio y fin de cada trabajo. LightBurn no lo valida. | P0 | B | Rayforge (macros/hooks) |
| 10 | Ventana Laser: Start/Pause/Stop | Inicia, pausa/reanuda y aborta el trabajo; atajos de teclado; estado Disconnected/Ready/Busy/Framing. | P0 | M | LaserGRBL |
| 11 | Frame (bounding box) | Recorre el rectángulo envolvente a la velocidad de la ventana Move. Opción de repetir continuamente; con láser encendido a baja potencia (diodo, Shift). | P0 | B | LaserGRBL |
| 12 | Rubber-band frame (casco convexo) | Recorre un contorno ajustado que envuelve las formas, útil para piezas diagonales. | P1 | M | - (algoritmo de casco convexo estándar) |
| 13 | Start From: Absolute Coords / User Origin / Current Position | Define cómo se coloca el trabajo sobre la cama. Absolute fija el 0,0 al origen de máquina. User Origin y Current Position colocan el trabajo relativo a un punto. | P0 | M | LaserGRBL (parcial), Rayforge (G54-G59), MeerK40t |
| 14 | Job Origin de 9 puntos | Elige qué punto del cuadro envolvente (esquina, medio de lado o centro) coincide con el punto de inicio. Se desactiva en Absolute. | P0 | B | - |
| 15 | Cut Selected Graphics / Use Selection Origin | Envía solo la selección; el origen puede calcularse con la selección o con todo el diseño. | P0 | B | - |
| 16 | Ventana Move: jog, home, origen, finish position | Flechas con distancia y velocidad, jog continuo ($J), Z/A, mover a posición, posiciones guardadas, "Move from machine zero", Set/Clear Origin, Set Finish Position, Fire y Focus Z. | P0 | M | LaserGRBL, Rayforge |
| 17 | Position Laser (clic en el espacio de trabajo) | Mueve el cabezal a donde se hace clic; se desactiva solo tras unos 10 s. | P1 | B | - |
| 18 | Ajuste de velocidad/potencia durante el trabajo (GCode) | Porcentaje sobre el valor original mientras corre el trabajo, con botón de reset. | P1 | B | LaserGRBL (feed override) |
| 19 | Consola | Muestra tráfico y mensajes del controlador; envío de comandos solo en GRBL ($I, $$, $#, ?, $X, $n=v, RST=...). Solo máquinas G-code. | P0 | B | LaserGRBL, MeerK40t (console) |
| 20 | Macros | Botones con comandos G-code guardados (reemplazaron los 6 slots antiguos en 2.0). | P1 | B | LaserGRBL (botones personalizados), Rayforge |
| 21 | Guardar/ejecutar archivos de máquina | Exporta .gc (GRBL/Smoothie/Marlin), .rd (Ruida), .out (TopWisdom), .ud5 (Trocen). "Run Machine Files" ejecuta uno ya guardado. | P0 (G-code), P1 (RD) | B (G-code), A (RD) | MeerK40t (loader/parser .rd), edutechwiki Ruida |
| 22 | Importar G-code / .rd / .plt / .nc / .hpgl / .sc | Abre archivos de máquina como geometría. | P2 | M | MeerK40t (rdjob/loader) |
| 23 | Send a la memoria del controlador (DSP) | Sube el trabajo como archivo con nombre para ejecutarlo desde el panel; Shift = enviar e iniciar; autoincremento de nombre; opción de sobrescribir sin aviso. | P1 | A | edutechwiki Ruida, MeerK40t |
| 24 | File List (archivos en controlador) | Refrescar, iniciar, subir, borrar, borrar todos, calcular tiempo (Ruida, simulado por el controlador). | P1 | M | - (no verificado en código abierto) |
| 25 | Machine Settings (Ruida/TopWisdom/GRBL) | Lee/escribe parámetros de firmware, guarda/carga .lbset, respaldo automático en cada escritura, calibrar eje (steps/mm o step length). | P1 | A | Rayforge (lee/escribe $$ de GRBL); Ruida: edutechwiki (tabla de parámetros DA) |
| 26 | Get Controller Info (Ruida) | Horas encendido, trabajos, recorrido acumulado y versión de firmware. | P2 | B | edutechwiki Ruida (lectura de parámetros) |
| 27 | Scanning Offset Adjustment | Tabla de compensaciones por velocidad de escaneo para corregir el retraso de disparo; lee/escribe en Ruida; importable. | P1 | M | - |
| 28 | Rotary DSP (Ruida) | Chuck/Roller, pasos por rotación, diámetros, circunferencia, espejo, prueba de giro. Se escribe al controlador y requiere reiniciar. | P1 | M | MeerK40t (módulo rotary) |
| 29 | Rotary GCode (eje Y reemplazado, eje Z/A) | Chuck/Roller, mm por rotación, pasos por grado, límites suaves ($20). | P1 | M | Rayforge (rotary 4.º eje o reemplazo de Y) |
| 30 | Rotary galvo | Split size, overlap, correr formas completas, centro de salida, frame por rebanadas, sanity check. | P2 | A | MeerK40t (balormk/rotary) |
| 31 | Eje Z / Focus Z | Habilitar Z, invertir, solo movimientos relativos, optimizar Z; Z por capa (material height); autoenfoque en algunos equipos. | P1 | M | LaserGRBL (mínimo) |
| 32 | Eje U y Laser 2 | U Axis con offset por capa; dual laser: nombres, solo uno a la vez, offset del cabezal 2. | P2 | M | - |
| 33 | Cámara: calibración de lente | Asistente con 5+ capturas de patrón AprilTag (esquinas y centro), 4 modelos de lente comparables, exporta .lbcm. | P1 | A | Rayforge (calibración fisheye), MeerK40t (camera) |
| 34 | Cámara: alineación | 8 AprilTags (mín. 4 detectados) grabados o 4 cruces marcadas a mano; calcula la posición de la cámara respecto a la cama; guardada por altura de material. | P1 | A | Rayforge |
| 35 | Cámara: overlay, trace, captura | Update Overlay, opacidad, DPM, tamaño, desplazamientos; trace desde el overlay; guardar fondo como bitmap; cámaras USB y de red (MJPEG, no RTSP); presets. | P1 | A | Rayforge (fondo y trace) |
| 36 | Cámara montada en cabezal | Compone imágenes tomadas moviendo el cabezal en un overlay. | P2 | A | - |
| 37 | Print & Cut | Alinea el trabajo a dos marcas físicas (rotación + escala uniforme opcional), en coordenadas absolutas. | P1 | M | - |
| 38 | Texto variable / serie / CSV | Modos fecha-hora, serial, merge CSV y valores de ajustes de corte; Current/Start/End/Advance, autoavance, Bake, Offset por objeto. | P1 | M | - |
| 39 | Tiling / divisiones de trabajos grandes | NO hay una función "Tiling" en LightBurn 2.1 (la navegación de docs no la tiene). Se resuelve con Cut Shapes + Print & Cut + registro manual. Para galvo existe Split Marking. | P2 | A | - |
| 40 | Split Marking y Repeat Marking (galvo) | Marcar más grande que el campo con mesa lineal o rotativa; repetir marcado con incremento. | P2 | A | MeerK40t (balormk) |
| 41 | Galvo: asistente de alta | Importar markcfg7 (EZCAD2), BslCAD.cfg/LmcPar.cfg (BSL) o markcfg0 (+Motors.ini, COR) (EZCAD3); tipo de láser, lente, campo. | P2 | A | galvoplotter, MeerK40t balormk |
| 42 | Galvo: framing con luz roja | Bounds / Hull / Contour, frame individual, repetición, contador, Stop At, teclas para mover/escalar/rotar en vivo. | P2 | A | galvoplotter (`light`), MeerK40t (livelightjob) |
| 43 | Galvo: calibración de campo/lente | Asistente de 9 puntos (mide líneas marcadas), COR import, escala/bulge/skew/trapezoid por galvo, offset y ángulo de campo, ajuste fino de punto rojo. | P2 | A | MeerK40t (balormk, balor_params) |
| 44 | Galvo: temporización | Laser On/Off TC, End TC, Polygon TC, jump speed y delays, frecuencia, Q-pulse (MOPA), wobble, pausas. | P2 | A | galvoplotter, MeerK40t |
| 45 | Galvo: puertos IO | Luz de listo/ocupado, punto rojo, pedal de inicio, parada, puerta de seguridad, segundo láser. | P2 | M | - |
| 46 | Cylinder Correction, Taper Warp (galvo/rotary) | Deforma el diseño para superficies cilíndricas sin rotary o para objetos cónicos. | P2 | A | MeerK40t (cylinder) |
| 47 | Feeder (auto-feeder) | Avanza el material y repite el trabajo (Ruida/Trocen). | P2 | M | - |
| 48 | Center Finder | Calcula el centro de un círculo desde 3 puntos tocados con el cabezal. | P2 | B | - |
| 49 | Optimization Settings | Planificador de orden de corte (por capa/grupo, interiores primero, reducir viajes, ocultar backlash, quitar líneas solapadas...). | P1 | A | MeerK40t (cutplan) |
| 50 | Conexión de red GRBL (TCP) y Ethernet Ruida (UDP) | GRBL por TCP (puerto 23 por defecto). Ruida/Trocen por UDP con IP estática; Bridge de LightBurn como alternativa más estable. | P1 | M | Rayforge (GRBL telnet/Ruida UDP) |
| 51 | Preferencias | Autosave, auto-update, modo principiante, cuadrículas, snaps, tema, DPI, importación/exportación (DXF, SVG 96/72 DPI...), cámara, log de trabajos CSV. | P1 | M | - |
| 52 | Multilenguaje | Selector de idioma; árabe y hebreo activan texto RTL. | P1 | M | Rayforge (7 idiomas), MeerK40t (carpeta locale) |
| 53 | Job checklist y protecciones | Lista de comprobación antes de Start/Send, advertencia fuera de límites, ignorar Start si el monitor duerme, bloquear foco del botón Start. | P1 | B | Rayforge (pre-flight) |
| 54 | Automatización por UDP | LightBurn acepta comandos UDP externos (existe guía oficial). Detalle del protocolo (no verificado). | P2 | M | MeerK40t (Telnet/Web remote) |

---

## 2. Tabla de controladoras

Convención de "protocolo documentado": **sí** = especificación pública utilizable; **parcial** = reversa comunitaria incompleta o estándar abierto pero con dialectos; **no** = sin documentación pública hallada.

| Familia | Qué expone LightBurn (verificado) | Protocolo documentado públicamente | Implementación open source | Riesgo | Prioridad sugerida |
|---|---|---|---|---|---|
| GRBL y variantes (GRBL 1.1f+, GRBL-M3 para <=1.1e, grblHAL, FluidNC) | Jog (incl. $J), home, frame, start/pause/stop, consola con $I/$$/$#/?/$X, S-max, buffered/synchronous, DTR, red TCP, Z, rotary, macros, cámara, aire M7/M8, ajuste de velocidad/potencia en vuelo. Fuentes: `D21Reference/DeviceSettings/BasicSettings/`, `ConsoleWindow/`, `Guides/GRBLConfiguration/` | **Sí** (protocolo abierto de GRBL; referencia de comandos reales-time y $J no verificada en esta sesión, solo confirmados por docs de LightBurn: $I, $$, $#, ?, $X, $J, \x18 como abort) | LaserGRBL (GPLv3, C#, GRBL 0.9/1.1, WiFi por WebSocket ESP8266); Rayforge (MIT, GRBL por serie/Telnet/red); MeerK40t (módulo grbl) | Bajo en protocolo; medio en variantes que no cumplen el estándar (xTool, WeCreat usan perfiles especiales) y en streaming buffered | **P0** |
| Smoothieware | Misma familia G-code; S-max también aplica. Opciones como "G0 para overscan" existen por tarjetas FabCreator. Rotary usa deltas de config.txt. Fuente: `D21Reference/DeviceSettings/GCode/`, `RotaryMode/RotaryModeGCode/` | Parcial (G-code estándar; configuración en texto en SD, no consultable por protocolo estándar; detalle de comportamiento no verificado) | Rayforge (soporta Smoothieware por Telnet); MeerK40t (G-code genérico) | Medio: quejas de conexión/PWM en tarjetas Cohesion3D (ver foro) | P1 |
| Marlin | Misma familia G-code; baud por defecto 250000; G0 y G1 se tratan igual. Find My Laser no compatible. Fuente: `D21GetStarted/FindMyLaser/`, `Reference/DeviceSettings/BasicSettings/` | Parcial: Marlin documenta modos inline M3/M4 con parámetro I (marlinfw.org/docs/configuration/2.0.9/laser_spindle.html), pero el modo dinámico se describe como experimental | Rayforge (Marlin serie desde 1.7.2) | Medio-alto: variantes inline, perfiles de Snapmaker generan M106/M107 | P2 |
| Custom GCode | Plantillas por evento, variables, flavors (incluye WeCreat, Duet/RepRap desde 2.1). Fuente: `D21Reference/DeviceSettings/CustomGCode/` | Sí (definido por el usuario) | Rayforge (dialectos editables), MeerK40t | Bajo | P1 |
| Ruida (RDC644xG/S etc.) | Frame, start/pause/stop, send/Start (streaming), Home=origen de máquina, Machine Settings (lectura/escritura de firmware), File List, Controller Info, Scanning Offset R/W, rotary a controlador, feeder, dual laser, Z, U, USB serie/Packet, Ethernet, Bridge. Fuentes: `D21Reference/LaserWindow/`, `MachineSettings/`, `FileListWindow/`, `GetControllerInfo/` | **Parcial**: reversa comunitaria en edutechwiki (UDP 50200 trabajo, 50207 control; checksum de 16 bits; ofuscación "swizzle" con valor mágico por modelo: 0x88 común, 0x11, 0x38; ACK en ~4 s; USB vía FT245R a 19200 8N1; comandos con bytes >= 0x80; coordenadas en micrómetros); muchos comandos marcados "??"; última edición mayo 2021. Fuente: https://edutechwiki.unige.ch/en/Ruida | MeerK40t: carpeta `meerk40t/ruida` (emulator, controller, udp/usb/serial transports, rdjob, loader); README la marca beta/INCOMPLETE, probada solo con RDC6442S + MP-570 en Fedora, control de hardware real descrito de forma inconsistente (principalmente emulador/puente). Rayforge: Ruida por UDP. | **Alto**: variaciones por modelo, firmware con quejas, sin especificación oficial | **P1** (el mayor valor comercial en CO2 industrial) |
| Trocen (AWC708 y similares) | Frame, send, UD5, feeder (con retardo tras avance), rotary configurado en la pantalla del controlador; Ethernet UDP junto con Ruida. Fuentes: `D21Reference/FeederSetup/`, `RotaryMode/RotaryModeDSP/`, `Guides/RuidaEthernet/` | **No** (búsqueda de reversa específica sin resultados; el formato .ud5 y el protocolo no se encontraron) | No encontrada | Alto (sin referencia pública) | P2 (posponer; evaluar tras Ruida) |
| TopWisdom | Salida .out, Machine Settings, rotary configurado desde Machine Settings. Fuentes: `D21Reference/MachineSettings/`, `UI/FileMenu/` | **No** (búsqueda sin resultados) | No encontrada | Alto | P2 |
| EZCAD2 / EZCAD2 Lite (JCZ/BJJCZ LMCV4, "JCZFiber") | Galvo: alta por markcfg7, framing luz roja, hatch vía sub-capas, wobble, Q-pulse (MOPA), puertos IO, split/repeat marking, rotary galvo. Z por EZCAD2 fue retirado en 2.1 (volverá). Fuentes: `D21GetStarted/AddGalvo/`, `Reference/DeviceSettings/GalvoBasicSettings/`, `GalvoPorts/` | **Parcial**: reversa comunitaria de Bryce Schroeder (Balor), reimplementada | galvoplotter (MIT, Python, pyusb, BJJCZ fibra y CO2); MeerK40t `balormk` (MIT, "compatibilidad completa con EZCAD2", USB por libusb) | Medio-alto (riesgo de disparo accidental, daño a la fuente láser) | **P2** (primera opción galvo) |
| EZCAD3 (placas DLC2) | Soporte oficial desde 2.1 en modo Labs: marcado estándar, rotary, split marking, jog Z; sin control simultáneo de 2+ ejes; Z automático planificado. Alta con markcfg0 (+ Motors.ini, COR). Fuentes: `D21GetStarted/AddGalvo/`, `NewFeatures/NewIn2.1/` y foro https://forum.lightburnsoftware.com/t/initial-ezcad3-support/188524 | **No** (los desarrolladores de LightBurn dicen que el fabricante no ayuda y que trabajan en la reversa: foro https://forum.lightburnsoftware.com/t/ezcad3-support/190047 ; no se halló especificación pública) | No encontrada | Muy alto | P2 (último) |
| BSL / BSLFiber (SeaCAD) | Dispositivo soportado desde 1.7. Alta con BslCAD.cfg o LmcPar.cfg. El clic derecho en Devices no reinicia la conexión con BSL. Fuentes: `D21GetStarted/AddGalvo/`, `Reference/LaserWindow/`, foro https://forum.lightburnsoftware.com/t/bsl-boards-seacad-support-in-lightburn/77669 | **No** (comentarios del foro: comandos desconocidos y fabricante sin interés) | No encontrada | Muy alto | P2 |
| xTool (D1, D1 Pro, S1 nativo por USB; F1 y otros por .lbdev) | Tipo de dispositivo nativo: autoenfoque y G-code corregido; solo por USB/Serial, sin red (foro: no soporta F1/P2 ni red). Aviso oficial: firmware reciente quitó compatibilidad con P2S, P3, F1, F2 (a 2026-06-29, docs). Fuentes: `D21Troubleshooting/DeviceSpecificTroubleshooting/`, foro https://forum.lightburnsoftware.com/t/setting-up-xtool-s1-with-new-xtool-device-type-in-lb-1-7-beta/135463 | **No** como protocolo propio; las máquinas basadas en GRBL usan una capa de compatibilidad (detalles no verificados) | Rayforge/LaserGRBL cubren solo la parte GRBL estándar (no verificado para xTool específicos) | Alto (cambios de firmware unilaterales: foro menciona límites de velocidad en rotary del D1) | P2 |
| WeCreat / Emblaser y otros | Se configuran como Custom GCode (WeCreat: deshabilitar G53, fetch config y offsets al conectar). Fuente: `D21Troubleshooting/DeviceSpecificTroubleshooting/` | Sí (G-code) | - | Bajo | P2 |
| LightBurn Bridge | Dispositivo físico de LightBurn: Ethernet hacia el controlador Ruida, WiFi hacia el PC; IP del Ruida 10.0.3.3. Solo Ruida. Fuente: `D21Reference/LightBurnBridge/` | No (propietario) | - | N/A (hardware propietario) | No aplica (solo UDP directo) |

Notas sobre puertos Ruida (de fuentes públicas): edutechwiki indica 50200 (trabajos) y 50207 (control); el README de MeerK40t lista 50200, 50207, 40200 y 40207 como puertos manejados por su controlador. Las docs oficiales de LightBurn solo dicen "UDP" sin puertos (`D21Guides/RuidaEthernet/`).

---

## 3. Detalle de funciones P0 y P1

Formato por función: comportamiento esperado, opciones visibles, casos borde y quejas del foro, fuentes.

### 3.1 Creación de dispositivo y gestor de dispositivos (P0)

**Comportamiento.** Dos caminos: autodetección (Find My Laser) y wizard manual. El wizard manual tiene seis pasos: tipo de dispositivo, tipo de conexión, IP/hostname (solo Ethernet y Bridge), nombre y área de trabajo, origen y homing, resumen. El perfil se guarda al pulsar OK en la ventana Devices; Cancelar descarta incluso el dispositivo recién creado. Debe existir al menos un dispositivo; si no, la app pide crear uno, crear una máquina ficticia o salir.

**Opciones visibles.**
- Tipos: GRBL y firmwares G-code (Smoothieware, Marlin, FluidNC, grblHAL, xTool), Custom GCode, DSP (Ruida, Trocen, TopWisdom), Galvo (EZCAD2, EZCAD2 Lite, EZCAD3, BSL), "No Machine".
- Conexión: Serial/USB, Ethernet o WiFi (TCP/UDP), Packet/USB (solo Ruida, alternativo), LightBurn Bridge (solo Ruida). Galvo solo USB.
- Origen: GRBL suele ser inferior izquierda; DSP usa la posición a la que el equipo hace home al encender. Origen mal elegido produce salida espejada o invertida. Auto-homing al conectar: dejar apagado si no hay finales de carrera.
- Devices: Find My Laser, Create Manually, Bridge, Import, Export, Make Default (asterisco), Edit (doble clic), Remove, Duplicate (nombre + "(Duplicate)"), Set Icon (hasta 1024x1024), Rename, ordenar por nombre o tipo.
- Galvo: se recomienda un perfil por lente (duplicar el dispositivo).

**Casos borde.**
- Find My Laser no detecta dispositivos de red; requiere drivers (FTDI para Ruida, CH340 en muchos diodos, driver propio en galvo EZCAD).
- Solo una aplicación puede hablar con el láser a la vez.
- macOS 15+ exige permiso de red local para IP.
- Importar bundle: conflictos crean duplicados.
- Clic derecho sobre Devices reinicia la conexión (no funciona con BSL) y refresca la lista de puertos.

**Quejas del foro.**
- La autodetección falla a veces y la solución es elegir el puerto manualmente; con Smoothieware el bed size puede detectarse mal porque la config vive en texto en la SD (https://forum.lightburnsoftware.com/t/finding-connecting-to-laser-after-0-9-0-upgrade/2329, https://forum.lightburnsoftware.com/t/installed-c3d-laser-board-installed-device-shows-disconnected/4412).
- Cohesion3D/Smoothie: "conectado" pero no responde (https://forum.lightburnsoftware.com/t/lightburn-says-connected-but-will-not-move-laser-head-or-run-projects/112902).

**Fuentes.** `D21GetStarted/FindMyLaser/`, `GetStarted/CreateManually/`, `Reference/Devices/`, `Reference/UserBundles/`, `GetStarted/LaserID/`.

### 3.2 Device Settings (P0)

**Comportamiento.** Pestañas: Dimensions/Units, Basic Settings, GCode, Additional Settings (solo para preview/estimación de tiempo), Custom GCode, y pestañas galvo.

**Opciones visibles (por pestaña).**
- Dimensions/Units: ancho/alto de trabajo (no cambian el recorrido real), "CNC Machine" (origen trasero derecho y coordenadas negativas), "Machine Coordinates" (G54; LightBurn funciona mejor con G54 en 0,0,0 y avisa que User Origin y jog continuo pueden fallar con offset), origen, laser offset, U Axis, unidades de dispositivo (métrico/imperial), unidades de control (mm/s en CO2 y galvo, mm/min en diodo) con enlace opcional, Z (habilitar, invertir, solo relativo, optimizar), OMTech Polar.
- Basic: tab pulse width, job checklist, frame continuo, sobrescribir archivos en el dispositivo, guardar rotary con proyecto, ajuste de offset de escaneo.
- GCode: auto-home, swap X/Y, fast whitespace scan, $J jogging (GRBL 1.1f+), DTR, botón Fire (solo diodo), láser encendido al enmarcar, aviso fuera de límites (requiere home y coordenadas correctas), clustering de G-code, return to finish position (requiere Absolute o User Origin), M7/M8 para aire, S-value max (1000 en GRBL actual, 255 en antiguo; debe coincidir con $30), baud (115200 GRBL, 250000 Marlin), puerto de red (23, alternativa 8080), modo de transferencia (Buffered por defecto, Synchronous si hay tirones), consulta regular de posición, G0 para overscan, emitir S con cada G1, traer settings al conectar, revisar offsets de trabajo al conectar.
- DSP: retardo de inicio/fin (Ruida), tubo láser 1 y 2 (nombre, RF, solo uno a la vez, offset de láser 2), incrementar nombre al enviar, deshabilitar Start, revisar límites al enmarcar, timeout de red (5000 ms por defecto), IP/hostname.
- Additional: velocidades máximas X/Y, aceleración de corte y de grabado, corner accuracy, velocidad mínima en esquina, escalas; botón "leer del controlador". Nota: solo afectan estimaciones.

**Casos borde.**
- Máquina sin finales de carrera: homing manual para usar Absolute o User Origin (posicionar antes de encender, o jog al origen y `G92 X0 Y0` con $10=0 según la guía GRBL).
- Z habilitado y sin altura de material en capas: LightBurn emite Z siempre; con altura 0 por defecto la advertencia dice que el cabezal puede chocar con la pieza.
- GRBL: $13=0 (mm), $32=1 (modo láser), $30 igual a S-max.

**Quejas del foro.**
- Tirones/paradas con Buffered: pasar a Synchronous (https://forum.lightburnsoftware.com/t/lb-2-0-01-air-assist-stops-during-cut/175296?page=4, https://forum.lightburnsoftware.com/t/lightburn-stops-while-engraving/68218). Un usuario de xTool D1 Pro atribuye los tirones en imágenes dithered al buffer serie fijo y sugiere conteo de caracteres (https://forum.lightburnsoftware.com/t/laser-stuttering-at-high-speed-ability-to-increase-grbl-buffer-size/87007).
- Opciones dispersas en tres pestañas distintas; el usuario no sabe cuál aplica a su tipo de láser (las propias docs dicen "revisa que estés en la pestaña correcta").

**Fuentes.** `D21Reference/DeviceSettings/BasicSettings/`, `.../GCode/`, `.../DimensionsUnits/`, `.../AdditionalSettings/`, `Guides/GRBLConfiguration/`, `Troubleshooting/GRBLCommunicationProblems/`.

### 3.3 Modos M3/M4 y transferencia (P0)

**Comportamiento.** Con GRBL 1.1f o superior y `$32=1` se usa el dispositivo "GRBL" y M4 (potencia proporcional a la velocidad, sin pausas al cambiar potencia). Con GRBL 1.1e o anterior se debe usar "GRBL-M3": potencia constante, esquinas sobrequemadas y líneas rectas más claras; pausar puede no apagar el haz. La versión se lee del mensaje de saludo en la consola.

**Transferencia.** Buffered (por defecto) envía por adelantado y es más rápida; Synchronous espera confirmación por línea. Streaming para DSP vía Start (sin guardar archivo) vs. Send (guarda en memoria).

**Casos borde.** Algunos dispositivos reinician o se detienen con Buffered. DTR en tarjetas Arduino provoca reset al abrir el puerto. Ruido eléctrico cerca del cable USB.

**Fuentes.** `D21Guides/GRBLConfiguration/`, `Troubleshooting/GRBLCommunicationProblems/`, `Troubleshooting/ConnectionTroubleshooting/`.

### 3.4 Ventana Laser: Start/Pause/Stop y Frame (P0, rubber-band P1)

**Comportamiento.** Start ejecuta el proyecto actual; en galvo abre antes la ventana de framing salvo que se desactive. Stop aborta (Ctrl+Break), Pause/Resume (tecla Pause). Send (solo DSP) sube el archivo; con Shift además lo inicia. Frame recorre el contorno a la velocidad de Move; "Frame continuously" repite hasta detener; "Laser on when framing" (Shift temporal) solo para diodo a potencia muy baja. Bounding Box Frame frente a Rubber Band Frame. "Show Last Position" pone una cruz que no se actualiza en vivo.

**Opciones visibles.** Start, Pause, Stop, Send, Frame, Save/Run Machine Files, Home, Go to Origin, Start From, Job Origin, Cut Selected, Use Selection Origin, Show Last Position, Enable Rotary, Cylinder Correction (galvo), Red Light (galvo), Optimization Settings, Devices, selector de dispositivo, puerto serie, estado del dispositivo (Disconnected/Ready/Busy/Framing).

**Casos borde.**
- Home en láser sin finales de carrera mueve la cabeza a ciegas (la doc advierte no usarlo).
- En DSP, Home lleva al origen de máquina.
- Ruida: el frame desde LightBurn parte de la posición actual mientras Start parte del origen (ver quejas).
- Preferencias protegen contra inicios accidentales (ignorar Start si el monitor duerme, no retener foco).

**Quejas del foro (Ruida origen/frame).**
- Frame en modo User Origin corre desde la posición del cabezal; hay que presionar Esc en el controlador o usar Send: https://forum.lightburnsoftware.com/t/frame-running-from-current-position-in-user-origin-mode/6613
- El trabajo empieza en el origen de Ruida y no en "Current Position": https://forum.lightburnsoftware.com/t/job-starts-from-ruida-origin-setting-rather-than-current-position-in-lb/17317 y https://forum.lightburnsoftware.com/t/origin-problems-using-ruida-controller/170985/2
- Start funciona pero Send y luego iniciar desde el panel vuelve al origen viejo: https://forum.lightburnsoftware.com/t/issues-origin-frame-and-start-using-the-controller/23887
- El modo "Multi-origin" del controlador interfiere en algunos RDC6445 (según respuestas del hilo anterior).
- Oportunidad: mostrar siempre, antes de ejecutar, un diagrama con las coordenadas absolutas reales donde irá el trabajo.

**Fuentes.** `D21Reference/LaserWindow/`, `Reference/Preview/`(no leída), `Reference/CutSelectedGraphics/`.

### 3.5 Start From, User Origin y Job Origin de 9 puntos (P0)

**Comportamiento.**
- Absolute Coords: el 0,0 del espacio de trabajo es el origen de máquina; el selector de 9 puntos queda gris.
- Current Position: el trabajo se coloca relativo a donde está el cabezal al pulsar Start.
- User Origin: igual, pero el punto es uno guardado previamente (Set Origin en GCode, o botón Origin del controlador en Ruida; Clear Origin y Go to Origin disponibles).
- Job Origin: de los 9 puntos del cuadro envolvente (esquinas, medios de lado, centro) el elegido coincide con el punto de inicio. Se muestra como cuadro verde.
- Finish Position (solo GCode): por defecto 0,0, se fija desde Move o Device Settings. DSP la gestiona internamente.
- Galvo siempre usa coordenadas absolutas y no muestra estos controles.

**Casos borde.** Absolute requiere origen fijo (homing manual en diodos sin finales de carrera). El indicador del cuadro envolvente aparece desfasado de los gráficos en Current Position y User Origin: es esperado. "Use Selection Origin" no funciona en Absolute ni sin Cut Selected. Si Cut Selected está activo sin selección, avisa al hacer preview, start, send o guardar.

**Quejas del foro.** Ver 3.4; además el modelo mental (tres modos × nueve puntos × selección) confunde a los nuevos (ejemplo: https://forum.lightburnsoftware.com/t/current-position-not-acting-as-expected/154953, https://forum.lightburnsoftware.com/t/setting-origin-correctly/21397).

**Fuentes.** `D21Reference/CoordinatesOrigin/`, `Reference/CutSelectedGraphics/`; resumen de terceros (Thunder Laser) en https://www.thunderlaserusa.com/knowledge/tips-and-tricks-for-using-the-job-origin-and-start-from-in-lightburn-with-your-thunder-laser-nova-odin-and-bolt-series-laser

### 3.6 Ventana Move (P0)

**Comportamiento.** Jog por pasos con distancia y velocidad configurables; con A-axis la distancia son grados. La velocidad también afecta a Frame y Position Laser. Jog continuo (GRBL 1.1f+ con $J habilitado en Device Settings). Atajos de teclado y teclado numérico. "Move from machine zero" convierte la distancia en destino absoluto desde el cero de máquina. Posiciones guardadas con gestor. Get Position muestra X, Y, Z, U. Set Origin, Clear Origin, Set Finish Position (ocultos en Ruida). Focus Z. Fire (diodo solamente, nunca CO2) con potencia. Durante un trabajo GCode: Adjust Speed y Adjust Power en porcentaje.

**Casos borde.** No se muestra por defecto en galvo. Algunas opciones dependen de Beginner Mode. Jog continuo puede ser incorrecto con offsets G54.

**Fuentes.** `D21Reference/MoveWindow/`, `Reference/PositionLaser/`.

### 3.7 Consola y macros (P0 consola, P1 macros)

**Comportamiento.** Muestra mensaje de arranque (firmware y versión), alarmas y errores; "Show All" alterna verbosidad total. Solo conexión directa a máquinas G-code. No existe para Ruida, TopWisdom, Trocen, EZCAD2/3 ni BSL. En GRBL acepta consultas ($I, $$, $#, ?), $X, cambio de ajustes `$n=v` y comandos propios de reset (RST=#, RST=$, RST=*) que son riesgosos. Macros: botones con etiqueta y uno o más comandos; añadir, editar, borrar y reordenar (arrastrar).

**Fuentes.** `D21Reference/ConsoleWindow/`, `Reference/MacrosWindow/`.

### 3.8 Guardar/ejecutar archivos de máquina y G-code (P0 G-code, P1 RD)

**Comportamiento.** El menú File cambia de etiqueta según la controladora: Save GCode (GRBL/Smoothie/Marlin), Save RD File (Ruida), Save OUT (TopWisdom), Save UD5 (Trocen). Atajo Alt/Option+Shift+L. Custom GCode permite cambiar la extensión (por defecto .gc) y guardar en un solo archivo sin diálogo. "Run Machine Files" lee un archivo ya guardado y lo ejecuta (requiere conexión directa).

**Casos borde.** Generación de .rd requiere conocer el modelo para el valor de ofuscación. Importar .rd existe en el menú de importación.

**Fuentes.** `D21Reference/UI/FileMenu/`, `Reference/LaserWindow/`, `Reference/DeviceSettings/CustomGCode/`, https://edutechwiki.unige.ch/en/Ruida

### 3.9 Envío a memoria de controladora y File List (Ruida/Trocen, P1)

**Comportamiento.** Send sube el trabajo como archivo con nombre para ejecutar desde el panel. File List permite refrescar, iniciar, subir un archivo guardado, borrar, borrar todos, y "Calc Time" (simulado por el controlador Ruida, puede tardar para grabados). Opciones: autoincremento del nombre (el nombre debe tener un dígito), sobrescribir sin aviso, deshabilitar Start (obliga a Send), timeout de red.

**Casos borde.**
- Borrar desde el fondo de la lista en Ruida (desde arriba la compactación tarda más).
- Resoluciones muy altas (arriba de unos 254 DPI o intervalos menores a 0.1 mm) pueden ser rechazadas por el controlador.
- Memoria llena: archivos acumulados; el límite de 99 archivos es mencionado en el foro.

**Quejas del foro.**
- Sobrescribir un archivo da "Machine busy or paused" hasta el tercer intento: https://forum.lightburnsoftware.com/t/error-when-overwriting-a-file-on-controller/26192
- "File send successful" pero el archivo no aparece: https://forum.lightburnsoftware.com/t/lightburn-not-actually-sending-files-via-ethernet-but-no-error-reported-file-send-successful/42452
- Start sobrescribe el mismo archivo; Send acumula archivos (hilo: https://forum.lightburnsoftware.com/t/files-in-controller-memory-responsible-for-status-error-on-omtech-polar/117830?page=3).
- Oportunidad: verificación posterior a la subida (listar y comparar tamaño), cuota visible, limpieza guiada.

**Protocolo.** edutechwiki: paquetes de aproximadamente 1000 bytes cortados en límites de comando, checksum de 16 bits, ACK requerido en unos 4 s, sin autenticación. Fuente: https://edutechwiki.unige.ch/en/Ruida

**Fuentes.** `D21Reference/FileListWindow/`, `Reference/LaserWindow/`, `Reference/DeviceSettings/BasicSettings/`, `Troubleshooting/ConnectionTroubleshooting/`.

### 3.10 Machine Settings por controladora (P1)

**Comportamiento.** Ventana con lectura/escritura de parámetros de firmware para Ruida, TopWisdom y GRBL; no cubre Trocen ni Smoothie. Nada se guarda hasta pulsar Write. Read descarta ediciones no escritas. Save to File / Load from File usan .lbset (cargar sobrescribe todos los valores). LightBurn hace un respaldo automático en cada escritura (Load from Backup). "Calibrate Axis" corrige steps/mm o step length: se elige eje y distancia, se mide el recorrido real, se introduce, y se escribe. La sección "Vendor Settings/Outputs Setup" está oculta por defecto con advertencia.

**Guía Ruida (parámetros relevantes).** Polaridad del limitador y de dirección por eje (cuatro combinaciones), invertir dirección del teclado, Max Travel por eje, Step Length (nuevo = actual × medida / solicitada), aceleración/velocidad en vacío, "PWM Rising Edge Valid" si los grabados salen sesgados, retardo de inicio/fin.

**Casos borde.** Si no se puede leer, comprobar encendido y conexión.

**Fuentes.** `D21Reference/MachineSettings/`, `Guides/ConfigureRuidaController/`.

### 3.11 Rotary (P1 GCode y DSP, P2 galvo)

**Comportamiento.** Configuración en Laser Tools > Rotary Setup (Ctrl/Cmd+Shift+R). Indicador visible cuando está activo; switch opcional en la ventana Laser. En tapered usar el diámetro mayor.

**Opciones.**
- Tipo: Chuck (mordazas) o Roller (rodillos). Habilitar, espejo de salida.
- Eje: en Ruida normalmente Y (si está en el puerto U puede requerir selección manual según firmware); en GCode: Y (reemplaza el motor Y) o Z/A (puerto propio).
- Escala: Ruida usa pasos por rotación; GCode usa mm por rotación; controladores GCode no estándar usan pasos por grado (estimación 200 × microstepping × reducción / 360).
- Roller diameter (solo roller), Object diameter (en chuck afecta la salida; en roller solo para calcular circunferencia), Circumference (vinculada al diámetro), botón Test (giro completo y regreso).
- Ruida: Read Settings; al pulsar OK se escribe al controlador y requiere reinicio.
- Galvo: Split Size, Overlap, Run Whole Shapes, Max Shape Size, Output Center, orden de formas vs todas las formas por rebanada, Return to Start, velocidades/aceleración del motor, Frame por rebanadas, Sanity Check.

**Casos borde.**
- Diámetro o mm/rotación mal medidos escalan mal en el sentido de rotación.
- Límites suaves (Alarm:2) por exceder el recorrido de Y; se puede poner `$20=0` temporalmente en consola.
- Slip en rodillos: material antideslizante, pesos o menor aceleración.
- Backlash: aplana la parte inferior; tensar correas o línea de bajo poder previa.
- En el puerto Y, centrar el cabezal antes de conectar.
- Trocen: se configura en la pantalla del controlador; TopWisdom en Machine Settings.

**Quejas del foro.** Entradas confusas entre roller diameter y mm por rotación, con valores no obvios que "funcionan": FoxAlien R42 (https://forum.lightburnsoftware.com/t/lightburn-1-4-05-foxalien-r42-rotary-setup/125151), K40 con GRBL-LPC (https://forum.lightburnsoftware.com/t/grbl-lpc-rotary-odd-behavior/13113), "ajustes que no tienen efecto" (https://forum.lightburnsoftware.com/t/lightburns-inner-rotary-settings-dont-affect-the-real-rotary-behaviour/111947). Oportunidad: calculadora con asistente que pida una sola medición física (por ejemplo, marcar y medir el avance de una vuelta).

**Fuentes.** `D21Reference/RotaryMode/`, `RotaryMode/RotaryModeDSP/`, `RotaryMode/RotaryModeGCode/`, `RotaryMode/RotaryModeGalvo/`.

### 3.12 Eje Z (P1)

**Comportamiento.** Se habilita en Dimensions/Units. LightBurn emite Z en todo trabajo; se necesita "Relative Z Moves Only" o definir Material Height en la ventana Cuts/Layers, si no, altura 0 por defecto (riesgo de colisión). Reverse Z, Optimize Z Moves (solo si la pieza es plana) y Focus Z en la ventana Move (requiere autoenfoque compatible). Posiciones guardadas pueden incluir Z. Galvo: pestaña de puertos con motor, pasos por unidad, aceleración, velocidades y timeout de homing; en EZCAD2 el control Z fue retirado en 2.1 y EZCad3 solo tiene jog Z.

**Fuentes.** `D21Reference/DeviceSettings/DimensionsUnits/`, `Reference/MoveWindow/`, `Reference/DeviceSettings/GalvoPorts/`, `NewFeatures/NewIn2.1/`.

### 3.13 Cámara: lente, alineación, overlay, trace (P1)

**Comportamiento (2.1).**
1. Añadir cámara: USB (dispositivo, resolución, fps), de red (URL del feed; MJPEG o cuadros sueltos, **no RTSP**), o presets (por ejemplo Creality Falcon A1 Pro, Thunder Laser Vision, WeCreat, xTool P2). Admite importar .lbcm (archivos de 1.7 o anteriores requieren agregar y conectar la cámara primero).
2. Calibración de lente: patrón de AprilTags en tablero plano, 5 capturas (cuatro esquinas y centro), se puede descartar y tomar más. Se calculan 4 modelos de lente y se elige el que muestra rectas rectas y esquinas de cama correctas. Extended Model para distorsión fuerte; Fisheye solo para lentes ojo de pez reales. El puntaje porcentual no garantiza mejor calibración.
3. Alineación: se graba un patrón en el material. Automática: 8 AprilTags, mínimo 4 detectados. Manual: cuatro cruces clic en orden. No mover el patrón tras grabarlo. Se guarda una alineación por altura de material; "Interpolate Alignment Height" (Labs) estima alturas intermedias.
4. Overlay: Update Overlay refresca el fondo; Overlay DPM, opacidad, ancho/alto, X/Y shift; Trace Overlay abre Trace Image con la imagen de cámara; Save Background Capture guarda el overlay como bitmap; captura de imagen cruda o calibrada.
5. Multi-cámara en cuadrícula y cámaras duales (2.1). Ayuda de selección: fps >= 12, 8-12 MP, obturador global recomendado, más de 16 MP puede exceder USB 2.0; ventana Verify Camera.
6. Cámara montada en cabezal: alineación automática o manual con Feature Width; Update Overlay mueve el cabezal, pausa y captura repetidamente; si hay selección, el overlay cubre solo esa zona.

**Casos borde.** Recalibrar al cambiar de lente, no al mover la cámara; realinear al moverla. Mala iluminación o baja resolución afectan la detección. Macs M1+ pueden tener problemas con cámaras USB 2.0 en puertos USB 3.0.

**Quejas del foro (2.1).**
- Overlay desfasado varias pulgadas respecto del grabado tras alinear: https://forum.lightburnsoftware.com/t/camera-overlay-does-not-match-actual-engraving-omtech-80w-ruida-lightburn-2-1-03/191507
- Botón Next gris con el modelo fisheye; modelos Standard y Extended muy distorsionados: https://forum.lightburnsoftware.com/t/lightburn-2-1-camera-alignment-calibration/189912
- Paralaje por altura de material: calibrar con 3 mm y poner una pieza de 10 mm da error (hilo https://forum.lightburnsoftware.com/t/camera-alignment-accuracy/118996/14).
- Pérdida de alineación tras reiniciar (1.6): https://forum.lightburnsoftware.com/t/camera-loses-alignment-lightburn-1-6-00/140153
- Overlay pixelado en comparación con la vista previa: https://forum.lightburnsoftware.com/t/camera-overlay-pixelated-but-preview-isnt/191914
- Peticiones de lectura de marcas de registro por cámara en la cabeza: https://forum.lightburnsoftware.com/t/head-mounted-camera-registration-reading/191191

**Fuentes.** `D21Reference/Cameras/`, `Cameras/Calibration/`, `Cameras/Alignment/`, `Cameras/CamerasWindow/`, `Cameras/HeadMountedCamera/`, `Cameras/AddACamera/`, `Cameras/CameraSelection/`, `Cameras/EditCamera/`, `Reference/SaveBackgroundCapture/`.

### 3.14 Print & Cut (P1)

**Comportamiento.** Alinea un nuevo trabajo a dos posiciones objetivo físicas (marcas de registro). Flujo: crear cruces como objetivos (una línea vertical y una horizontal agrupadas), abrir el asistente (Alt/Option+0), seleccionar la primera cruz, mover el láser sobre el objetivo físico (disparar/pulsar o punto rojo) y pulsar Set; repetir para la segunda; elegir Scaled (escala uniforme por la distancia entre objetivos) o No Scaling; hacer frame y preview (puede aparecer rotado). Requiere máquina gantry, conectada, con home y coordenadas absolutas (se selecciona solo). No mover el láser a mano. Los objetivos deben ser objetos únicos o un grupo, lejanos entre sí; si están demasiado cerca da error. Con la doc: solo manual (sin cámara) y solo rotación + escala uniforme (no se menciona sesgo).

**Quejas del foro.** Piden tercer objetivo para mejorar escala y rotación (https://forum.lightburnsoftware.com/t/print-and-cut-feature-adding-a-3rd-target-would-be-a-huge-improvement/116150), cuatro objetivos para contornos (https://forum.lightburnsoftware.com/t/contour-cutting-print-cut-light-burn-camera/14114), registro con cámara en cabezal, y la precisión del punto rojo es limitada.

**Fuentes.** `D21Reference/PrintAndCut/`.

### 3.15 Texto variable, serialización y CSV (P1)

**Comportamiento.** Cuatro modos desde la barra de opciones de texto: Date/Time, Merge/CSV, Serial Number, Cut Setting. Ventana Variable Text: Current, Start, End (Current envuelve), Advance By, Auto-Advance (en Start/Send/guardar archivo de máquina), Previous/Next, Reset, Test (mientras se mantiene pulsado), Bake (convierte en texto estático), Browse/Clear para CSV. Por objeto: Offset (suma a Current para que un diseño con varias copias muestre filas distintas). Shape Properties: Max Width, Squeeze, Ignore Empty Vars. Formato: fecha-hora con códigos y literales entre comillas simples; serie decimal o hexadecimal con ceros a la izquierda ("bad serial format" si se mezclan); CSV con columnas `%0`, `%1`..., UTF-8; Cut Setting con letras para velocidad, potencia, pasadas, DPI, intervalo, láser, Z, etc.

**Casos borde.** Si Start no es 0, hacer Reset antes de la primera corrida. Test no refleja Virtual Array; usar Preview. ANSI muestra mal caracteres especiales. Documentación de "Next" contiene un error tipográfico (dice "decreases").

**Quejas del foro.**
- CSV no carga tras cambios de parser (comillas y comas): https://forum.lightburnsoftware.com/t/csv-file-not-reading-in-0-9-09/9180
- Contador End no se actualiza al cargar otro CSV (mismo hilo de errores de CSV y https://forum.lightburnsoftware.com/t/potential-bug-in-variable-text/191559)
- El serial no persiste al cerrar (https://forum.lightburnsoftware.com/t/variable-text-not-saving-serial/64332)
- QR serializados que no incrementan por inicio (https://forum.lightburnsoftware.com/t/serialized-qr-codes/109171)
- Texto variable que se ve en Test pero no sale en preview/láser (https://forum.lightburnsoftware.com/t/variable-text-shows-with-test-button-but-does-not-output-in-preview-or-to-laser/167601)

**Fuentes.** `D21Reference/VariableText/`, `Reference/VariableText/VariableTextFormatting/`.

### 3.16 Tiling / trabajos más grandes que la cama (P2)

**Hallazgo.** No existe una función "Tiling" en LightBurn 2.1 (confirmado por el índice de la documentación). Para galvo hay Split Marking (3.18). Para gantry los usuarios combinan Cut Shapes (para partir vectores), Print & Cut (para registrar) y soluciones físicas (agujeros de alineación en la mesa, solapes de 2 mm). Fuentes: https://forum.lightburnsoftware.com/t/cutting-a-project-bigger-than-the-laser-bed/50216, https://forum.lightburnsoftware.com/t/tile-a-shape-larger-than-the-bed-size/66828, https://forum.lightburnsoftware.com/t/cutting-larger-area-than-my-lasermachine/19045, https://docs.lightburnsoftware.com/Tools/CutShapes.html. Queja: la imagen raster grande sin solución clara (https://forum.lightburnsoftware.com/t/engraving-image-larger-than-stage/77394); una petición de avance automático con pass-through (https://forum.lightburnsoftware.com/t/is-there-an-option-to-auto-feed-and-continue-cutting-when-the-file-is-larger-than-the-bed/110314/9). Oportunidad clara de diferenciación: tiling con marcas de registro automáticas, solape configurable y paso por cámara.

### 3.17 Preferencias y multilenguaje (P1)

**Preferencias (global, independiente del láser).** Editor: actualizaciones automáticas, autosave (0 desactiva; respaldos con sufijo `_backup`), abrir archivos en la instancia activa, Beginner Mode, estados por defecto de Cut Selected y Use Selection Origin, ignorar formas fuera de límites, ignorar Start si el monitor duerme, no retener foco en Start, zoom invertido, cargar capas por defecto, diálogos de archivo (interno/externo/sistema), sonido de error al fallar el envío, guardar Cut Selected y Selection Origin y Job Origin en el proyecto, log de trabajos `LightBurnJobs.csv`, portapapeles del sistema. Unidades y cuadrículas: tolerancia de clic, contraste, snaps, incrementos de movimiento, unidades de diseño, espaciado de cuadrícula. Pantalla: tema claro/oscuro/sistema, tamaño de cursores, calidad de curva, antialiasing, High DPI, relleno, tamaño de fuente, iconos, mostrar rotary y cylinder en la ventana principal, cruz central. Importación/exportación: agrupar, capas ocultas de AI, DXF (unidades, auto-cierre en mm, arcos), SVG 72/96 DPI, precisión HPGL/PLT, curve tolerance de salida (0.05 mm por defecto). Cámara: sistema de captura (Windows), overlay a color o B/N, calidad y límite de framerate. Administración: User Bundles, atajos, restaurar diseño.

**Multilenguaje.** Menú de idioma con punto en el activo; la página lista seis idiomas (Español, Deutsch, Português, Français, Italiano, 漢語) además del inglés por defecto, aunque afirma que hay 25 traducciones; árabe y hebreo activan texto de derecha a izquierda automáticamente. Fuentes: `D21Reference/SettingsPreferences/`, `Reference/UI/LanguageMenu/`. Quejas del foro sobre traducciones: (no verificado).

### 3.18 Funciones que quedan P2 (resumen verificado)

- Split Marking (galvo): mesa lineal o giratoria, Split Size, Overlap, Run Whole Shapes con Max Shape Size, Output Center no negativo, orden de formas o todo por rebanada, Frame por rebanadas, Sanity Check. Repeat Marking: Count, Increment, Calculate (rotativo), Pause After Move, incompatible con Rotary Mode, el Frame no avanza la mesa. Fuentes: `D21Reference/SplitMarking/`, `RepeatMarking/`.
- Dual Laser: solo DSP; cada capa puede usar láser 1, 2 o ambos; ambos = cabezales gemelos sin offset; offset del láser 2 medido. Fuente: `D21Reference/DualLaserControl/`.
- Cylinder Correction (galvo; EZCAD lo llama ProjectMark): distancia de espejos, diámetro, eje X/Y, convexo/cóncavo, límite válido en capa T1. Taper Warp: solo mm, diámetro superior, longitud, inferior, lado superior; no corrige distancia focal. Fuentes: `D21Reference/CylinderCorrection/`, `TaperWarp/`.
- Feeder (Ruida/Trocen), Center Finder (3 puntos), Optimization Settings, Set Start Point (nodo de inicio y dirección en vectores; no afecta rellenos ni imágenes). Fuentes: `D21Reference/FeederSetup/`, `CenterFinder/`, `OptimizationSettings/`, `SetStartPoint/`.
- Galvo (EZCAD2): campo (ancho/alto/offset/ángulo), punto rojo (offset/escala/reposo), COR, por galvo: eje X, invertir, escala, bulge, skew, trapezoid; velocidades (frame, máx), saltos (velocidad, delays mín/máx, límite de distancia), delays (Laser On/Off TC, End TC, Polygon TC); fuente láser (CO2, Fibra, UV, SPI; MOPA con Q-pulse; PWM; Tickle y First Pulse Killer en CO2; UV min/max; SPI simmer); IO ports; cut settings: frecuencia, Q-pulse width, passes globales, láser 2, wobble (paso y tamaño), ángulo incremental, auto-rotate, dot width adjust, 3D sliced (16 bits), pausas. Mapeo de hatch EZCAD a capas de LightBurn (sub-capas, Offset Fill, flood fill, bidireccional). Fuentes: `D21Reference/DeviceSettings/GalvoBasicSettings/`, `GalvoPorts/`, `Reference/CutSettingsEditor/GalvoSpecificCutSettings/`, `Explainers/EZCADHatchQuickTranslation/`, `Explainers/AboutGalvos/`, `Reference/GalvoFraming/`, `Reference/CalibrateGalvoLens/`.
- Calibración de lente galvo de 9 puntos: se marca un cuadro de 85-90 por ciento del campo, se elige la orientación del "Dragon" de prueba, se miden distancias horizontales y verticales entre las líneas, se revisan correcciones por galvo y se puede maximizar el área de trabajo (puede bajar potencia fuera del área especificada). Después "Calibrate Axis" con una línea o cuadro de 100 mm. Los coeficientes concretos que calcula no están documentados (la página no lista escala/skew/bulge; no verificado). Quejas: offsets enormes al importar configuración de otra máquina, cuadrado sale rectángulo tras recalibrar con lente 70 mm: https://forum.lightburnsoftware.com/t/fiber-70-lens-engraving-wrong-size-after-calibration/163982, https://forum.lightburnsoftware.com/t/new-set-up-field-offset-lb-unable-to-compensate-properly-enough/103030, https://forum.lightburnsoftware.com/t/galvo-lens-calibration-monport-mopa-60w/168803
- Linux: el soporte de LightBurn en Linux termina en 1.7 (Galvo Troubleshooting) y los lasers fibra requieren una regla udev; relevante para un reemplazo multiplataforma en Rust.

---

## 4. Cámara: algoritmos públicos y bibliotecas candidatas

La documentación de LightBurn solo explica el flujo de usuario (AprilTags, 5 capturas, 4 modelos, 4 puntos de alineación). Los algoritmos de abajo son estándar de visión por computador y no vienen de LightBurn. Los detalles internos de LightBurn (qué modelos exactamente son sus cuatro) son (no verificado); se infiere que corresponden a variantes de modelos pinhole con distorsión radial/tangencial y un modelo fisheye.

### 4.1 Calibración de lente (intrínsecos y distorsión)

1. **Detección de objetivo.** Alternativas: tablero de ajedrez (esquinas internas con refinamiento subpíxel), tablero de círculos, o AprilTags/ArUco (LightBurn 2.1 usa AprilTags según `Cameras/Calibration/`). Los AprilTags toleran oclusión parcial y bordes de imagen, mejores para esquinas del campo.
2. **Método de Zhang.** Con M vistas del patrón plano: estimar una homografía por vista (DLT normalizado), obtener restricciones lineales sobre la matriz de intrínsecos K (focal fx, fy, punto principal cx, cy, sesgo), refinar todo por minimización no lineal del error de reproyección (Levenberg-Marquardt), incluyendo coeficientes de distorsión. Referencias de flujo en material público: https://hackmd.io/@CV-Group5/H1cE3pjJJe
3. **Modelos de distorsión.** Radial k1..k3 (o hasta k6 racional) y tangencial p1,p2 para lentes normales/anchas; modelo fisheye (equidistante con polinomio en theta) para lentes ojo de pez. El criterio de aceptación visual de LightBurn (rectas rectas, esquinas correctas) se puede automatizar con el error RMS de reproyección más un chequeo de líneas rectas.
4. **Salida.** Matriz K, vector de distorsión, tamaño de imagen, modelo elegido; mapas de remapeo (undistort) para el video en vivo. Persistir en un archivo propio (equivalente al .lbcm de LightBurn).

### 4.2 Alineación (homografía imagen a cama)

1. Con la lente corregida, detectar al menos 4 puntos grabados con posición conocida en mm (los 8 AprilTags del patrón o cuatro cruces).
2. Estimar la homografía H (3x3, 8 grados de libertad) con DLT normalizado y RANSAC si hay más de 4 puntos y posibles valores atípicos; refinar con LM.
3. Generar el overlay: remapear la imagen corregida al plano de la cama con H (warp de perspectiva) a una resolución objetivo (el overlay DPM de LightBurn).
4. **Altura del material.** La homografía solo es válida para el plano en que se grabó el patrón. Para otro espesor, almacenar una H por altura y interpolar (la doc de LightBurn guarda alineación por altura y ofrece "Interpolate Alignment Height" como función Labs). Las quejas de paralaje del foro confirman la necesidad.
5. **Cámara en cabezal.** Se compone un mosaico: para cada captura se conoce la posición del cabezal, se aplica H local y se fusiona (blending/feathering) en un lienzo global.
6. **Trace.** El overlay se entrega como bitmap al módulo de vectorización.

### 4.3 Print & Cut y registro con cámara

- Dos puntos: transformación de similitud (rotación, escala uniforme, traslación) cerrada (Umeyama/Procrustes). Tres puntos: afín. Cuatro o más: homografía o afín por mínimos cuadrados. Esto cubre las peticiones del foro (3 o 4 objetivos).
- Cámara: detectar marcas de registro (AprilTag/cruces) en el overlay alineado, convertir a mm y alimentar la misma transformada.

### 4.4 Crates candidatos (consultados en crates.io el 2026-10-12)

| Necesidad | Crate | Versión / actualización | Notas |
|---|---|---|---|
| Binding a OpenCV (calibrateCamera, fisheye, findChessboardCorners, findHomography, remap) | `opencv` | 0.101.1 / 2026-10-09 | Madura y activa, pero requiere OpenCV nativo (complica empaquetado multiplataforma) |
| Calibración pura en Rust | `camera-intrinsic-calibration` | 0.11.1 / 2025-12-30 | Evaluar cobertura de modelos |
| Familia de calibración | `vision-calibration` (+ `-core`, `-linear`, `-optim`) | 0.9.0 / 2026-09-29 | Joven, pocos usuarios; evaluar |
| Modelos de cámara (pinhole, fisheye, omnidireccional) | `apex-camera-models` | 0.4.0 / 2026-09-29 | Útil como base de modelo |
| Homografía | `homography` | 0.2.0 / 2024-10-08 | Estimación a partir de puntos y líneas |
| Geometría de visión | `vision-geometry`, `tpt-cv-geo` | 0.9.0 / 0.1.0 | Jóvenes |
| Objetivos de calibración | `calib-targets-core`, `ringgrid` | docs.rs / 0.13.1 | `calib-targets-core` es geométrico y no incluye detector de esquinas (según su documentación) |
| AprilTag (binding a la librería C) | `apriltag` (+ `apriltag-sys`, `apriltag-image`) | 0.4.0 / 2023-01-28; sys 0.4.0 / 2025-11-25 | Depende de librería C |
| AprilTag en Rust puro | `kornia-apriltag` | 0.2.0 / 2026-10-12 | Joven (pocas descargas) |
| Procesamiento de imágenes | `imageproc` | 0.27.0 / 2026-06-02 | Sin calibración, útil para preproceso |
| Visión (tensores, imgproc) | `kornia-imgproc` y `kornia-rs` | 0.2.0 / 0.1.5 | Evaluar |
| Álgebra (solvers LM, SVD) | `nalgebra` | 0.35.0 / 2026-05-24 | Estándar |
| Captura de cámara multiplataforma | `nokhwa` | 0.10.11 / 2026-05-15 | Webcam; las cámaras de red MJPEG necesitan cliente HTTP aparte |
| USB directo (galvo, Ruida Packet/USB) | `rusb` | 0.9.4 / 2024-04-27 | Libusb; galvo EZCAD2 sobre pyusb en referencia |
| Puerto serie | `serialport` | 4.10.1 / 2026-09-08 | GRBL/Ruida serie |
| Parser G-code | `gcode` | 0.7.0 / 2026-04-02 | no_std; evaluar para importar |

Recomendación: prototipar con `opencv` (para validar resultados) y plan de migración a crates en Rust puro si el empaquetado nativo es un problema; usar `nalgebra` para las transformaciones propias.

---

## 5. Quejas frecuentes del foro (oportunidades de mejora)

Cada elemento es anecdótico (hilos individuales), salvo donde la documentación oficial lo reconoce. No se encontró estadística de frecuencia.

1. **Origen/frame/Start en Ruida inconsistentes** entre Frame (desde posición actual), Start (desde origen) y Send/ejecutar desde panel. Oportunidad: visor de "dónde va exactamente" y cálculo único y explícito del punto de inicio, el mismo para frame, preview, start y send. (https://forum.lightburnsoftware.com/t/frame-running-from-current-position-in-user-origin-mode/6613, https://forum.lightburnsoftware.com/t/issues-origin-frame-and-start-using-the-controller/23887)
2. **Modelo mental de Start From × Job Origin × Cut Selected** es difícil. Oportunidad: asistente visual con el rectángulo real sobre la cama y el punto de anclaje.
3. **Desconexiones y tirones en GRBL con Buffered.** La doc recomienda cambiar a Synchronous. Oportunidad: streaming con conteo de caracteres (character-counting) y modo automático degradable, métricas de ocupación de buffer, y mensajes claros. (https://forum.lightburnsoftware.com/t/laser-stuttering-at-high-speed-ability-to-increase-grbl-buffer-size/87007)
4. **Detección automática poco fiable** (puerto equivocado, Smoothie con bed size mal leído, Marlin no detectable, red no detectable). Oportunidad: sondeo activo de firmware con identificación ($I, versión) y reintentos; descubrimiento de red (mDNS/UDP broadcast donde exista).
5. **Archivos en controlador Ruida:** envíos que parecen exitosos pero no llegan, sobrescritura con errores, memoria llena (límite de 99 archivos citado en foro), borrado lento. Oportunidad: verificación post-envío y gestor con cuota. (https://forum.lightburnsoftware.com/t/error-when-overwriting-a-file-on-controller/26192)
6. **Cámara 2.1:** overlay desfasado, fisheye bloqueado, paralaje por espesor, overlay pixelado, pérdida de alineación tras reinicio. Oportunidad: homografía por altura con interpolación, validación numérica (error de reproyección visible), alineación persistente verificada al arrancar. (enlaces en 3.13)
7. **Print & Cut limitado** a 2 puntos, sin cámara, solo gantry y absolutas. Oportunidad: N puntos con ajuste afín/homográfico y lectura de marcas por cámara.
8. **Trabajos más grandes que la cama:** no existe Tiling nativo; registro manual. Oportunidad: tiling con solapes y marcas automáticas. (3.16)
9. **Rotary confuso:** relación entre diámetro de rodillo, mm por rotación y circunferencia; ajustes que "no hacen nada" hasta reiniciar el controlador. Oportunidad: asistente de calibración medido y aviso explícito de reinicio. (3.11)
10. **Texto variable:** CSV con comillas y UTF-8, contador End desactualizado, serial no persistente, QR serializado, Test vs preview. Oportunidad: motor de plantillas con vista previa por fila y persistencia del contador. (3.15)
11. **Galvo:** desajustes de campo tras recalibrar, importación de configuraciones ajenas con offsets grandes, dependencia de archivos del fabricante (markcfg7/COR), EZCAD3/BSL sin soporte completo, Z retirado en 2.1 para EZCAD2. (3.18)
12. **Smoothieware/Cohesion3D:** pérdidas de conexión intermitentes y láser que no dispara (PWM). (https://forum.lightburnsoftware.com/t/loosing-connection-intermittently/2890)
13. **Marlin:** salida inline (M3 I / M4 I) ambigua; perfil de Snapmaker genera M106/M107. (https://forum.lightburnsoftware.com/t/marlin-dynamic-inline-mode-support-m3i-vs-m4-i/112649)
14. **xTool:** cambios de firmware del fabricante rompen compatibilidad (aviso oficial de LightBurn de 2026-06-29) y el tipo nativo solo soporta USB. (docs de `DeviceSpecificTroubleshooting/`)
15. **Parámetros dispersos:** opciones en Device Settings repartidas en 7+ pestañas, muchas visibles solo para ciertos tipos. Oportunidad: UI contextual por controladora con búsqueda.
16. **Seguridad operativa:** el modo "Laser on when framing" y el botón Fire dependen de advertencias en texto. Oportunidad: confirmaciones, límites de potencia máxima por perfil y puerta/interlock configurable.
17. **Plataforma:** Linux deja de ser soportado por LightBurn tras 1.7 y la fibra requiere reglas udev manuales (Galvo Troubleshooting). Oportunidad: Linux de primera clase con instalación de reglas.

---

## 6. Lo más difícil de replicar (síntesis)

1. **Protocolos DSP/galvo propietarios** (Ruida con variaciones por modelo y firmwares; Trocen, TopWisdom, EZCAD3, BSL sin documentación pública). Solo Ruida y EZCAD2 tienen reversa comunitaria utilizable y ambas son parciales.
2. **Cámara con precisión de milímetro**: calibración de lente, alineación por altura de material y overlay, con robustez en hardware heterogéneo (USB, red MJPEG, presets de fabricante).
3. **Streaming fiable multi-controlador y sincronización de estado** (origen/frame/start coherentes, buffers, recuperación de errores) sobre decenas de variantes de firmware.

## 7. No verificado / lagunas

- Protocolo exacto de las 4 variantes de calibración de lente de LightBurn y qué coeficientes calcula el asistente galvo de 9 puntos.
- Protocolos de Trocen, TopWisdom, EZCAD3, BSL y xTool (no se halló documentación pública).
- Formato del archivo .lbdev, .lbset, .lbcm, .lbvendor (la página de bundles solo documenta .lbzip).
- Protocolo de "Automation with UDP" (existe la guía en la navegación pero no se leyó).
- Quejas del foro sobre traducciones y sobre Smoothieware/Marlin con datos cuantitativos.
- Repositorio canónico de Rayforge: la fuente LinuxLinks lo asigna a github.com/barebaric/rayforge (leído en esta sesión), mientras que otra búsqueda mostró github.com/yoyey/rayforge; usar el primero con reserva.
- Comparaciones de rendimiento de LaserGRBL/Rayforge/MeerK40t frente a LightBurn (no investigadas).

## 8. Fuentes principales

- Documentación oficial 2.1: https://docs.lightburnsoftware.com/2.1/ (secciones citadas con `D21`).
- Wiki de Ruida: https://edutechwiki.unige.ch/en/Ruida
- MeerK40t: https://github.com/meerk40t/meerk40t (README, carpetas `ruida`, `balormk`, `grbl`, `camera`, `rotary`, `cylinder`).
- galvoplotter: https://github.com/meerk40t/galvoplotter
- LaserGRBL: https://github.com/arkypita/LaserGRBL
- Rayforge: https://github.com/barebaric/rayforge
- Marlin (modos inline láser): https://marlinfw.org/docs/configuration/2.0.9/laser_spindle.html
- Hilos del foro citados en línea en cada sección.
