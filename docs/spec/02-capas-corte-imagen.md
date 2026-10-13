# 02 - Capas, ajustes de corte y procesamiento de imagen (inventario funcional)

Documento de sala limpia. Todo el comportamiento está descrito con palabras propias a partir de la documentación pública de LightBurn, el foro y tutoriales. No se copió texto ni capturas, y no se consultó ni se infirió código de LightBurn.

Convenciones:
- "(no verificado)" = no se pudo confirmar en una fuente pública consultada; es hipótesis o conocimiento general de la industria.
- "(inferencia)" = deducción razonable a partir del comportamiento documentado, pero LightBurn no la declara.
- Prioridad: P0 = MVP, P1 = importante, P2 = avanzado.
- Fecha de investigación: 2026-10-12. Versión de documentación base: 2.1 (cotejada con 2.0 y 1.7 cuando aplica).

## 0. Fuentes principales

Prefijo `D21` = `https://docs.lightburnsoftware.com/2.1/`

| Clave | URL |
|---|---|
| S-CutsLayers | D21`Reference/CutsLayersWindow/` |
| S-Editor | D21`Reference/CutSettingsEditor/` (índice) |
| S-Shared | D21`Reference/CutSettingsEditor/SharedSettings/` |
| S-Line | D21`Reference/CutSettingsEditor/LineMode/` |
| S-Fill | D21`Reference/CutSettingsEditor/FillMode/` |
| S-Offset | D21`Reference/CutSettingsEditor/OffsetFillMode/` |
| S-Image | D21`Reference/CutSettingsEditor/ImageMode/` |
| S-Sub | D21`Reference/CutSettingsEditor/SubLayers/` |
| S-Galvo | D21`Reference/CutSettingsEditor/GalvoSpecificCutSettings/` |
| S-Defaults | D21`Reference/CutSettingsEditor/WorkingWithDefaults/` |
| S-LayerModes | D21`Explainers/LayerModes/` |
| S-Overscan | D21`Explainers/Overscanning/` |
| S-Optim | D21`Reference/OptimizationSettings/` |
| S-JobOpt | D21`Collections/JobOptimization/` |
| S-ShapeProps | D21`Reference/ShapeProperties/` |
| S-Adjust | D21`Reference/AdjustImage/` |
| S-Tabs | D21`Reference/AddTabs/` |
| S-ScanOff | D21`Guides/ScanningOffsetAdjustment/` |
| S-DevBasic | D21`Reference/DeviceSettings/BasicSettings/` |
| S-DevAdd | D21`Reference/DeviceSettings/AdditionalSettings/` |
| S-MatLib | D21`Reference/MaterialLibrary/` |
| S-MatTest | D21`Reference/MaterialTest/` |
| S-IntTest | D21`Reference/IntervalTest/` |
| S-Preview | D21`Reference/Preview/` |
| S-TimeEst | D21`Troubleshooting/PreviewWindow/InaccurateTimeEstimates/` |
| S-3D | `https://docs.lightburnsoftware.com/galvo/3DSlice.html` |
| S-GitImg | `https://github.com/LightBurnSoftware/Documentation/blob/1741e66e9b2a583d00e5630da6b953ca9382ebf4/EngravingImages.md` (doc histórica) |

Hilos del foro citados: ver sección 5 (cada queja lleva su URL).

Nota: la página D21`Reference/CutSettings/` y D21`Reference/OptimizationSettings/` respondieron según se indica; la ruta `ScanningOffsetAdjustment/` bajo `Reference/` dio 404, la correcta es la de `Guides/`.

---

## 1. Tabla resumen

Referencias OSS: licencias no verificadas en esta sesión salvo que se indique; confirmar antes de reutilizar código (solo se recomienda estudiar algoritmos, no copiar).

