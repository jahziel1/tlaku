# Arquitectura

El motor es una biblioteca en Rust organizada en crates pequeños. Ninguno conoce la interfaz:
todo lo que haga la aplicación se puede ejecutar y probar desde la línea de comandos.
Los crates intermedios (`geom`, `plan`, `fill`, `raster`, `sim`) dependen solo de `tlaku-core`;
`tlaku-pipeline` los une y es lo que usan el CLI y la app de escritorio (ver [app-api.md](app-api.md)).

## Pipeline

```mermaid
flowchart TD
    SVG[tlaku-svg<br/>SVG] --> JOB
    DXF[tlaku-dxf<br/>DXF] --> JOB
    IMG[imagen PNG/JPEG/BMP] --> RASTER

    JOB["Job: capas con polilíneas<br/>(tlaku-core)"] --> GEOM["tlaku-geom<br/>booleanas, offset, kerf, auto-join"]
    GEOM --> PLAN["tlaku-plan<br/>orden de corte explicable"]
    GEOM --> FILL["tlaku-fill<br/>relleno por barrido"]
    RASTER["tlaku-raster<br/>ajustes, dithering, barrido"]

    PLAN --> TP
    FILL --> TP
    RASTER --> TP
    JOB -. "modo Line directo:<br/>Toolpath::from_line_job" .-> TP

    TP["Toolpath: bloques de Travel y Cut<br/>(tlaku-core)"] --> SIM["tlaku-sim<br/>tiempo, overscan, muestreo"]
    TP --> GCODE["tlaku-grbl::gcode<br/>G-code M3/M4"]
    GCODE --> STREAM["tlaku-grbl::streamer + serial<br/>envío por conteo de caracteres"]
    SIM -. "estimación / vista previa" .-> OUT[(usuario)]
    STREAM --> MACHINE[(máquina GRBL)]
```

Los drivers de otras controladoras también parten de un `Toolpath`: `tlaku-ruida` (experimental, controladoras Ruida por UDP o a un archivo `.rd`; ver [hardware/ruida.md](hardware/ruida.md)) ya existe y el galvo queda para después.

## Responsabilidad de cada crate

