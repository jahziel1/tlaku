# Controladoras: qué soporta LightBurn y qué debería soportar Tlaku

Fecha de investigación: 2026-10-13. Meta del dueño: soportar la mayoría de las controladoras que soporta LightBurn. Lo que sigue es la investigación (hecha antes de escribir código); el estado real de lo que Tlaku soporta hoy está en la tabla de abajo, actualizada al cerrar la ola 4.

Documentos hermanos (los escriben otros agentes; aquí solo se enlazan):

- GRBL a fondo: [grbl.md](grbl.md)
- Ruida (protocolo, transporte, formato `.rd`): [ruida.md](ruida.md)
- Creality Falcon2: [falcon2.md](falcon2.md)
- Inventario funcional de LightBurn (ventanas, cámara, rotativo...): [../spec/03-maquinas-avanzadas.md](../spec/03-maquinas-avanzadas.md)

## Estado de soporte en Tlaku (2026-10-13, tras la ola 4)

Esta tabla es lo que hay **en el código**, no lo que recomienda la investigación. Ninguna fila se ha probado con una máquina real. Detalle y pruebas: [../spec/04-cobertura.md](../spec/04-cobertura.md) y [../manual/13-controladoras.md](../manual/13-controladoras.md).

| Familia | `DeviceKind` | Transporte | Estado | Probado en emulador | Por validar con hardware |
|---|---|---|---|---|---|
| GRBL 1.1 (M4) | `Grbl` | USB serie; TCP y WebSocket | Estable en emulador | Flujo completo (`tlaku-emu`, máquina virtual) y conexión por red con sockets de verdad | Todo; la primera prueba será una Falcon2 |
| GRBL-M3 (antiguo) | `GrblM3` | USB serie | Por validar | Solo el G-code (pruebas del motor); sin máquina virtual propia | Todo |
| GRBL-LPC (Cohesion3D) | `GrblLpc` | USB serie | Por validar | Solo el G-code; sin máquina virtual propia | Todo |
| grblHAL | `GrblHal` | USB serie; TCP (Ethernet) | Estable en emulador | Máquina virtual `grblhal` | Todo, incluida la red |
| FluidNC | `FluidNc` | USB serie; TCP (Telnet); WebSocket | Estable en emulador | Máquina virtual `fluidnc` | Todo, incluida la red |
| Marlin (láser) | `Marlin` | USB serie | Estable en emulador (básico) | Máquina virtual `marlin` | Todo |
| Smoothieware | `Smoothie` | USB serie | Estable en emulador (básico) | Máquina virtual `smoothie` | Todo |
| G-code personalizado | `CustomGcode` | USB serie | Por validar | El dialecto lo escribe el usuario | Todo |
| Ruida (RDC644x...) | `Ruida` | UDP | **Experimental** | Emulador UDP propio (`tlaku-ruida`): sondeo, envío guardando o ejecutando, estado, pausa, detener, saltos, encuadre, información y lista de documentos (solo lectura) | Todo: direcciones y unidades de la información, numeración de documentos, home, estados, y el comportamiento ante una subida cortada |
| K40 con M2 Nano | `LhyMicro` | USB (CH341) | **Experimental**; solo línea de comandos (`tlaku k40`) por ahora <!-- K40-APP --> | Placa simulada (`tlaku-lhymicro`): CRC, paquetes, estados, velocidades por placa contra 1511 vectores de MeerK40t | Todo: el USB real, el home, los tiempos y los códigos de estado |
| Galvo JCZ/BJJCZ (EZCAD2) | — | — | No soportada | — | — |
| EZCAD3/DLC, BSL, Trocen, TopWisdom | — | — | No soportada | — | — |
| xTool por Wi-Fi propietario, Glowforge | — | — | No soportada | — | — |

## 0. Reglas y honestidad de las fuentes