| ID | Función | Qué hace | Prio | Compl. | Referencia open source útil |
|---|---|---|---|---|---|
| F01 | Ventana Cuts/Layers | Lista de capas (por color de paleta) con modo, vel/pot, output, show, air; menús contextuales; reordenar | P0 | media | UI propia; MeerK40t (operaciones en "tree") |
| F02 | Output on/off por capa | Excluye la capa de preview/envío/guardado | P0 | baja | - |
| F03 | Show/Hide por capa | Visibilidad en el lienzo sin afectar salida | P0 | baja | - |
| F04 | Air assist por capa | Enciende/apaga aire (M7/M8 en G-code) | P1 | baja | LaserGRBL/GRBL (M7/M8) |
| F05 | Orden de capas + "Sort Cuts Last" | Ejecución de arriba a abajo; atajo que pone grabados antes que cortes | P0 | baja | - |
| F06 | Copiar/pegar ajustes, borrar capa, seleccionar objetos de capa | Gestión de capa | P1 | baja | - |
| F07 | Editor de ajustes: Nombre, Modo, Speed, Max Power, Pass count | Núcleo de parámetros de capa | P0 | baja | - |
| F08 | Min Power | Potencia a baja velocidad (esquinas); en DSP y Grayscale | P1 | media | Ruida (MeerK40t `ruida`) |
| F09 | Constant vs Variable power (M3/M4, GRBL) | Elige modo láser dinámico o constante | P0 | baja | GRBL 1.1 docs, LaserGRBL |
| F10 | Modo Line | Traza contornos | P0 | baja | Inkscape (flatten), `kurbo`/`lyon_geom` |
| F11 | Passes | Repite la capa N veces | P0 | baja | - |
| F12 | Kerf offset | Desplaza la trayectoria hacia fuera/dentro en formas cerradas | P1 | media | `cavalier_contours`, `i_overlay`, `clipper2` |
| F13 | Perforation mode | Corta X mm, salta Y mm (línea punteada) | P1 | baja | - |
| F14 | Tabs / bridges (manual y auto) | Deja huecos sin cortar para sujetar piezas | P1 | alta | MeerK40t (tabs en cut ops, no verificado) |
| F15 | Lead-in / Lead-out | Línea/arco de entrada y salida fuera del contorno | P2 | alta | CAM CNC (FreeCAD Path, no verificado) |
| F16 | Overcut | Extiende el corte pasado el cierre (última pasada) | P2 | media | - |
| F17 | Start/End pause + Cut Through | Pausa con/sin potencia al inicio/fin de trazo | P2 | baja | - |
| F18 | Dot mode | Pulsos puntuales a intervalos (Ruida) | P2 | baja | MeerK40t Ruida |
| F19 | Z offset / Z step por pasada | Mueve eje Z entre/ durante pasadas | P1 | media | GRBL `G0 Z` |
| F20 | Ramp length | Rampa de potencia en bordes (sellos de goma) | P2 | alta | `imageproc` distance transform |
| F21 | Modo Fill (scanline) | Rellena formas cerradas con líneas paralelas | P0 | alta | `i_overlay`/`clipper2` (even-odd), LaserGRBL, MeerK40t `rasterplotter` |
| F22 | Line interval / LPI / DPI | Separación entre líneas de barrido | P0 | baja | - |
| F23 | Scan angle | Ángulo de barrido (0/90 óptimo; otros emulados en DSP) | P0 | media | rotación + scanline |
| F24 | Cross-hatch | Segunda pasada a 90 grados | P1 | baja | - |
| F25 | Bidireccional | Dispara en ida y vuelta | P0 | baja | - |
| F26 | Overscanning | Recorrido sin disparo antes/después de cada línea | P0 | media | GRBL accel; MeerK40t raster |
| F27 | Fill grouping | Todas juntas / por grupo / por forma | P1 | media | - |
| F28 | Flood fill | Ruta de relleno que evita viajes por zonas vacías | P2 | alta | boustrophedon/coverage planning (literatura) |
| F29 | Offset fill | Relleno concéntrico siguiendo el contorno | P1 | alta | `cavalier_contours`, `i_overlay` outline, `clipper2` offset |
| F30 | Sub-capas (hasta 11) | Varios ajustes sobre la misma geometría | P1 | media | - |
| F31 | Cut order priority (por objeto) y Power Scale | Prioridad numérica; escala de potencia por objeto | P1 | baja | - |
| F32 | Optimization Settings (orden/ caminos) | Cut planner: capa/grupo/prioridad, interiores primero, etc. | P0 | alta | `vpype` (linesort, linemerge), 2-opt/NN; MeerK40t `cutplan` |
| F33 | Remove overlapping lines | Elimina segmentos duplicados entre formas | P1 | alta | `i_overlay`, `geo` |
| F34 | Hide backlash (orden) | Orden que minimiza desalineación por holgura | P2 | alta | (no verificado) |
| F35 | Default layer settings | Valores por defecto por color y dispositivo | P1 | baja | - |
| F36 | Modo Image: parámetros generales | DPI/Interval, negativo, bidireccional, overscan, ángulo, pasadas | P0 | media | `image`, `imageproc` |
| F37 | Threshold | Binarización por umbral | P0 | baja | `image` |
| F38 | Dither (Floyd-Steinberg) | Difusión de error | P0 | baja | Floyd-Steinberg 1976 |
| F39 | Stucki | Difusión de error amplia | P0 | baja | Stucki 1981 |
| F40 | Jarvis | Difusión de error amplia | P0 | baja | Jarvis-Judice-Ninke 1976 |
| F41 | Atkinson | Difusión parcial | P1 | baja | Atkinson (Apple, 1984) |
| F42 | Ordered (Bayer) | Matriz de umbrales | P1 | baja | Bayer 1973 |
| F43 | Newsprint | Puntos de tamaño variable en rejilla fija | P2 | media | halftoning clásico (Ulichney 1987) |
| F44 | Halftone (cell/angle) | Semitono con celdas por pulgada y ángulo | P2 | media | Ulichney 1987; GIMP/Inkscape |
| F45 | Sketch | Solo engrava bordes duros (dibujos/escritura) | P2 | media | Sobel/Canny (`imageproc`) (algoritmo exacto no verificado) |
| F46 | Grayscale | Potencia modulada entre Min y Max según gris | P1 | media | LaserGRBL (M4 + S por píxel) |
| F47 | 3D Slice (galvo) | Mapa de profundidad en N pasadas con umbrales | P2 | media | foro (staff); ver sec. 3.8 |
| F48 | Pass-through | Sin remuestreo, 1 píxel = 1 punto | P1 | baja | - |
| F49 | Dot width correction | Acorta cada línea por el ancho de haz | P1 | baja | - |
| F50 | Angle increment (imagen/relleno) | Rota el ángulo entre pasadas | P2 | baja | - |
| F51 | Adjust Image (brillo, contraste, gamma, enhance, denoise, presets) | Preprocesado por imagen/capa con vista previa | P0 | media | `image::imageops`, `imageproc` unsharp |
| F52 | Scanning Offset Adjustment | Tabla velocidad -> desfase de línea (backlash de disparo) | P1 | media | (no hay referencia pública) |
| F53 | Fast Whitespace Scan | Viaje rápido sobre zonas blancas (GCode) | P1 | media | LaserGRBL (modo "fast whitespace" no verificado) |
| F54 | Material Library | Presets de capa por material/espesor; Assign vs Link | P1 | media | - |
| F55 | Material Test | Rejilla N x N variando 2 parámetros | P1 | media | - |
| F56 | Interval Test | Cuadros con distintos intervalos | P2 | baja | - |
| F57 | Preview | Simulación, slider de tiempo, viajes en rojo, shade por potencia | P0 | alta | `egui`/`wgpu`, GRBL planner |
| F58 | Estimación de tiempo | Simulación con aceleración/velocidades configurables | P0 | alta | GRBL planner (junction deviation) |
| F59 | Ajustes dependientes de controlador (PWM freq, PPI, U offset, Frequency, Q-pulse, Wobble, timings) | Parámetros específicos (Ruida/Trocen/Galvo) | P2 | media | MeerK40t |
| F60 | Beginner mode | Oculta opciones avanzadas | P2 | baja | - |

Total: 60 funciones. P0: 22, P1: 23, P2: 15.

---

## 2. Detalle de P0 y P1

Notas generales sobre la salida a la máquina: LightBurn emite para GRBL, Ruida, etc. El comportamiento de G-code descrito abajo es el patrón estándar de GRBL 1.1 (`G0/G1`, `M3/M4/M5`, `S`, `F`); cómo LightBurn lo compone exactamente (p. ej. cuantos puntos por línea) no es público y se marca (inferencia) o (no verificado).

### F01-F06 Ventana Cuts/Layers

Fuente: S-CutsLayers.

Comportamiento:
- Una fila por capa del proyecto; cada capa se identifica por color de la paleta. Las capas se ejecutan de arriba hacia abajo.
- Columnas: número/nombre, color, Modo (Line/Fill/Offset Fill; las capas de imagen quedan fijas en Image; si hay sub-capas muestra "Multi"), velocidad/potencia, Output, Show y Air (solo láseres de gantry).
- Campos editables bajo la lista: Pass Count, Speed, Power Max, Power Min (solo DSP o Grayscale no-UV), Interval (Fill/Offset Fill), Frequency (solo galvo), Q-Pulse (solo MOPA/UV galvo).
- Menú del encabezado: habilitar/deshabilitar todas, invertir selección, mostrar/ocultar todas, invertir visibles, air assist global, y "Sort Cuts Last".
- Menú de la fila: deshabilitar esta / todas menos esta; ocultar esta / todas menos esta; seleccionar todas las formas de la capa; "flash" del contenido (con Shift+clic derecho).
- Sort Cuts Last: ordena capas de más débil a más fuerte (según potencia, velocidad y pasadas) y después mueve las capas de modo Line al final, para que todo el grabado ocurra antes del corte.
- Copiar/pegar copia los ajustes de una capa a otra. Borrar capa elimina también sus objetos; pide confirmación salvo con Shift.
- Shift+clic izquierdo selecciona todos los objetos de la capa.

Parámetros / rangos: no se publican rangos en la documentación (rangos reales dependen del dispositivo). Unidades de velocidad: mm/s o mm/min según Control Units en Device Settings (S-Shared).

Impacto en la salida:
- Output desactivado: la capa no aparece en preview, ni se envía ni se guarda. El objeto se atenúa en el lienzo.
- Show no afecta la salida.
- Air = comando de aire (M7 o M8 en GCode, seleccionable en Device Settings; S-DevBasic). Sin soporte, no hace nada.
- El orden de capas es la clave primaria de salida solo si "Order by Layer" está activo en Optimization (F32).

Casos borde / quejas:
- El cambio de unidades mm/s vs mm/min ha provocado "pérdida" aparente de ajustes tras actualizar (ver sec. 5, Q10).
- El orden por capa puede ser pisado por otras opciones de ordenación (Q4).

Decisión de diseño sugerida: modelar Capa como objeto con lista de sub-capas desde el inicio (aunque la UI inicial muestre una sola).

### F07 Editor de ajustes (parámetros compartidos)

Fuente: S-Shared, S-Editor.