| Crate | Responsabilidad |
|---|---|
| `tlaku-core` | Tipos base (`Point`, `Polyline`, `Rgb`), capas, `CutSettings`, `Job`, orden de corte básico (`optimize`) y `Toolpath`. |
| `tlaku-svg` | SVG a curvas en mm, una capa por color; voltea Y al convertir a `Job`. Aplica `clip-path` de verdad (con `tlaku-geom`): las formas rellenas se intersectan y los trazos se parten; `mask` se aproxima como recorte binario y siempre avisa. |
| `tlaku-dxf` | DXF a polilíneas en mm con color ACI/true color; no voltea Y; cada omisión genera una advertencia. |
| `tlaku-geom` | Booleanas y soldar, offset, compensación de kerf, auto-join, quitar segmentos duplicados (rejilla espacial), puentes (`tabs`), regla par-impar de anidamiento. Trabaja sobre polilíneas. |
| `tlaku-plan` | Decide el orden de las figuras (capa, grupo, prioridad, interior primero, recorrido corto) y explica cada paso con un `Reason`. Determinista. "Interior primero" incluye solapes: A va antes que B si más de la mitad de A cae dentro de B (muestreo fijo); en solapes mutuos gana la de menor área, así que no hay ciclos. |
| `tlaku-fill` | Relleno vectorial por barrido (ángulo, serpentina, crosshatch, overscan) o por contornos concéntricos (`FillPattern::Offset`); produce un `ToolpathBlock`. |
| `tlaku-raster` | Imagen a bitmap de potencia (ajustes, remuestreo, dithering/umbral, semitono, líneas de periódico, boceto) y barrido a `ToolpathBlock` con cualquier ángulo (0° conserva el camino original). |
| `tlaku-sim` | Estima el tiempo con aceleración por eje y desviación de unión, calcula el overscan (`v²/2a`) y muestrea la trayectoria. |
| `tlaku-grbl` | G-code GRBL 1.1 a partir de un `Toolpath`, puertos serie, envío con conteo de caracteres, pausa y cancelación, y la seguridad previa al envío (`preflight`: solo se transmite en reposo, con consejos para el CLI o la consola de la app; `frame`: G-code del encuadre). |
| `tlaku-ruida` | **Experimental, sin probar con una máquina real.** De un `Toolpath` a un archivo `.rd` de controladoras Ruida (vectores y ráster con potencia variable `C7`; el origen depende de la esquina del home, `HomeCorner`), paquetes UDP con checksum, enlace por datagramas (`UdpLink` al puerto 50200 escuchando en el 40200, `MemoryLink` para pruebas) y sesión (`RuidaSession`: sondeo de solo lectura que detecta el magic, envío con progreso y parada segura si falla, estado, pausa, reanudar, detener, saltos con el láser apagado y encuadre). Incluye un emulador de controladora (en memoria y por UDP en localhost; feature `emu`). Depende solo de `tlaku-core`, sin dependencias externas. |
| `tlaku-lhymicro` | **Experimental, sin probar con una placa real.** K40 con placa Lihuiyu M2 Nano (LightBurn no la soporta): de un `Toolpath` a LHYMICRO-GL (saltos rápidos y modo compacto, ocho direcciones con Bresenham, velocidad según la placa), paquetes de 30 bytes con CRC-8 (34 en el cable), estado de 6 bytes, transporte CH341 en modo EPP por USB con `nusb` (Rust puro) detrás del trait `Transport`, sesión (`LhySession`: sondeo que detecta una Moshiboard, home, envío con progreso y parada segura si falla, `JobControl` para pausar, reanudar y detener desde otro hilo, saltos y encuadre) y una placa simulada (feature `emu`) que registra dónde estuvo encendido el láser. La placa no controla la potencia por software: se ignora con aviso. Subcomando `tlaku k40`. |
| `tlaku-doc` | Tipos del contrato con la app (`Document`, `LayerSettings`, `DeviceProfile`, `JobOptions`, `JobPreview`...) con `serde` y `ts-rs`, más transformaciones afines y cajas envolventes. No depende del resto del motor. |
| `tlaku-pipeline` | De `Document` + `DeviceProfile` + `JobOptions` a `ComputedJob` (`Toolpath`, vista previa, avisos): validación de capas, orden automático, kerf, plan, puentes, relleno, imágenes, eje Z, corrección de barrido, colocación (`start_from`, `job_origin`), límites de la cama, rotativo y G-code. También `import_file` y `material_test`. |
| `tlaku-cli` | Binario `tlaku`; traduce los argumentos a un `Document` y delega en `tlaku-pipeline`. Para Ruida: `tlaku ruida export-rd | send | status`; para K40 con M2 Nano: `tlaku k40 send | status | list | export-egv` (`--emu` usa la placa simulada). |
| `tlaku-desktop` (`apps/desktop/src-tauri`) | App Tauri: comandos y eventos de [app-api.md](app-api.md) sobre `tlaku-pipeline` y `tlaku-grbl`; la conexión vive en un hilo dedicado. |

## Contrato de `Toolpath`

`Toolpath` (en `tlaku-core/src/toolpath.rs`) es lo único que ven los drivers y el simulador:
una lista de `ToolpathBlock`. Cada bloque comparte velocidad (`speed_mm_min`), asistencia de aire
(`air_assist`) y modo de potencia (`PowerMode`), y contiene una lista de `Move`:

| Movimiento | Significado |
|---|---|
| `Move::Travel { to }` | Desplazamiento rápido con el láser apagado (G0 en GRBL). |
| `Move::Cut { to, power_pct }` | Movimiento a la velocidad del bloque con el láser a `power_pct` (0-100). |

- `Cut` con `power_pct == 0` es **overscan**: el cabezal acelera o frena sin disparar. En GRBL sale
  como G1 con `S0`; el muestreo del simulador lo marca con el láser apagado.
- Los movimientos son segmentos rectos desde la posición anterior.
- `PowerMode::Dynamic` (M4) deja que la controladora escale la potencia con la velocidad real;
  `Constant` (M3) la mantiene fija.
- Un driver solo traduce movimientos a su protocolo: no decide orden, relleno ni overscan.
- `z_mm` (opcional) es la Z absoluta al empezar el bloque (0 = foco); GRBL la emite como `G0 Z`.

## Decisiones del taller (fase 4)

- **Puentes:** se calculan después del kerf y del planificador (el orden de corte no cambia con o
  sin puentes). Cada hueco es `Cut` a potencia 0, no `Travel`: así la máquina no frena en cada
  puente. Nunca ocupan más de la mitad de un contorno; si no caben, se reducen con aviso. El haz
  se come `2 * |kerf|` de cada puente (el kerf es el desplazamiento por lado): se avisa con
  `width < 2 * |kerf|` (no queda puente) y con `width < 4 * |kerf|` (frágil).