- Sala limpia: solo documentación pública, páginas oficiales de LightBurn, foros y fichas de fabricantes. No se leyó código de ningún proyecto de terceros. De los proyectos mencionados solo se consultó su licencia (metadatos de GitHub) y su README o ficha.
- **LightBurn no publica una lista de modelos.** Su documentación agrupa por familia de controladora (GCode, DSP, Galvo). Fuentes: [Tipos de láser (2.1)](https://docs.lightburnsoftware.com/2.1/Explainers/LaserTypes/), [Identifica tu láser (2.1)](https://docs.lightburnsoftware.com/2.1/GetStarted/LaserID/). La lista de modelos concretos de Ruida y Trocen viene del foro oficial: [hardware-list-available](https://forum.lightburnsoftware.com/t/hardware-list-available/13190).
- Las cifras de popularidad son **estimaciones propias para ordenar prioridades, sin fuente**, salvo la única cifra publicada que encontré (xTool, autoinformada, ver §3).
- Los "modelos que usan firmware X" salen de hilos del foro de LightBurn. Son reportes de usuarios, no especificaciones del fabricante. Marcados "(foro)".

## 1. Qué soporta LightBurn hoy

### 1.1 Familias según la documentación oficial

| Tipo | Controladoras / firmware | Licencia de LightBurn | Fuente |
|---|---|---|---|
| GCode | GRBL, Smoothieware, Marlin, FluidNC, grblHAL, xTool | Core o superior | [LaserTypes 2.1](https://docs.lightburnsoftware.com/2.1/Explainers/LaserTypes/) |
| DSP | Ruida, Trocen, TopWisdom | Pro | idem |
| Galvo | EZCAD2, EZCAD2 Lite, EZCAD3, BSL (cualquier láser que use EZCAD2, EZCAD3 o SeaCAD como software de fábrica) | Pro | idem |

Además, el asistente de dispositivos ofrece "Custom GCode" (plantillas propias para máquinas con G-code no estándar) y "No Machine" (cama virtual): [Crear manualmente](https://docs.lightburnsoftware.com/2.1/GetStarted/CreateManually/).

Variantes que aparecen en el selector de LightBurn y en su documentación:

- **GRBL** (1.1f o posterior, usa M4) y **GRBL-M3** (1.1e o anterior, solo M3, por ejemplo X-Carve anterior a 2018 con GRBL 1.0c): [Configuraciones GRBL comunes](https://docs.lightburnsoftware.com/CommonGrblSetups).
- **GRBL-LPC** (port de GRBL a placas LPC17xx): aparece en la lista oficial de firmware del foro, junto con Smoothieware y Marlin ([hardware-list-available](https://forum.lightburnsoftware.com/t/hardware-list-available/13190)).
- **grblHAL** y **FluidNC**: listados en la documentación 2.x.
- **xTool**: tipo nativo desde LightBurn 1.7.00 para D1, D1 Pro y S1 por serie/USB. No soporta F1 ni P2 ni ninguna conexión de red: [hilo oficial](https://forum.lightburnsoftware.com/t/setting-up-xtool-s1-with-new-xtool-device-type-in-lb-1-7-and-beyond/135463).

### 1.2 Modelos concretos citados por LightBurn (foro oficial)

- Ruida: RDC6442G/S, RDC6445G, RDC6332G, RDLC-320A, R5-DSP.
- Trocen (Anywells): AWC708C (incluye la "lite"), AWC608, TL-3120. LightBurn lo agregó en 0.8.04, [solo Ethernet](https://lightburnsoftware.com/blogs/news/lightburn-0-8-04-awc-support-mixed-inch-mm-mode-ai-importer-fixes-and-much-more).
- TopWisdom: soportado, sin modelos nombrados. Staff dijo que no soportan su Ethernet porque el protocolo no es un port del serie ([hilo](https://forum.lightburnsoftware.com/t/network-control-solution-suggestion-for-topwisdom-tl-a3-and-others/154537)).
- Si la placa "es GCode, Ruida, Trocen o TopWisdom aunque no esté en la lista, puede funcionar" (resumen de la web de LightBurn en [búsqueda](https://forum.lightburnsoftware.com/t/hardware-list-available/13190)).

### 1.3 Lo que LightBurn NO soporta (útil para saber qué es ventaja competitiva)

| Hardware | Estado | Fuente |
|---|---|---|
| K40 con placa M2 Nano de fábrica | No soportado: el autor lo consideró cerrado y no rentable de ingeniería inversa; recomienda cambiar de placa (Cohesion3D, Mini Gerbil, MKS SBase con Smoothie) o usar K40 Whisperer/MeerK40t | [foro](https://forum.lightburnsoftware.com/t/k40-laser-with-m2-board/34739), [foro](https://forum.lightburnsoftware.com/t/lightburn-detects-the-m2-nano-via-ser2usb-as-grbl-m3-1-1-or-earlier/7497) |
| Glowforge | No: nube propietaria, sin modo local | [foro](https://forum.lightburnsoftware.com/t/glowforge-initial-setup/62115), [foro](https://forum.lightburnsoftware.com/t/lightburn-and-glowforge-aura/159740) |
| xTool F1, P2, M1, y S1/D1 por Wi-Fi | No nativo | [hilo xTool](https://forum.lightburnsoftware.com/t/setting-up-xtool-s1-with-new-xtool-device-type-in-lb-1-7-and-beyond/135463), [M1](https://forum.lightburnsoftware.com/t/support-for-xtool-m1/71053) |
| Ethernet TopWisdom | No | [hilo](https://forum.lightburnsoftware.com/t/network-control-solution-suggestion-for-topwisdom-tl-a3-and-others/154537) |
| Wi-Fi de xTool | No (hasta 1.5) | [GRBL comunes](https://docs.lightburnsoftware.com/CommonGrblSetups) |

### 1.4 Marcas de diodo y su firmware (casi todo GRBL)

| Marca / modelo | Firmware observado | Conexión | Fuente |
|---|---|---|---|
| Creality Falcon / Falcon2 | GRBL (ver [falcon2.md](falcon2.md)) | USB serie | otros agentes |
| Sculpfun S9/S30 | GRBL 1.1h; algunas versiones de fábrica sin homing; varias placas según el año (8 bits y 32 bits) | USB serie (CH340) | [foro](https://forum.lightburnsoftware.com/t/s9-laser-no-longer-firing-after-following-the-grbl-firmware-update/95845), [guía de flasheo](https://docs.lightburnsoftware.com/Guides/SculpfunS9-Flashing.html) |
| Atomstack (X7, A5 Pro, A10, S10) | GRBL; 115200; `$32=1`; S max 1000. M100 usa grbl-ESP32 | USB serie (CH340) | [foro X7](https://forum.lightburnsoftware.com/t/adding-atomstack-x7-a10-s10-in-lightburn/59803), [M100](https://forum.lightburnsoftware.com/t/atomstack-m100-grbl-esp32-and-33/106511) |
| Ortur Laser Master 3 | grblHAL (el banner dice GrblHAL 1.1f) | USB serie y Wi-Fi | [foro](https://forum.lightburnsoftware.com/t/ortur-lm3-not-connecting-with-either-wifi-or-usb/76960), [foro](https://forum.lightburnsoftware.com/t/lightburn-device-setting-for-ortur-lm3-20w/106380) |
| Longer Ray5 | Grbl_ESP32 1.3a; el miniS pide un perfil `.lbdev` del fabricante | USB serie | [Longer](https://www.longer3d.com/blogs/software/import-configuration-file), [foro](https://forum.lightburnsoftware.com/t/longer-ray5-10w-lgt-laser-ray5-v1-1-grbl-esp32-1-3a-works-in-lasergrbl-but-not-lightburn-2-1-03/191293) |
| Neje Master 2S / 2S Plus | GRBL (Atmel 1.1f; placas nuevas con GD32); algunos modelos exigen 38400 baudios | USB serie | [foro](https://forum.lightburnsoftware.com/t/neje-master-2s-plus/43427), [foro](https://forum.lightburnsoftware.com/t/laser-power-does-not-increase-using-lightburn-but-is-ok-with-nejes-own-software-neje-neje-master-2s-a40640/63890) |
| xTool D1 / D1 Pro / S1 | Dialecto GRBL propio con peculiaridades; LightBurn creó un controlador nativo (en GRBL genérico había fallos, como rectángulos cortados como triángulos) | USB serie; red solo con XCS | [hilo xTool](https://forum.lightburnsoftware.com/t/setting-up-xtool-s1-with-new-xtool-device-type-in-lb-1-7-and-beyond/135463) |
| Snapmaker 2.0 (módulo láser) | Marlin modificado (FreeRTOS), perfil "Snapmaker (Marlin)" en LightBurn, potencia inline en firmware reciente | USB serie | [foro](https://forum.lightburnsoftware.com/t/laser-inline-power-with-snapmaker-2-0-and-most-recent-firmware/113589), [firmware GPL-3.0](https://github.com/Skreelink/Snapmaker2-Controller) |
| Shapeoko / X-Carve (con láser) | GRBL 1.1f / 1.0c | USB serie | [GRBL comunes](https://docs.lightburnsoftware.com/CommonGrblSetups) |
| Two Trees, Eleksmaker, Acro, otras | No verifiqué el firmware de cada una; la documentación solo las llama "menos comunes, pueden necesitar ajustes" | USB serie | [GRBL comunes](https://docs.lightburnsoftware.com/CommonGrblSetups) |

Conclusión: la gran mayoría de marcas de diodo son GRBL por serie USB; los matices son baudios, `$30`/S max, M3 vs M4, homing ausente, y dos familias derivadas (grblHAL, grbl-ESP32/FluidNC).

### 1.5 CO2 chino y galvo (de la documentación de LightBurn y del foro)

- CO2 chino de caja metálica con pantalla: DSP (Ruida mayoritario; Trocen y TopWisdom detrás). Ejemplo de un CO2 de 180 W con Ruida 644XG: [foro](https://forum.lightburnsoftware.com/t/644xg-ruida-controller-in-a-brand-new-180w-blue-and-white-chinese-co2-laser-mac-or-windows/35768). Marcas concretas (Thunder, OMTech, Monport, Cloudray): mi conocimiento previo dice que usan sobre todo Ruida, pero **no lo verifiqué en esta sesión**.
- Fibra / UV / CO2 galvo (cabezal fijo, espejos): JCZ/BJJCZ (LMC "EZCAD2"), EZCAD3 y BSL. LightBurn agregó EZCAD3 en 2.1 como función "Labs", paridad parcial con EZCAD2 (rotativo, split marking, jog de Z; Z automático pendiente; solo control de un eje a la vez): [anuncio 2.1](https://lightburnsoftware.com/blogs/news/lightburn-2-1-quick-nest-enhanced-camera-support-undo-history-and-more), [hilo EZCAD3](https://forum.lightburnsoftware.com/t/initial-ezcad3-support/188524).

## 2. Una fila por familia

Leyenda de dificultad (sin hardware): **B** baja, **M** media, **A** alta. "Doc" = documentación pública del protocolo.

| Familia | Protocolo | Transporte | Doc pública | Popularidad (est. propia) | Dificultad / riesgo | Simulador o emulador | Funciones especiales |
|---|---|---|---|---|---|---|---|
| **GRBL 1.1 (M4)** | G-code + `$` ajustes + comandos en tiempo real; `ok`/`error:n`; estado `?` | USB serie (CH340/CP210x/FTDI), 115200 típico; Wi-Fi vía puente ESP en algunas | Sí, oficial (wiki del proyecto). Ver [grbl.md](grbl.md) | Diodo hobby: mayoría absoluta del segmento | **B** (ya hecho) | Existen simuladores de comunidad (p. ej. [grbl-simulator](https://github.com/tmpvar/grbl-simulator), sin licencia declarada); Tlaku ya tiene `memory_pair` | Z por G0 en modelos que lo tienen, rotativo reemplazando Y, aire M7/M8, frame por G-code |
| **GRBL-M3 (0.9/1.0/1.1e-)** | Igual, sin M4; S constante | Igual | Igual | Cola larga, X-Carve antiguo, placas viejas | **B**: ya existe `PowerMode::Constant` | Igual | Sin modo láser `$32` |
| **GRBL-LPC** | GRBL portado a LPC17xx (Cohesion3D, MKS SBase) | USB serie | Pocas; sigue la wiki de GRBL (licencia "Other" en GitHub: [cprezzi/grbl-LPC](https://github.com/cprezzi/grbl-LPC), tratar como GPL) | Pequeña (K40 con placa cambiada) | **B** | Igual | Pocas |
| **grblHAL** | Superconjunto de GRBL 1.1: más ejes, E/S, G-code ampliado, nivel de compatibilidad ajustable | USB serie; Ethernet y Wi-Fi según placa; SD en algunas | Sí, org [grblHAL](https://github.com/grblHAL) | Media-baja (Ortur LM3, CNC) | **B**: banner distinto; `$I` y `$$` extra | Hay [simuladores GRBL de comunidad](https://github.com/tmpvar/grbl-simulator), no específicos | Más ejes (A/B/C), más ajustes |
| **FluidNC / grbl-ESP32** | Protocolo GRBL; configuración en YAML en vez de `$`; `$`-comandos para radio | USB serie, **Wi-Fi (telnet y websocket)**, Bluetooth (no a la vez que Wi-Fi), SD | Sí, [wiki del proyecto](https://wiki.fluidnc.com/), pero no el detalle de websocket/telnet (no lo encontré) | Media y creciente (Longer, Atomstack M100) | **B** por serie; **M** por Wi-Fi | [fluidnc-simulator](https://github.com/ahedderich/fluidnc-simulator) (de comunidad, licencia sin declarar) | WebUI, archivos en SD, aire/relés por pin |
| **Smoothieware** | G-code; S de 0 a 1 por defecto (`maximum_s_value` configurable); M3/M4 poco usados; `?` devuelve estado estilo GRBL | USB serie, Ethernet (telnet/web), SD con `play` | Sí, [smoothieware.org/laser](http://smoothieware.org/laser); firmware GPL-3.0 | Baja (Cohesion3D, MKS SBase, K40 modificados) | **M** por flujo `ok` y escala de S; riesgo bajo | Ninguno conocido | Pantalla LCD, SD, mejor en vectores que en raster según foros ([foro](https://forum.lightburnsoftware.com/t/diode-laser-recommended-hardware-firmware/4405)) |
| **Marlin (láser)** | G-code; `M3/M4 S` en PWM 0-255 (o % o RPM por configuración) y potencia **inline** con `M3 ... I`; sin `?` de GRBL (usa `M114`/`M105`) | USB serie; arranca lento al abrir el puerto (reinicio) | Sí, [M3](https://marlinfw.org/docs/gcode/M003.html); firmware GPL-3.0 | Baja como láser puro; alta como base de Snapmaker y otras | **M**: otro flujo de estado y de homing; riesgo medio (variantes) | Marlin tiene build nativa de simulación (no verificada aquí) | Aire con M106, Z, eje B en Snapmaker rotativo |
| **Custom GCode** | Plantillas definidas por el usuario | Serie | Es una función de LightBurn ([ver spec fila 8](../spec/03-maquinas-avanzadas.md)) | Cola larga | **M** (diseño del motor de plantillas) | Se puede probar con salida a archivo | Cubre firmwares raros sin código nuevo |
| **xTool D1 / D1 Pro / S1** | GRBL con extensiones propias; en LightBurn hay un controlador nativo | USB serie; red solo en XCS | No (solo foros) | **Alta en volumen**: xTool dice tener el 47 % del GMV de grabadoras y cortadoras en los 9 primeros meses de 2025 (autoinformado en su folleto de salida a bolsa: [HKEX](https://www1.hkexnews.hk/app/sehk/2026/108021/a128793/sehk26010100112.pdf)), pero mucho es F1/P2/M1 con red propietaria | **M** | No | Z propio con macro `M111 S2`, eje U de rotativo, tapa con sensor |
| **Ruida (RDC644x, RDC633x...)** | Binario propio (`.rd` ofuscado) sobre UDP 50200/40200 con checksum de 16 bits y ACK por paquete; también USB serie (FTDI) y archivo en USB | USB serie, USB "Packet", **Ethernet UDP**, LightBurn Bridge (TCP→UDP), memoria USB | Comunidad: [EduTech Wiki](https://edutechwiki.unige.ch/en/Ruida); más en [ruida.md](ruida.md) | **Alta** en CO2 chino (mi estimación: segunda familia tras GRBL) | **A**: sin hardware solo se prueba contra un emulador propio; UDP sin reintentos ([foro](https://forum.lightburnsoftware.com/t/anyone-know-has-the-ruida-firmware-ever-been-updated-to-fix-their-network-limitation/151129/9)); mala escritura de parámetros puede dañar la máquina | Existe un emulador en [MeerK40t](https://github.com/meerk40t/meerk40t) (MIT; no lo leí) | Rotativo en el controlador, archivos en memoria, lista de archivos, scanning offset, enfoque, ajustes del firmware |
| **Trocen / Anywells (AWC708C, AWC608, TL-3120)** | Binario propio; formato de archivo `.ud5` | Ethernet (LightBurn solo da esto; usan IP fija), USB y memoria USB en el controlador | **No encontré** documentación pública ni ingeniería inversa publicada | Media-baja en CO2 | **A** (necesita hardware o captura de tráfico propia) | Ninguno conocido | Rotativo se configura en el controlador, no desde LightBurn ([foro](https://forum.lightburnsoftware.com/t/lightburn-with-awc708c-over-ethernet-and-rotary/121912)) |
| **TopWisdom** | Binario propio; archivos `.out`; el protocolo Ethernet no es un port del serie | USB serie; Ethernet **no** soportado por LightBurn | **Ninguna oficial**; hilos de ingeniería inversa incompletos ([foro](https://forum.lightburnsoftware.com/t/network-control-solution-suggestion-for-topwisdom-tl-a3-and-others/154537)) | Baja | **A** | Ninguno | Pocas |
| **Galvo JCZ / BJJCZ (LMC, EZCAD2)** | Binario sobre USB: comandos de 12 bytes (códigos de 16 bits little-endian), listas de comandos, tabla de corrección 63x63, sin cifrado | USB directo (VID 0x9588, PID 0x9899) con endpoints en bloque | Comunidad: [EduTech Wiki LMCV4](https://edutechwiki.unige.ch/en/LMCV4-FIBER-M) (ingeniería inversa de Schroeder y Dorie, **sin licencia declarada**) y [galvoplotter](https://github.com/meerk40t/galvoplotter) (MIT, no lo leí) | **Alta en fibra/UV/CO2 galvo** (marcadoras de fibra baratas) y creciente | **A**: riesgo de seguridad real (disparo accidental; las listas son rápidas); calibración de lente; sin hardware no se valida | Ninguno conocido | Rotativo galvo, Z, frecuencia/ancho de pulso, tabla de corrección, split marking |
| **EZCAD3 / DLC** | Variante extendida del conjunto anterior | USB | Parcial en la misma wiki | Baja-media, nuevo | **A** (LightBurn lo marca "Labs") | Ninguno | Z, rotativo |
| **BSL** | No encontré documentación | USB | Ninguna | Baja | **A** | Ninguno | Requiere driver del fabricante |
| **K40 con M2 Nano** (LightBurn **no** lo soporta) | LHYMICRO-GL (comandos por letra) en paquetes de 30/34 bytes con CRC sobre CH341 en modo EPP | USB directo (VID 0x1a86, PID 0x5512) | Comunidad: [EduTech Wiki](https://EduTechWiki.unige.ch/en/Lhystudios_M2_Nano) (sin licencia declarada). Implementaciones: MeerK40t (MIT); K40 Whisperer (GPL según su autor, **no verificado, no leer**) | **Altísima como base instalada**, baja como compra nueva | **M**: formato simple y bien documentado; sin potencia variable fina ni homing preciso | MeerK40t tiene emulación (MIT) | Raster por barrido; potencia solo con PWM de panel |
| **Glowforge** | Nube propietaria | Internet | Ninguna | Alta en EE. UU. | No factible (sin acceso local) | No | - |

## 3. Cuota de mercado: lo poco que se sabe

- La única cifra publicada que encontré es de xTool (autoinformada, §2). Ninguna otra marca publica números.
- Mi estimación para ordenar prioridades, en usuarios de un software tipo LightBurn (no en unidades vendidas), **sin fuente**:
  - Familia GRBL y derivadas (diodo, algunas CNC pequeñas, K40 con placa cambiada): 55 a 70 %.
  - Ruida: 15 a 25 %.
  - Galvo (JCZ y EZCAD3): 5 a 10 %, creciendo.
  - Smoothieware y Marlin: 3 a 5 %.
  - Trocen y TopWisdom: 3 a 5 % juntos.
  - xTool con red propietaria y Glowforge: fuera de LightBurn, pero son un público grande.
- Si el dueño consigue estadísticas reales (por ejemplo, encuestas en el foro o telemetría opcional futura), este orden debe recalcularse.

## 4. Recomendación de prioridad para Tlaku

Criterio: porcentaje de usuarios cubierto dividido por esfuerzo y riesgo. Nunca se transmite a una máquina que no se pueda probar al menos contra un emulador propio.

### Nivel 1: ya cubierto con GRBL más ajustes de dialecto

Orden sugerido:

1. **GRBL 1.1** (hecho), **GRBL-M3** (existe `PowerMode::Constant`).
2. **grblHAL** y **GRBL-LPC**: mismo flujo; solo detección por banner (`$I`) y tolerancia a ajustes extra.
3. **FluidNC / grbl-ESP32 por USB**: igual; el Wi-Fi (telnet/websocket) es un transporte nuevo y pasa a nivel 2.
4. **Marcas de diodo** (Sculpfun, Atomstack, Ortur, Longer, Neje, Falcon): son perfiles prediseñados (baudios, `s_max`, origen, homing o no), no código. Un catálogo de perfiles `.json` importables vale más que cualquier driver nuevo. LightBurn lo hace con `.lbdev`/`.lbvendor` ([Dispositivos](https://docs.lightburnsoftware.com/2.1/Reference/Devices/)).
5. **xTool D1/D1 Pro/S1 como GRBL con perfil especial** (Z con `M111 S2`, rotativo con eje U): probar primero con GRBL genérico y solo escribir un dialecto si aparecen los fallos que describe LightBurn.

### Nivel 2: factible con documentación (con emulador propio)

1. **Transporte TCP y WebSocket** para GRBL/FluidNC/grblHAL: poca ingeniería, mucho valor (Wi-Fi).
2. **Dialecto G-code configurable** (equivale a "Custom GCode"): cubre Smoothieware y Marlin sin código nuevo, y los firmwares raros. Smoothieware y Marlin requieren además streaming por `ok` en vez de conteo de caracteres, y otro comando de estado.
3. **Ruida**: gran base de usuarios y protocolo documentado por la comunidad ([ruida.md](ruida.md)). Su riesgo es de validación, no de documentación: hace falta un emulador UDP propio y, antes de lanzarlo, probar en máquina real con un voluntario. Empezar por **exportar `.rd`** a archivo (sin riesgo de conexión) y después transmisión.
4. **K40 con M2 Nano**: ventaja sobre LightBurn (no lo soporta) y protocolo sencillo y documentado. Es la forma más barata de capturar usuarios que LightBurn no atiende.
5. **Galvo JCZ/BJJCZ (EZCAD2)**: hay documentación y una biblioteca MIT, pero el riesgo de seguridad es alto (láser de fibra, listas rápidas). Hacerlo solo después de tener la capa de seguridad (§5.5) y con un voluntario con hardware.

### Nivel 3: necesita hardware o ingeniería inversa propia

- **Trocen (AWC)**: sin protocolo público. Requiere una máquina y captura de tráfico Ethernet. Alternativa realista: exportar el archivo a memoria USB si se llega a documentar el formato.
- **TopWisdom**: ídem, y ni LightBurn soporta su Ethernet.
- **EZCAD3/DLC y BSL**: parcialmente documentado (EZCAD3) o nada (BSL).
- **xTool F1/P2/M1 y S1 por Wi-Fi, Glowforge**: red propietaria; ni LightBurn lo cubre. Fuera de alcance salvo que aparezca documentación oficial.

### Orden recomendado resumido

1. Catálogo de perfiles de diodo + detección por banner (nivel 1, sin driver nuevo).
2. grblHAL, GRBL-LPC, FluidNC por USB (nivel 1).
3. TCP/WebSocket para GRBL-familia (nivel 2).
4. Dialecto configurable + streaming por `ok` (Smoothieware, Marlin).
5. Exportar `.rd` y, después, Ruida por red.
6. M2 Nano.
7. Galvo EZCAD2 con capa de seguridad.
8. Trocen, TopWisdom, EZCAD3, BSL solo si aparece hardware o documentación.

## 5. Arquitectura sugerida

Lo que hay hoy (leído en el código):

- `tlaku-core::toolpath` define el IR: `Move::{Travel, Cut{power_pct}}` por `ToolpathBlock` con `speed_mm_min`, `air_assist`, `power_mode` y `z_mm`. Es neutral y alcanza para GRBL, Ruida y mucho galvo vectorial.
- `tlaku-doc::DeviceProfile` tiene `kind: DeviceKind` (solo `Grbl`), pero además mezcla campos de GRBL (`s_max`, `baud`, `rx_buffer`, `start_gcode`, `end_gcode`).
- `tlaku-pipeline::gcode` traduce `DeviceProfile` a `GcodeOptions` y llama a `tlaku_grbl::gcode::generate`. El cambio de origen y la escala del rotativo ya viven en el pipeline, no en el driver, lo cual es correcto y debe mantenerse.
- `tlaku-grbl` contiene a la vez el generador (`gcode.rs`), el trait `Transport` de bytes (`serial.rs`), el streamer con conteo de caracteres y el análisis de `?`.
- `apps/desktop/src-tauri/src/machine.rs` tiene un `Worker` en un hilo que sondea `?`, sabe iniciar el trabajo y emite eventos por `Sink`. Está acoplado a GRBL (`$I`, `?`, `Streamer`), pero los eventos (`machine://status`, `machine://connection`, `machine://log`) ya son neutrales.

### 5.1 Separar cuatro responsabilidades

```
Document + DeviceProfile
   -> pipeline -> Toolpath (IR neutral, ya existe)
   -> OutputBackend::generate(Toolpath, DeviceProfile) -> JobArtifact
   -> JobDelivery: transmitir (Link) o guardar como archivo
```

1. **Generador de salida (`OutputBackend`)**: puro, sin E/S, fácil de probar. Entrega un `JobArtifact`:
   - `Gcode(Vec<String>)` (dialecto configurable)
   - `RuidaRd(Vec<u8>)`
   - `GalvoList(Vec<GalvoCmd>)`
   - `Nano(Vec<u8>)` (LHYMICRO-GL)
   Cada backend también produce su `frame()` y su extensión de archivo (`.gc`, `.rd`, `.ud5`, `.out`).
2. **Transporte (`Transport`)**: el trait de bytes que ya existe, movido a un crate común `tlaku-link`, con implementaciones `Serial`, `Tcp`, `WebSocket`, `Udp` y `UsbBulk{vid,pid}` (para M2 Nano y JCZ), más `Memory` para pruebas. Un transporte no sabe qué protocolo lleva.
3. **Sesión (`MachineSession`)**: lo que hoy hace `Worker`, pero detrás de un trait con estas operaciones: `identify`, `poll_status`, `jog`, `home`, `frame`, `start(JobArtifact)`, `pause`, `resume`, `cancel`, `realtime`, `capabilities`. Cada familia implementa una sesión (`GrblSession`, `SmoothieSession`, `RuidaSession`...). Los eventos que salen hacia la UI no cambian.
4. **Entrega del trabajo (`JobDelivery`)**: no todas las máquinas funcionan igual y esto debe ser explícito en `capabilities`:
   - `Stream`: GRBL, Smoothie, Marlin, FluidNC (el PC alimenta línea por línea con control de flujo).
   - `Upload`: Ruida, Trocen, TopWisdom (se envía el archivo completo y se ordena iniciar; permite "ejecutar desde la memoria del controlador").
   - `BufferedList`: galvo y M2 Nano (el PC llena un búfer del controlador con listas de comandos).
   - `FileOnly`: salida a memoria USB sin conexión.

Opción más barata y suficiente para empezar: en vez de un trait de "driver" gigante, dos traits pequeños (`OutputBackend` y `MachineSession`) elegidos por una fábrica según `kind`. El trait único tentador (`Driver::send_toolpath`) esconde las diferencias entre `Stream` y `Upload` y termina con una API de mínimo común denominador.

### 5.2 Crates

| Crate | Contenido |
|---|---|
| `tlaku-link` (nuevo) | `Transport` y sus implementaciones; detección de puertos; reglas VID:PID. |
| `tlaku-machine` (nuevo) | Traits `OutputBackend`, `MachineSession`, `Capabilities`, registro de familias, detección. |
| `tlaku-grbl` | Se queda como driver de la familia GRBL (GRBL, M3, LPC, grblHAL, FluidNC). El streamer y el parser de `?` se mantienen. |
| `tlaku-gcode` (nuevo, opcional) | Generador G-code parametrizado por `GcodeDialect`; `tlaku-grbl::gcode` pasa a usarlo. Smoothie y Marlin viven aquí. |
| `tlaku-ruida`, `tlaku-jcz`, `tlaku-nano` (futuros) | Un crate por protocolo binario, cada uno con su emulador para pruebas. |

### 5.3 `DeviceProfile`: `kind` más capacidades

Cambios compatibles hacia atrás (todo con `#[serde(default)]`, así los perfiles existentes siguen cargando):

```text
kind:        Grbl | GrblM3 | GrblHal | FluidNc | GrblLpc | Smoothie | Marlin
           | CustomGcode | Ruida | Trocen | TopWisdom | Jcz | Ezcad3 | Nano | NoMachine
connection:  Serial{port, baud} | Tcp{host, port} | WebSocket{url}
           | Udp{host, port} | UsbBulk{vid, pid} | FileOnly
dialect:     GcodeDialect   // solo kinds de G-code
capabilities: Capabilities  // valores por defecto según kind, sobreescribibles
```

`Capabilities` (cada campo con valor por defecto por `kind` y editable en la interfaz):

| Campo | Valores | Ejemplo |
|---|---|---|
| `power.s_max` | número | GRBL 1000; Smoothie 1.0; Marlin 255 |
| `power.s_float` | bool | Smoothie sí |
| `power.modes` | `{Dynamic (M4), Constant (M3), Inline}` | GRBL 1.1f: Dynamic+Constant; Marlin: Inline |
| `air` | `None`, `M7`, `M8`, `M106` | |
| `z` | `None`, `G0Z`, `Macro(String)` | xTool: macro |
| `rotary` | `None`, `ReplaceY`, `AxisA`, `AxisB`, `Internal` | Ruida: `Internal` |
| `frame` | `None`, `Gcode`, `Hardware`, `LaserLowPower` | |
| `home` | bool y comando (`$H`, `G28`) | |
| `status` | `GrblQuery`, `M114`, `Udp`, `None` | |
| `delivery` | `Stream`, `Upload`, `BufferedList`, `FileOnly` | |
| `device_storage` | bool (archivos en la memoria del controlador) | Ruida sí |
| `camera`, `autofocus` | bool | Fase posterior |

`GcodeDialect` (datos, no código): plantilla de láser encendido y apagado, M3/M4, escala de S, flotante o entero, plantilla de comentario, fin de línea, comando de pausa y de dwell, comando de aire, comando de estado, espera de reinicio tras abrir el puerto (Marlin tarda), tipo de control de flujo (`CharCount` o `OkPerLine`), modo relativo o absoluto. Los dialectos de GRBL, Smoothie y Marlin son constantes incorporadas; "Custom" lo edita el usuario (equivale a Custom GCode de LightBurn, fila 8 de [la spec](../spec/03-maquinas-avanzadas.md)).

Los campos actuales (`s_max`, `power_mode`, `z_axis`, `rotary`) se mantienen como alias de lectura para migrar: al cargar un perfil viejo se construye `capabilities` desde ellos. `ts-rs` regenera `DeviceKind.ts` y `DeviceProfile.ts` en `apps/desktop/src/bindings`.

### 5.4 Interfaz: asistente "Agregar máquina"

Hoy la pestaña Máquina asume GRBL (`ConnectionSection`, `DeviceFields`, `grblCodes.ts`, consola con `$`). Propuesta:

1. **Detectar** (como "Find My Laser" de LightBurn, [doc](https://docs.lightburnsoftware.com/2.1/GetStarted/FindMyLaser/)): listar puertos con VID:PID. Para cada candidato, abrir con baudios del catálogo y esperar el banner sin enviar nada; si calla, enviar solo `?`/`$I` (GRBL) o `M115` (Marlin). Banners a reconocer: `Grbl 1.1x [...]`, `[VER:...]` y `[OPT:...]`, `GrblHAL`, `FluidNC`, `Smoothie`, `FIRMWARE_NAME:Marlin`. Hay que avisar de que abrir el puerto puede reiniciar la placa y que Marlin tarda en volver (LightBurn lo declara incompatible con su detección por eso: [FindMyLaser](https://docs.lightburnsoftware.com/2.1/GetStarted/FindMyLaser/)). Los USB directos (JCZ 9588:9899, M2 Nano 1a86:5512) se reconocen por VID:PID, no por banner.
2. **Elegir modelo** si hay coincidencia: catálogo de perfiles (marca, modelo, `kind`, cama, origen, baudios, `s_max`, homing, notas). Importable y exportable como archivo, estilo `.lbdev`.
3. **Manual**: elegir familia, luego conexión (solo las que la familia admite), IP para red. Las máquinas de red **no se detectan automáticamente** (tampoco LightBurn): el usuario escribe la IP.
4. **Verificar**: lectura de ajustes (`$$`, `$30`) y propuesta de corregir `s_max` o activar modo láser (`$32=1`), como hace el preflight actual.
5. **Capacidades visibles**: las secciones de la interfaz (Z, rotativo, aire, frame, archivos en el controlador, consola) se muestran según `capabilities`. La consola y `grblCodes.ts` pasan a depender del dialecto.
6. Para máquinas `Upload` (Ruida y similares) aparecen botones nuevos: "Enviar a la memoria", "Lista de archivos" y "Ejecutar archivo". Para `FileOnly`, "Guardar en memoria USB".

### 5.5 Seguridad y pruebas sin hardware

- Detección: nunca enviar comandos de movimiento ni escribir parámetros durante la autodetección. Solo lectura. No escanear redes automáticamente.
- Cada protocolo binario nuevo debe llegar con su **emulador en proceso** (el `memory_pair` actual ya hace esto para GRBL) y pruebas con vectores capturados de la documentación pública. Sin emulador no se publica el driver.
- Galvo y Ruida: botón de parada y "láser apagado en todo error" como prueba obligatoria; activación por interruptor "experimental" en ajustes mientras no haya un voluntario con hardware que lo valide.
- Licencias: Tlaku es MIT. MeerK40t y galvoplotter son MIT, pero la regla de sala limpia manda partir de documentación; consultar su código solo con aprobación del dueño. Marlin, Smoothieware y Snapmaker2-Controller son GPL-3.0; GRBL, grblHAL, FluidNC y grbl-LPC figuran como "Other" en GitHub (tratar como GPL); LaserGRBL y K40 Whisperer también (K40 Whisperer: licencia no verificada). **No leer su código**, solo sus wikis y documentación.

### 5.6 Fases de implementación sugeridas

1. Refactor sin cambio de comportamiento: `DeviceKind` ampliado, `connection` y `capabilities` con migración, `MachineSession` con una única implementación (`GrblSession`). Catálogo de perfiles de diodo.
2. `tlaku-link`: TCP y WebSocket. FluidNC y grblHAL por Wi-Fi.
3. `GcodeDialect` y streaming por `ok`: Smoothieware, Marlin, Custom.
4. Exportar `.rd`, luego `RuidaSession` con emulador.
5. `tlaku-nano` (M2 Nano).
6. `tlaku-jcz` con la capa de seguridad.

## 6. Resumen de licencias consultadas (solo metadatos)

| Proyecto | Licencia | Fuente |
|---|---|---|
| Marlin | GPL-3.0 | [GitHub](https://github.com/MarlinFirmware/Marlin) |
| Smoothieware | GPL-3.0 | [GitHub](https://github.com/Smoothieware/Smoothieware) |
| Snapmaker2-Controller | GPL-3.0 | [GitHub](https://github.com/Skreelink/Snapmaker2-Controller) |
| GRBL, grblHAL, FluidNC, grbl-LPC | "Other" en GitHub (tratar como GPL) | [grblHAL](https://github.com/grblHAL), [grbl-LPC](https://github.com/cprezzi/grbl-LPC) |
| LaserGRBL | sin licencia detectable en GitHub (tratar como GPL) | [LaserGRBL](https://lasergrbl.com/) |
| MeerK40t | MIT | [GitHub](https://github.com/meerk40t/meerk40t) |
| galvoplotter | MIT | [GitHub](https://github.com/meerk40t/galvoplotter) |
| jnweiger/ruida-laser, kallaballa/rdint | GPL-2.0 | GitHub |
| smiilaserpants/RuiDa-Machine-Code | GPL-3.0 | GitHub |
| kkaempf/ruida | "Other" | GitHub |
| Notas de protocolo de EduTech Wiki (Ruida, LMCV4, M2 Nano) | No declaran licencia: usar como referencia de hechos, no copiar texto | [Ruida](https://edutechwiki.unige.ch/en/Ruida) |

## 7. Pendientes y dudas abiertas

- Verificar con fabricantes los controladores de Thunder, OMTech, Monport, Cloudray y Two Trees (§1.4 y §1.5).
- Confirmar si Marlin tiene simulador nativo utilizable en pruebas.
- No se encontró protocolo público de Trocen ni TopWisdom ni BSL. No se intentó ingeniería inversa.
- Las cifras de popularidad (§3) son estimaciones sin fuente.
- La wiki de FluidNC no estuvo accesible durante la sesión (error de conexión); los datos de FluidNC salen de espejos y foros.