| Parámetro | Unidad / rango | Comportamiento |
|---|---|---|
| Name | texto | Etiqueta de la capa |
| Output | on/off | Ver F02 |
| Mode | Line / Fill / Offset Fill (Image automático) | Modo de procesado |
| Speed | mm/s o mm/min según config; rango no publicado | Velocidad máxima en el trabajo |
| Max Power | % ; 0-100 típico (no verificado) | Potencia a velocidad plena |
| Min Power | % | Potencia a baja velocidad; solo DSP y Grayscale |
| Default Speed / Default Power switch | on/off, solo Ruida | Usa el valor guardado en el controlador |
| Constant Power Mode | on/off, solo GRBL | Ver F09 |
| Air Assist | on/off | Ver F04 |
| Pass count | entero >=1 | Repeticiones |

Impacto: Max Power -> valor `S` escalado por `S-value Max` del dispositivo (255 en GRBL antiguo, 1000 en moderno; S-DevBasic). Speed -> `F` en G-code (en mm/min). Min Power: en DSP (Ruida) se envía como par min/max y el controlador rampa por velocidad; en GRBL no existe (solo para Grayscale, donde se usa para el nivel de gris más claro; S-Image).

Casos borde: si la velocidad configurada es inalcanzable por aceleración, en modo M4 la potencia efectiva nunca llega al máximo (S-Shared).

### F09 Constant vs Variable power (GRBL)

Fuente: S-Shared; GRBL documentación pública (conocimiento general).

- Por defecto LightBurn usa potencia variable (M4: la potencia escala con la velocidad real del cabezal, así en aceleraciones y esquinas no se sobrequema). Constant (M3) mantiene potencia fija; coincide con el comportamiento por defecto de LaserGRBL (según S-Shared).
- Salida: encabezado `M4 S0` o `M3 S0`; fin `M5`.
- Recomendación para réplica: M4 por defecto; exponer M3 como opción; permitir `$32` laser mode check (no verificado).

### F10 / F11 Modo Line y Passes

Fuente: S-Line, S-LayerModes.

- Line traza la geometría (curvas se aplanan a segmentos). La diferencia entre marcar y cortar es solo velocidad/potencia.
- Passes repite la capa N veces. Combinado con Z step (F19) cambia el foco por pasada.
- Salida G-code: `G0 X Y` al inicio de cada trazo, `G1 ... S<p> F<v>` por segmento.

### F12 Kerf offset

Fuente: S-Line; foro (sec. 5 Q5).

- Valor positivo desplaza hacia afuera de formas cerradas, negativo hacia adentro; el objeto original no se modifica (solo la salida). No aplica a formas abiertas.
- Los agujeros (formas interiores) invierten su dirección automáticamente según LightBurn clasifica interior/exterior; por eso un kerf "hacia fuera" encoge los agujeros (foro, sec. 5 Q5; confirmación staff-style).
- Parámetros: distancia con signo, unidad del sistema; rango no publicado.
- Salida: trayectoria offset (join/miter no documentado, (no verificado)); recomendación `cavalier_contours` (polilíneas con arcos, offset paralelo robusto) o `clipper2`/`i_overlay` tras aplanar curvas.
- Casos borde: la clasificación interior/exterior es sensible a la capa en que esté cada forma, a Break Apart y a formas del mismo layer anidadas. El editor de tabs no muestra el kerf en la preview de edición (Q6).

### F13 Perforation mode

Fuente: S-Line.

- Activo: patrón corte (laser on) / salto (laser off) a lo largo del trazo. Parámetros Cut y Skip en mm (ejemplo de la documentación: empezar en 0.1 mm cada uno; rangos no publicados).
- Salida: segmentos `G1 S<p>` alternados con `G1 S0` (o M5) (inferencia). Con valores muy pequeños la potencia efectiva baja por la respuesta del láser.
- Distinción con Dot Mode (F18) y PPI (F59): la perforación es lógica de software; PPI es hardware (Trocen).

### F14 Tabs / bridges

Fuentes: S-Tabs, S-Line; foro `https://forum.lightburnsoftware.com/t/still-big-problems-with-tabs-bridges/111930`.

- Solo en capas modo Line. Manuales: herramienta "Add Tabs", clic para colocar (punto rojo), arrastrar para mover, doble clic o Shift+clic para quitar. Ancho manual por defecto 0.5 mm; no pueden estar más juntos que su ancho.
- Automáticos: opción Tab Generation = Automatic. Controles: Even Spacing (una pestaña al inicio de cada sección continua y luego repetición cada "Spacing"), Limit Max Tabs, Tabs Per Shape (n exacto por trazo, perímetro dividido en n+1 segmentos; en formas cerradas el punto de inicio se desplaza medio segmento), Skip Inner Shapes (no pone tabs en formas cerradas contenidas en otra de la misma capa; no afecta las manuales).
- Parámetros compartidos: Tabs/Bridges on/off (conserva datos al apagar), Tab Size (longitud total de todas las tabs de la forma, unidades del sistema), Tab Cut Power (0% = salta completamente; >0% = pulsa a lo largo del ancho, y más % = pestaña más débil), Clear Tabs.
- Device Settings tiene Tab Pulse Width, que define el ancho de pulso para simular cortes de potencia reducida (S-DevBasic).
- Salida: el trazo se parte; en la zona de tab se emite un salto con láser apagado o pulsado.
- Casos borde / quejas (Q6): la generación automática es "mejor esfuerzo" y existe un mínimo de 3 tabs para ciertos casos; la vista previa de edición no refleja el kerf; la salida final puede diferir de lo mostrado al editar.

### F19 Z offset / Z step

Fuentes: S-Line, S-Fill, S-Image.

- Requiere habilitar movimiento Z en Device Settings y soporte del controlador. Z offset positivo acerca el cabezal al material (enfoca más profundo), negativo lo aleja (línea más ancha).
- Z step per pass: cambia Z la misma cantidad tras cada pasada; solo tiene efecto con más de 1 pasada.
- Advertencia: riesgo de choque; los sistemas con autoenfoque pueden tratar la altura de autoenfoque como mínimo.
- Salida: `G0 Z<h>` (GRBL con eje Z) o comandos propietarios; rangos y signo de Z step no publicados.

### F21-F25 Modo Fill

Fuentes: S-Fill, S-LayerModes.

Comportamiento:
- Solo formas cerradas. Se rellena con líneas paralelas separadas por el intervalo. Regla de solapamiento: dentro de la misma capa, un anidamiento deja el área entre contornos rellena (regla par-impar); dos formas que se superponen en la misma capa dejan sin grabar la superposición; en capas distintas, el solapamiento se graba dos veces.
- Line interval: mm entre líneas; más bajo = más denso. El ideal es el valor en que las líneas apenas se tocan. Se puede expresar como Lines per inch (LPI) equivalente. (Para imagen DPI = 25.4 / intervalo.)
- Scan angle: 0 = horizontal, 90 = vertical; 180 = barrido invertido. En DSP se recomienda múltiplos de 90 (otros ángulos se emulan con movimientos de corte y son un recurso de estilo).
- Cross-hatch: segunda pasada a 90 grados.
- Bidireccional: ida y vuelta disparando; apagado = solo en un sentido (más lento, pero evita errores de desfase).
- Pasadas, Z offset, Z step, Fill grouping (ver F27).
- Avanzado: Ramp length (F20), Flood fill (F28), PWM/PPI/U offset (F59).
- Defaults universales (S-Defaults): intervalo 0.100 mm, rellena todas las formas a la vez, misma velocidad/potencia que la capa.

