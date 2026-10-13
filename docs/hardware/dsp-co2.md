# Controladoras DSP de CO₂ (no Ruida): Trocen/Anywells, TopWisdom, Leetro, Lihuiyu M2 Nano, Moshiboard

Investigación para Tlaku (sala limpia). Fecha de consulta: 2026-10-13. Ruida se documenta aparte en `docs/hardware/ruida.md`.

## 0. Método y límites

- Solo se leyeron descripciones en prosa o tablas: wikis, foros, blog y documentación de LightBurn, tutoriales de distribuidores y manuales.
- **No se leyó código fuente** de implementaciones existentes. Existen y quedan anotadas como "existe, no leída": K40 Whisperer (GPL), MeerK40t (MIT; verificado después en su LICENSE, incluye `lhystudios`, `moshi` y emulación Ruida), `k40nano` (PyPI), `richard-sim/mpc05ls`, `dumpmol` (London Hackerspace), `pointless.net/hg/mpc6515`.
- Aviso de procedencia: la wiki de EduTechWiki sobre M2 Nano y Moshiboard resume ingeniería inversa hecha principalmente por el proyecto MeerK40t. Los hechos de protocolo (bytes, comandos, IDs) no son código, pero conviene documentarlos como "observados en la wiki" y validarlos con capturas propias de hardware real o de LaserDRW antes de fiarse.
- Lo que **no se encontró públicamente**: protocolo de Trocen/AWC, formato UD5, protocolo de TopWisdom, formato `.OUT`, y un manual de usuario de AWC708C. Todo esto está cerrado y solo LightBurn lo implementa (código no público).

## 1. Trocen / Anywells (AWC608, AWC708C/Lite/S, AWC7813, AWC7824, AWC7846, LO-X7)

