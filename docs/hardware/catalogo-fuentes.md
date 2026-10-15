# Catálogo de máquinas: fuentes, descartes y cómo mejorarlo

Estado: 2026-10-14. El catálogo vive en `apps/desktop/src/features/devices/` (`catalog.ts` con los primeros 23 perfiles, `catalog.ruida.ts` y `catalog.diode.ts` con los añadidos; tipos y constructores en `catalogBase.ts`). Aquí se documenta de dónde sale cada perfil nuevo, qué máquinas se descartaron y por qué, y cómo la comunidad puede corregir un perfil.

Sala limpia: los datos son cifras de fichas públicas de fabricantes, tiendas, reseñas y mensajes de foros; las notas están escritas con palabras propias. No se copió texto, código, íconos ni perfiles `.lbdev` de LightBurn (solo se leyeron mensajes públicos del foro, que describen la máquina de un usuario). Los datos de la Falcon2 siguen en [falcon2.md](falcon2.md) y los de firmware de marcas anteriores en [controladoras.md](controladoras.md) sección 1.4.

## 1. Cómo se lee la confianza

Cada cifra del catálogo lleva su URL y uno de cuatro niveles (`SourceKind`):

| Nivel | Qué es | Cómo tratarlo |
|---|---|---|
| `oficial` | Ficha, manual o wiki del fabricante | La más fiable, pero es lo que se anuncia, no lo que la máquina tenga hoy |
| `tienda` | Ficha de tienda, reseña o copia de un manual alojada por un tercero | Puede traer errores de transcripción |
| `foro` | Mensaje de un usuario (volcado de `$$`, `$I` o comentario) | Puede estar modificado por el usuario; sirve como pista |
| `medido` | Leído por el proyecto en una máquina real | Solo la Falcon2 Pro por ahora |

Reglas: lo que no tiene fuente queda sin definir (el asistente pide leerlo de la máquina) y los valores de partida que usa `buildProfile` se marcan como tales en el paso «Guardar». No hay aceleración con fuente salvo la Falcon2 y la Falcon A1 Pro. Las pulgadas se convirtieron a mm tal como las da el propio fabricante (p. ej. 20 × 28 in = 500 × 700 mm) y la cama va como ancho × alto (el lado largo en X). Si las fuentes no coinciden, el dato lleva una nota que lo dice.

El campo `controller` (controladora o firmware tal como lo dice la fuente) se añadió para dejar a la vista de dónde sale el tipo de dispositivo de cada perfil.

## 2. Perfiles añadidos (2026-10-14)

Las 23 entradas nuevas: 7 Ruida y 16 GRBL / grblHAL.

### 2.1 CO2 con Ruida (conexión UDP; soporte experimental, ver [ruida.md](ruida.md))