Algoritmo propuesto (propio): 1) aplanar curvas con tolerancia; 2) aplicar rotación por -ángulo; 3) para cada línea y = k*intervalo, intersectar con polígonos y ordenar cruces; rellenar tramos par-impar; 4) ordenar tramos en serpentina (bidireccional) o unidireccional; 5) añadir overscan; 6) rotar de vuelta.

Salida G-code (patrón): por cada tramo `G0 X<x0 - d> Y<y>` (arranque con margen de overscan), `G1 X<x0> S0`, `G1 X<x1> S<p> F<v>`, `G1 X<x1 + d> S0`. En modo M4 se dispara a potencia constante dentro del tramo.

Casos borde / quejas: formas abiertas no se rellenan (sin aviso claro, Q13); intervalo demasiado grande -> rayas visibles; superposiciones entre formas confunden a los usuarios (regla par-impar vs. capas distintas).

### F26 Overscanning

Fuentes: S-Overscan, S-Fill, S-Image, S-ScanOff.

- Añade recorrido con láser apagado antes y después de cada línea de barrido para que el cabezal alcance velocidad nominal antes de disparar y desacelere después; evita bordes más oscuros que el centro.
- Parámetro: porcentaje de la velocidad (GCode); la UI muestra la distancia resultante. La fórmula exacta no está publicada (no verificado). DSP: automático, sin porcentaje. Galvo: no aplica (usa tiempos/Delays).
- Implementación sugerida: distancia física mínima `d = v^2 / (2 a)` según la aceleración configurada del eje (física estándar), con opción de porcentaje manual. Esta es nuestra propuesta, no el comportamiento documentado.
- Riesgo de fuera de límites: si el diseño está cerca del borde, el overscan puede pasar el área de trabajo; LightBurn avisa con "Cut may be out of bounds due to overscan settings", GRBL devuelve ALARM:2, o el DSP "Not Enough Extend Space". Soluciones: bajar velocidad o %, mover el diseño al centro, origen personalizado.
- En Preview, el overscan se ve como líneas rojas de viaje; cruzar un borde del área puede ser aceptable, dos bordes es excesivo (S-Overscan).
- Trade-off: a mayor velocidad crece el overscan y el tiempo total puede aumentar (ejemplo de la doc: 100 mm/s tardó unos 26 min y 400 mm/s casi 3 min más).
- El encuadre (framing) no simula overscan, por lo que puede pasar el frame y fallar al iniciar (foro, Q9).

### F27 Fill grouping

Fuente: S-Fill, S-JobOpt.

- Opciones: llenar todas las formas a la vez (default; suele ser más eficiente a 300 mm/s o más), por grupos, o forma por forma (útil con grabado lento, aceleración alta o diseños dispersos). Se compara con el tiempo estimado del Preview.
- En el motor: determina si el barrido se hace sobre el conjunto de la capa (una línea de barrido cruza varias formas) o por cluster/forma.
- En imagen: idem (S-Image).

### F29 Offset fill

Fuentes: S-Offset, S-LayerModes.

- Rellena con contornos concéntricos que siguen el borde. Solo formas cerradas; ideal para diseños con grandes áreas huecas; pobre para formas complejas. Parámetros bi-direccional, cross-hatch, overscan y ángulo se muestran deshabilitados.
- Usa Line interval/LPI, pasadas, Z, Fill grouping.
- Computacionalmente muy costoso. Intervalos pequeños + diseños complejos + nodos extra pueden colgar la aplicación (Q1).
- Algoritmo: offsets iterativos interior (negativos) a distancia `k*intervalo` hasta que el polígono desaparece; ordenar anillos; unir con saltos. Para mantener robustez usar `cavalier_contours` (con arcos) o `clipper2` en polígono aplanado; ejecutar en hilo aparte con cancelación real.

### F30 Sub-capas

Fuente: S-Sub.

- Una capa lleva varios conjuntos de ajustes sobre la misma geometría. Máximo 11. Cada sub-capa tiene su pestaña, nombre opcional y su propio Output; el Output de la capa gobierna sobre las sub-capas.
- Se ejecutan en el orden de las pestañas (izquierda a derecha), reordenables.
- En Cuts/Layers el modo se ve como "Multi"; allí se puede cambiar un subconjunto de ajustes pero no el modo.
- Fill+Line antiguo se convierte en dos sub-capas secuenciales. Usos: grabado + corte, pasadas de limpieza en galvo.
- La documentación no explica la interacción con "Pass count" (no verificado); en galvo hay "Global Passes" (S-Galvo).

### F31 Prioridad por objeto y Power Scale

Fuente: S-ShapeProps, S-Optim.

- Cut Order Priority: entero (0 = primero); solo surte efecto si "Order by Priority" está activo en Optimization.
- Power Scale: porcentaje que escala la potencia entre Min Power y Max Power; sin Min Power, escala entre 0 y Max.
- Locked: bloquea ediciones.

### F32 Optimization Settings (cut planner)

Fuente: S-Optim.

Comportamiento:
- Interruptor maestro "Enable Optimization and Output Re-ordering". Apagado: salida en orden de creación.
- Order By (clave primaria, apilables): por Layer, por Group, por Priority. Cada opción nueva subdivide los grupos de la anterior; seleccionar una existente la sube al tope; "Remove" la quita. Orden por defecto: Layer primero, Priority segundo.
- Optimizaciones sobre cada grupo: Cut inner shapes first, Cut in direction order (con botones de dirección), Reduce travel moves, Hide backlash, Reduce direction changes, Choose best starting point, Choose corners if possible, Choose best direction, Remove Overlapping Lines (con distancia).
- Botones: Set as Defaults, Reset to Defaults (Shift = fábrica), OK, Cancel. Los ajustes también se guardan dentro del archivo del proyecto (.lbrn/.lbrn2) y al abrir un proyecto sobrescriben los actuales.
- El estado por defecto de cada casilla no se indica (no verificado).

Implementación sugerida:
- Etapas: (1) clave primaria (ordenamiento estable por tupla); (2) por grupo, orden interior -> exterior usando árbol de contención (point-in-polygon + área); (3) TSP heurístico: vecino más cercano + 2-opt/Or-opt, con elección de punto de inicio y sentido por forma (cerradas pueden rotar su inicio; abiertas pueden invertirse); (4) penalización de cambios de dirección (opcional).
- "Choose corners": elegir el vértice con ángulo más agudo como inicio.
- "Hide backlash": no hay descripción pública del algoritmo (sólo "similar a Backlash Repay Optimize de RDWorks"); (no verificado).
- Referencias: `vpype` (pasos `linesort`, `linemerge`, `reloop`), MeerK40t (`cutplan`), literatura de TSP.

Casos borde y quejas (Q4): el orden de "Cut inner shapes first" falla si Order by Group/Layer/Priority tiene precedencia, si las formas no están cerradas, si hay sub-capas, o si el DXF importó prioridades erróneas. Recomendación: mostrar en la UI qué regla ganó y por qué.

### F33 Remove overlapping lines

Fuente: S-Optim, S-JobOpt.

- Elimina líneas duplicadas compartidas entre dos gráficos para que el láser no pase dos veces por el mismo lugar; campo Distance = qué tan cerca deben estar dos líneas para contarse como solapadas.
- Algoritmo: segmentar, indexar en R-tree (`rstar`), detectar colineales/casi colineales dentro de la tolerancia, fusionar. `i_overlay` puede ayudar con booleanas pero este caso es de polilíneas, no de áreas.
- Queja (Q1): en 1.17.01 aparecieron cuelgues en el mensaje "Removing overlapping lines" al cancelar.