- **Eje Z:** solo con `DeviceProfile.z_axis`. Con `z_step != 0` y varias pasadas, la capa se divide
  en un bloque por pasada (una fila de vista previa por pasada). El simulador ignora Z.
  - *Convención interna:* `z_offset`/`z_step` positivos = foco más hacia dentro del material (el
    cabezal baja o la cama sube), medidos desde la altura del cabezal al empezar el trabajo.
  - *Relativa, no absoluta:* el G-code usa `G91` / `G0 Z..` / `G90` con la cuenta de lo acumulado.
    Tlaku no conoce el cero de Z de la máquina (no usa G54/G10), así que la Z absoluta "0 = foco"
    chocaba el cabezal con la pieza. Se eligió esto frente a una Z absoluta con altura de material
    declarada porque no exige ningún dato más del usuario.
  - *Seguridad:* el foco baja justo antes del primer corte del bloque (la aproximación se hace a la
    altura segura) y vuelve a la altura inicial al terminar cada bloque, así ni los
    desplazamientos entre capas ni el regreso al origen se hacen con el cabezal bajo. Los
    movimientos Z son `G0` (con `S0` si el firmware no apaga el láser en `G0`).
  - *Signo:* `DeviceProfile.z_invert` solo cambia el signo del G-code; sin él, profundidad positiva
    sale como Z negativa (GRBL: +Z aleja el cabezal).
- **Rotativo:** el documento y la vista previa quedan en mm de superficie de la pieza; la
  conversión a mm de la controladora se hace solo al generar G-code y encuadre, en Y absoluta
  (sin `G92`). F se reescala por tramo para que la velocidad sobre la superficie sea la pedida.
  Con rotativo no se valida Y contra la cama; se avisa si el diseño excede la circunferencia y si
  la Y de la controladora pasa del área (soft limits: `ALARM:2`). Con rodillo no se valida el
  diámetro de la pieza. `mirror` refleja la Y dentro de la caja del trabajo solo en el G-code.
  Los valores por defecto son los de un accesorio real (rodillo xTool RA2 Pro: 32 mm/vuelta y
  17 mm; mandril Creality: 40 mm/vuelta). `rotary_test` genera la prueba de una vuelta.
- **Corrección de barrido:** posproceso sobre bloques fill (patrón de barrido) e image. La
  separación medida `offset` se reparte: las líneas de ida se corren `-offset/2` y las de vuelta
  `+offset/2` en el sentido del barrido, con el desfase interpolado por velocidad; así el grabado
  queda centrado sobre los vectores (antes solo se movían las de vuelta y todo quedaba corrido).
  `scan_initial_offset` corre todas las líneas, también las unidireccionales. Se avisa si el
  overscan es menor que |offset|.
- **Dialectos de G-code:** `DeviceProfile.dialect` (`GcodeDialect`, datos puros) hace que el
  mismo `generate()` sirva a GRBL, GRBL-M3, grblHAL, FluidNC, GRBL-LPC, Smoothie, Marlin y G-code
  propio. La escala de `S` se queda en `DeviceProfile.s_max`; el dialecto fija formato y
  comandos. Si falta al leer un perfil se deriva del `kind`, así un perfil viejo da el mismo
  G-code. Aviso por limitaciones del dialecto (sin aire, sin potencia dinámica, imágenes con S
  en línea aparte). `tlaku-grbl` depende de `tlaku-doc` solo por este tipo; el enlace (control de
  flujo, pausa, estado) sigue pendiente. Ver `docs/hardware/dialectos-gcode.md`.
- **Prueba de materiales:** `material_test` genera un `Document` normal (una capa por celda y otra
  de rótulos con una fuente de trazo sencillo propia), así que pasa por el mismo pipeline que
  cualquier diseño.

### Orden, sub-capas y optimización (ola 2)

- **Sub-capas:** cada entrada de `LayerSettings.sub_layers` es otra operación sobre **los mismos
  objetos** de la capa, con su propio modo y ajustes, que se ejecuta después de la principal.
  Se ignoran su `color` (usa el de la capa), `name`, `visible`, `order` y sus propias
  `sub_layers`; su `output` sí cuenta, pero el de la capa manda sobre todas (máximo 11). Cada
  operación es un bloque con su `LayerReport` (nombre `#rrggbb · 2/3`, la principal es `1/3`) y
  su fila de vista previa ("sub-capa 2/3"); las pasadas con Z se dividen igual que en una capa.
  Un objeto que el modo de la sub-capa no admite (p. ej. vector en una sub-capa image) se omite
  con un solo aviso por sub-capa.