| Perfil (`id`) | Controladora y fuente | Datos y fuente | Confianza |
|---|---|---|---|
| OMTech Maker 20 × 28 in, 60/90/100 W (`omtech-maker-2028`) | «Ruida control panel» en la ficha oficial | Cama 500 × 700 mm y 600 mm/s ([ficha](https://omtech.com/products/90w-co2-laser-engraver-cutting-machine-with-20-x-28-working-area-with-autofocus)) | oficial |
| OMTech Maker 16 × 24 in, 60 W (`omtech-maker-1624`) | «Ruida control panel» en la [ficha](https://omtech.com/products/60w-co2-laser-engraver-with-16-x-24-working-area); un [hilo](https://forum.lightburnsoftware.com/t/omtech-not-showing-put-right/188371) lo llama «Ruida DSP» | Cama 400 × 600 mm y 600 mm/s (la misma ficha). Un [usuario](https://forum.lightburnsoftware.com/t/omtech-60w-co2-laser-engraver-work-area/175769) vio 600 × 324 mm en LightBurn: Ruida guarda su propio tamaño | oficial |
| OMTech Pronto 40, 90 W (`omtech-pronto-40`) | La [ficha](https://omtech.com/products/pronto-40-90w-co2-laser-engraver-and-cutter) solo trae RDWorks (el software de Ruida); **no nombra la controladora** (inferencia, dicha en el perfil) | Cama 900 × 600 mm y 1000 mm/s (la misma ficha) | oficial en cifras; controladora inferida |
| OMTech Polar Lite, 55 W (`omtech-polar-lite-55w`) | La [ficha](https://omtech.com/products/omtech-polar-lite-55w-desktop-co2-laser-engraver-and-cutter) trae RDWorks, sin nombrar la controladora (misma inferencia) | Cama 510 × 300 mm y 500 mm/s (la misma ficha) | oficial en cifras; controladora inferida |
| Monport CO2 20 × 28 in, 60/90 W (`monport-co2-20x28`) | Un [hilo](https://forum.lightburnsoftware.com/t/monport-60w-20-x-28-co2-with-autofocus-lightburn-profile/115154): 60 W con DSP, RDWorks, FTDI/USB o Ethernet. La ficha de Monport no nombra la controladora | Cama 500 × 700 mm de la [Effi7S de 90 W](https://monportlaser.com/products/monport-90w-co2-laser-engraver-cutter-28-x-20-with-autofocus); sin velocidad (el Effi7S anuncia 1400 mm/s, otras versiones no) | oficial en cama; foro en controladora |
| Thunder Laser Nova 35, 80/100 W (`thunder-nova-35`) | El fabricante no la nombra. [Foro](https://forum.lightburnsoftware.com/t/184980): un Nova 51/130 trae RDC6442G-LY-B(EC) y los Nova se manejan con RDWorks | Cama 900 × 600 mm ([Thunder Laser Europa](https://eu.thunderlaser.com/?p=508)) y 1000 mm/s ([hoja del distribuidor](https://nz.thunderlaser.com/wp-content/uploads/2026/01/Nova-35-80w-Dec25.pdf)) | oficial en cifras; foro en controladora (el Nova 35 se supone igual) |
| Ruida genérico RDC644x (`ruida-generic`) | Familia, sin cifras | Solo notas: conexión por red y confirmar cama/origen en la pantalla | n/a |

Las cifras de velocidad no significan que el Ruida las acepte tal cual: la controladora guarda sus propios límites.

### 2.2 Diodos y CO2 de escritorio con GRBL / grblHAL (USB serie)

| Perfil (`id`) | Firmware (fuente) | Datos (fuente) | Confianza |
|---|---|---|---|
| Comgrow Z1, 1.6/5/10 W (`comgrow-z1`) | GRBL con LaserGRBL/LightBurn; el manual manda S máx 1000, M3/M4 y 115200 baudios | Cama 400 × 400, S 1000, M4, 115200, origen delantero izquierdo: [manual](https://drucktipps3d.de/wp-content/uploads/2022/02/COMGO-Z1-manual-V1.1.pdf), **copia alojada por un tercero** | tienda |
| Longer B1, 20/30 W (`longer-b1`) | GRBL con LaserGRBL/LightBurn; aire M8/M9; finales de carrera con límites ([wiki](https://wiki.longer3d.com/en/b1-30w)) | S 1000, 115200 y aire: wiki (oficial). Cama 450 × 440, 36 000 mm/min y Z manual: [reseña](https://the-gadgeteer.com/2023/06/23/longer-b1-30-watt-laser-engraver-review-with-great-power-comes-great-responsibility/) | oficial + tienda |
| Two Trees TS2, 10 W (`twotrees-ts2`) | No verificado (la reseña habla de 32 bits, LaserGRBL y LightBurn) | Cama 450 × 450 y 10 000 mm/min de grabado: [reseña](https://the-gadgeteer.com/2023/01/03/twotrees-ts2-10w-diode-laser-engraver-review-a-powerful-auto-focusing-laser-makes-this-unit-great-for-newbies/) | tienda |
| Atomstack A20 Pro V2 (`atomstack-a20-pro-v2`) | No se encontró `$I`; la ficha lista LightBurn y LaserGRBL | Cama 400 × 365 mm y 400 mm/s: [ficha oficial](https://atomstack.com/it/products/atomstack-ace-pro-v2) | oficial |
| Atomstack A20 Pro original (`atomstack-a20-pro`) | Grbl 1.1h, dos finales de carrera: [volcado de usuario](https://forum.lightburnsoftware.com/t/168593) | `$30`=1000, `$130/$131`=400. Su `$110`=6000 se descarta (ajuste del usuario) | foro |
| Atomstack A40 Pro / V2 (`atomstack-a40-pro`) | «Grbl C10-0132-V1002» sobre ESP32 con M8 en el pin 21: [hilo](https://forum.lightburnsoftware.com/t/163085) | Cama 385 × 400 mm (385 × 850 con extensión): [ficha oficial](https://atomstack.com/fr/products/atomstack-a40-pro-laser-machine-with-f60-air-assist-kit-f3-matrix-cutting-panel-fb2-enclosure-and-r1-pro-rotary-roller) | oficial en cama |
| Sculpfun SF-A9, 20/40 W (`sculpfun-sf-a9`) | Grbl 1.1h; `$I` dice «Using machine:Sculpfun SF-A9»: [volcado](https://forum.lightburnsoftware.com/t/157620) (firma de reconocimiento) | Cama 400 × 400, 36 000 mm/min y aire: [ficha oficial](https://sculpfun.com/products/sculpfun-sf-a9-40w-laser-engraver-cutting-machine). `$30`=1000 del volcado | oficial + foro |
| Sculpfun iCube Pro, Pro Max, Ultra (`sculpfun-icube-*`) | Placa de 32 bits con LightBurn/LaserGRBL según el fabricante; sin `$I` publicado | Cama 125 × 125, 120 × 120 y 150 × 150 mm; 10 000 mm/min: fichas oficiales de [Pro](https://sculpfun.com/products/sculpfun-icube-pro-5w-portable-laser-engraving-machine), [Pro Max](https://sculpfun.com/products/sculpfun-icube-pro-max-10w-portable-laser-engraving-machine) y [Ultra](https://sculpfun.com/products/icube-ultra-portable-laser-engraving-machine) | oficial |
| AlgoLaser Delta, 22 W (`algolaser-delta`, grblHAL) | «AlgoLaser Delta Ready! … GrblHAL 1.1f»: [hilo](https://forum.lightburnsoftware.com/t/123448) (firma de reconocimiento) | Cama 440 × 415 y 40 000 mm/min de movimiento (30 000 grabando): [ficha](https://algolaser.com/products/algolaser-delta-22w-diode-laser-engraver) | oficial + foro |
| AlgoLaser DIY Kit, 5/10 W (`algolaser-diy-kit`) | No verificado (LaserGRBL/LightBurn) | Cama 400 × 400: [FAQ oficial](https://algolaser.com/pages/faq). El Alpha no se añadió: sus páginas dan 400 × 400 y 425 × 415 | oficial |
| Neje Max 4 (`neje-max-4`) | Grbl 1.1f, elegir GRBL y no GRBL-LPC: [hilo](https://forum.lightburnsoftware.com/t/117525) y [hilo](https://forum.lightburnsoftware.com/t/156455) | Sin cama base (con extensión 750 × 1030 mm); `$110/$111` de fábrica agresivos | foro |
| Atezr P20 Plus, 20 W (`atezr-p20-plus`) | «Grbl 1.1h»: [foro v1e](https://forum.v1e.com/t/atezr-20w-and-36w-laser-machines-lightburn-miscellaneous-ops-and-observations/36785) | Cama 430 × 430: [reseña](https://the-gadgeteer.com/2022/10/21/atezr-p20-plus-laser-engraving-machine-review-its-not-for-everyone-but-perfect-for-those-who-want-one) | tienda + foro |
| Monport 40 W Pro CO2 (`monport-40w-pro`) | GRBL (la [ficha](https://monportlaser.com/products/monport-40w-lightburn-laser-engraver) lista GRBL y LightBurn). **No es Ruida** | Cama 300 × 200 (8 × 12 in) y 350 mm/s | oficial |
| Creality Falcon A1 Pro (`creality-falcon-a1-pro`, grblHAL) | GrblHAL 1.1f con firmware propio; el soporte de LightBurn dice que no es un GRBL estándar: [hilo con `$$`](https://forum.lightburnsoftware.com/t/177926) | Cama 358 × 268 y 600 mm/s: [ficha](https://www.creality.com/products/falcon-a1-pro-20w); `$30`=1000, `$120`=1000 y `$130/$131`=363/273 del volcado | oficial + foro |

Reconocimiento por firma: la SF-A9, la Delta y la Ortur LM3 (foro) y la Falcon2 Pro (medida, ver [falcon2.md](falcon2.md) sección 10) traen una firma en el catálogo (`signatures`). Si `$I` o el saludo contiene su cadena, el asistente preselecciona el modelo y enseña la línea que coincidió; si varias firmas coinciden, la cama (`$130/$131`) desempata o se muestran las candidatas. Solo se añade una firma cuando hay una fuente pública con la cadena exacta.

## 3. Descartadas (no se añaden como compatibles)

| Máquina | Por qué | Fuente |
|---|---|---|
| Gweike Cloud (CO2 de 50 W) | Es una Ruida 644XG, pero **solo por USB**: según el FAQ del foro de LightBurn su placa usa Ethernet solo para mandar la imagen y el control va por Wi-Fi, así que LightBurn no puede controlarla por red. El Ruida de Tlaku es UDP; con firmware especializado, sin panel | [FAQ Gweike Cloud](https://forum.lightburnsoftware.com/t/97874), [comparación con Thunder](https://forum.lightburnsoftware.com/t/156384) |
| xTool F1, F1 Ultra, F2 Ultra | F1/F2 Ultra son galvo con driver EZCAD (no GRBL serie); el hilo de F1 Ultra pide cambiar láser con G-code propio. Ya estaba descartada la red de xTool | [F2 Ultra](https://forum.lightburnsoftware.com/t/183981), [F1 Ultra](https://forum.lightburnsoftware.com/t/166490) |
| xTool P2 | LightBurn no recibe los ajustes del controlador por USB ni por Ethernet | [hilo](https://forum.lightburnsoftware.com/t/124983) |
| LaserPecker 2/4/5, LX1 | LP4 usa G-code propio (`M3011`) con su `.lbdev`; LP5 se anuncia compatible y no lo es (sin rotativo, sin control de la fibra) | [LP5](https://forum.lightburnsoftware.com/t/169455), [LP4](https://forum.lightburnsoftware.com/t/184587) |
| WeCreat Vision | Solo se controla por una API HTTP propia; un tercero hizo un puente (Weburn) que la expone como puerto serie. Sin USB GRBL | [Weburn](https://forum.lightburnsoftware.com/t/135403) |
| Mr Carve M1 | Es un láser de fibra (galvo); sin datos | [hilo](https://forum.lightburnsoftware.com/t/81201) |
| Creality CR-Laser Falcon 10 W (módulo) | Es un módulo para impresoras 3D (Ender 3, CR-10): la controladora y el firmware son los de la impresora, no los del módulo. Hay que perfilar la impresora anfitriona | [guía de Creality](https://store.creality.com/uk/blog/creality-laser-model-ultimate-guide), [hilo](https://forum.creality.com/t/10w-laser-kit-on-ender-3-s1-without-falcon-module/2696) |
| Creality Falcon2 Pro S / Falcon A1 (sin Pro) | Pro S: sin ninguna salida de `$I`/`$$` pública encontrada. Falcon A1: la A1 Pro sí se añadió; la A1 sin Pro no tiene datos propios | [hilo Pro S](https://forum.lightburnsoftware.com/t/190497) |
| Thunder Laser Bolt | Ni el fabricante ni el foro nombran la controladora de forma clara (solo menciones indirectas a «Ruida» en un hilo del Bolt Pro) | [hilo](https://forum.lightburnsoftware.com/t/175306) |
| OMTech K40+ 45 W (8 × 12 in) | La ficha no dice la controladora (software «Shenzhouyi»); no se sabe si es M2 Nano, GRBL o Ruida | [ficha de OMTech](https://omtech.com/products/40w-co2-laser-engraver-cutter-usb-032b-us) |
| Monport Reno 45 W | Hay tres variantes: la Basic dice «LightBurn Core, LaserGRBL» y la Pro/Vision «LightBurn Pro» (pueden ser dos controladoras distintas); falta la prueba | [ficha](https://monportlaser.com/products/monport-reno-45w-desktop-co2-laser-engraver-cutter-16-x-12-with-brand-new-semi-autofocus) |
| AlgoLaser Alpha | Las páginas del fabricante dan 400 × 400 y 425 × 415 mm | [ficha Delta](https://algolaser.com/products/algolaser-delta-22w-diode-laser-engraver) |
| Ortur LM3 (variantes) | Ya está el LM3; no se separaron variantes sin fuente que las distinga | n/a |

## 4. Cómo mejorar un perfil

1. Abre el formulario [«Real machine test report»](../../.github/ISSUE_TEMPLATE/machine_report.yml) (en la app: *Ayuda > Compartir resultados de mi máquina…*, que ya llena la controladora y `$I`). Pide por separado la salida de **`$I`** y la de **`$$`** de una máquina GRBL (las dos son de solo lectura desde *Máquina > Consola > Consultas rápidas*) y la marca, el modelo exacto, el firmware y la conexión; sigue los niveles de [validacion-hardware.md](../validacion-hardware.md) en orden.
2. Para una Ruida (sin `$$`) anota la controladora que muestra su pantalla (RDC6442/6445…), cómo la conectas (IP/Ethernet) y el tamaño de cama que dice la máquina.
3. Para proponer un cambio al catálogo, abre un PR (o comenta el reporte) con:
   - el dato, **su URL** (ficha, manual, hilo con el volcado) y el nivel (`oficial`, `tienda`, `foro` o `medido`);
   - la cadena exacta de `$I` si quieres añadir una firma de reconocimiento (`signatures`), con la línea completa;
   - los valores de `$30`, `$110/$111`, `$120/$121`, `$130/$131` y `$32` si los tienes: el asistente compara el catálogo contra lo que lea la máquina y propone usar lo de la máquina cuando difiere.
4. No pegues archivos `.lbdev`, textos ni capturas de LightBurn: solo cifras con su fuente.

## 5. Pendientes

- Ninguna de las 23 entradas se probó con máquina real; los volcados de foro pueden estar modificados por el usuario.
- Falta confirmar la controladora de las OMTech Pronto y Polar Lite y del Thunder Nova 35 con la pantalla de una máquina o su manual.
- `docs/hardware/controladoras.md` sección 1.5 dice que no verificó que OMTech, Monport y Thunder usen Ruida: queda verificado solo para las filas de arriba.