### F35 Default layer settings

Fuente: S-Defaults.

- Los ajustes persisten por capa (color) de la última vez, o se reinician a defaults al empezar proyecto (opción en configuración "Load default layer settings on new or restart"). Botones: Make Default (por color y por dispositivo), Reset to Default (Shift = universal), Make Default for All.
- Default universal: Line a 6000 mm/min (100 mm/s) y 20% de potencia, sin opciones avanzadas. Si se cambia a Fill/Offset/Image: mismo vel/potencia, intervalo 0.100, rellenar todo a la vez. Capas de imagen: Stucki por defecto.

### F36 Modo Image: parámetros

Fuente: S-Image, S-GitImg.

| Parámetro | Unidad / rango | Comportamiento |
|---|---|---|
| Bi-directional | on/off | Ver F25 |
| Negative Image | on/off | Invierte claro/oscuro; útil en pizarra o vidrio |
| Overscanning | % de Speed | Ver F26 |
| Line Interval | mm (probable) | Separación entre filas; DPI = 25.4 / intervalo |
| DPI | puntos/pulgada | Equivalente al intervalo |
| Dot Width Correction | 0 .. Line Interval | Acorta cada línea; ideal ~mitad del ancho del haz |
| Scan Angle | grados | 0 = horizontal; 90 = vertical; múltiplos de 90 en DSP |
| Angle Increment | grados | Cambia el ángulo entre pasadas (90 reproduce el cross-hatch del 3D slicing anterior a 2.0) |
| Cells per Inch, Halftone Angle | celdas/pulg, grados | Solo en Halftone |
| Pasadas, Z offset, Fill grouping | - | Igual que Fill |
| Ramp Length | distancia | Solo con Threshold |
| Pass-through | on/off | Ver F48 |
| Image Mode | lista | Threshold, Ordered, Atkinson, Dither, Stucki, Jarvis, Newsprint, Halftone, Sketch, Grayscale, 3D Slice (galvo) |

Salida: por línea, la imagen procesada (1 bit o niveles de gris) se convierte en una lista de tramos; en modos binarios, tramos encendidos consecutivos = `G1 S<p>` y apagados = `S0` (o `G0`/Fast Whitespace). En Grayscale, el valor de gris determina `S` entre Min y Max Power. El flujo es muy largo, de modo que hay que proteger el buffer de GRBL (streaming con conteo de caracteres; práctica común de LaserGRBL).

Casos borde / quejas: en Grayscale con GRBL, algunos diodos muestran poca diferencia entre niveles de potencia (Q8); modo Grayscale no es compatible con UV (salida fija en 20%, produce rectángulo gris; S-Image).

### F37-F42, F46 Modos de imagen (resumen funcional)

Fuente comportamiento: S-Image, S-GitImg. Algoritmos: ver sección 3.

| Modo | Efecto declarado | Cuándo usarlo según docs |
|---|---|---|
| Threshold | Binario por brillo | Solo imágenes ya de 2 tonos |
| Ordered | Rejilla regular de puntos | Áreas sólidas grandes |
| Atkinson | Difusión parcial | Intermedio; pierde en luces/sombras extremas |
| Dither (FS) | Difusión de error | Fotos suaves; puede dejar vetas en zonas planas grandes |
| Stucki | Difusión; algo más rápido que Jarvis | Fotos; flojo en medios tonos |
| Jarvis | Difusión; algo más lento | Fotos; buen medio tono |
| Newsprint | Puntos que crecen en zonas oscuras | Efecto decorativo; mejor con DPI alto |
| Halftone | Como Newsprint con celda y ángulo ajustables | Imágenes de DPI alto |
| Sketch | Solo bordes duros | Dibujo lineal/escritura |
| Grayscale | Potencia entre Min y Max | Sombras en diodo o profundidad 3D en CO2; usar "Shade according to power" en Preview |

### F48 Pass-through

Fuentes: S-Image, S-GitImg, foro `https://forum.lightburnsoftware.com/t/pass-through-still-enables-the-image-mode/42325`.

- No remuestrea: cada píxel preprocesado -> un punto (Line Interval/DPI queda atado al tamaño de la imagen). Se envía en modo Threshold. Se permite escalar uniformemente; no se permiten escalado no uniforme ni rotación porque requerirían remuestreo.
- Queja: confusión en la UI porque seguía mostrando el selector de modo (Q12).

### F49 Dot width correction

Fuente: S-Image, S-Galvo.

- Acorta cada línea de barrido para compensar el grosor del haz; valor entre 0 y el intervalo.
- Implementación: al convertir píxeles a tramos, recorta `w/2` en cada extremo de cada tramo encendido (inferencia razonable).

### F51 Adjust Image

Fuente: S-Adjust, S-ShapeProps; foro `https://forum.lightburnsoftware.com/t/adjust-image-enhance-radius-enhance-amount-function-not-working/147055`.

- Ventana con original (arriba izq.) y resultado (arriba der.) sincronizados en zoom/pan. Se abre con doble clic, menú contextual o Alt/Option+I.
- Grupo "Layer Settings": mismos ajustes del editor de capa en modo Image; afectan a todas las imágenes de la capa. Grupo "Image Settings": Brightness, Contrast, Gamma (1.0 normal; menor aclara medios tonos, mayor oscurece), Enhance (máscara de desenfoque / unsharp mask), Enhance Radius (extensión del efecto), Enhance Amount (intensidad), Enhance Denoise (reduce ruido en zonas suaves). Por imagen, no por capa.
- Invert Display: simula marcar sobre material oscuro; solo afecta la vista previa.
- Presets: Basic y "Black Paint on White"; guardar con interruptores por ajuste, borrar, importar/exportar (también en User Bundle).
- Rangos y valores por defecto: no publicados (no verificado). Quejas de bugs: Enhance Radius decrece al reabrir la ventana, gamma se fija en 3.000 (Q11).
- Orden de pipeline sugerido: color -> escala de grises (luma) -> brillo/contraste -> gamma -> unsharp/denoise -> remuestreo a DPI de capa -> negativo -> dither.

### F52 Scanning Offset Adjustment

Fuentes: S-ScanOff, S-DevBasic.

- Problema: el láser tarda en encenderse (microsegundos a ms) y las correas se estiran, así que a alta velocidad las líneas de ida y vuelta quedan desplazadas (bordes fantasma). Ejemplo: 1 ms a 500 mm/s son unos 0.5 mm; a 1000 mm/s el sesgo en un cuadrado de 20 mm puede alcanzar ~1 mm.
- Procedimiento de calibración: rectángulo de 50 x 10 mm en Fill, intervalo 0.5 mm, overscan >=5% en GCode (Ruida lo hace solo), correr a >=2 velocidades, medir el hueco entre extremos de línea.
- Tabla: Speed, Line Shift (la mitad del hueco medido, porque cada línea se mueve esa cantidad), Initial Offset (desplaza todo el grabado, normalmente innecesario). El software calcula valores para velocidades no medidas (método de interpolación no publicado, proponemos lineal).
- Habilitar con "Enable Scanning Offset Adjustment"; soporta segunda fuente (Laser 2); importa/exporta archivos `.lbso`; con Ruida compatible hay botones Read/Write.
- Ubicación: pestaña Basic Settings de Device Settings.
- Salida: al emitir tramos de ida y vuelta, desplazar en X las líneas en sentido contrario `+/- shift(v)` (inferencia). El "Initial Offset" se aplica a todas.
- Distinción: oscilación ("wobble") de líneas por exceso de aceleración se corrige bajando aceleración, no con este ajuste.