- **Orden de las sub-capas:** en el orden automático **cada sub-capa se ordena por su propio rol**
  (grabado, línea, corte), no pegada a su capa. Razón: un contorno de corte en una sub-capa
  (relleno + corte) suelta la pieza; si siguiera a su capa, otras capas grabarían después sobre
  una pieza ya suelta. Para no invertir lo que el usuario puso, el rol efectivo de una sub-capa es
  el mayor entre el suyo y el de las operaciones anteriores de su capa: "capa de corte + sub-capa
  de grabado" no adelanta el grabado. Con `layer_order = file` cada capa va seguida de sus
  sub-capas. Una capa con `order=` arrastra a las suyas, que la siguen juntas; las posiciones
  `order=` cuentan solo las operaciones que no son sub-capas de una capa fijada.
- **Prioridad y grupo por objeto:** el pipeline lleva por trayectoria su objeto, grupo, prioridad
  y `power_scale` (`PathMeta` en `place.rs`, paralelo a `LayerWork.paths`) y pasa grupo y
  prioridad al planificador (`None` = 0). Con grupos anidados (`"g2/g1"`) cuenta el de más afuera,
  como al seleccionar. Solo ordenan las capas line (el relleno no se reordena por objeto, salvo
  `FillGrouping::ByGroup`, que recibe un id por grupo, o por objeto si no tiene). Con kerf, que
  reordena y regrupa las trayectorias, cada resultado hereda los datos de la original más cercana
  a su primer punto (aproximado si dos objetos distan menos de un kerf).
- **`JobOptions.optimization`:** `by_group`, `by_priority`, `inner_first`, `best_start`,
  `choose_direction` y `reduce_travel` van tal cual a `PlanOptions`. `by_layer = false` junta las
  capas de corte **consecutivas en el orden de ejecución con exactamente los mismos ajustes** (y
  sin `order=`) en una sola planificación y un solo bloque; capas con ajustes distintos nunca se
  mezclan, porque un bloque lleva una sola velocidad/potencia. `remove_overlaps` corre
  `dedupe_paths` por capa line antes del planificador, conservando el trazo de la primera
  trayectoria en el orden del documento; no se aplica con kerf (las abiertas que deja no se
  compensan) y se avisa. `hide_backlash` fija el sentido antihorario de todas las formas cerradas
  (`PlanOptions.closed_winding`), deja las abiertas sin invertir (`choose_direction` se apaga) y
  graba rellenos e imágenes en un solo sentido; avisa que es más lento. Cuando no son las de por
  defecto, las opciones activas se explican en `order_note`.
- **`angle_increment`:** en fill (barrido y regiones) e image, la pasada `k` usa
  `angle + k * angle_increment`. Como la geometría cambia en cada pasada, cada una se calcula
  aparte y se reparte como las pasadas con Z: un bloque por pasada si hay `z_step` con eje Z,
  concatenadas en un solo bloque si no. En el relleno por contornos no aplica (aviso).
- **Cortar la selección:** `selection` filtra los objetos en `place`; con inicio relativo el
  origen del trabajo se ancla a la caja de **todo el diseño con salida** y la selección conserva
  su lugar respecto a él, y con `use_selection_origin` se ancla a la caja de la selección. En modo
  absoluto, o sin selección, esa opción se ignora con un aviso. Una selección sin nada con salida
  da un error que lo explica.

## Convenciones

- **Unidades:** milímetros; velocidades en mm/min; potencia de 0 a 100 %; ángulos en grados en
  los ajustes y radianes en el cálculo interno.
- **Sistema de coordenadas:** el de la máquina, con origen abajo a la izquierda y Y hacia arriba.
- **SVG** tiene Y hacia abajo, por lo que se voltea al convertir a `Job` (`--no-flip` lo desactiva
  en el CLI). **DXF** ya es Y hacia arriba y no se voltea.
- **Imágenes:** la fila 0 de un bitmap es la de arriba; el barrido hace la conversión al sistema
  de la máquina.
- **Polilíneas cerradas** no repiten el primer punto al final. La pertenencia a una región sigue la
  regla par-impar: profundidad 0 = pieza, 1 = agujero, 2 = isla dentro del agujero.
- **Kerf:** `kerf_offset_mm` es cuánto se desplaza cada trayectoria (normalmente la mitad del ancho
  del corte). **Positivo = la pieza sale del tamaño dibujado**: los contornos exteriores se
  agrandan y los agujeros se encogen. Negativo es lo contrario; cero no cambia nada.
- **Errores:** las entradas degeneradas (vacías, con NaN, de área cero) se ignoran sin pánico y lo
  que no se puede importar se reporta con advertencias, nunca en silencio.
- **Pruebas:** la salida observable (G-code, rellenos, bitmaps) se verifica con archivos dorados;
  ver [CONTRIBUTING.md](../CONTRIBUTING.md).
