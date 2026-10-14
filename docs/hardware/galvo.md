# Galvo (tarjetas JCZ/BJJCZ): investigación de viabilidad

Fecha: 2026-10-13. Estado: investigación, no hay código. Método: sala limpia. Solo se leyeron
descripciones en prosa o tablas (wikis, artículos, manuales públicos, foros). Las fuentes que
son solo código se anotan como "existe, no leída". Cada dato lleva su URL.

Niveles de confianza: **A** = descrito por una fuente primaria (manual del fabricante) o por
dos fuentes independientes; **M** = una sola fuente secundaria legible; **B** = foro, fragmento
de buscador o inferencia mía; **?** = no se encontró.

## 0. Resumen ejecutivo

- Técnicamente es factible: LightBurn, MeerK40t y otros ya controlan las tarjetas LMC por USB.
  Pero la **especificación en prosa disponible es incompleta**: se conocen el transporte, el
  formato de 12 bytes y una lista de nombres y códigos de comando, y **no** se conocen las
  unidades y codificaciones de los parámetros (velocidad, frecuencia, retardos), los bits de
  estado, la secuencia de arranque ni el formato de la tabla de corrección.
- Esos huecos solo se cierran de dos formas legítimas para nosotros: (a) capturar y describir
  el tráfico de **nuestra propia tarjeta** con EZCAD2 (otra persona implementa desde esa
  descripción), o (b) probar con hardware real. Leer el código de balor/MeerK40t queda
  descartado por la regla de sala limpia.
- Se puede construir y verificar sin hardware la capa pura (codec, compilador de listas,
  geometría, límites de seguridad, emulador propio). **No se puede verificar sin hardware**
  que la tarjeta haga lo que esperamos, y un error aquí puede dañar una fuente láser de fibra
  o dejar un haz invisible activo.
- Recomendación: **esperar a la fase 5 (M5)**; mientras tanto, preparar solo el modelo de
  datos (IR, perfil de dispositivo) y el paquete de captura. No publicar nada con salida
  real a láser sin la prueba mínima de la sección 9.

## 1. Hardware

### 1.1 Tarjetas JCZ/BJJCZ y sus identificadores USB

| Tarjeta / software | VID:PID | Fuente | Conf. |
|---|---|---|---|
| LMCV4-FIBER-M (EZCAD2), enumerada lista para usar | 0x9588:0x9899 ("BJJCZ USBLMCV4") | https://edutechwiki.unige.ch/en/LMCV4-FIBER-M ; https://www.bryce.pw/engraver.html ; lsusb en https://forum.lightburnsoftware.com/t/device-not-found-bjjcz-device-laser-mark-control-board-v1/137554 | A |
| INF del driver de EZCAD2 | 9588:9990 y 9588:9999 | mismo hilo del foro | M |
| INF del driver de LightBurn | 9588:9980 y 9588:9899 | mismo hilo del foro | M |
| Clon que identifica 9588:9980 | 9588:9980 | https://forum.lightburnsoftware.com/t/device-not-found-bjjcz-device-laser-mark-control-board-v1/137554 | M |
| DLC / DLC2 (EZCAD3) | **no encontrado** | LightBurn solo dice que EZCAD3 maneja DLC2 y EZCAD2 maneja LMCV4: https://forum.lightburnsoftware.com/t/ezcad3-support/190047 | ? |

Notas:
- LightBurn identifica el dispositivo por su VID:PID, y cambiar el driver no hace que una
  tarjeta con otro ID funcione ("different VID/PID pairs may mean a different protocol", en el
  hilo anterior). Conf. M.
- Los nombres en el Administrador de dispositivos de Windows: los de EZCAD2 contienen "LMC"
  (por ejemplo USBLMCV2) y los de EZCAD3 una variante de "DLC":
  https://docs.lightburnsoftware.com/2.1/Guides/GalvoDriverInstallation/ (Conf. M). En Windows
  LightBurn usa un driver compatible con libusb/WinUSB (Zadig como alternativa); en macOS no
  hace falta driver; en Linux se trata como dispositivo libusb y basta una regla udev
  (`SUBSYSTEM=="usb", MODE="0666"`): https://docs.lightburnsoftware.com/galvo/ (Conf. M).
- **Transporte propio sobre USB bulk**: la tarjeta presenta tres endpoints: uno de órdenes por
  USB BULK, uno de estado y uno de "dongle" (EZCAD2 lo consulta; la máquina no exige esa
  interacción). Endpoints citados: 0x02 (host a máquina, órdenes), 0x88 (máquina a host,
  estado), y 0x01/0x81 para el dongle, que se puede ignorar. Fuentes:
  https://www.bryce.pw/engraver.html y https://edutechwiki.unige.ch/en/LMCV4-FIBER-M (Conf. A:
  dos fuentes; la segunda cita a la primera, así que no son del todo independientes).
  Las órdenes viajan "en claro, sin ofuscación ni bits de control adicionales" (edutechwiki).