### F53 Fast Whitespace Scan

Fuente: S-DevBasic, S-JobOpt.

- Si el valor es mayor que la velocidad de grabado, acelera el viaje por zonas en blanco, en imagen y relleno. Solo GCode. En Marlin (G0 = G1) este valor define la velocidad de rápido.
- Salida: tramos vacíos con `G0` o `G1 S0 F<fast>`.

### F54 Material Library

Fuente: S-MatLib; foros (Q10).

- Biblioteca de presets: nombre de material (carpeta), espesor (subcarpeta) o "No Thickness" + título (para grabado/marcado), descripción. "Create new from layer" guarda los ajustes (incluidas sub-capas).
- Assign: copia ajustes a la capa; capa independiente. Link: la capa queda sincronizada con el preset, pero ya no se edita en el Cut Settings Editor (útil para archivos recurrentes).
- Edit Cut, Update (reemplaza con la capa actual), Edit Description, Duplicate, Delete.
- Archivos de biblioteca `.clb` (Load/Save/Save As/New/Merge/Rename/Unload). Una biblioteca por láser; se asocia al dispositivo activo y se carga al seleccionarlo; al cambiar de biblioteca se re-vinculan capas Linked por nombre+espesor+descripción coincidentes. Se puede compartir por red/nube (con riesgos, Q10).
- No hay importar/exportar preset individual (solo bibliotecas completas, según la página).
- Para nuestro formato: JSON/TOML versionado, con unidades explícitas (mm/s siempre internamente).

### F55 Material Test

Fuente: S-MatTest, S-IntTest.

- Genera una rejilla de rectángulos; por defecto 10 x 10; dos parámetros varían por eje (Power, Speed, Interval, Passes, y Frequency/Q-Pulse si el láser lo soporta); los demás ajustes (base) valen para todas las celdas.
- Campos: Count, Param, Min, Max, Height/Width, X/Y Center (solo en coordenadas absolutas). Etiquetas de texto y borde (Border desde 1.5) con su propio editor de ajustes y toggles.
- Posición de salida según el modo "Start From" elegido antes de abrirlo. Galvo siempre en absolutas.
- Orden de ejecución: de menor a mayor riesgo de quemar (más velocidad, menos potencia, menos intervalo y menos pasadas primero).
- Presets propios (guardar, borrar, importar/exportar; archivo `material_test_presets.lbmt` en carpeta de preferencias), 4 incluidos.
- Acciones: Preview, Frame, Start/Pause/Stop, Save RD/GCode, Send.
- Espaciado de valores entre Min y Max no declarado (suponer lineal; (no verificado)).
- Implementación: es una función generadora de proyecto (capas + formas + textos); reutiliza el motor normal. Fácil y de gran valor.

### F57-F58 Preview y estimación de tiempo

Fuentes: S-Preview, S-DevAdd, S-TimeEst, S-Overscan; foro `https://forum.lightburnsoftware.com/t/time-estimates-way-way-off-no-other-post-suggestions-helped/192261`.

Comportamiento del Preview:
- Simula el trabajo tal como se enviará: cortes en negro, viajes en rojo. Se abre con la barra de herramientas, Alt/Option+P o menú. Slider temporal (arrastrar para ver el orden), Play con velocidad de 0.1x a 40x del tiempo real.
- Opciones (solo visuales): Show Traversal Moves, Shade According to Power (escala de grises; funciona con Grayscale, Power Scale o Ramp), Invert (blanco sobre negro), Show Legend.
- Start Here: reanudar un trabajo desde el punto del slider; Save Image: exportar la imagen del preview.
- Zoom con rueda/+/-; pan con botón central o Espacio+arrastre; borde verde = área de la cama.
- Estadísticas: distancia de corte con tiempo, movimientos rápidos con tiempo, y tiempo total.

Estimación de tiempo:
- Se basa en parámetros de Device Settings > Additional Settings (no afectan la salida real): velocidad máxima X/Y, aceleración de corte y de grabado X/Y por separado, Corner Accuracy, Minimum Corner Speed, Cut Speed Scale y Global Speed Scale. Botón "Read From Controller" (GRBL, Ruida) sincroniza casi todo salvo Corner Accuracy y las escalas.
- Modelo recomendado: perfil trapezoidal por segmento con limitación por aceleración por eje y velocidad de cruce de esquinas por "junction deviation" (el nombre "Corner Accuracy" sugiere ese modelo, (inferencia)). Las líneas de barrido cortas con overscan dominan el tiempo, por lo que el overscan debe incluirse en el cómputo.
- Queja: las estimaciones de imagen salen 1.5x-2x más cortas que lo real (Q7).

---

## 3. Algoritmos de dithering y procesado de imagen (referencias públicas para implementación desde cero)

Importante: los kernels son datos matemáticos publicados en literatura, no código de LightBurn. LightBurn no documenta su implementación (serpentina, gamma, clamp); eso es un detalle a decidir.

### 3.1 Esquema general de difusión de error

Para cada píxel en orden de barrido (izq->der, arriba->abajo, o serpentina), con `v` = valor de gris (0..255) más el error acumulado:
1. `q = v < 128 ? 0 : 255` (umbral; para imágenes láser, 255 = no marcar si luz = blanco, o al revés según convención).
2. `e = v - q`.
3. Distribuir `e * w_i / divisor` a los vecinos pendientes según el kernel.
Usar buffers float o i16, con clamp opcional. Para evitar vetas ("tendrils") en zonas muy planas, algunas implementaciones usan serpentina y/o un pequeño ruido; esto responde a la advertencia de la propia documentación (S-GitImg) sobre errores que se acumulan en áreas sólidas.

Kernels (posición `*` = píxel actual; filas hacia abajo; offsets en columnas):

**Floyd-Steinberg** (divisor 16). Publicación: R. W. Floyd y L. Steinberg, "An adaptive algorithm for spatial grey scale", Proceedings of the Society for Information Display 17(2), 1976.
```
        *   7
    3   5   1
```

**Jarvis-Judice-Ninke** (divisor 48). Publicación: J. F. Jarvis, C. N. Judice, W. H. Ninke, "A survey of techniques for the display of continuous tone pictures on bilevel displays", Computer Graphics and Image Processing 5(1), 1976.
```
            *   7   5
    3   5   7   5   3
    1   3   5   3   1
```

**Stucki** (divisor 42). Publicación: P. Stucki, "MECCA - a multiple-error correcting computation algorithm for bilevel image hardcopy reproduction", IBM Research Report RZ1060, 1981.
```
            *   8   4
    2   4   8   4   2
    1   2   4   2   1
```

**Atkinson** (divisor 8; solo se propaga 6/8 del error, el resto se descarta). Origen: Bill Atkinson, algoritmo usado en el Macintosh original (MacPaint), sin paper original; documentado en retrospectivas y descripciones públicas (cita exacta no verificada).
```
        *   1   1
    1   1   1
        1
```

**Ordered / Bayer**. Publicación: B. E. Bayer, "An optimum method for two-level rendition of continuous-tone pictures", IEEE International Conference on Communications, 1973. Matriz recursiva: `M(2n) = [[4*M(n)+0, 4*M(n)+2],[4*M(n)+3, 4*M(n)+1]]` con `M(1)=[[0]]`; umbral por píxel = `(M[y mod n][x mod n] + 0.5) / n^2 * 255`. Tamaños típicos 2x2, 4x4, 8x8. Cuál usa LightBurn: no documentado (no verificado).