### Popularidad y máquinas
- LightBurn añadió soporte en la 0.8.04 como "primer paso", con AWC 708c lite, AWC 608 y el LightObject LO-X7 DSP (reetiquetado de AWC): https://lightburnsoftware.com/blogs/news/lightburn-0-8-04-awc-support-mixed-inch-mm-mode-ai-importer-fixes-and-much-more
- Lista de LightBurn DSP: AWC 708C Lite y AWC 608 (https://lightburnsoftware.com/products/lightburn-dsp). Otra página de LightBurn añade TL-3120 (https://lightBurnSoftware.com/pages/about-us). Los AWC suelen venir en máquinas Boss Laser HP, OMTech y similares (fuente indirecta en los resultados de búsqueda; no verificado en la página).
- Hay modelos nuevos (AWC7813, AWC7824, AWC7846) que se venden como reemplazo del AWC708C (https://www.ebay.de/itm/386135416279). Un usuario con AWC7813 no conseguía conectar con LightBurn hasta mantener LaserCAD abierto y reconectar USB: https://forum.lightburnsoftware.com/t/laser-not-recognized-does-lightburn-support-the-new-awc-7813-vs-old-awc-708/39008. Esto sugiere variaciones de firmware/driver entre generaciones.

### Transporte
- **Ethernet/UDP**: es el modo de LightBurn. IP estática configurada en el menú "Network Settings" del panel (https://docs.lightburnsoftware.com/Guides/Trocen-Ethernet.html). La guía no indica puertos.
- **USB**: el 608 antiguo usa chip FTDI (puerto serie virtual, COM), los más nuevos y el 708 usan un driver propio accesible vía LibUSB (https://forum.lightburnsoftware.com/t/newbie-cant-connect-awc608-at-all-please-help/6923, https://forum.lightburnsoftware.com/t/sending-files-to-awc608-doesnt-work-anymore/60316). El driver de LaserCAD suele fallar y se instala el driver FTDI CDM a mano (https://buildyourcnc.com/blogs/tutorial/tutorial-trocen-anywells-lasercad-install). VID/PID no publicados en las fuentes leídas (el FTDI usa los genéricos de FTDI; no verificado).
- **Memoria USB (pendrive)**: LaserCAD guarda archivos UD5 que se copian al USB y se cargan desde el panel; FAT32 funciona. Se necesita PC conectado para ciertos parámetros que el panel no cambia (buildyourcnc arriba; https://forum.lightburnsoftware.com/t/sending-files-to-dsp-controller-so-dont-have-to-have-pc-connected-while-running/63243).

### Protocolo y formato
- LaserCAD envía por red un archivo intermedio `.ud5`, no hace streaming de comandos (comentario de foro, no verificado: https://forum.lightburnsoftware.com/t/sending-files-to-dsp-controller-so-dont-have-to-have-pc-connected-while-running/63243).
- Los `.pwj5` (proyecto de LaserCAD) son ASCII con caracteres especiales y alguien los habría "reverse-engineered" (mención sin detalle; fuente no localizada). Ver la misma búsqueda: https://forum.lightburnsoftware.com/t/sending-files-to-dsp-controller-so-dont-have-to-have-pc-connected-while-running/63243.
- No hay documentación pública del protocolo UDP, del formato UD5, ni de ofuscación. No se encontró ningún proyecto de ingeniería inversa, ni código ni prosa.

### Funciones
- Envío (Send) y ejecución desde el panel; lista de archivos en la memoria interna que se llena y requiere "All Delete" / "Memory Manage → Check/Format Memory" (https://forum.lightburnsoftware.com/t/sending-files-to-awc608-doesnt-work-anymore/60316).
- LaserCAD importa AI, CorelDraw, DXF, PLT (buildyourcnc). Rotativo y Z existen en el hardware (la serie AWC lo anuncia; sin fuente leída). Estado/encuadre: LightBurn reconoce "Found Trocen / AWC device"; no hay descripción pública de los comandos de estado.
- Hay fallos conocidos de LightBurn con rampa de potencia (https://forum.lightburnsoftware.com/t/awc608-ramp-feature-ineffective/35537, solo título visto).

### Viabilidad sin hardware
- **Muy baja.** No hay emulador, ni capturas descritas, ni documentación. Habría que capturar tráfico de LaserCAD/LightBurn con hardware real. Sin hardware, no hay forma de validar. Riesgo alto (variantes de firmware, perfiles distintos por modelo). Dificultad: alta. Se descarta salvo que aparezca un contribuyente con una placa.

## 2. TopWisdom (TL-403CA/CB, TL-410C, TL-A1, TL-A3, TL-A4; también la TE308 de bordado)

### Popularidad y máquinas
- Soporte oficial en LightBurn desde la 0.9.10 (marzo de 2020). Modelos en el foro: TL-403CB, TL-410C, TL-A1, TL-A3, TL-A4 (https://forum.lightburnsoftware.com/c/lightburn-hardware/topwisdom/27). Ejemplos de máquinas: CM1290 80W con TL-403CB (https://forum.lightburnsoftware.com/t/topwisdom-software-usage/17973), SaleCNC.
- Menos popular que Ruida; presente en máquinas chinas de gama económica.

### Transporte
- **USB solamente** en LightBurn. Red no soportada: el desarrollador dijo en 2019 que el 403 es "el primer controlador que usa TCP en lugar de UDP" y que debía escribir un relé intermedio (https://forum.lightburnsoftware.com/t/top-wisdom-controller-support/81); en 2020-2021 el equipo repitió que no hay red (https://forum.lightburnsoftware.com/t/tl-403cb-network-connection-how-to/12329). Un hilo de TL-A3 de 2024 sigue sin red: https://forum.lightburnsoftware.com/t/network-ethernet-and-wifi-for-topwisdom-tl-a3/124615.
- Driver: FTDI (puerto serie virtual) o el que instala AutoLaser (https://forum.lightburnsoftware.com/t/usb-device-not-recognized-topwisdom-tl-410c/11135, https://forum.lightburnsoftware.com/t/topwisdom-software-usage/17973). VID/PID no publicados en las fuentes leídas.
- Hay también transferencia con memoria USB (archivos `.OUT`; hilo "Topwisdom screaming 'other type file'": https://forum.lightburnsoftware.com/t/topwisdom-screaming-other-type-file/152961). El formato `.OUT` no tiene descripción pública.

### Protocolo y formato
- Sin documentación pública. El desarrollador de LightBurn dijo en 2019 que había desensamblado gran parte del formato de archivo del 403 (https://forum.lightburnsoftware.com/t/top-wisdom-controller-support/81); no se publicó. No se menciona NDA ni cooperación del fabricante.
- Los ajustes (incluido rotativo) viven en el controlador y se editan en "Machine Settings", como en AutoLaser (https://forum.lightburnsoftware.com/t/current-topwisdom-development-status/13899).

### Funciones
- Faltaban (2020): modo atravesar (dwell), modo punto, escalado de potencia en corte vectorial, movimientos Z durante el corte (posiblemente sin soporte de hardware). Rotativo funciona vía Machine Settings; solo la ventana Rotary Setup no (mismo hilo de estado).
- Z: un usuario de TL-403 no veía controles Z; sin respuesta definitiva (mismo hilo).
- Algunos usuarios migran a Ruida (hilo "TL403CB to ruida rdc 6442s-b", categoría TopWisdom).

### Viabilidad sin hardware
- **Muy baja.** Protocolo cerrado, sin ingeniería inversa pública, mezcla de USB serie y TCP. Misma conclusión que Trocen.

## 3. Leetro (MPC6515/6525/6535, MPC05LS)

### Popularidad
- Controladoras antiguas de CO₂ (varias marcas de 2010-2015, p. ej. GWeike); el software LaserCut 5.3/6.1 y las MPC6515/6535 se consideran obsoletos (https://sawmillcreek.org/threads/new-leetro-release-lasercut-6-1.222654/, comentario de usuario). LightBurn **no** las soporta; la comunidad las reemplaza por Ruida (https://forum.lightburnsoftware.com/t/i-am-looking-for-pinout-of-mpc6515c-leetro-controller/10011).

### Transporte
- USB, VID 0x0548, PID 0x1005; escritura al endpoint 0x04 y lectura del 0x88 (https://edutechwiki.unige.ch/en/Leetro).

### Protocolo
- Propietario. Las únicas pistas públicas son un trabajo de London Hackerspace que decodifica archivos `.mol` y una tabla de unos 90 códigos de comandos (mover, pausa, curva, potencia PWM, retardo, etc.) en la wiki de EduTechWiki, descrita como "muy menor y dispersa" (misma URL). Existen un DLL falso (`mpc05ls`), `dumpmol` y `pointless.net/hg/mpc6515`: existe, no leída. La página del hackerspace de Londres no pudo recuperarse (404).
- Manual de hardware v2.0 de la MPC6515/6525A en https://wiki.bristolhackspace.org/_media/equipment/cnc/laser/mpc6515_6525a-hardware_manual-v2.0.pdf (no se pudo leer: 404 en la consulta; revisar manualmente si sigue disponible). Otra copia: https://www.cnc.info.pl/download/file.php?id=20421 (no abierta).

### Viabilidad sin hardware
- **Muy baja**: hardware viejo y en decadencia, protocolo en parte desconocido. No recomendado.

## 4. Lihuiyu / Lihui Labs M2 Nano (K40 de serie) y familia (M, M1, M2, M3, A, B, B1, B2)

### Popularidad
- Es con diferencia la placa más extendida en láseres chinos baratos de 40 W (K40). LightBurn **no** la soporta ("genuinely not a supported controller"; la DSP no ayuda; incluso han reembolsado licencias): https://forum.lightburnsoftware.com/t/laser-not-supported/25768 y https://forum.lightburnsoftware.com/t/laser-not-available-after-using-find-my-laser/8531. LightBurn detecta la placa pero muestra "(Not Supported)".
- Las alternativas de la comunidad son K40 Whisperer (GPL) y MeerK40t (MIT), ambos de código abierto. Detalles de uso en prosa: https://www.scorchworks.com/K40whisperer/k40whisperer.html.

### Transporte
- USB: puente **CH341A/B en modo paralelo EPP 1.9** conectado a un MCU tipo 8051 (LHY730318). VID 0x1A86, PID 0x5512; lectura por el endpoint 0x82 y escritura por 0x02 (https://EduTechWiki.unige.ch/en/Lhystudios_M2_Nano).
- No requiere dongle. Requiere driver libUSB en Windows (K40 Whisperer, URL anterior); en Linux/macOS bastaría con libusb sin driver.

### Protocolo (descrito públicamente en prosa)
Todo proviene de https://EduTechWiki.unige.ch/en/Lhystudios_M2_Nano y https://forum.makerforums.info/t/explanation-how-the-m2-nano-works/80910.
- **Placa sin memoria ni matemáticas**: solo unos pocos bytes de RAM (128 bytes en el 8051) y firmware de ~4K; el PC hace toda la planificación. No acepta G-code.
- **Paquetes**: 34 bytes (a veces descrito como 30 de datos + CRC): `0xA6`, `0x00`, 30 bytes de carga útil, un `0x00` final y un byte CRC calculado sobre la carga útil (el tamaño exacto es inconsistente en la propia wiki: 30, 32 o 34 bytes según el texto; validar con captura). Consulta de estado: un byte `0xA0` y respuesta de 6 bytes.
- **Códigos de estado**: 206 listo, 238 ocupado, 207 error de CRC, 236 tarea completa, 239 sin energía suficiente, 204 error desconocido.
- **Lenguaje LHYMICRO-GL**: flujo ASCII tipo trazador. Mayúsculas son comandos (I inicializar, P pausa, B/T dirección X, R/L dirección Y, D/U láser encendido/apagado, C corte, G paso de trama, V código de velocidad, @ reset, F finalizar, N confirmar, E/S1E modo compacto, S1P/S2P ejecutar). Minúsculas son magnitudes (a–y = 1–25, z = 255, `|` suma 25, tres dígitos literales). La unidad es 1/1000 de pulgada (1 mil).
- **Modo por defecto vs. compacto**: en compacto cada comando se ejecuta de inmediato y la dirección cambia el paso de trama automáticamente.
- **Códigos de velocidad**: ecuaciones por engranaje y por modelo (A, B, B1, B2, M, M1, M2) con reloj de 21.1184 MHz; hay rangos inválidos y errores conocidos en los códigos del software chino. Es la parte más difícil de reproducir fielmente. La wiki da un ejemplo resuelto (M2, 0.5 in/s = `CV1410801013003004`).
- **Trama (raster)**: pasos G cuando se invierte la dirección; el chip se queda sin buffer con alta densidad de píxeles a alta velocidad.
- **Potencia**: las placas de serie **no controlan potencia por software** (Whisperer: usan semitonos); la M3 Nano sí, si está cableada para ello. No hay "capas con potencia mín/máx" a nivel de protocolo; Tlaku tendría que simular escala de grises con semitonos o con modulación PWM del ciclo de trabajo.
- **Formato de archivo EGV** (LHYMICRO-GL): cabecera "LHYMICRO-GL file", versión 1.0.01, línea de dimensiones y el código.
- **Ofuscación**: ninguna descrita. Aviso: los paquetes de Moshiboard sí están "revueltos" (sección 5).

### Funciones
- Movimiento absoluto/relativo, home (hasta los finales de carrera), pausa/reanudar y abortar. Sin lectura de estado más allá de los 6 bytes; sin archivos en memoria; sin panel de control (solo botones físicos de la máquina); rotativo y Z no son parte de la placa (Z es manual en la mesa del K40).

### Viabilidad sin hardware
- **Media-alta.** Es el único protocolo de este informe con descripción pública suficiente en prosa. Se puede construir un emisor de LHYMICRO-GL y un **simulador de placa** (CH341 falso + intérprete de 12 estados) para pruebas de regresión sin hardware: paquetes, CRC y estados 206/238/207/236 están descritos. El punto débil son las ecuaciones de velocidad (diferencias de ~8.9 % entre placas y el software chino) y el comportamiento de rampa. Riesgo: eso se prueba bien solo con una máquina real; con una mala velocidad no hay daño, pero con el láser siempre hay riesgo físico (por ejemplo láser encendido durante una pausa o un choque contra el final de carrera), por lo que el primer despliegue debe ser con rampas conservadoras y detección de fallo ("fail-safe" a láser apagado ante error de CRC o desconexión).
- Nota posterior: MeerK40t es MIT y el driver de Tlaku (`tlaku-lhymicro`) se apoyó en él con crédito en THIRD_PARTY.md. Whisperer es GPL: no se copió. Para validar sigue siendo útil obtener por medios propios: (a) un K40 barato para capturar con Wireshark/usbmon la salida de LaserDRW, o (b) el simulador anterior.

## 5. Moshiboard / MoshiDraw

- Placa de CO₂ chino-asiática (menos común que M2 Nano); el software MoshiDraw es propietario. Máquinas: no listadas en las fuentes. K40 Whisperer **no** soporta Moshi.
- **Transporte**: también CH341A en modo EPP 1.9 con los mismos VID/PID 0x1A86/0x5512 que la M2 Nano (https://edutechwiki.unige.ch/en/Moshiboard, espejo https://w.arbores.tech/wiki/Moshiboard).
- **Protocolo**: bytes "revueltos" (swizzle): cada byte tiene 4 bits de comando y 4 bits aparentemente aleatorios, con una permutación de bits que depende del bit menos significativo, más una tabla 16×16 de desrevuelto. Coordenadas absolutas de 2 bytes. De 16 comandos posibles solo se conocen mover/cortar en X, Y, XY absolutos y el inicio de cabecera; la mayoría figura como UNKNOWN.
- Formato de archivo: no descrito. Potencia: no descrita.
- **Viabilidad sin hardware**: **baja**. Ingeniería inversa incompleta (la última edición de la wiki es de agosto de 2020). Hacer una detección por VID/PID (coincide con M2 Nano) puede confundir: habría que distinguirlas probando el protocolo. Prioridad mínima.

## 6. Tabla comparativa

| Controladora | Popularidad | Soporte en LightBurn | Transporte | Doc. pública del protocolo | Ofuscación | Potencia/capas | Viabilidad sin hardware | Riesgo |
|---|---|---|---|---|---|---|---|---|
| Trocen/Anywells AWC608/708 | Alta en máquinas medianas | Sí (DSP, Ethernet/UDP y USB) | UDP, USB (FTDI o propio), USB stick (UD5) | Ninguna (UD5 y UDP cerrados) | Desconocida | Sí (ramp, rotativo) | Muy baja | Alto |
| TopWisdom TL-403/410/A1/A3 | Media-baja | Sí (solo USB, con huecos) | USB (FTDI), TCP no resuelto, USB stick (.OUT) | Ninguna | Desconocida | Sí, con carencias | Muy baja | Alto |
| Leetro MPC6515/6525 | Baja, obsoleta | No | USB bulk (0548:1005) | Parcial (tabla de comandos, .mol), prosa en wiki | Desconocida | Sí (PWM, potencia) | Muy baja | Alto |
| Lihuiyu M2 Nano (K40) | **Muy alta** | **No** (explícito) | USB CH341 (1A86:5512) | **Buena** (paquetes, CRC, estados, lenguaje, velocidades) | No | **No** en placas de serie (semitonos) | Media-alta (simulador posible) | Medio (físico) |
| Moshiboard | Baja | No | USB CH341 (1A86:5512) | Parcial (swizzle, 16 comandos, la mayoría desconocidos) | Sí (bits revueltos) | Desconocida | Baja | Alto |

## 7. Recomendación de prioridad para Tlaku

1. **Primero Ruida** (documento aparte): es el objetivo compatible con LightBurn con más documentación pública en comparación.
2. **M2 Nano (K40) como segunda prioridad y la única con camino realista sin hardware.**
   - A favor: base de usuarios enorme, LightBurn no la cubre (hueco de mercado claro), protocolo descrito en prosa (paquete, CRC, estados, lenguaje, velocidades), USB estándar sin dongle ni ofuscación, y se puede escribir un simulador de placa para pruebas.
   - En contra: no hay control de potencia por software en las placas de serie (todo es semitono y la potencia se ajusta en el panel), sin rotativo/Z/archivos, ecuaciones de velocidad delicadas, y el usuario típico es poco técnico y tiene el láser sin protecciones.
   - Plan: crate propio `tlaku-lhymicro` (sala limpia, MIT) con generador de flujo, empaquetador/CRC, planificador de velocidades conservador y simulador; trabajo de validación con un K40 real prestado o comprado por un colaborador, antes de declararlo estable. Etiquetarlo "experimental".
3. **Trocen/Anywells y TopWisdom: posponer** hasta que haya un colaborador con hardware que pueda capturar tráfico (Wireshark/usbmon) de LaserCAD/AutoLaser y publicar descripciones en prosa. Sin hardware no hay vía segura. Prioridad: Trocen antes que TopWisdom (más presente en máquinas occidentales, Ethernet/UDP con IP configurable y ya soportado por LightBurn, lo que sugiere un protocolo más sencillo).
4. **Leetro y Moshiboard: no soportar** (obsoletas o con protocolo incompleto). Si se detecta una Moshiboard, avisar al usuario de que no es una M2 Nano aunque compartan VID/PID.
5. Una vía alternativa de bajo coste para todo lo anterior: documentar la **sustitución de placa** (Ruida 6442, GRBL/Cohesion3D, MKS DLC32), como hace la comunidad de LightBurn.

## 8. Fuentes principales
- https://lightburnsoftware.com/products/lightburn-dsp
- https://lightburnsoftware.com/blogs/news/lightburn-0-8-04-awc-support-mixed-inch-mm-mode-ai-importer-fixes-and-much-more
- https://docs.lightburnsoftware.com/2.1/Explainers/LaserTypes/
- https://docs.lightburnsoftware.com/Guides/Trocen-Ethernet.html
- https://forum.lightburnsoftware.com/t/current-topwisdom-development-status/13899
- https://forum.lightburnsoftware.com/t/top-wisdom-controller-support/81
- https://forum.lightburnsoftware.com/t/tl-403cb-network-connection-how-to/12329
- https://forum.lightburnsoftware.com/t/laser-not-supported/25768
- https://forum.lightburnsoftware.com/t/why-no-k40-w-lightburn/74355?page=2
- https://buildyourcnc.com/blogs/tutorial/tutorial-trocen-anywells-lasercad-install
- https://EduTechWiki.unige.ch/en/Lhystudios_M2_Nano
- https://forum.makerforums.info/t/explanation-how-the-m2-nano-works/80910
- https://www.scorchworks.com/K40whisperer/k40whisperer.html
- https://edutechwiki.unige.ch/en/Leetro
- https://edutechwiki.unige.ch/en/Moshiboard