- Interior de la LMCV4-FIBER-M: FPGA Altera y microcontrolador USB Cypress tipo 8051, memoria
  RAM y una EEPROM junto a un cabezal sin poblar que carga la configuración del FPGA
  (https://www.bryce.pw/engraver.html y https://hackaday.com/tag/balor/). Conf. M.
- Revisión citada: el manual JCZ habla de REV 20150723
  (https://cdn.shopify.com/s/files/1/2222/7859/files/LMCV4-FIBER-M_1.pdf); la placa de Bryce es
  rev. 20171014. Hay variantes y clones; no se puede asumir un solo protocolo para todos.
- El software EZCAD2 usa la propia tarjeta como dongle: sin tarjeta no guarda archivos
  (https://charliex2.wordpress.com/2020/01/31/fibre-laser-arrives-let-the-games-begin/, Conf.
  B). Para nosotros es irrelevante: el protocolo no exige el dongle (Bryce).
- EZCAD3/DLC2: protocolo distinto al de LMC; LightBurn dice que "no sabe lo suficiente" y que
  está en reversa; no hay especificación pública (ver spec/03-maquinas-avanzadas.md, filas de
  EZCAD3 y BSL). Placa DLC2-M4-2D/3D (2 o 3 ejes de galvo, Beijing JCZ):
  https://findlight.net/laser-material-processing/machine-parts/controllers/2d-3d-laser-and-galvo-controller-dlc-series-ezcad3
  (la página devolvió 404 al leerla; el dato viene del resumen del buscador, Conf. B).
  **Conclusión: EZCAD3/DLC queda fuera de alcance** hasta que haya documentación.
- Otras variantes con protocolo propio: BSL/BSLFiber (SeaCAD), fuera de alcance
  (https://forum.lightburnsoftware.com/t/bsl-boards-seacad-support-in-lightburn/77669).

### 1.2 Conectores de la LMCV4-FIBER-M (manual oficial de JCZ)

Fuente: https://cdn.shopify.com/s/files/1/2222/7859/files/LMCV4-FIBER-M_1.pdf (Conf. A para
esta tarjeta y esta revisión).

| Conector | Contenido |
|---|---|
| CON1, DB15 | Cabezal: CLK, SYNC, canales X, Y, Z, todo diferencial (pares +/-), señales de estado reservadas, GND. Para un cabezal 2D bastan CLK, SYNC, X, Y y GND. Se ofrece un convertidor D/A para cabezales analógicos. "Los protocolos de cada fabricante no siempre son iguales: confirma cuál usa el tuyo." |
| CON2, DB25 | Láser: P0-P7 (potencia, TTL), PLATCH (latch de potencia), MO (oscilador maestro), AP (amplificador), PRR (frecuencia de repetición), RedPt (puntero rojo), EMSTOP (parada de emergencia), LASERST0-3 (alarmas del módulo láser), VCC 5 V, GND. |
| CON4, DB15 | Alimentación 5 V (se recomiendan 5 V/3 A), eje de extensión X (XDIR, XPUL, XORG0), entradas generales In0-In3, XORG0 (IN14), IN15; salidas OUT4 y OUT5 (TTL, no cortocircuitar); ReMark (repetir el contenido de la caché). |

Otros datos del manual: hasta 8 tarjetas por PC; marcado al vuelo con encoder; USB 2.0;
temperatura de operación 25 C +/- 10 C; **enciende primero la tarjeta y la fuente del cabezal
antes que el láser, o hay riesgo de desviación incontrolada del haz; se recomienda usar un
obturador (shutter)**.

### 1.3 Cabezal galvo XY2-100 (16 bits)

- XY2-100 es un bus serie síncrono: reloj (CLK), sincronía (SYNC) y un canal por eje. Cada
  trama de 20 bits lleva 3 bits de control, 16 bits de datos y 1 de paridad; el reloj típico
  es 2 MHz con 100 kHz de tramas (20 bits x 100 kHz); Raylase publica hasta 10 MHz.
  Fuentes: https://techoverflow.net/2021/11/01/what-is-the-typical-maximum-clock-frequency-for-xy2-100
  (reloj y trama, Conf. M) y fragmento de https://forums.parallax.com/discussion/comment/621281
  (3+16+1, Conf. B; esa página no se pudo releer). No se encontró la especificación oficial;
  ScannerMAX remite a un manual propio de 100 páginas (https://forums.pangolin.com/posts/41896).
- Para nuestro caso **la tarjeta genera el XY2-100**: el host solo envía posiciones de 16
  bits. No necesitamos implementar el bus; sí conviene saber que el rango es de 65536
  posiciones por eje con el centro en 0x8000 (Bryce, https://www.bryce.pw/engraver.html).
- Los cabezales XY2-100 pueden tener ejes invertidos o intercambiados; LightBurn expone
  "Galvo N es eje X" y "Negar" por eje
  (https://docs.lightburnsoftware.com/2.1/Reference/DeviceSettings/GalvoBasicSettings/).

### 1.4 Fuentes de fibra: Q-switched frente a MOPA

- **Q-switched**: ancho de pulso esencialmente fijo (del orden de 100 ns); el operador cambia
  frecuencia y potencia. **MOPA**: oscilador maestro con amplificador; el ancho de pulso y la
  forma se ajustan en el oscilador (rangos de catálogo 1-500 ns, frecuencia hasta
  1-4000 kHz). Las cifras de catálogo varían y son comerciales. "MOPA" describe una
  arquitectura, no un mecanismo único. Fuentes: https://jpt-laser.com/mopa-vs-q-switched-fiber-laser/ ,
  https://craftetch.com/mopa-vs-q-switched-fiber-laser-machine ,
  https://eprints.soton.ac.uk/77834/1/1757.pdf (resúmenes del buscador, Conf. B). 
- JPT MOPA, según lo que citan usuarios del manual: frecuencia 1-4000 kHz, a 1 kHz pasa de
  pulsado a CW, unas 15-17 formas de onda (Q-pulse) con frecuencia de corte propia, y valores
  intermedios se redondean al más cercano de la lista (lista de 2 a 250 ns en pasos como 2, 4,
  6, 9, 13, 20, 30). Fuentes: https://forum.lightburnsoftware.com/t/mopa-laser-waveforms/168542 ,
  https://forum.lightburnsoftware.com/t/question-on-frequency-50w-mopa-jpt/99015 (Conf. B).
  Los manuales de JPT (alojados por Thunder Laser,
  https://support.thunderlaserusa.com/portal/en/kb/articles/aurora-8-pro-fiber-mopa-jpt-source-user-manual)
  respondieron 403 y **no se leyeron**.
- Raycus (los manuales en ManualsLib respondieron 403; datos de resúmenes del buscador):
  - Potencia: valor de 8 bits en PIN1-PIN8, 0-255 equivale a 0-100 % de potencia, no
    necesariamente lineal (RFL-P60M y RFL-P200:
    https://www.manualslib.com/manual/2268735/Raycus-Rfl-P60m.html , https://www.manualslib.com/manual/1689495/Raycus-Rfl-P200.html). Conf. B.
  - RFL-P20QB (Q-switched): la señal MO debe activarse **al menos 5 ms antes** que AP, "o el
    láser puede dañarse"; frecuencia de repetición mayor que 20 kHz; el tiempo máximo sin pulso
    es de 100 microsegundos; nivel alto >3 V = emisión ON, <1 V u abierto = OFF
    (https://www.manualslib.com/manual/1725704/Raycus-Rfl-P20qb.html). Conf. B, y es de otro
    modelo que el de Bryce (RFL-P20QS).
  - RFL-P20QS: el sistema de control debe estar encendido antes que el láser, y hay que
    esperar al menos 1 minuto tras encender (https://www.manualslib.com/manual/2457910/Raycus-Rfl-P20qs.html). Conf. B.
  - DB25 es la interfaz principal; DB9 solo RS-232 de depuración (manuales P60M/P200, Conf. B).
- La **tarjeta** controla estas señales de la fuente (manual JCZ y LightBurn):
  - Potencia: bus P0-P7 con latch PLATCH.
  - Frecuencia: salida PRR; LightBurn la llama "Frequency" en kHz.
  - MO y AP: habilitación del oscilador y del amplificador; LightBurn tiene "Open MO Delay (ms)".
  - Ancho de pulso Q (solo MOPA y UV): en ns, "Q-Pulse Width".
  - En CO2: PWM, "Tickle" (potencia baja constante) y "First Pulse Killer".
  - Retardos de marcado y salto (sección 2.3) y puntero rojo (RedPt).
  Fuentes: https://docs.lightburnsoftware.com/galvo/ConfigureGalvoSettings.html y
  https://docs.lightburnsoftware.com/2.1/Reference/CutSettingsEditor/GalvoSpecificCutSettings
  (Conf. M). LightBurn advierte que estos valores deben importarse de EZCAD y no adivinarse.
- Cómo se envían ancho de pulso y frecuencia por el cable USB: ver sección 2; solo se
  conocen los nombres de los comandos (listMarkFreq, listMarkPulseWidth,
  listIPGYLPMPulseWidth, listIPGOpenMO, listJptSetParam...).

## 2. Protocolo USB de las tarjetas LMC (según descripciones públicas)

Fuentes en prosa: https://edutechwiki.unige.ch/en/LMCV4-FIBER-M (tablas de comandos) y
https://www.bryce.pw/engraver.html (artículo). Sobre el origen: edutechwiki indica que la
reversa la hizo Bryce Schroeder (herramienta "Balor") observando el USB generado por EZCAD, y
Jason Dorie para LightBurn; varias entradas de las tablas ("stack value", códigos IOCTL de las
DLL) parecen venir además de analizar las DLL de EZCAD, no solo del USB. Conviene tenerlo
presente por el riesgo de licencia de EZCAD (ver sección 6, último punto).

### 2.1 Comandos inmediatos frente a comandos de lista

- Código de comando de 16 bits, little-endian. Byte alto 0x00 = **comando inmediato**
  (ejecuta al llegar); 0x80 = **comando de lista** (se almacena en la lista de marcado y se
  ejecuta cuando la lista corre). Conf. M (edutechwiki).
- Todos los comandos miden **12 bytes**: seis enteros int16 little-endian: código, parm1 a
  parm5; los no usados van a cero. Conf. A (edutechwiki y Bryce coinciden).
- Los inmediatos suelen ir seguidos de un `GetState` que lee 6 bytes de respuesta (estado).
  Conf. M.
- **Las listas se envían "en tandas de 256" comandos**, rellenando con comandos de no-operación
  si hacen falta; 256 x 12 = 3072 bytes por tanda (la cifra 3072 es aritmética mía, no de la
  fuente). Un protocolo simple de estado decide si la tarjeta puede recibir más. EZCAD2 consulta
  el estado "cada pocos milisegundos", necesario sobre todo para vigilar enclavamientos y
  pedal (Bryce). Los bits de estado **no están descritos en prosa**.
- Tamaño de paquete de USB (bulk de 512 bytes en USB 2.0 alta velocidad) es una propiedad
  del estándar, no una cita de las fuentes; no sabemos si la tanda de 3072 se parte en varias
  transferencias.

### 2.2 Tabla de comandos conocidos

Tablas de https://edutechwiki.unige.ch/en/LMCV4-FIBER-M (la propia página tiene códigos
duplicados que no explica: por ejemplo 0x8004 para tres nombres, 0x800b y 0x8006; hay que
tratar el mapeo como ambiguo hasta validarlo con captura). Conf. M.

**Comandos de lista (0x80xx)**

| Nombre | Código | Para qué (por el nombre; parámetros ?) |
|---|---|---|
| listJumpTo | 0x8001 / 0x800d | Salto con láser apagado |
| listMarkTo | 0x8005 | Marca hasta el punto |
| listLaserOnPoint | 0x8003 | Disparo en un punto |
| listJumpSpeed | 0x8006 | Velocidad de salto |
| listMarkSpeed | 0x800c | Velocidad de marcado |
| listMarkFreq | 0x801b, 0x800a, 0x800b, 0x8013 (según tipo de láser / ajustes) | Frecuencia |
| listMarkPowerRatio | 0x8012 (0x800b si CO2) | Potencia relativa |
| listMarkPulseWidth | 0x800b | Ancho de pulso |
| listIPGYLPMPulseWidth | 0x8026 | Ancho de pulso IPG YLPM |
| listLaserOnDelay / listLaserOffDelay | 0x8007 / 0x8008 | Retardos de encendido / apagado |
| listPolygonDelay | 0x800f | Retardo en esquinas |
| listMarkEndDelay | 0x8004 | Retardo final |
| listDelayTime / listDelayTimeUs | 0x8004 | Espera genérica |
| listEndofList | 0x8002 | Fin de lista |
| listFlyEnable / listFlyDelay / lsFlyWaitInput / listFlyEncoderCount | 0x801a / 0x801d / 0x801f / 0x8028 | Marcado al vuelo (cinta/encoder) |
| listWritrPort / listWaitForInput | 0x8011 / 0x8022 | Salidas / esperar entrada |
| listDirectLaserSwitch | 0x801c | Conmutar láser |
| SetCo2FPK | 0x801e | First pulse killer CO2 |
| listIPGOpenMO | 0x8021 | Abrir MO (fibra) |
| listChangeMarkCount | 0x8023 | Contador de marcas |
| listSetDaZWord | 0x8029 | Eje Z / D/A |
| listJptSetParam | 0x8050 | Parámetros de fuente JPT |
| listEnableWeldPowerWave / listSetWeldPowerWave | 0x8025 / 0x8024 | Onda de potencia (soldadura) |
| ReadyMark | 0x8051 | Preparar el marcado |
| ScanBmpPtBuf | 0x8006 (varias veces) | Mapa de bits |

**Comandos inmediatos (0x00xx), los más relevantes**

| Nombre | Código | Parámetros |
|---|---|---|
| DisableLaser / EnableLaser | 0x0002 / 0x0004 | ninguno |
| ExecuteList | 0x0005 | ninguno |
| GetVersion / GetSerialNo | 0x0007 / 0x0009 | parm1=1 en GetVersion |
| GetListStatus | 0x000a | ninguno |
| GetPositionXY / GotoXY | 0x000c / 0x000d | GotoXY: parm1=x, parm2=y |
| LaserSignalOn / LaserSignalOff | 0x000f / 0x000e | ninguno |
| WriteCorTable | 0x0015 | booleano, y suele seguir una tabla xy de 8 x 63 x 63 bytes |
| ResetList / RestartList | 0x0012 / 0x0013 | ninguno |
| SetEndOfList | 0x0019 | ninguno |
| SetFirstPulseKiller / SetFpkParam / SetFpkParam2 | 0x001a / 0x0062 / 0x002e | ? |
| SetLaserMode / SetControlMode / SetDelayMode / SetMaxPolyDelay / SetTiming / SetStandby | 0x001b / 0x0016 / 0x0017 / 0x0018 / 0x001c / 0x001d | ? |
| SetPwmHalfPeriod / SetPwmPulseWidth | 0x001e / 0x0006 | ? |
| StopExecute / StopList | 0x001f / 0x0020 | ninguno |
| ReadPort / WritePort | 0x0025 / 0x0021 | WritePort: puerto y valor |
| WriteAnalogPort1 / 2 / X | 0x0022 / 0x0023 / 0x0024 | ? |
| IPG_OpemMO / IPG_GETStMO_AP | 0x0033 / 0x0034 | ? |
| SetSPISimmerCurrent | 0x003c | ? |
| GetMarkCount / GetFlyWaitCount | 0x002d / 0x002b | ? |
| Ejes: MoveAxisTo, AxisGoOrigin, GetAxisPos, Set*Param; Z: ENABLEZ, SETZDATA | 0x0026-0x002a, 0x0039-0x003b | ? |

Lo que **no** dice ninguna fuente en prosa: unidades y escalas de velocidad (galvo unidades
por ms frente a mm/s), cómo se codifica la frecuencia (periodo en ciclos de reloj, kHz,
divisor), el ancho de pulso en ns, los retardos en microsegundos o décimas, el contenido de
los 6 bytes de GetState y de GetListStatus, cuántas listas hay y cómo se alterna entre ellas,
qué hace exactamente ExecuteList cuando la lista aún se está llenando, y la secuencia exacta
de inicialización. Ver sección 7 para cómo cerrarlos.

### 2.3 Saltos, marcas, velocidades y retardos

- **Salto (jump)**: movimiento del espejo con el láser apagado (listJumpTo) a una velocidad de
  salto. **Marca (mark)**: movimiento con el láser encendido (listMarkTo) a la velocidad de
  marcado, con potencia y frecuencia previamente fijadas por comandos de lista. Los nombres
  vienen de las tablas; la semántica de LightBurn es la misma
  (https://docs.lightburnsoftware.com/2.1/Reference/DeviceSettings/GalvoBasicSettings/).
- Coordenadas internas: enteros de 16 bits con el centro en 0x8000 (Bryce). Para un campo de
  150 mm, una unidad vale unos 2,3 micras (150 mm / 65536 = 2,29 micras, de acuerdo con
  Bryce, que añade que varía un 10 % entre X e Y y a lo largo del campo).
- Parámetros de temporización que expone LightBurn (todos configurables por dispositivo y por
  subcapa; fuentes: GalvoBasicSettings y GalvoSpecificCutSettings arriba):

  | Parámetro | Unidad | Significado |
  |---|---|---|
  | Jump Speed | no especificada | velocidad de salto; más alta = más vibración |
  | Min / Max Jump Delay | no especificada | espera para que el espejo se asiente tras un salto |
  | Jump Distance Limit | no especificada | saltos mayores usan el máximo; menores, interpolan |
  | Laser On TC | microsegundos | espera antes de disparar tras empezar a moverse el espejo (admite valores negativos según el foro: ver abajo) |
  | Laser Off TC | microsegundos | retardo antes de apagar al final de una marca |
  | End TC | microsegundos | pausa tras terminar un trazo |
  | Polygon TC | microsegundos | pausa en las esquinas dentro de un trazo |
  | Pause Before / After | milisegundos | pausa antes/después de una subcapa |
  | Frequency | kHz | repetición de pulso |
  | Q-Pulse Width | ns | solo MOPA y UV |
  | Open MO Delay | ms | solo MOPA |

  Los foros discrepan sobre si Laser On/Off TC aceptan valores negativos
  (https://forum.lightburnsoftware.com/t/laser-not-turning-off-in-time-while-traveling/167562 ,
  https://charliex2.wordpress.com/2020/01/31/fibre-laser-arrives-let-the-games-begin/ cita
  "Start TC -200"). Conf. B. Equivalencia con nombres de comandos de lista (inferencia mía por
  el nombre): LaserOnDelay=0x8007, LaserOffDelay=0x8008, PolygonDelay=0x800f,
  MarkEndDelay=0x8004, JumpSpeed=0x8006, MarkSpeed=0x800c.
- No se encontró un parámetro de "retardo de salto" como comando propio; en LightBurn el
  retardo se calcula según la distancia del salto, así que probablemente lo inserta el
  software como una espera (listDelayTime) tras el salto. **Inferencia (Conf. B).**

### 2.4 Wobble

Wobble en LightBurn: "traza pequeños círculos a lo largo de una línea para engrosarla y
aumentar el tiempo de permanencia"; parámetros Step (separación entre círculos, unidad no
especificada) y Size (diámetro en mm) (GalvoSpecificCutSettings). No hay comando de wobble en
las tablas de comandos de la tarjeta. **Conclusión (inferencia, Conf. B): es una
transformación de la trayectoria en el host** (muchos segmentos cortos de marca). Hay que
confirmarlo con una captura, pero encaja con la IR actual.

### 2.5 Frame y luz (puntero rojo), entradas y salidas, estado

- **Puntero rojo**: señal RedPt de CON2 (pin 22, manual JCZ). En LightBurn: Frame Speed, Red
  Dot Offset/Scale/Rest Position, "Red Dot always on", "Enable blanking" (no soportado en
  todos los dispositivos) y "Require Framing Before Start" (GalvoBasicSettings y
  ConfigureGalvoSettings). El encuadre mueve el espejo con el láser deshabilitado y el
  puntero rojo encendido; se hace con GotoXY y/o listas de salto (inferencia, Conf. B).
  Atención: el puntero rojo es una señal distinta de la del láser; en el blog de charliex2
  hay una queja de que en EZCAD F1 muestra el puntero y F2 **dispara el láser**, "no seguro
  para los ojos".
- **Entradas y salidas**: In0-In3, XORG0 (IN14), IN15, OUT4-OUT5, ReMark (manual JCZ).
  LightBurn: 16 indicadores "Red 0-15" de estado, Ready Light, Red Dot, Start Marking (pedal)
  con la advertencia de ponerlo en "Disabled" si no hay pedal. Comandos: ReadPort 0x0025,
  WritePort 0x0021, listWritrPort 0x8011, listWaitForInput 0x8022.
- **Alarmas del láser**: LASERST0-3 y EMSTOP en CON2. Cómo se leen por USB: ?.
- **Estado**: GetState (6 bytes), GetListStatus, GetPositionXY, GetMarkCount. Formato: ?.
- Tiempo de parada: detener un trabajo en marcha tarda unos dos segundos, y pausar y
  reanudar solo funciona en parte (observación sobre el plugin balor de MeerK40t:
  https://github.com/tatarize/balor-meerk40t, solo prosa del README).

## 3. Calibración galvo

### 3.1 Tamaño de campo y escala a 16 bits

- El campo depende de la lente F-theta (valores típicos 110, 150, 200, 300 mm; también
  hay F420 como la de este hilo: https://forum.lightburnsoftware.com/t/cor-files-and-needing-more-resolution-for-non-linear-lens-distortion/189590 ).
  LightBurn guarda "Field Size" ancho/alto, offset y ángulo en el dispositivo y recomienda un
  perfil por lente (GalvoBasicSettings).
- La escala es campo/65536 por eje: 110 mm = 1,68 micras/unidad, 150 mm = 2,29, 200 mm = 3,05,
  300 mm = 4,58 (cálculo mío; la dependencia real tiene deriva por altura y distorsión, y
  Bryce informa de ~10 % de diferencia entre ejes). Por eso hace falta calibración.
- Hay que cuantificar a enteros de 16 bits; cada eje queda limitado a 0..65535, con el centro
  0x8000.

### 3.2 Archivo de corrección (.cor)

- Lo genera la utilidad **CorFile2** de EZCAD; "el formato .cor de EZCAD no está documentado
  públicamente y habría que hacer ingeniería inversa"; los archivos antiguos no se cargan en
  LightBurn y los de CorFile2 sí (LightBurn, 2022:
  https://forum.lightburnsoftware.com/t/correction-file-does-not-work/70681/4 , Conf. M).
  Hay quejas de .cor exportados que fallan o cambian el campo a 10x10 mm
  (https://forum.lightburnsoftware.com/t/ezcad2-cor-file-not-accepted-corfile2-file-needed/72956).
- Contenido conceptual: corrección global de 9 puntos (3x3) más una rejilla densa opcional (en
  CorFile2, "multi-points" hasta unos 65 x 65) donde cada punto guarda solo un desplazamiento
  DX, DY, interpolado entre vecinos. Descripción de un usuario, no oficial (hilo de arriba y
  https://forum.lightburnsoftware.com/t/multipoint-correction/192200). Conf. B.
- En el cable: `WriteCorTable` (0x0015) seguido de una tabla de 8 x 63 x 63 = 31 752 bytes
  (edutechwiki); la tabla se carga al arrancar la máquina (Bryce). Formato interno de la
  tabla (¿4 bytes por eje?, ¿tipo de dato?, ¿cómo se indexa?): **?**. Que la tarjeta use 63 x 63
  y CorFile2 hable de 65 x 65 indica una conversión intermedia que no conocemos.
- Parámetros que LightBurn ajusta sin .cor: escala por eje (%), Bulge (distorsión cerca del
  centro), Skew (en bordes/esquinas) y Trapezoid (conicidad de esquinas, "keystone"); los
  importa de markcfg7 de EZCAD2 (GalvoBasicSettings; ConfigureGalvoSettings).
  Bryce además tiene un archivo propio con función de base radial (cal_0002.csv) que
  convierte unidades galvo a mm. 
- **Barril y cojín (barrel/pincushion)**: una lente F-theta bien diseñada tiene poca
  distorsión, pero queda la del sistema óptico completo y la geometría del espejo (el
  "pincushion" de dos espejos). La corrección es una transformación 2D de mm a unidades galvo
  (polinomio radial, rejilla bilineal o RBF). Esa es una decisión de diseño nuestra: podemos
  **aplicar la corrección en el host** y enviar posiciones ya corregidas (dejando la tabla de
  la tarjeta como identidad o la que ya tenga cargada), lo cual evita depender del formato
  .cor. Falta comprobar que la tarjeta lo permite y cuál es la tabla "neutra". Ya está
  anotado en docs/ideas-mejoras.md como calibración con rejilla grabada y fotografiada.

## 4. Firmware

- Bryce: no extrajo ni modificó el firmware del 8051 ni la configuración del FPGA ("I haven't
  at this point bothered to try to get the firmware"). La tarjeta tiene una EEPROM junto a un
  cabezal sin poblar que carga la configuración del FPGA (https://www.bryce.pw/engraver.html).
  O sea que **en su placa el FPGA arranca solo desde su propia EEPROM** (Conf. M).
- La tarjeta de Bryce y la del README de balor-meerk40t aparecen ya como 0x9588:0x9899 en
  Linux con libusb **sin mencionar carga de firmware**
  (https://github.com/tatarize/balor-meerk40t : "no menciona firmware"). Conf. M.
- Pero hay indicios de **carga de firmware con renumeración en otras variantes**: el INF de
  EZCAD lista 9990 y 9999, y un usuario escribió que el driver detecta una tarjeta
  programable, sube firmware y reenumera con otro ID (dos "pitidos" de Windows)
  (hilo de BJJCZ Device antes citado; el propio hilo **no confirma** la transición 9990 a
  9999, solo que ambos están en el INF). Un usuario de Linux dijo que lo resolvió extrayendo
  el firmware del driver de Windows (fragmento del buscador, no verificado). Conf. B.
- También existe un cargador para otra familia (LMCV2U): https://github.com/safarir/lmcv2u-loader
  : **existe, no leída** (es código).
- Mecanismo genérico de Cypress FX2 (para entender el patrón, no para esta tarjeta): se carga
  la RAM con peticiones vendor 0xA0 y se reenumera
  (https://www.retro11.de/manp/w11/man/cat1/fx2load_wrapper.0.html ,
  https://elixir.bootlin.com/linux/v4.4.65/source/drivers/media/common/cypress_firmware.c ).
  **No hay evidencia de que la LMCV4 sea FX2**; Bryce solo dice "Cypress 8051 modernizado".
- Conclusión: para la LMCV4-FIBER-M con ID 9899, **probablemente no hay que subir firmware**
  (Conf. M). Para clones con otros IDs (9980, 9990) o para DLC, **no se sabe**. Los archivos
  de inicialización que importa LightBurn son del lado del host (no de la tarjeta): markcfg7
  (EZCAD2), BslCAD.cfg/LmcPar.cfg (BSL), markcfg0 + Motors.ini + COR (EZCAD3), según
  docs/spec/03-maquinas-avanzadas.md fila 41. El .cor se escribe a la tarjeta al arrancar.
  LightBurn dice que importar proyectos de EZCAD no está soportado, solo los archivos de lente
  (https://docs.lightburnsoftware.com/galvo/).
- Regla de seguridad: **no re-flashear nunca la EEPROM** de la tarjeta (riesgo de dejarla
  inservible; Trenz lo advierte para FX2: https://wiki.trenz-electronic.de/x/eAyi). Y el
  comando de escritura de EEPROM existe en la API de Windows (MIO_WriteEpprom, ver el IOCTL en
  edutechwiki): Tlaku no lo implementará.

## 5. Seguridad

Riesgos concretos documentados:

1. **Haz invisible clase 4 y reflexiones difusas**: 1064 nm no se ve; incluso reflexiones
   difusas pueden dañar el ojo; los lentes deben estar rotulados con longitud de onda y DO
   (https://support.thunderlaserusa.com/portal/en/kb/articles/safety-info-fiber-laser-radiation-ppe-fire-suppression ,
   https://forum.makerforums.info/t/fiber-laser-engraver-safety/84499 ,
   https://www.voortman.net/en/knowledge-base/laser-classes-for-fiber-lasers ). Para los
   cálculos oficiales remitir a ANSI Z136.1 / IEC 60825-1 (no leídos).
2. **Galvo parado con haz encendido**: hay reportes de que "pausar" detiene el espejo pero el
   haz sigue al 95 %, de que el láser queda encendido tras terminar un trabajo, y de que el
   galvo sigue sin potencia o se detiene sin apagar el láser
   (https://forum.lightburnsoftware.com/t/galvo-fiber-laser-safety-issue/188757 ,
   https://forum.lightburnsoftware.com/t/galvo-burning-after-cut-ends/150904 ,
   https://forum.lightburnsoftware.com/t/galvo-jpt-60w-mopa-stops-in-the-middle-of-the-job/86108 ).
   LightBurn responde que es responsabilidad de la controladora. Un haz fijo en un punto puede
   perforar la pieza, el soporte o el propio cabezal. Conf. B (anecdótico).
3. **Orden de encendido de la fuente**: MO antes que AP (>= 5 ms en la RFL-P20QB), frecuencia
   mínima (>20 kHz) y no más de 100 us sin pulso en fuentes Q-switched; si se rompen,
   "puede dañarse" el láser (Raycus RFL-P20QB, Conf. B). A 1 kHz una fuente JPT MOPA pasa a CW.
   Parámetros de pulso fuera de rango o inconsistentes con la forma de onda de la fuente son
   un riesgo de daño (retropropagación en el módulo láser). 
4. **Cabezal sin alimentar o sin inicializar**: el manual JCZ avisa de que la tarjeta y el
   cabezal deben estar encendidos antes que el láser, o el haz se desvía sin control.
5. **Tarjeta distinta de la esperada**: mandar comandos de una revisión a otra, a un clon o a
   una DLC, o el comando de EEPROM, puede producir comportamientos que no se pueden predecir.
6. **Pedal de inicio / entrada de seguridad mal configurados** (LightBurn recomienda
   "Disabled" sin pedal).

Mitigaciones que proponemos (todas verificables por prueba unitaria, salvo las físicas):

- Software: apagar siempre el láser (DisableLaser y potencia 0) al empezar, al terminar, ante
  error de USB, al cancelar, al pausar y en un `Drop`/hook de pánico. Nunca dejar el láser
  habilitado por defecto.
- Listas siempre con `EndOfList`; ninguna marca antes de haber fijado potencia, frecuencia,
  velocidad y retardos; validar frecuencia y ancho de pulso contra el perfil de la fuente
  (rangos, lista discreta de anchos de pulso para JPT, secuencia MO/AP).
- Límite de potencia máxima por perfil, "dry run" (todo el trabajo con el láser deshabilitado
  y con el puntero rojo) y encuadre sin dispararse. Un trabajo nuevo siempre empieza por el
  encuadre.
- Confirmación explícita del usuario y comprobación del tipo de tarjeta por VID:PID y
  GetVersion/GetSerialNo antes de enviar; rechazar tarjetas desconocidas (modo
  "experimental" solo con bandera).
- El bit de EMSTOP/enclavamiento debe leerse antes de cada trabajo y detener el trabajo si está
  activo.
- Documentación al usuario: gafas con la DO correcta, recinto cerrado con enclavamiento,
  obturador (el propio manual JCZ lo recomienda), extinción de incendio. El software no
  puede garantizar la seguridad ocular: es un cabezal clase 4 sin recinto.
- Publicación: etiqueta "no probado con hardware real" hasta cumplir la sección 9; la
  primera versión exige activar una opción de "hardware láser real" con aviso.

## 6. Cómo validar sin hardware

- **Capturas USB descritas**: Bryce afirma haber deducido los comandos observando USB y pide
  volcados de otras tarjetas (https://hackaday.com/tag/balor/). **No hay capturas
  publicadas como vectores de prueba** que hayamos encontrado. El único vector parcial es el
  tamaño de la tabla COR (31 752 bytes), la forma 12 bytes y la tanda de 256 comandos.
- **Emuladores**: no se encontró ningún emulador de tarjeta LMC. El plugin balor de MeerK40t
  tiene comandos de consola que imprimen y guardan paquetes y un modo "light job" y una imagen
  de simulación del trabajo, pero solo con dispositivo (README en prosa:
  https://github.com/tatarize/balor-meerk40t, licencia GPL-3.0; el repositorio **existe, no
  leído** más allá de su README). MeerK40t principal tiene un emulador de Ruida, que es otra
  familia.
- Nota de licencia: docs/spec/03-maquinas-avanzadas.md da MIT a galvoplotter y a balormk de
  MeerK40t. El README del plugin balor-meerk40t dice GPL-3.0 y el artículo de Hackaday no
  indica la licencia de Balor. **No hemos verificado esas licencias**; para la sala limpia
  trátalas como contaminantes de todos modos: "existe, no leída".
- Lo que **sí** se puede verificar con certeza sin hardware (ver sección 8.4): codificación
  de bytes, relleno de tandas, matemática de cuantificación a 16 bits, geometría y
  corrección, invariantes de seguridad, cálculo de tiempos y un emulador propio que
  interprete nuestras listas y las dibuje.
- Lo que **no** se puede verificar: que la tarjeta interprete nuestros parámetros igual que
  EZCAD2.
- Camino de captura legítimo (para el equipo que tenga placa): grabar con Wireshark/usbmon el
  tráfico de **trabajos de prueba mínimos conocidos** hechos con EZCAD2 (un punto, una línea,
  un cuadrado, con cada parámetro cambiado de uno en uno), y escribir una especificación
  propia en prosa y tablas; implementa otra persona que no haya leído código ajeno. Hay que
  revisar la licencia de EZCAD2 (puede prohibir ingeniería inversa de su software); capturar
  el USB de una tarjeta propia es distinto que desensamblar DLL, pero conviene consultarlo.

## 7. Brechas de conocimiento (qué falta y cómo cerrarlo)

| Falta | Cómo cerrarlo |
|---|---|
| Unidades y codificación de velocidad, frecuencia, ancho de pulso y retardos | Captura de EZCAD2 variando un parámetro por vez; comprobar con osciloscopio en PRR y en CLK/SYNC |
| Bits de estado (ocupado, listo, lista llena, EMSTOP, alarmas) | Captura de GetState / GetListStatus en reposo, en marcha y con entradas activadas |
| Secuencia de arranque (versión, ¿WriteCorTable obligatorio?, EnableLaser) | Captura del arranque de EZCAD2 |
| Formato de la tabla de corrección | Captura de WriteCorTable con un .cor de 9 puntos conocido; comparar con CorFile2 |
| Fuente: orden MO/AP y manual real de Raycus/JPT | Descargar los PDF oficiales del modelo exacto y leerlos |
| DLC/EZCAD3 | No documentado; fuera de alcance |
| Mapeo ambiguo de códigos duplicados | Captura |

## 8. Viabilidad y diseño

### 8.1 Lo que hay hoy en la IR (crates/tlaku-core/src/toolpath.rs)

`Toolpath { blocks: Vec<ToolpathBlock> }`; cada bloque tiene `speed_mm_min`, `air_assist`,
`power_mode` (Dynamic/Constant), `z_mm` y una lista de `Move::Travel { to }` o
`Move::Cut { to, power_pct }`. La potencia por movimiento ya existe (sirve para imágenes y
overscan a potencia 0). `CutSettings` en tlaku-core tiene velocidad, potencia, pasadas y
aire; `LayerSettings` (crates/tlaku-doc/src/layer.rs) añade modo, relleno, intervalo, ángulo,
overscan, kerf, imagen, tabs y Z. `DeviceProfile` (crates/tlaku-doc/src/device.rs) está
pensado para GRBL (`s_max`, baudios, `rx_buffer`, G-code, aceleración).

Correspondencia directa: `Travel` es salto y `Cut` es marca. Cabe casi todo.

### 8.2 Qué le falta a la IR y a `LayerSettings`

Principio: no ensuciar la IR común. Añadir un bloque opcional de parámetros de galvo
(`#[serde(default)]`, `Option<GalvoParams>` en `ToolpathBlock`) que solo lee el driver
galvo; GRBL y el simulador lo ignoran.

| Necesidad | Dónde | Propuesta |
|---|---|---|
| Frecuencia (kHz) | por bloque | `GalvoParams.frequency_khz` |
| Ancho de pulso Q (ns), MOPA/UV | por bloque | `GalvoParams.pulse_width_ns: Option<u32>` (validado contra la lista discreta del perfil) |
| Retardos: Laser On TC, Laser Off TC, End TC, Polygon TC (us, con signo) | por bloque | `GalvoParams.timing` |
| Salto: velocidad, retardo mín/máx, distancia límite | por bloque (con valores por defecto en el perfil) | `GalvoParams.jump` |
| Velocidad en mm/s | unidad | la IR está en mm/min; el driver convierte (el galvo se expresa normalmente en mm/s; la conversión exacta a la unidad de la tarjeta es ?) |
| Wobble (paso, tamaño) | `LayerSettings` y pipeline | transformación **previa** a la IR: genera Cuts cortos; la IR no necesita campo |
| Pasadas rápidas ("global passes") | `LayerSettings.passes` ya existe | la IR expande pasadas como movimientos repetidos; para galvo conviene un `repeat: u32` por bloque para no repetir miles de movimientos y desbordar la lista de la tarjeta |
| Pausas antes y después de la subcapa (ms) | `GalvoParams` | `pause_before_ms`, `pause_after_ms` |
| Esquina vs. punto intermedio | IR | `Cut` consecutivos con ángulo bajo no necesitan el retardo de polígono; el driver decide por ángulo o recibe un indicador `corner: bool` por vértice (decisión abierta) |
| Overscan | `tlaku-sim`, `tlaku-fill` | no aplica a galvo (espejos con poca inercia): el planificador debe poder pedir 0 de overscan; a cambio, retardos de salto y de línea |
| Imágenes | `tlaku-raster` | hoy produce barrido con potencia variable; en galvo funciona mejor un punto por píxel o líneas con frecuencia constante y dithering; no hay evidencia de que la tarjeta acepte mapas de potencia por línea (existe `ScanBmpPtBuf`, semántica ?) |
| Coordenadas | pipeline | centro del campo, Y invertido/negado, límites de campo en vez de "cama" |
| Segundo láser, puertos, pedal | `DeviceProfile` | fuera de la primera versión |
| `PowerMode` | IR | no aplica; el galvo es siempre potencia constante por segmento con frecuencia (el modo dinámico no existe) |

`DeviceKind::Galvo` y un bloque `galvo: GalvoDevice` en `DeviceProfile`: campo (ancho/alto),
lente, offset y ángulo, tipo de láser (fibra Q-switched, fibra MOPA, CO2, UV), rangos de
frecuencia, lista de anchos de pulso, escala/negar/intercambio por eje, bulge/skew/trapezoid
o tabla de corrección, valores por defecto de saltos y retardos, ajustes del puntero rojo y
puertos, límites de seguridad (potencia máxima, frecuencia mínima), identificador de la
tarjeta (VID:PID, versión).

`LayerSettings` nuevos (todos con `#[serde(default)]` para no romper proyectos): `frequency`,
`pulse_width`, `laser_on_tc`, `laser_off_tc`, `end_tc`, `polygon_tc`, `jump_speed`,
`jump_delay_min`, `jump_delay_max`, `jump_distance_limit`, `wobble: Option<{step, size}>`,
`global_passes`, `pause_before`, `pause_after`. Opcionalmente `override_timings: bool` como en
LightBurn. `tlaku-pipeline` valida estos rangos y emite avisos.

### 8.3 Arquitectura propuesta para `tlaku-galvo`

```
tlaku-galvo/
  wire.rs        enum Command + codec de 12 bytes (int16 LE), tandas de 256 con relleno NOP
  codes.rs       tabla de códigos conocidos con nivel de confianza (conocido / inferido / ?)
  compile.rs     Toolpath + GalvoParams + perfil -> lista de comandos
                 (cuantifica a u16 con centro 0x8000, aplica corrección, inserta retardos)
  calib.rs       campo, escala, negar, intercambio; modelos bulge/skew/trapezoid; rejilla
  safety.rs      validador: invariantes de la lista y límites del perfil
  transport.rs   trait Transport { write_bulk, read_status } y errores
  usb.rs         (feature "usb", rusb) implementación real
  mock.rs        MockTransport grabador + emulador propio de la tarjeta
  session.rs     conexión, comprobación de identidad, arranque, envío con control de flujo,
                 parada segura (DisableLaser en cancelar y en Drop)
  preview.rs     decodifica una lista y la dibuja/mide para pruebas y vista previa
```

Reglas del repo ya existentes que encajan: el driver solo traduce movimientos
(docs/arquitectura.md), el motor no conoce la UI, y todo debe poder probarse desde el CLI con
archivos dorados. Dependencias: `rusb` (MIT) enlaza con libusb (LGPL, enlace dinámico); hay
que comprobarlo con `cargo-deny` antes de aceptarla (la regla del repo es solo licencias
permisivas). La característica `usb` va apagada por defecto y `tlaku-desktop` solo la activa
con una opción explícita. `tlaku-sim` necesita un perfil de tiempo propio para galvo (sin
aceleración de ejes; tiempos de salto y retardos).

### 8.4 Qué se puede verificar con certeza sin hardware

- Codec: tamaños (12 bytes), endianness, relleno a tanda de 256, ida y vuelta (round trip)
  con pruebas de propiedades y fuzzing (no debe haber pánicos).
- Matemática: cuantificación a 16 bits, saturación en el borde del campo, centro 0x8000,
  escala por lente (110/150/200/300), inversión y giro de ejes, corrección (monotonía y
  error de ida y vuelta).
- Compilador: que cada Travel/Cut/retardo/pasada salga en el orden esperado; conteo de
  comandos y tamaño de lista; que la lista siempre empiece desactivada y termine en
  `EndOfList` con el láser apagado.
- Seguridad: ningún `Cut` con potencia fuera de rango, frecuencia o pulso fuera del perfil;
  nunca un Cut tras la última habilitación; el estado final es láser apagado (con prueba sobre
  el emulador propio).
- Estimación de tiempo y vista previa a partir de la lista compilada.
- Lo que **no** se puede verificar: unidades reales, estado, secuencias de arranque, tabla
  COR, comportamiento al detener, tiempos de señal de la fuente.
- Nota de método: las pruebas contra un emulador propio solo prueban que nuestro código es
  consistente con lo que *creemos* que dice el protocolo. No sustituyen la prueba real.

### 8.5 Pipeline galvo (implementado en `tlaku-pipeline`)

- **Bloques.** Con `DeviceKind::Galvo` cada bloque lleva `GalvoParams`: frecuencia y ancho de pulso
  de la capa; retardos y saltos de la capa solo con `override_timings` y si no del perfil;
  `repeat` y las pausas de la capa. `PowerMode::Constant`. Avisos si la frecuencia, el pulso o la
  potencia salen de lo que admite el perfil.
- **Unidades.** La velocidad de capa y de bloque sigue en mm/min; la interfaz del galvo muestra
  mm/s y guarda `mm/s * 60`. `jump_speed` es mm/s. Las coordenadas son del documento: el campo
  (`field_mm`, cuadrado) está centrado en el centro de la cama.
- **Sin overscan** en rellenos ni imágenes. Los límites son los del campo (`build` avisa,
  `compute` falla, salvo con `split.enabled`).
- **Bamboleo.** Espiral por trazo de una capa line: ancho cubierto `size`, avance `step` por vuelta
  (12 muestras por vuelta). Se aplica sobre los movimientos ya planificados del bloque.
- **Grabado 3D por cortes (C47).** `GalvoLayer.slice3d`: N cortes de la imagen con umbral
  `(N-k)/N`, un bloque por corte, con ángulo por corte y Z por corte opcionales; sirve también en
  pórtico.
- **Cilindro y cono (M46).** `GalvoDevice.cylinder` y `GalvoDevice.taper`: proyección en
  perspectiva desde el espejo (ver `cylinder.rs`); modelo propio, sin verificar con hardware.
- **Marcado dividido, repetido y rotativo galvo (M40, M30).** `tlaku_pipeline::mark_sequence`
  devuelve las piezas (`TileSpec` para `JobOptions::tile`), el movimiento de la mesa o del
  rotativo entre ellas y las pausas; el envío es de la integración.

## 9. Prueba mínima con máquina real antes de publicar

1. Una LMCV4-FIBER-M (u otra tarjeta objetivo) con cabezal galvo alimentado, **sin fuente
   láser conectada** (o con el cable DB25 desconectado). Osciloscopio o analizador lógico en
   CLK/SYNC de CON1 y en MO, AP, PRR, PLATCH y P0-P7 de CON2.
2. Con esa configuración, comprobar: identificación de la tarjeta; arranque; GotoXY y
   cuadrado de saltos (visible con el puntero rojo); centro 0x8000; escala del campo; estado
   durante y tras la marcha; que `DisableLaser`/EMSTOP dejan MO, AP y PRR en reposo; que
   cancelar un trabajo a mitad apaga las señales; que desconectar el USB a mitad del trabajo
   deja el láser deshabilitado (esto es lo más importante: ninguna fuente propia puede
   garantizarlo por software, hay que medirlo).
3. Medir frecuencia generada en PRR contra la pedida, y los retardos TC contra lo pedido
   (resolución, signo, unidad).
4. Con la fuente conectada: primero con la potencia mínima, recinto cerrado, gafas con DO
   correcta y obturador, sobre un material que no importe; verificar un punto, una línea y un
   cuadrado; comparar con el mismo trabajo hecho con EZCAD2; medir distorsión del campo con
   una rejilla grabada (ya en ideas-mejoras.md).
5. Repetir con una segunda tarjeta/clon distinto si se quiere declarar compatibilidad más
   amplia que "probado con X".
6. Solo entonces publicar, con la etiqueta "probado con: <tarjeta, fuente, lente>" y bandera
   explícita para el resto.

Con Falcon2 como único hardware (docs/spec/00-resumen.md, estado) no se puede hacer
nada de lo anterior: hace falta comprar o pedir prestada al menos una tarjeta con cabezal.

## 10. Plan por etapas y decisión sobre la fase 5

Decisión recomendada: **el driver galvo espera a la fase 5 (M5 "Pro")**, como ya dice
docs/spec/00-resumen.md. Razones: el hardware no está disponible, la especificación pública es
incompleta, el riesgo de daño y de seguridad ocular es mayor que en GRBL, y el trabajo previo
(M2 a M4) no depende de él. Lo que sí puede adelantarse sin riesgo:

| Etapa | Cuándo | Contenido | Salida láser real |
|---|---|---|---|
| 0 | ahora (cuando convenga) | Este documento; modelo de datos: `GalvoParams` opcional en la IR, `DeviceKind::Galvo` y bloque `galvo` en `DeviceProfile`, campos nuevos de `LayerSettings` con `serde(default)`; validación en `tlaku-pipeline`. Sin driver. | No |
| 1 | M4 o inicio de M5, sin hardware | Crate `tlaku-galvo` con `wire`, `compile`, `calib`, `safety`, `mock`/emulador y `preview`; pruebas de la sección 8.4; salida "lista de comandos" como archivo (volcado), sin `usb`. Marcado como experimental y no expuesto en la app. | No |
| 2 | M5, con una tarjeta propia | Capturas de EZCAD2 de la tarjeta propia, redactar una especificación en prosa (otra persona que implemente), cerrar las brechas de la sección 7. | No |
| 3 | M5, con tarjeta y cabezal, sin fuente | `usb.rs`, `session.rs`; pruebas 1 a 3 de la sección 9. | Solo señales |
| 4 | M5, con fuente | Pruebas 4 a 6; publicar para la combinación probada, con aviso. | Sí, controlado |
| 5 | después | Marcado al vuelo, rotativo galvo, split marking, DLC/EZCAD3 si existe documentación. | Sí |

Confianza global: viabilidad técnica **alta**; confianza en implementar solo desde la prosa
pública **baja** (faltan unidades, estado y arranque); confianza en que la etapa 1 sea útil
sin cambios grandes después **media** (el contenedor de comandos, el modelado de datos y la
parte geométrica son estables; los parámetros no).

## 11. Fuentes consultadas y no leídas

Leídas (prosa): las URL citadas arriba. No se pudieron leer por 403/404 y se usaron solo
resúmenes del buscador: manuales de Raycus en ManualsLib, manual JPT de Thunder Laser, página
FindLight de DLC y LMCV4, Parallax (XY2-100).

Existen, no leídas (son código o tienen riesgo de contaminación): repositorio de Balor
(https://gitlab.com/bryce15/balor), plugin balor-meerk40t
(https://github.com/tatarize/balor-meerk40t, solo el README en prosa), MeerK40t
(https://github.com/meerk40t/meerk40t), galvoplotter (mencionado en docs/spec), cargador
lmcv2u (https://github.com/safarir/lmcv2u-loader), código de los proyectos de la reversa de
LightBurn.