Otros kernels públicos opcionales para extender el catálogo: Sierra, Burkes (también en literatura); no aparecen en LightBurn.

Referencia de conjunto: R. Ulichney, "Digital Halftoning", MIT Press, 1987 (cubre dither ordenado, difusión de error, halftones de agrupamiento de puntos y ruido azul).

### 3.2 Threshold
`out = gray >= T`. LightBurn no muestra el umbral en la documentación consultada (no verificado si es fijo en 128). En nuestro caso: exponer umbral.

### 3.3 Halftone (celdas por pulgada + ángulo) y Newsprint
- Documentado: Halftone tiene `Cells per Inch` y `Halftone Angle`; Newsprint es rejilla fija con puntos que crecen en las zonas oscuras (S-Image).
- Algoritmo clásico (Ulichney): rotar coordenadas del píxel por el ángulo, calcular posición dentro de la celda `(u,v)` en [-0.5,0.5], umbral = función de espectro de punto (p. ej. `cos(pi u) * cos(pi v)` o distancia al centro) y comparar con el gris. El tamaño de punto crece con la oscuridad. La forma exacta del punto en LightBurn no es pública (no verificado).

### 3.4 Sketch
Documentado solo como "engrava únicamente donde claro y oscuro se encuentran en bordes duros". Algoritmo interno no público (no verificado). Propuesta: gradiente Sobel o diferencia de gaussianas, umbralizar la magnitud, adelgazar opcionalmente. Disponibles en `imageproc` (`gradients::sobel_gradients`, `edges::canny`).

### 3.5 Grayscale (potencia variable)
`S = S_min + (1 - L/255) * (S_max - S_min)` con L = luma 0..255 (oscuro = más potencia). Con "Negative Image", se invierte. Requiere que el controlador mantenga la velocidad (M4) para que la potencia sea proporcional al nivel de gris. Al ser lineal, conviene permitir una curva de gamma de potencia -> energía.

### 3.6 3D Slice (galvo)
Mecánica deducida del foro (staff "Oz") `https://forum.lightburnsoftware.com/t/3d-slice-algorithim/158259`, S-3D:
- Dado N = número de pasadas, el umbral desciende de 255 a 0 en pasos iguales; en la pasada k, los píxeles con valor < umbral_k se graban (valor >= umbral se salta). Blanco puro nunca se graba; negro puro se graba en todas.
- Mapa de profundidad esperado a rango completo 0-255, 8 bits (soporte de 16 bits en la lista de planes del staff). Con N>256 se repiten cortes.
- Opciones: Negative Image invierte; pasadas de limpieza con sub-capa aparte (intervalo más fino, mayor frecuencia, potencia más baja); el preview muestra capas de abajo hacia arriba.
- Aplicable sin galvo con pocos cambios (proyecto GRBL con diodo lo reproduce con N pasadas de Threshold), aunque LightBurn lo limita a galvo.

### 3.7 Dot width / reducción de DPI
Antes de dither se remuestrea a la resolución objetivo (`DPI = 25.4 / interval`). Para calidad, usar Lanczos3 o área (en Rust: `image::imageops::resize` con `FilterType::Lanczos3`, o `fast_image_resize`). Hacerlo siempre antes del dither, y nunca después (LightBurn indica que pass-through evita este paso).

### 3.8 Ramp (modo Threshold)
Pendiente de potencia cerca de bordes: la documentación solo dice que "inclina los lados del grabado variando la potencia" a lo largo de una distancia desde el diseño; algoritmo no publicado (no verificado). Propuesta: transformada de distancia (`imageproc::distance_transform`) sobre el binario; potencia = f(distancia / ramp_length).

---

## 4. Detalle de P2 (resumen breve)

Para completar el inventario, sin análisis profundo:

- F15 Lead-in/out (S-Line): línea o arco al inicio o al final; ángulo positivo = afuera, negativo = adentro; longitud y estilo; mismos valores para toda la capa; nodos agudos (<90 grados) pueden fallar en lead-in interno.
- F16 Overcut (S-Line): extiende el cierre de una forma cerrada, solo en la última pasada.
- F17 Start/End pause (S-Line): milisegundos al inicio/fin; Cut Through mantiene el haz encendido (potencia propia). No disponible en TopWisdom; sin Cut Through no disponible en Trocen.
- F18 Dot mode (S-Line, solo Ruida): pausa y pulso a intervalos; Time en ms y Spacing.
- F20 Ramp length (S-Fill, S-Image): ver 3.8; en DSP, Min Power debe quedar justo sobre el umbral de disparo.
- F28 Flood fill (S-Fill, S-JobOpt): calcula una ruta que reduce/elimina viajes sobre espacios vacíos; muy sensible a backlash/afinación; puede cambiar de dirección y partir formas en segmentos (Q2).
- F34 Hide backlash (S-Optim): ver F32.
- F43-F45 Newsprint/Halftone/Sketch: ver 3.
- F47 3D Slice: ver 3.6.
- F50 Angle increment: rota ángulo entre pasadas (imagen) o en galvo acumulando (Auto-Rotate, S-Galvo).
- F56 Interval Test (S-IntTest): cuadros con intervalos entre Min y Max (sugerencia de la doc: 0.08 a 0.16 mm), relleno simple o degradado; el mejor es donde las líneas apenas se tocan.
- F59 PWM freq (Ruida), PPI (Trocen), U offset (Ruida), Frequency/Q-Pulse (galvo), Wobble, timings (Jump, Laser On/Off TC, End TC, Polygon TC), Global Passes, Laser 2 (S-Galvo, S-Fill, S-Line). Específicos del controlador; ignorar para MVP.
- F60 Beginner mode: oculta opciones (p. ej. tabs) (S-Tabs, S-Shared).

---

## 5. Quejas frecuentes del foro (oportunidades de mejora)

Cada queja cita hilos revisados (resumen en español, sin texto copiado).

| # | Queja | Fuente | Oportunidad |
|---|---|---|---|
| Q1 | Offset Fill cuelga o cierra la app en diseños complejos/intervalos pequeños; Cancelar no siempre funciona; fugas de memoria; "Removing overlapping lines" también se cuelga | `https://forum.lightburnsoftware.com/t/offset-fill-causing-the-software-to-lock-up/94887`, `https://forum.lightburnsoftware.com/t/offset-fill-freezing-lightburn/116179`, `https://forum.lightburnsoftware.com/t/offset-fill-in-1-17-01-crashes-lb/153022` | Cálculo en hilo de trabajo con cancelación real, límites y barra de progreso; simplificar nodos antes; offsets incrementales en lugar de recomputar |
| Q2 | Flood fill deja huecos/líneas perdidas por holgura mecánica | `https://forum.lightburnsoftware.com/t/missed-lines-when-using-floodfill/191170`, `https://forum.lightburnsoftware.com/t/my-laser-creat-gaps-with-flood-fill/83423` | Avisar de la sensibilidad, ofrecer compensación de backlash por dirección, preview que marque cambios de dirección |
| Q3 | La estimación de tiempo se queda corta con imágenes (1.5x a 2x), el tiempo desaparece al terminar el trabajo, no se entiende para qué sirven las escalas | `https://forum.lightburnsoftware.com/t/time-estimates-way-way-off-no-other-post-suggestions-helped/192261` | Modelo de aceleración por eje + calibración automática con un trabajo de prueba; mantener el registro de tiempo real vs. estimado |
| Q4 | "Cut inner shapes first" no se respeta: lo pisan Order by Layer/Group/Priority, formas abiertas, sub-capas, prioridades corruptas por importar DXF, "Reduce travel moves" | `https://forum.lightburnsoftware.com/t/cut-inner-shapes-first-not-functional/91728`, `https://forum.lightburnsoftware.com/t/laser-not-cutting-inner-shapes-first/138349`, `https://forum.lightburnsoftware.com/t/cut-order-changing-each-time-i-preview-or-cut/25522` | Explicar el orden resultante (por qué esta pieza va aquí); reglas determinísticas y comprobables; avisar si hay formas abiertas |
| Q5 | Kerf offset ambiguo: signo, dirección en agujeros vs. exteriores, formas anidadas, Break Apart, y un reporte de signo invertido en Ruida 2.0.01 | `https://forum.lightburnsoftware.com/t/kerf-offset-clarification/162137`, `https://forum.lightburnsoftware.com/t/lightburn-miscalculation-for-kerf-offset/130516`, `https://forum.lightburnsoftware.com/t/updated-lightburn-2-0-01-positive-kerf-shrinks-negative-expands-ruida-controller/175588` | Clasificación explícita pieza/agujero (nesting con regla par-impar, visible en UI), vista previa del offset aplicado |
| Q6 | Tabs: preview de edición no muestra kerf; en auto, mínimo de 3 tabs y espaciado grande se ignora; salida difiere de la edición | `https://forum.lightburnsoftware.com/t/still-big-problems-with-tabs-bridges/111930` | Preview que muestre exactamente las tabs finales; documentar reglas; tabs manuales prioritarias |
| Q7 | Material Test: presets no cargan tras actualizaciones | `https://forum.lightburnsoftware.com/t/lb-1-7-01-material-test-not-loading-saved-presets/152993` | Formato versionado con migraciones y pruebas de regresión |
| Q8 | Grayscale en GRBL con diodo muestra poca diferencia entre niveles; confusión con S-max y PWM | `https://forum.lightburnsoftware.com/t/grayscale-issues/6161` | Asistente de calibración de curva potencia/oscuridad (rejilla de grises); leer `$30` automáticamente |
| Q9 | Alarma fuera de límites por overscan (ALARM:2); el framing no simula overscan | `https://forum.lightburnsoftware.com/t/this-is-the-alarm-signal-alarm-2/118048`, `https://forum.lightburnsoftware.com/t/alarm-2-out-of-bounds/42330`, `https://docs.lightburnsoftware.com/2.1/Troubleshooting/PreviewWindow/ExcessiveOverscanning/` | Frame que incluya el overscan; avisar antes de enviar y sugerir la corrección concreta |
| Q10 | La biblioteca de materiales "desaparece" tras actualizar, rotura con sincronización en la nube, cambio de unidades mm/s <-> mm/min parece cambiar valores | `https://forum.lightburnsoftware.com/t/2-1-03-updates-wiped-my-material-library-settings/191032`, `https://forum.lightburnsoftware.com/t/watch-the-new-update-it-cleared-out-my-art-and-material-library/163955`, `https://forum.lightburnsoftware.com/t/material-and-art-library-lost/121297` | Almacenar siempre en unidades internas (mm/s) y mostrar la unidad; backups automáticos; formato abierto; soportar carpetas compartidas con bloqueo |
| Q11 | Bugs en Adjust Image: Enhance Radius decrece en cada apertura, gamma se pone en 3.000 | `https://forum.lightburnsoftware.com/t/adjust-image-enhance-radius-enhance-amount-function-not-working/147055`, `https://forum.lightburnsoftware.com/t/adjust-image-is-setting-gamma-value-to-3-000/38955` | Pipeline puro y estado inmutable; pruebas de ida y vuelta |
| Q12 | Pass-through confuso: sigue mostrando selector de modo / selecciona threshold | `https://forum.lightburnsoftware.com/t/pass-through-still-enables-the-image-mode/42325`, `https://forum.lightburnsoftware.com/t/why-is-lightburn-selecting-threshold-when-pass-through-is-selected/60632` | Deshabilitar controles incompatibles con indicación clara |
| Q13 | Rellenos con resultados inesperados al solapar formas o con formas abiertas | `https://docs.lightburnsoftware.com/2.1/Explainers/LayerModes/` (reglas documentadas); hilos de foro sobre "welded object missing output" | Resaltar formas abiertas y mostrar el resultado par-impar |
| Q14 | 3D Slice: asume rango completo 0-255 de 8 bits, artefactos con mapas de profundidad discretos, sin 16 bits | `https://forum.lightburnsoftware.com/t/3d-slice-algorithim/158259`, `https://forum.lightburnsoftware.com/t/16-bit-height-map-are-more-than-256-3d-sliced-layers-useful/166298` | Aceptar PNG de 16 bits; normalizar histograma opcional |
| Q15 | Hallazgo propio de documentación: la interacción de sub-capas con Pass count no se describe; rangos/unidades de muchos campos no se publican | S-Sub, S-Line | Documentar con precisión; mostrar rangos válidos del dispositivo en la UI |
| Q16 | Scanning Offset: sensación de que está escondida (Device Settings, no en capa) y necesita calibración manual con regla | `https://forum.lightburnsoftware.com/t/where-did-scanning-offset-go/106795`, `https://forum.lightburnsoftware.com/t/lightburn-scanning-offset-adjustment-diode-laser-grbl/104515` | Asistente guiado: genera el patrón y mide con la cámara o permite teclear lectura por velocidad; el ajuste debe ser visible en la capa |

Nota: las afirmaciones de cada queja provienen de resúmenes de las páginas de búsqueda/lectura; algunos hilos están cerrados sin causa confirmada.

---

## 6. Recomendaciones para el MVP (P0) y dependencias

1. Modelo de datos: `Layer { color, name, output, show, air, sublayers: Vec<SubLayer> }`, `SubLayer { mode, speed, power_min, power_max, passes, params por modo }`. Unidades internas en mm y mm/s.
2. Motor de geometría: aplanado de curvas, regla par-impar, scanline fill, offset (kerf/offset fill). Crates a evaluar: `i_overlay`, `clipper2`, `cavalier_contours`, `geo`, `kurbo`.
3. Motor de imagen: `image` + `imageproc`; implementar dither propio (kernels de sec. 3).
4. Cut planner: orden por capa/grupo/prioridad, interiores primero, NN+2-opt, elección de punto de inicio y sentido.
5. Emisor G-code GRBL (M4/M3, S, F, overscan, bidireccional, serpentina, offset de línea); streaming con control de buffer.
6. Preview + simulador de tiempo con modelo trapezoidal por eje.
7. Dejar para después: sub-capas completas, tabs, Material Library/Test, scanning offset (P1), y todo lo específico de Ruida/galvo (P2).

## 7. Cosas no verificadas en esta investigación (resumen)

- Fórmula exacta del overscan (porcentaje de velocidad -> distancia).
- Algoritmos internos de Sketch, Newsprint, Halftone, Ramp, Hide backlash, auto-tabs; kernels concretos de "Ordered" y "Dither".
- Rangos y valores por defecto de la mayoría de campos (potencia, kerf, lead-in, Z step, etc.).
- Número de colores de la paleta de capas.
- Formato interno de `.lbrn2`, `.clb`, `.lbso`, `.lbmt` (solo se conocen extensiones y propósitos).
- Licencias de proyectos OSS citados, y detalles de sus implementaciones (LaserGRBL, MeerK40t, Rayforge, Inkscape); se citan como punto de partida para estudio.
