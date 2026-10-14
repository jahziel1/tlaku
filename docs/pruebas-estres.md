# Pruebas de estrés con archivos reales

Fecha de la corrida: 2026-10-13. Equipo: macOS (Apple Silicon), 16 GB de RAM, compilación `--release`.
Herramienta: `crates/tlaku-bench` (motor) y `apps/desktop/scripts/canvas-stress.mjs` (lienzo).

## Resumen

- **Ningún pánico, ningún cuelgue y ningún cierre inesperado** en 178 ejecuciones de importación a
  emulador (165 archivos reales y 13 sintéticos), más 4 archivos extremos. Los 22 fallos son errores
  controlados con un mensaje; ver [Fallos](#fallos).
- **El láser nunca se enciende en un movimiento rápido** y nunca se queda quieto encendido, en los 131
  trabajos que pasaron por el emulador GRBL. La longitud quemada coincide con la del trayecto
  calculado con menos de 1 % de diferencia en todos.
- **Lo que hoy es lento es un solo paso: "interior primero" del planificador.** Cuatro archivos tardan
  más de 10 s en calcular el trabajo (hasta 29,5 s) y apagar esa opción lo baja a 0,1 a 1,7 s (18 a 100 veces menos). Con 1 millón de segmentos sueltos, el culpable es "reducir recorridos en vacío"
  (40 s contra 1 s).
- **La memoria es el otro límite:** una imagen de 100 megapíxeles necesita de 3,4 a 4,8 GB de RAM; solo
  importarla ya cuesta 2,3 GB (se decodifica entera para leer su tamaño).
- **El lienzo aguanta bien:** 50 000 objetos se pintan en 0,3 s y se mueven a unos 7 cuadros por
  segundo; un solo trayecto de 1 millón de nodos se pinta en 0,5 s pero un clic cuesta 0,3 s y arrastrar
  0,37 s por cuadro. Detalle en [Lienzo](#lienzo).
- **El tiempo estimado se parece al simulado:** mediana +1,2 %, 93 de 115 trabajos dentro de ±5 %, pero
  hay valores de hasta +26 % y -40 % con segmentos muy cortos o velocidades por encima de la máquina.

## Método

### Herramienta del motor: `tlaku-bench`

```text
cargo build --release -p tlaku-bench
tlaku-bench gen DIR [--xl]                 # archivos sintéticos (o los extremos con --xl)
tlaku-bench run CARPETA... --csv o.csv --summary o.md [--modes line,fill] [--threshold 10]
tlaku-bench worker ARCHIVO [--mode line|fill] [--no-emu]   # un archivo en este proceso (JSON por fase)
```

`run` lanza **un proceso por archivo y modo** (`worker`). Así un pánico, un desborde de pila o una falta
de memoria no tumban la corrida, se puede matar un proceso que pasa del plazo duro (300 s) y la memoria
de cada archivo se mide limpia. El trabajador imprime una línea JSON acumulada al terminar cada fase:
si el proceso muere, el padre sabe en qué fase quedó. Estados: `ok`, `error` (la fase devolvió un
error), `panic` (pánico capturado), `crash` (el proceso murió por una señal) y `timeout`. Además marca
como lenta cualquier fase de más de 10 s (importar, calcular, G-code, `.rd`; el emulador va aparte porque
su duración es el tiempo simulado).

Fases, con la API que usa la app:

1. **Importar:** `tlaku_pipeline::import_file` (SVG, DXF, PDF, AI, HPGL, G-code, imágenes, `.lbrn`
   y `.lbrn2`). Se cuentan objetos, trayectos (`BezPath`), subtrayectos, nodos y avisos.
2. **Calcular:** `compute` con el perfil por defecto (`DeviceProfile::default()`: GRBL de diodo,
   cama de 400 x 415 mm, 1 000 mm/s² y 12 000 mm/min) y `JobOptions::default()`. Los ajustes de capa
   son los que crea el importador (en `.lbrn` los del proyecto). Se anotan el tiempo estimado
   (`preview.total_s`), los movimientos, la longitud de corte y los avisos.
3. **G-code:** `gcode()`; se anotan bytes y líneas.
4. **Ruida:** `export_rd_for_device()`; se anotan bytes.
5. **Emulador:** el G-code se envía con el `Streamer` real a una `VirtualMachine` GRBL 1.1h con
   `ScaledClock`. Se comprueba con el rastro (`LaserTrace`) que no hay tramos quemados en rápidos
   (`burned_during_rapids`) ni quemaduras con la cabeza quieta (`stationary_burns`), que la longitud
   quemada coincide con la longitud de los `Cut` con potencia del trabajo, y se compara la duración
   simulada con la estimada.

Detalles que importan para leer los números:

- **Ajuste a la cama.** Si el diseño no cabe en 400 x 415 mm (casi todos los mapas y todas las
  imágenes), se escala y se traslada al origen antes de calcular (`fit_scale` en el CSV). Sin eso
  `compute` falla por la cama y no se mediría nada. En una imagen el escalado la deja de unos 400 mm,
  pero los píxeles son los mismos.
- **Memoria.** Es el máximo residente del proceso (`getrusage`), que solo sube: cada columna
  `*_rss_mb` es el pico acumulado hasta el final de esa fase.
- **Reloj del emulador.** El tiempo simulado corre `speed` veces más rápido que el real. Se elige
  `speed = min(estimado / 8 s, tope según líneas por segundo simulado, 400)`. El tope existe porque el
  emulador vive en el mismo proceso: si el anfitrión no alcanza a darle líneas, el planificador se
  vacía y el tiempo simulado se infla. Se mide con `emu_starved_pct` (fracción de muestras con el
  planificador vacío mientras quedaban líneas por enviar); en los trabajos de 20 s o más es 0 a 1 %.
  La duración simulada se lee al terminar el envío con sondeo `?` cada 1 ms reales, así que lleva un
  sesgo de hasta `speed` ms por encima: de ahí buena parte del +1 % de mediana.
- El emulador se salta los trabajos de más de 250 000 líneas de G-code (tardaría minutos por
  archivo); 28 de 178 ejecuciones quedaron fuera por eso (imágenes, mapas grandes y los sintéticos más pesados).
- **Modo `fill`.** Pasa todas las capas vectoriales a relleno de 0,2 mm. Solo se corrió sin emulador:
  26 archivos no tienen formas cerradas ("nada que producir") y varios `.lbrn` se salen de la cama por
  el overscan; no son fallos.
- `TLAKU_BENCH_DISABLE=inner_first,reduce_travel,...` apaga optimizaciones del planificador para
  averiguar cuál domina el tiempo de cálculo.

### Lienzo

`apps/desktop/scripts/canvas-stress.mjs` (Chrome headless por CDP, igual que `screenshots.mjs`, sin
dependencias). Arranca su propio Vite en el puerto 1431 con `scripts/vite.stress.config.mjs` (sin
recarga en caliente, para que las ediciones de otra persona a mitad de una medición no reinicien la
página) y usa la app en modo mock. Carga el diseño con `replaceDocument` y mide, con eventos reales de
ratón y rueda:

- carga y primer cuadro;
- paneo (15 pasos), zoom con rueda (10 acercando y 10 alejando);
- 20 clics sobre bordes de objetos (comprueba que seleccionaron);
- rectángulos de selección (rincón de 20 mm, cuarto de cama, toda la cama);
- seleccionar todo y arrastrar 10 pasos, confirmar el movimiento, deshacer y `moveObjects` directo.

Cada tiempo es hasta que el navegador completa el siguiente cuadro (dos `requestAnimationFrame`), más
la suma de lo que tardó `Canvas2DRenderer.render()` (solo JavaScript). El **suelo del método es 32 ms**
(dos cuadros a 60 Hz sin trabajo), así que 32 ms significa "sin costo medible", 65 ms son cuatro cuadros,
etcétera. Chrome rasteriza por software (SwiftShader, como en `screenshots.mjs`): los tiempos de pintura
son pesimistas frente a una GPU real; los de JavaScript son representativos.

```text
cd apps/desktop && node scripts/canvas-stress.mjs [--quick | --only obj-100000]
```

## Corpus

178 archivos medidos: 165 reales y 13 sintéticos (`tlaku-bench gen`). Todo vive fuera del repo
(`scratchpad/corpus`); no se ejecutó nada de lo descargado, solo se importó con el motor.

| Categoría | Archivos | Fuentes (licencia) |
|---|---:|---|
| SVG | 69 | resvg (25 pruebas), Wikimedia Commons (32, mapas de hasta 26 MB, escudos, mandalas), OpenClipart (12, CC0) |
| DXF | 35 | ezdxf (MIT), dxf-viewer (MPL-2.0), LibreCAD (GPL, solo local) |
| PDF | 9 | Wikimedia Commons, pdf.js (Apache-2.0) |
| AI | 5 | Apache PDFBox y Tika (Apache-2.0) |
| Imágenes | 6 | Wikimedia Commons, de 8 744 x 9 502 a 18 681 x 18 681 px (JPEG, PNG, TIFF) |
| G-code | 17 | cncjs (MIT), LinuxCNC y Universal G-Code Sender (GPL, solo local), jsdc-laser, otros |
| HPGL | 5 | chiplotle (GPL, solo local), ImageMagick, fake_parallel_printer (BSD-2) |
| LightBurn | 19 | `.lbrn2` y `.lbrn` de repos con licencia verificada (MIT, Apache-2.0, CC0, Unlicense, CERN-OHL-P) |
| Sintéticos | 13 | `tlaku-bench gen` |

- La lista exacta con licencia, autor, URL de descarga, página y sha256 de cada archivo está en
  `scripts/fetch-corpus.sh`. El script descarga a `tests/corpus-downloaded/` (ignorado, nunca dentro de
  `tests/corpus/`), verifica los sha256 y escribe `LICENSES.tsv`.
- En el repo solo hay 11 archivos CC0 o de dominio público (1,4 MB) en `tests/corpus/`, con su
  `README.md` de licencias. Una prueba de `tlaku-bench` los pasa por el pipeline completo.
- Los de CC BY-SA, GPL y MPL se miden pero no se redistribuyen; ninguno entra al repo.
- Limitaciones: no hay ningún `.plt` real con licencia clara (los HPGL son `.hpgl`); los `.ai` son
  cuatro de PDFBox basados en PDF y uno PostScript antiguo; de G-code de láser solo hay cuatro
  archivos (el resto es fresado e impresión 3D).
- Sintéticos: 50 000 rectángulos; un trayecto de 1 048 576 nodos (curva de Hilbert); 120 líneas de
  texto denso (16 346 contornos); 300 grupos anidados con transformaciones y `clipPath`; 20 000 círculos;
  200 000 curvas Bézier; DXF de 100 000 `LINE`, 20 000 `LWPOLYLINE` con bulge y 5 000 `SPLINE`; G-code
  de 1 M de líneas; HPGL de 500 000 instrucciones; PNG de 10 000 x 8 000 y JPEG de 12 000 x 9 000 px.
  Extremos (`gen --xl`): trayecto de 4,2 M de nodos, 200 000 rectángulos, 1 M de `LINE` en un DXF y
  4,2 M de líneas de G-code.

## Resultados

### Tiempos y memoria (178 ejecuciones, modo línea, solo las `ok`)

| Fase | mediana | p90 | p99 | máximo |
|---|---:|---:|---:|---:|
| Importar | 0,02 s | 0,21 s | 1,21 s | 2,20 s |
| Calcular | 0,01 s | 1,02 s | 11,2 s | 29,5 s |
| G-code | 0,01 s | 0,68 s | 4,70 s | 6,54 s |
| `.rd` | 0,00 s | 0,04 s | 0,35 s | 0,43 s |
| RAM pico | 19 MB | 637 MB | 4 206 MB | 4 772 MB |

Mayores en cada fase:

- **Calcular:** `wmc-amhara-topographic-map.svg` 29,5 s (35 413 trayectos); `wmc-cascadia-scenario-median.pdf`
  15,5 s (596 trayectos, 313 mil nodos); `wmc-hooke-bluefly-vector-trace.svg` 11,2 s (28 742 trayectos);
  `wmc-germany-districts-municipalities-map.svg` 10,7 s (20 841 trayectos, 4,3 M de nodos).
- **Importar:** `wmc-prc-national-emblem.svg` 2,8 s; `wmc-cia-factbook-political-world.pdf` 2,2 s
  (29 606 trayectos, 2,3 M de nodos); las imágenes de 60 a 100 Mpx 1,0 a 1,3 s.
- **G-code:** 6,5 s para la imagen de 83 Mpx (8,9 M de movimientos, 139 MB de G-code).
- **Memoria:** 4,8 GB (JPEG sintético de 108 Mpx), 4,3 GB (PNG de 100 Mpx), 4,0 GB (JPEG de 83 Mpx),
  2,9 GB (PDF del mundo, 2,3 M de nodos), 2,4 GB (mapa de Alemania, 4,3 M de nodos).

### Archivos extremos (`gen --xl`, una vez cada uno, con el emulador desactivado por tamaño)

| Archivo | Importar | Calcular | G-code | RAM pico |
|---|---:|---:|---:|---:|
| 1 M de `LINE` sueltas (DXF 74 MB) | 2,1 s | **39,1 s** | 1,3 s | 1 806 MB (todo en el import) |
| Trayecto de 4,2 M de nodos (SVG 65 MB) | 1,2 s | 1,1 s | 2,7 s | 2 426 MB |
| 4,2 M de líneas de G-code (107 MB) | 1,8 s | 1,0 s | 2,8 s | 2 558 MB |
| 200 000 rectángulos (SVG 11 MB) | 0,5 s | 0,8 s | 0,6 s | 935 MB |

## Tabla por archivo (modo línea)

Columnas: tamaño del archivo, estado, tiempo de importar, trayectos y nodos importados, tiempo de
calcular, tiempo estimado del trabajo (`estimado`), movimientos, tamaño del G-code, tiempo de exportar
`.rd`, RAM pico y diferencia entre la duración simulada en el emulador y la estimada (`(corto)`: menos
de 20 s estimados, donde el sesgo de medición pesa; `omitido`: más de 250 000 líneas). Los tiempos
estimados son largos porque las capas por defecto cortan a 1 000 mm/min. El CSV completo está en
[pruebas-estres-resultados.csv](pruebas-estres-resultados.csv) (más columnas: avisos, ruido de relleno,
bytes y líneas de G-code, memoria por fase, quemaduras en rápidos, etcétera).

| archivo | MB | estado | import s | trayectos | nodos | cálculo s | estimado | movs | G-code MB | .rd s | RAM pico MB | sim vs est |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| **svg** | | | | | | | | | | | | |
| ocl-10057-bielskobiala-coat-of-arms.svg | 0.41 | ok | 0.02 | 610 | 30591 | 0.03 | 5 min | 32131 | 0.6 | 0.00 | 47 | +1.7 % |
| ocl-10062-gdansk-coat-of-arms.svg | 0.20 | ok | 0.01 | 526 | 4459 | 0.09 | 15 min | 24068 | 0.5 | 0.00 | 31 | +1.2 % |
| ocl-123043-mandala.svg | 0.11 | ok | 0.01 | 120 | 1579 | 0.03 | 12 min | 24529 | 0.5 | 0.00 | 31 | +2.4 % |
| ocl-126205-simple-green-mandala.svg | 0.10 | ok | 0.03 | 378 | 3040 | 0.06 | 16 min | 11120 | 0.2 | 0.00 | 20 | +0.0 % |
| ocl-153361-london-skyline-20.svg | 0.11 | ok | 0.02 | 256 | 2321 | 0.01 | 5 min | 5620 | 0.1 | 0.00 | 14 | +0.7 % |
| ocl-168169-mandala-yellow.svg | 0.34 | ok | 0.02 | 2700 | 15855 | 0.25 | 32 min | 35648 | 0.7 | 0.00 | 50 | +0.3 % |
| ocl-169694-wonderful-celtic-dragons-frame.svg | 0.22 | ok | 0.01 | 1283 | 8770 | 0.25 | 21 min | 59337 | 1.2 | 0.00 | 61 | +2.5 % |
| ocl-17415-coat-of-arms-gilman-1.svg | 0.37 | ok | 0.01 | 1046 | 14083 | 0.50 | 44 min | 119026 | 2.4 | 0.01 | 97 | +2.9 % |
| ocl-17417-coat-of-arms-gilman-2.svg | 0.68 | ok | 0.02 | 2710 | 28556 | 1.38 | 68 min | 206410 | 4.2 | 0.01 | 149 | +4.0 % |
| ocl-175200-weaving-iris-mandala.svg | 0.38 | ok | 0.02 | 1836 | 7344 | 0.04 | 90 min | 9180 | 0.2 | 0.00 | 21 | -0.2 % |
| ocl-175206-108-eyes-mandala.svg | 0.11 | ok | 0.04 | 324 | 1512 | 0.01 | 15 min | 10613 | 0.2 | 0.00 | 19 | +1.4 % |
| ocl-176911-mandala-flames.svg | 0.24 | ok | 0.02 | 788 | 8423 | 0.04 | 36 min | 44700 | 0.8 | 0.00 | 48 | +2.8 % |
| resvg-filters_feTurbulence_stitchTiles-stitch.svg | 0.00 | ok | 0.01 | 2 | 8 | 0.00 | 0 min | 10 | 0.0 | 0.00 | 9 | (corto) |
| resvg-filters_filter-functions_color-adjust-functions-default-value.svg | 0.00 | ok | 0.02 | 86 | 1684 | 0.00 | 1 min | 2461 | 0.0 | 0.00 | 13 | +6.8 % |
| resvg-masking_clipPath_clip-path-on-children.svg | 0.00 | ok | 0.02 | 2 | 34 | 0.00 | 0 min | 36 | 0.0 | 0.00 | 10 | +1.1 % |
| resvg-masking_clip_simple-case.svg | 0.00 | ok | 0.01 | 1 | 4 | 0.00 | 0 min | 5 | 0.0 | 0.00 | 9 | (corto) |
| resvg-masking_mask_recursive-on-child.svg | 0.00 | ok | 0.02 | 3 | 12 | 0.00 | 0 min | 15 | 0.0 | 0.00 | 9 | +1.4 % |
| resvg-masking_mask_with-image.svg | 0.01 | ok | 0.02 | 2 | 8 | 0.00 | 0 min | 10 | 0.0 | 0.00 | 9 | +0.5 % |
| resvg-paint-servers_linearGradient_gradientTransform-via-href.svg | 0.00 | ok | 0.02 | 5 | 20 | 0.00 | 1 min | 25 | 0.0 | 0.00 | 9 | +1.6 % |
| resvg-paint-servers_pattern_patternTransform-via-href.svg | 0.00 | ok | 0.01 | 5 | 20 | 0.00 | 1 min | 25 | 0.0 | 0.00 | 9 | +1.6 % |
| resvg-painting_context_with-pattern-objectBoundingBox-in-use.svg | 0.00 | ok | 0.03 | 4 | 18 | 0.00 | 1 min | 22 | 0.0 | 0.00 | 9 | +0.6 % |
| resvg-painting_marker_nested.svg | 0.00 | ok | 0.02 | 27 | 104 | 0.00 | 1 min | 130 | 0.0 | 0.00 | 10 | -0.6 % |
| resvg-painting_marker_target-with-subpaths-2.svg | 0.00 | ok | 0.02 | 13 | 44 | 0.00 | 1 min | 56 | 0.0 | 0.00 | 10 | -0.4 % |
| resvg-painting_stroke-dasharray_negative-values.svg | 0.00 | ok | 0.02 | 2 | 8 | 0.00 | 0 min | 10 | 0.0 | 0.00 | 9 | +0.7 % |
| resvg-shapes_path_multi-line-data.svg | 0.00 | ok | 0.03 | 3 | 10 | 0.00 | 0 min | 11 | 0.0 | 0.00 | 9 | +2.0 % |
| resvg-structure_image_embedded-16bit-png.svg | 0.02 | ok | 0.02 | 1 | 4 | 0.00 | 0 min | 5 | 0.0 | 0.00 | 9 | (corto) |
| resvg-structure_image_embedded-jpeg-luma.svg | 0.00 | ok | 0.02 | 1 | 4 | 0.00 | 0 min | 5 | 0.0 | 0.00 | 9 | (corto) |
| resvg-structure_image_embedded-svg.svg | 0.01 | ok | 0.02 | 1 | 4 | 0.00 | 0 min | 5 | 0.0 | 0.00 | 9 | (corto) |
| resvg-structure_image_nested-embedded-png.svg | 0.01 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 8 | - |
| resvg-structure_style_rule-specificity.svg | 0.00 | ok | 0.01 | 2 | 8 | 0.00 | 0 min | 10 | 0.0 | 0.00 | 9 | +0.6 % |
| resvg-structure_svg_nested-svg-with-rect-and-viewBox-1.svg | 0.00 | ok | 0.04 | 2 | 24 | 0.00 | 0 min | 26 | 0.0 | 0.00 | 9 | (corto) |
| resvg-structure_symbol_with-size-on-use-and-relative-units.svg | 0.00 | ok | 0.02 | 1 | 4 | 0.00 | 0 min | 5 | 0.0 | 0.00 | 9 | (corto) |
| resvg-structure_transform_nested-transforms-1.svg | 0.00 | ok | 0.04 | 3 | 12 | 0.00 | 0 min | 15 | 0.0 | 0.00 | 9 | +1.8 % |
| resvg-structure_use_nested-xlink-to-svg-element-with-rect-and-size.svg | 0.00 | ok | 0.02 | 2 | 16 | 0.00 | 0 min | 18 | 0.0 | 0.00 | 9 | (corto) |
| resvg-text_font-size_named-value.svg | 0.00 | ok | 0.04 | 108 | 2257 | 0.02 | 2 min | 3697 | 0.1 | 0.00 | 14 | +5.0 % |
| resvg-text_textPath_dy-with-tiny-coordinates.svg | 0.00 | ok | 0.03 | 39 | 794 | 0.00 | 0 min | 919 | 0.0 | 0.00 | 11 | +3.6 % |
| resvg-text_textPath_with-filter.svg | 0.00 | ok | 0.02 | 39 | 794 | 0.00 | 1 min | 1869 | 0.0 | 0.00 | 12 | +3.0 % |
| wmc-alchemy-distribution.svg | 0.88 | ok | 0.05 | 588 | 4643 | 0.03 | 15 min | 27486 | 0.6 | 0.00 | 34 | +1.0 % |
| wmc-amhara-topographic-map.svg | 15.59 | ok LENTO | 0.27 | 35413 | 395170 | 29.45 | 2.9 h | 1244709 | 25.0 | 0.05 | 658 | omitido |
| wmc-arms-mariavite-bishop.svg | 3.76 | ok | 0.09 | 1097 | 8144 | 0.05 | 32 min | 47853 | 0.9 | 0.00 | 58 | +1.4 % |
| wmc-blank-world-dymaxion.svg | 3.00 | ok | 0.08 | 2889 | 180013 | 0.39 | 26 min | 182845 | 3.7 | 0.01 | 190 | +23.5 % |
| wmc-blank-world-secondary-divisions.svg | 3.90 | ok | 0.15 | 5840 | 105756 | 0.49 | 55 min | 361128 | 7.3 | 0.01 | 274 | omitido |
| wmc-bolduc-house-floor-plan.svg | 2.71 | ok | 0.09 | 6081 | 22258 | 0.28 | 32 min | 84017 | 1.6 | 0.00 | 94 | +1.4 % |
| wmc-chablais-topographic-map.svg | 14.73 | ok | 0.14 | 3810 | 104866 | 0.34 | 37 min | 129546 | 2.5 | 0.01 | 157 | +6.7 % |
| wmc-chenrezing-mandala.svg | 0.40 | ok | 0.06 | 4197 | 20672 | 0.08 | 34 min | 94956 | 1.8 | 0.00 | 92 | +1.6 % |
| wmc-circle-celtic-ornament.svg | 0.20 | ok | 0.04 | 374 | 7969 | 0.07 | 22 min | 33354 | 0.7 | 0.00 | 48 | +1.1 % |
| wmc-coat-of-arms-east-prussia.svg | 3.58 | ERROR (rd) | 0.10 | 7183 | 98675 | 0.28 | - | - | - | - | 166 | - |
| wmc-coat-of-arms-france-navarre.svg | 6.25 | ok | 0.11 | 6732 | 106566 | 0.37 | 3.1 h | 473021 | 9.5 | 0.02 | 312 | omitido |
| wmc-emperor-penguin-illustration.svg | 9.05 | ok | 0.19 | 23831 | 126506 | 2.63 | 106 min | 632512 | 12.0 | 0.03 | 549 | omitido |
| wmc-flag-virginia.svg | 0.40 | ok | 0.02 | 1307 | 16997 | 0.37 | 33 min | 90834 | 1.8 | 0.00 | 81 | +2.6 % |
| wmc-germany-districts-municipalities-map.svg | 26.49 | ok LENTO | 0.83 | 20841 | 4273227 | 10.70 | 8.3 h | 4300235 | 80.5 | 0.17 | 2423 | omitido |
| wmc-hooke-bluefly-vector-trace.svg | 9.83 | ok LENTO | 0.16 | 28742 | 244185 | 11.18 | 4.2 h | 1330223 | 27.0 | 0.06 | 855 | omitido |
| wmc-human-skeleton-front.svg | 0.86 | ok | 0.04 | 1334 | 21462 | 0.06 | 20 min | 75096 | 1.5 | 0.00 | 80 | +4.0 % |
| wmc-i-ching-mandala.svg | 0.07 | ok | 0.01 | 126 | 382 | 0.00 | 10 min | 1080 | 0.0 | 0.00 | 11 | +0.0 % |
| wmc-large-blank-world-map.svg | 3.99 | ok | 0.11 | 4669 | 57888 | 0.15 | 41 min | 239957 | 4.8 | 0.01 | 179 | +8.1 % |
| wmc-mandala-border-01.svg | 1.22 | ok | 0.05 | 671 | 77107 | 0.20 | 18 min | 131879 | 2.5 | 0.01 | 118 | +10.6 % |
| wmc-mantra-clipart.svg | 0.07 | ok | 0.01 | 109 | 841 | 0.01 | 7 min | 6312 | 0.1 | 0.00 | 16 | +0.7 % |
| wmc-map-generalization-simplification.svg | 0.24 | ok | 0.06 | 612 | 8934 | 0.09 | 12 min | 24517 | 0.5 | 0.00 | 31 | +2.3 % |
| wmc-municipalities-lombardy.svg | 9.10 | ok | 0.15 | 1572 | 575836 | 0.57 | 52 min | 575836 | 11.3 | 0.03 | 477 | omitido |
| wmc-municipalities-piedmont.svg | 13.21 | ok | 0.21 | 1333 | 827309 | 0.66 | 50 min | 827309 | 16.6 | 0.03 | 628 | omitido |
| wmc-no5443-batsfjord-map.svg | 0.23 | ok | 0.01 | 168 | 4874 | 0.03 | 8 min | 26810 | 0.5 | 0.00 | 35 | +6.3 % |
| wmc-ornamental-alphabet.svg | 0.81 | ok | 0.02 | 1406 | 20475 | 0.32 | 29 min | 104610 | 2.0 | 0.00 | 94 | +5.8 % |
| wmc-outline-of-texas-detailed.svg | 0.10 | ok | 0.03 | 120 | 24158 | 0.27 | 3 min | 24014 | 0.4 | 0.00 | 29 | +26.5 % |
| wmc-pc-powersupply-schematic.svg | 0.11 | ok | 0.06 | 976 | 5373 | 0.01 | 16 min | 10501 | 0.2 | 0.00 | 21 | +1.0 % |
| wmc-pine-tree-flag-detailed.svg | 0.43 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 9 | - |
| wmc-prc-national-emblem.svg | 1.98 | ERROR (rd) | 2.81 | 1737 | 64263 | 0.27 | - | - | - | - | 74 | - |
| wmc-scarab-ornament-1905.svg | 0.08 | ok | 0.03 | 255 | 1807 | 0.03 | 4 min | 11884 | 0.2 | 0.00 | 20 | +4.5 % |
| wmc-teng-fang-lee-floor-plan.svg | 1.15 | ok | 0.07 | 8365 | 19886 | 0.07 | 77 min | 20141 | 0.3 | 0.00 | 36 | -1.6 % |
| wmc-tomruen-mandala.svg | 0.35 | ok | 0.07 | 8184 | 41248 | 0.17 | 51 min | 204600 | 4.0 | 0.01 | 177 | +1.6 % |
| **dxf** | | | | | | | | | | | | |
| dxfviewer-block-recursive.dxf | 0.02 | ok | 0.00 | 5 | 10 | 0.00 | 1 min | 10 | 0.0 | 0.00 | 9 | +5.1 % |
| dxfviewer-circle-arc.dxf | 0.01 | ok | 0.00 | 2 | 1965 | 0.00 | 1 min | 1966 | 0.0 | 0.00 | 11 | +8.3 % |
| dxfviewer-layers-colors.dxf | 0.01 | ok | 0.00 | 4 | 8 | 0.00 | 2 min | 8 | 0.0 | 0.00 | 9 | +1.4 % |
| dxfviewer-mtext-stacked.dxf | 0.01 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 8 | - |
| dxfviewer-pattern-hatch-concave.dxf | 0.01 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 8 | - |
| dxfviewer-polyline-no-seqend.dxf | 0.01 | ok | 0.00 | 2 | 4 | 0.00 | 1 min | 4 | 0.0 | 0.00 | 9 | +4.3 % |
| dxfviewer-polyline.dxf | 0.01 | ok | 0.00 | 2 | 8 | 0.00 | 1 min | 9 | 0.0 | 0.00 | 9 | +2.9 % |
| dxfviewer-solid-hatch-edge-paths.dxf | 0.01 | ok | 0.00 | 1 | 2 | 0.00 | 0 min | 2 | 0.0 | 0.00 | 9 | +9.1 % |
| ezdxf-3dface.dxf | 0.13 | ok | 0.00 | 5 | 20 | 0.00 | 3 min | 25 | 0.0 | 0.00 | 9 | +0.7 % |
| ezdxf-Minimal_DXF_AC1006.dxf | 0.00 | ok | 0.00 | 1 | 2 | 0.00 | 0 min | 2 | 0.0 | 0.00 | 8 | (corto) |
| ezdxf-Minimal_DXF_AC1009.dxf | 0.00 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 7 | - |
| ezdxf-addons-optimize-forms.dxf | 0.08 | ok | 0.01 | 40 | 882 | 0.00 | 6 min | 922 | 0.0 | 0.00 | 10 | +0.6 % |
| ezdxf-circle_radius_le_0.dxf | 0.01 | ok | 0.00 | 1 | 497 | 0.00 | 1 min | 498 | 0.0 | 0.00 | 9 | +1.3 % |
| ezdxf-clockwise_arcs_hatch.dxf | 0.02 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 8 | - |
| ezdxf-colors.dxf | 0.15 | ok | 0.00 | 38 | 1862 | 0.00 | 4 min | 1900 | 0.0 | 0.00 | 11 | +0.7 % |
| ezdxf-columns_R2018.dxf | 0.09 | ok | 0.00 | 30 | 120 | 0.00 | 2 min | 150 | 0.0 | 0.00 | 9 | +0.7 % |
| ezdxf-dimension_in_nested_blocks.dxf | 0.09 | ok | 0.01 | 14 | 56 | 0.00 | 3 min | 70 | 0.0 | 0.00 | 9 | +1.1 % |
| ezdxf-edgeminer-1_polylines.dxf | 0.08 | ok | 0.00 | 74 | 12640 | 0.00 | 4 min | 12640 | 0.3 | 0.00 | 21 | +3.2 % |
| ezdxf-hatches_1.dxf | 0.13 | ok | 0.01 | 22 | 252 | 0.00 | 1 min | 266 | 0.0 | 0.00 | 10 | +2.1 % |
| ezdxf-hatches_2.dxf | 0.21 | ok | 0.01 | 7 | 1874 | 0.00 | 1 min | 1875 | 0.0 | 0.00 | 11 | +2.1 % |
| ezdxf-insert_bricscad_level_1.dxf | 0.09 | ok | 0.01 | 330 | 672 | 0.01 | 7 min | 678 | 0.0 | 0.00 | 11 | -0.7 % |
| ezdxf-mtext_complex_inline_codes.dxf | 0.17 | ok | 0.00 | 4 | 8 | 0.00 | 1 min | 8 | 0.0 | 0.00 | 10 | +4.6 % |
| ezdxf-multi_insert_with_attribs.dxf | 0.02 | ok | 0.00 | 50 | 7975 | 0.00 | 3 min | 8000 | 0.2 | 0.00 | 17 | +1.8 % |
| ezdxf-text.dxf | 0.23 | ok | 0.00 | 125 | 406 | 0.00 | 4 min | 484 | 0.0 | 0.00 | 11 | -0.0 % |
| ezdxf-text_alignments.dxf | 0.02 | ok | 0.00 | 42 | 4740 | 0.00 | 2 min | 4772 | 0.1 | 0.00 | 13 | +2.7 % |
| ezdxf-transform_hatch_pattern.dxf | 0.08 | ok | 0.00 | 12 | 48 | 0.00 | 2 min | 60 | 0.0 | 0.00 | 9 | +2.0 % |
| ezdxf-uncommon.dxf | 0.34 | ok | 0.01 | 29 | 723 | 0.00 | 4 min | 728 | 0.0 | 0.00 | 11 | +0.8 % |
| ezdxf-valid_lineweights.dxf | 0.02 | ok | 0.00 | 25 | 50 | 0.00 | 4 min | 50 | 0.0 | 0.00 | 9 | +0.7 % |
| ezdxf-visibility.dxf | 0.12 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 8 | - |
| ezdxf-xclip.dxf | 0.09 | ok | 0.00 | 36 | 8442 | 0.00 | 6 min | 8460 | 0.2 | 0.00 | 17 | +1.0 % |
| librecad-dim_sample.dxf | 0.05 | ok | 0.00 | 10 | 170 | 0.00 | 1 min | 174 | 0.0 | 0.00 | 9 | +1.6 % |
| librecad-pattern-gost_sand.dxf | 0.35 | ok | 0.01 | 1030 | 2060 | 0.01 | 2 min | 2060 | 0.0 | 0.00 | 14 | -12.1 % |
| librecad-sheet-A0H.dxf | 0.80 | ok | 0.02 | 4077 | 8160 | 0.22 | 4 min | 8160 | 0.1 | 0.00 | 22 | +0.9 % |
| librecad-test-big5_hkscs.dxf | 0.00 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 7 | - |
| librecad-test-eed_binary_r2007.dxf | 0.00 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 7 | - |
| **pdf** | | | | | | | | | | | | |
| pdfjs-tracemonkey.pdf | 1.02 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 13 | - |
| wmc-cascadia-scenario-median.pdf | 6.35 | ok LENTO | 0.15 | 596 | 312813 | 15.50 | 21 min | 318812 | 6.3 | 0.02 | 386 | omitido |
| wmc-cia-factbook-political-world.pdf | 24.38 | ok | 2.20 | 29606 | 2301112 | 4.35 | 2.5 h | 2394761 | 46.2 | 0.11 | 2872 | omitido |
| wmc-design-with-fontforge.pdf | 8.07 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 29 | - |
| wmc-dibujo-tecnico-evau-2023.pdf | 0.19 | ok | 0.00 | 479 | 1054 | 0.00 | 4 min | 1101 | 0.0 | 0.00 | 14 | -0.5 % |
| wmc-linguistic-heritage-vector-maps.pdf | 9.38 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 27 | - |
| wmc-political-map-of-europe.pdf | 6.70 | ok | 0.52 | 799 | 1302916 | 2.02 | 51 min | 1304938 | 25.1 | 0.06 | 1520 | omitido |
| wmc-thelio-mega-drawing-009005.pdf | 0.25 | ok | 0.02 | 3192 | 31916 | 0.05 | 17 min | 41717 | 0.8 | 0.00 | 77 | +1.2 % |
| wmc-tokyo-map-sotm.pdf | 7.15 | ok | 0.52 | 211400 | 1116981 | 2.33 | 7.1 h | 1116976 | 21.5 | 0.05 | 1508 | omitido |
| **ai** | | | | | | | | | | | | |
| pdfbox-26101_Colors.ai | 0.08 | ok | 0.00 | 8 | 272 | 0.00 | 5 min | 1770 | 0.0 | 0.00 | 11 | +0.5 % |
| pdfbox-ARCHIVERGB.ai | 1.12 | ok | 0.01 | 58 | 5579 | 0.04 | 13 min | 12576 | 0.3 | 0.00 | 26 | +1.8 % |
| pdfbox-FANTASTICCMYK.ai | 0.18 | ok | 0.00 | 7 | 565 | 0.00 | 10 min | 617 | 0.0 | 0.00 | 10 | +0.6 % |
| pdfbox-HOTRODCMYK.ai | 0.77 | ok | 0.01 | 441 | 12129 | 0.28 | 96 min | 87995 | 1.8 | 0.00 | 104 | +0.9 % |
| tika-testAI_PS.ai | 0.00 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 7 | - |
| **image** | | | | | | | | | | | | |
| wmc-hubble-25th-8919x6683.jpg | 66.60 | ok | 1.21 | 0 | 0 | 3.07 | 6.6 h | 4601878 | 62.1 | 0.21 | 2464 | omitido |
| wmc-laocoon-group-8744x9502.jpg | 57.22 | ok | 1.26 | 0 | 0 | 5.09 | 8.8 h | 8933768 | 139.5 | 0.43 | 3995 | omitido |
| wmc-mandelbrot-109-10000x10000.png | 77.33 | ok | 1.00 | 0 | 0 | 3.78 | 8.8 h | 5992120 | 81.0 | 0.30 | 3444 | omitido |
| wmc-noaa-18681x18681.tif | 37.62 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 43 | - |
| wmc-noaa-9340x9340.tif | 37.57 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 376 | - |
| wmc-sierpinski-pyramid-10000x10000.png | 78.54 | ok | 1.14 | 0 | 0 | 2.35 | 4.1 h | 3118314 | 42.7 | 0.15 | 4262 | omitido |
| **gcode** | | | | | | | | | | | | |
| cncjs-Circle_Diamond_Square_Calibration.gcode | 0.02 | ok | 0.00 | 41 | 860 | 0.00 | 11 min | 1891 | 0.0 | 0.00 | 10 | +0.1 % |
| cncjs-arc-ijk.gcode | 0.00 | ok | 0.00 | 1 | 3 | 0.00 | 0 min | 29 | 0.0 | 0.00 | 8 | (corto) |
| cncjs-full-circle-without-xyz.gcode | 0.00 | ok | 0.00 | 3 | 6 | 0.00 | 0 min | 9 | 0.0 | 0.00 | 8 | (corto) |
| cncjs-github.gcode | 4.39 | ok | 0.08 | 2 | 187696 | 0.05 | 47 min | 187697 | 3.6 | 0.01 | 139 | +3.1 % |
| cncjs-helical-thread-milling.gcode | 0.00 | ok | 0.00 | 1 | 17 | 0.00 | 0 min | 106 | 0.0 | 0.00 | 8 | (corto) |
| cncjs-jsdc-laser.gcode | 0.05 | ok | 0.00 | 8 | 2361 | 0.00 | 1 min | 2363 | 0.0 | 0.00 | 11 | +5.3 % |
| cncjs-octocat.gcode | 1.12 | ok | 0.02 | 1 | 23995 | 0.01 | 90 min | 23995 | 0.5 | 0.00 | 26 | +1.0 % |
| cncjs-part.gcode | 1.98 | ok | 0.04 | 1 | 41905 | 0.01 | 95 min | 41905 | 0.8 | 0.00 | 46 | +2.6 % |
| cncjs-sample.gcode | 0.54 | ok | 0.01 | 5 | 18060 | 0.00 | 32 min | 18060 | 0.3 | 0.00 | 25 | +3.1 % |
| cncjs-tball-engrave-laser.gcode | 0.08 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 7 | - |
| cncjs-tball-pocket-laser.gcode | 0.90 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 8 | - |
| linuxcnc-stellabee1.ngc | 3.24 | ok | 0.06 | 8553 | 168587 | 0.08 | 30 min | 168587 | 1.8 | 0.01 | 153 | -9.4 % |
| linuxcnc-trochoidal.ngc | 0.33 | ok | 0.01 | 4 | 11339 | 0.00 | 4 min | 11339 | 0.2 | 0.00 | 17 | +3.0 % |
| ugs-Gates_combined_R12.nc | 0.44 | ok | 0.01 | 21 | 15871 | 0.02 | 9 min | 18547 | 0.3 | 0.00 | 27 | +3.1 % |
| ugs-ShapeOko_Calibration_Pattern_01b.ngc | 0.10 | ok | 0.00 | 146 | 4180 | 0.00 | 6 min | 4269 | 0.1 | 0.00 | 13 | +0.2 % |
| ugs-rotation.ngc | 1.32 | ERROR (import) | 0.00 | 0 | 0 | 0.00 | - | - | - | - | 8 | - |
| ugs-stress_test.gcode | 0.17 | ok | 0.01 | 1 | 12000 | 0.00 | 0 min | 12000 | 0.1 | 0.00 | 17 | (corto) |
| **hpgl** | | | | | | | | | | | | |
| chiplotle-bird.hpgl | 0.08 | ok | 0.00 | 105 | 7242 | 0.00 | 2 min | 7243 | 0.1 | 0.00 | 14 | +6.7 % |
| chiplotle-foal.hpgl | 0.03 | ok | 0.00 | 135 | 3208 | 0.00 | 2 min | 3234 | 0.0 | 0.00 | 12 | +1.9 % |
| chiplotle-square.hpgl | 0.00 | ok | 0.00 | 1 | 4 | 0.00 | 0 min | 5 | 0.0 | 0.00 | 8 | (corto) |
| fakeparallelprinter-tds420a.hpgl | 0.05 | ok | 0.00 | 1006 | 3829 | 0.01 | 1 min | 3858 | 0.0 | 0.00 | 12 | -9.4 % |
| imagemagick-perlmagick-input.hpgl | 0.07 | ok | 0.00 | 919 | 6747 | 0.01 | 9 min | 6863 | 0.1 | 0.00 | 15 | +0.3 % |
| **lbrn** | | | | | | | | | | | | |
| 6502-ace-board-top.lbrn2 | 0.03 | ok | 0.00 | 28 | 340 | 0.00 | 4 min | 1239 | 0.0 | 0.00 | 10 | +0.6 % |
| enigma-dial-laser-cuts.lbrn2 | 0.10 | ok | 0.00 | 44 | 231 | 0.00 | 4 min | 4161 | 0.1 | 0.00 | 15 | -0.1 % |
| gestolumina-lilygobox.lbrn2 | 0.68 | ok | 0.01 | 120 | 5247 | 0.02 | 43 min | 87036 | 1.3 | 0.00 | 70 | -2.8 % |
| laserpost-complex-ll-top.lbrn | 0.11 | ok | 0.00 | 42 | 1390 | 0.00 | 2 min | 3312 | 0.1 | 0.00 | 12 | -1.3 % |
| laserprojects-box-38x10.lbrn2 | 0.15 | ok | 0.00 | 458 | 1819 | 0.01 | 31 min | 14669 | 0.2 | 0.00 | 19 | -4.9 % |
| lbrnts-absolute-coordinate-grid.lbrn2 | 0.17 | ok | 0.00 | 141 | 204 | 0.00 | 12 min | 14144 | 0.2 | 0.00 | 17 | -13.3 % |
| lbrnts-repro04-motor-controller.lbrn2 | 1.40 | ok | 0.01 | 662 | 31373 | 0.04 | 52 min | 123298 | 2.7 | 0.01 | 102 | -40.4 % |
| lightburn-tester-example1.lbrn | 0.06 | ok | 0.11 | 160 | 112 | 0.01 | 10 min | 10329 | 0.2 | 0.00 | 21 | -13.3 % |
| lightburn-tester-example2.lbrn | 0.05 | ok | 0.02 | 116 | 112 | 0.00 | 11 min | 9386 | 0.2 | 0.00 | 19 | -12.0 % |
| magic-crystal-lightguide-layer2.lbrn | 0.04 | ok | 0.00 | 8 | 288 | 0.00 | 1 min | 1032 | 0.0 | 0.00 | 10 | +2.4 % |
| maker-file-index-shelf.lbrn2 | 0.01 | ok | 0.00 | 1 | 6 | 0.00 | 1 min | 174 | 0.0 | 0.00 | 8 | +0.3 % |
| mopa-jackolantern.lbrn2 | 0.13 | ERROR (rd) | 0.00 | 20 | 2749 | 0.03 | - | - | - | - | 91 | - |
| mothbox-allacrylic-v4.lbrn2 | 0.28 | ok | 0.00 | 338 | 4609 | 0.02 | 17 min | 17361 | 0.3 | 0.00 | 27 | -0.9 % |
| mothbox-cutc-internals.lbrn2 | 0.41 | ok | 0.00 | 134 | 10332 | 0.01 | 6 min | 8522 | 0.2 | 0.00 | 19 | -0.6 % |
| openscad-cutter-bob-dishwasher-sheet1.lbrn2 | 0.08 | ERROR (compute) | 0.00 | 27 | 2111 | 0.00 | - | - | - | - | 8 | - |
| pi-py-ardu-pong-cutout.lbrn2 | 0.20 | ok | 0.02 | 15 | 369 | 0.04 | 18 min | 11566 | 0.2 | 0.00 | 94 | -1.3 % |
| pmk-keycaps-engraving.lbrn2 | 0.07 | ok | 0.00 | 29 | 93 | 0.00 | 7 min | 1520 | 0.0 | 0.00 | 10 | -0.2 % |
| stargatebadge-text-ring.lbrn2 | 0.44 | ok | 0.05 | 0 | 0 | 0.09 | 9 min | 12396 | 0.2 | 0.00 | 212 | -34.7 % |
| valvetester-analyser-cuts.lbrn2 | 0.01 | ok | 0.00 | 26 | 220 | 0.00 | 9 min | 646 | 0.0 | 0.00 | 9 | +0.4 % |
| **synthetic** | | | | | | | | | | | | |
| synth-circles-20k.svg | 0.83 | ok | 0.12 | 20000 | 135271 | 0.21 | 2.2 h | 500000 | 9.7 | 0.02 | 391 | omitido |
| synth-curves-200k.svg | 8.21 | ok | 0.39 | 200 | 200200 | 1.02 | 14.5 h | 4329236 | 86.7 | 0.23 | 2307 | omitido |
| synth-dxf-bulge-20k.dxf | 4.03 | ok | 0.12 | 20000 | 1140000 | 0.89 | 18.1 h | 1160000 | 23.3 | 0.05 | 984 | omitido |
| synth-dxf-lines-100k.dxf | 7.38 | ok | 0.21 | 100000 | 200000 | 0.64 | 7.5 h | 200000 | 4.0 | 0.01 | 324 | -5.9 % |
| synth-dxf-splines-5k.dxf | 1.63 | ok | 0.10 | 5000 | 257336 | 0.08 | 3.6 h | 257336 | 5.2 | 0.01 | 164 | omitido |
| synth-gcode-1m.nc | 26.67 | ok | 0.47 | 256 | 1048831 | 0.22 | 8.3 h | 1048831 | 12.2 | 0.04 | 637 | omitido |
| synth-hpgl-500k.plt | 4.90 | ok | 0.04 | 9804 | 509807 | 0.69 | 25.5 h | 509808 | 5.4 | 0.02 | 347 | omitido |
| synth-image-10000x8000.png | 66.24 | ok | 0.54 | 0 | 0 | 3.30 | 7.1 h | 7855980 | 106.8 | 0.38 | 4206 | omitido |
| synth-image-12000x9000.jpg | 21.71 | ok | 0.72 | 0 | 0 | 3.16 | 6.6 h | 7374722 | 100.2 | 0.35 | 4772 | omitido |
| synth-nested-groups.svg | 0.05 | ok | 0.13 | 300 | 1200 | 0.06 | 5.6 h | 1500 | 0.0 | 0.00 | 16 | +0.0 % |
| synth-path-1m-nodes.svg | 16.25 | ok | 0.32 | 1 | 1048576 | 0.23 | 8.3 h | 1048576 | 12.2 | 0.05 | 701 | omitido |
| synth-rects-50k.svg | 2.72 | ok | 0.14 | 50000 | 200000 | 0.18 | 5.8 h | 250000 | 2.3 | 0.01 | 241 | omitido |
| synth-text-dense.svg | 0.03 | ok | 0.78 | 16346 | 287540 | 0.24 | 36 min | 317706 | 5.4 | 0.01 | 304 | omitido |

## Fallos

Todos son errores controlados (mensaje, sin pánico). Para reproducir cualquiera:

```text
scripts/fetch-corpus.sh                         # una vez; descarga a tests/corpus-downloaded/
cargo run --release -p tlaku-bench -- worker tests/corpus-downloaded/<ruta> --no-emu
```

(`worker` imprime una línea JSON por fase; la última trae `error` y `error_phase`. Los sintéticos salen
de `tlaku-bench gen DIR`.)

| # | Archivo(s) | Fase | Qué pasa | Reproducción | ¿Bug? |
|---|---|---|---|---|---|
| 1 | `svg/wmc-coat-of-arms-east-prussia.svg`, `svg/wmc-prc-national-emblem.svg` | `.rd` | "el trabajo tiene 129 capas con cortes y Ruida admite 128". El SVG crea una capa por color (129 colores) y el G-code sale bien; solo falla la exportación a Ruida. | `worker` sobre cualquiera de los dos | Límite sin salida: el usuario no puede exportar y no hay forma de fusionar colores desde el aviso |
| 2 | `lbrn/mopa-jackolantern.lbrn2` | `.rd` | "velocidad 48000 mm/min fuera del rango permitido (hasta 36000)". Es un proyecto con una capa a 800 mm/s; el cálculo y el G-code pasan sin ningún aviso de cálculo aunque la velocidad supera lo que la máquina del perfil admite (12 000 mm/min). | `worker` sobre el archivo | Sí, menor: debería recortar o avisar al calcular, no fallar al exportar |
| 3 | `image/wmc-noaa-18681x18681.tif` | importar | "Unhandled TIFF color type YCbCr(8)": el decodificador no soporta TIFF YCbCr. | `worker` | Límite del decodificador; el mensaje no sugiere convertir |
| 4 | `image/wmc-noaa-9340x9340.tif` (87 Mpx) | importar | "Memory limit exceeded" del decodificador TIFF, mientras un PNG de 100 Mpx sí carga. | `worker` | Sí: el límite de memoria por defecto de la biblioteca rechaza una imagen válida |
| 5 | `dxf/dxfviewer-mtext-stacked.dxf`, `dxfviewer-pattern-hatch-concave.dxf`, `ezdxf-clockwise_arcs_hatch.dxf`, `ezdxf-visibility.dxf`, `ezdxf-Minimal_DXF_AC1009.dxf`, `librecad-test-big5_hkscs.dxf`, `librecad-test-eed_binary_r2007.dxf` | importar | "el archivo no contiene trayectorias". Los que solo traen `HATCH`, `TEXT` o `MTEXT` no dicen que se omitieron: `tlaku-dxf` sí genera esos avisos, pero `import_file` los descarta al devolver el error (`crates/tlaku-pipeline/src/import.rs`, rama `layers.is_empty()`). | `worker` sobre cualquiera | Sí: se pierde la causa que el importador ya conoce |
| 6 | `svg/resvg-structure_image_nested-embedded-png.svg`, `svg/wmc-pine-tree-flag-detailed.svg` | importar | Solo contienen una imagen incrustada: "no contiene trayectorias", sin mencionar la imagen. | `worker` | Mensaje pobre; se podría importar como imagen |
| 7 | `pdf/pdfjs-tracemonkey.pdf`, `pdf/wmc-design-with-fontforge.pdf`, `pdf/wmc-linguistic-heritage-vector-maps.pdf` | importar | La página 1 no tiene trayectos (solo texto con fuentes o una imagen). El mensaje es bueno y explica que hay que convertir el texto a contornos. | `worker` | No (comportamiento esperado) |
| 8 | `ai/tika-testAI_PS.ai` | importar | AI antiguo en PostScript: mensaje claro con la salida alternativa. | `worker` | No |
| 9 | `gcode/cncjs-tball-engrave-laser.gcode`, `cncjs-tball-pocket-laser.gcode`, `ugs-rotation.ngc` | importar | "no dibuja nada con el láser encendido": usan `M3 S0` y cortan sin `S`, es decir, con potencia 0. | `worker` | No (coherente con la regla de importación); se podría sugerir fijar una potencia |
| 10 | `lbrn/openscad-cutter-bob-dishwasher-sheet1.lbrn2` | calcular | "el diseño está vacío (o ningún objeto tiene salida)": la única capa tiene `doOutput 0`. | `worker` | Mensaje mejorable: no dice que la capa está sin salida |

Además hay cuatro archivos que **no fallan pero son lentos** (más de 10 s al calcular):
`svg/wmc-amhara-topographic-map.svg`, `pdf/wmc-cascadia-scenario-median.pdf`,
`svg/wmc-hooke-bluefly-vector-trace.svg` y `svg/wmc-germany-districts-municipalities-map.svg`.
Reproducción: `worker` sobre cada uno; con `TLAKU_BENCH_DISABLE=inner_first` el cálculo baja a 0,6 s,
0,14 s, 0,6 s y 1,7 s respectivamente. Ver [Límites](#límites-encontrados).

## Límites encontrados

**Planificador, "interior primero" (el principal cuello).** Tiempo de `compute` con y sin la opción
(`TLAKU_BENCH_DISABLE=inner_first`):

| Archivo | Trayectos | Nodos | Con | Sin |
|---|---:|---:|---:|---:|
| wmc-amhara-topographic-map.svg | 35 413 | 395 170 | 29,6 s | 0,6 s |
| wmc-cascadia-scenario-median.pdf | 596 | 312 813 | 14,4 s | 0,14 s |
| wmc-hooke-bluefly-vector-trace.svg | 28 742 | 244 185 | 11,2 s | 0,6 s |

Cascadia tiene pocos trayectos pero muy largos: el costo depende también de los nodos (el solape se
decide muestreando puntos de cada figura contra las demás). Un diseño que el usuario no considera
"grande" (un PDF de 6 MB) se queda 15 s calculando; hay que recordar que la app vuelve a calcular en
cada vista previa.

**Planificador, "reducir recorridos en vacío".** Con trayectos sueltos crece más que lineal: 100 000
`LINE` tardan 0,64 s y 1 000 000 tardan 40 s (62 veces para 10 veces más); sin la opción, 0,09 s y 1,0 s.
Con rectángulos en rejilla no se nota (200 000 en 0,8 s) porque el orden ya es casi bueno.

**Memoria de imágenes.** Pico por fase, en MB:

| Imagen | Mpx | Importar | Calcular | G-code | `.rd` |
|---|---:|---:|---:|---:|---:|
| JPEG sintético 12 000 x 9 000 | 108 | 2 212 | 4 772 | 4 772 | 4 772 |
| PNG Mandelbrot 10 000 x 10 000 | 100 | 2 275 | 3 334 | 3 334 | 3 444 |
| PNG Sierpinski 10 000 x 10 000 | 100 | 2 276 | 4 262 | 4 262 | 4 262 |
| JPEG Laocoon 8 744 x 9 502 | 83 | 1 940 | 3 995 | 3 995 | 3 995 |
| PNG sintético 10 000 x 8 000 | 80 | 1 673 | 3 586 | 4 152 | 4 206 |
| JPEG Hubble 8 919 x 6 683 | 60 | 1 442 | 2 464 | 2 464 | 2 464 |

Importar cuesta unos 22 a 28 bytes por píxel porque `import_image` decodifica la imagen entera
(`tlaku_raster::gray::load`) solo para leer su ancho y su alto; calcular llega a 33 a 45 bytes por
píxel. Extrapolando (no se pudo medir: el TIFF de 349 Mpx no se decodifica), una imagen de 350 Mpx
necesitaría más de 12 GB. Con 8 GB de RAM el límite práctico está alrededor de 100 Mpx.

**Tamaño de las salidas.** Un mapa de 4,3 M de nodos genera 4,3 M de líneas y 80 MB de G-code; una imagen
de 83 Mpx a 0,1 mm genera 8,9 M de líneas y 139 MB. Con 115 200 baudios (unos 11,5 KB/s, 600 líneas por
segundo con 19 bytes por línea) solo el envío ya es largo, y el enlace pasa a ser el límite antes que
la máquina: `wmc-political-map-of-europe.pdf` necesita 423 líneas/s a 1 000 mm/min (el 71 % del
enlace), así que por encima de unos 1 400 mm/min la máquina se detendría esperando líneas. Las curvas
se aplanan con 0,02 mm de tolerancia: `amhara` pasa de 395 mil nodos a 1,24 M de movimientos.

**Otros límites medidos.**

- Importar 4,3 M de nodos (SVG de 26 MB) toma 0,8 s; un DXF de 1 M de `LINE`, 2,1 s y 1,8 GB (todo en
  el importador).
- Un trayecto de 4,2 M de nodos se calcula en 1,1 s y genera 4,2 M de líneas sin problema.
- Ruida admite 128 capas y 36 000 mm/min (ver fallos 1 y 2).
- El emulador GRBL se ejecuta a unas 3 000 líneas por segundo real (el reloj se limita a propósito para
  que el anfitrión no deje sin líneas a la máquina): 250 000 líneas son unos 80 s. Por eso el análisis simulado se limitó a 250 000 líneas (28 trabajos quedaron fuera).

### Lienzo

Escenarios en modo mock (Chrome headless, rasterización por software). Todos los valores son
milisegundos hasta el siguiente cuadro completo; el suelo del método es 32 ms. Entre paréntesis, p95.

| Escenario | Carga (1er cuadro) | `render()` de la carga | Paneo | Zoom + / - (rueda) | Clic | Clics que seleccionan |
|---|---:|---:|---:|---:|---:|---:|
| 1 000 objetos | 46 | 7 | 32 | 66 / 66 | 46 | 20/20 |
| 10 000 objetos | 94 | 40 | 33 | 67 / 66 | 44 | 20/20 |
| 50 000 objetos | 293 | 174 | 33 | 83 (117) / 83 | 77 | 20/20 |
| 100 000 objetos (extra) | 572 | 299 | 49 (72) | 99 (229) / 99 | 171 | 20/20 |
| 1 trayecto de 100 000 nodos (extra) | 82 | 48 | 32 | 66 / 66 | 47 | 20/20 |
| 1 trayecto de 1 048 576 nodos | 525 | 417 | 32 (máx 509) | 66 (**370**) / 66 | **296** | 20/20 |

Selección y movimiento:

| Escenario | Rect. rincón (n) | Rect. cuarto (n) | Rect. toda la cama (n) | Seleccionar todo | Arrastre por cuadro | Confirmar mover | Deshacer |
|---|---|---|---|---:|---:|---:|---:|
| 1 000 objetos | 33 (0) | 32 (240) | 32 (1 000) | 44 | 49 | 49 | 49 |
| 10 000 objetos | 47 (8) | 50 (2 500) | 66 (10 000) | 72 | 66 | 83 | 59 |
| 50 000 objetos | 49 (48) | 83 (12 430) | 116 (50 000) | 140 | **150** | **300** | 232 |
| 100 000 objetos | 116 (99) | 166 (24 974) | 283 (100 000) | 255 | 249 | 565 | 482 |
| 1 trayecto de 1 M de nodos | 32 (0) | 233 (1) | 232 (1) | 233 | **366** | **699** | 482 |

(Los tiempos del rectángulo son los de soltar el botón, que incluyen calcular qué objetos toca y pintar la selección; durante el arrastre del rectángulo cada cuadro cuesta 49 ms con 1 000 a 10 000 objetos, 50 a 66 con 50 000 y 83 con 100 000.)

Lo que se ve:

- Hasta 10 000 objetos todo está en el suelo del método: no hay costo medible para el usuario.
- **50 000 objetos:** se carga en 0,3 s, el paneo y el zoom siguen fluidos (el renderer usa un bitmap
  reutilizable por encima de 12 000 segmentos, `HEAVY_SEGMENTS`), pero mover la selección completa es
  ~7 cuadros por segundo y confirmar el movimiento tarda 0,3 s (más 0,23 s deshacer). Es aceptable.
- **100 000 objetos:** confirmar el movimiento tarda 0,57 s y el clic 0,17 s: el punto en que se nota.
- **1 M de nodos en un solo trayecto:** pintar es lo peor (0,42 s en la carga; un cuadro de zoom llega a 0,37 s en
  el p95 y el repintado nítido al quedarse quieto cuesta 75 ms), el clic tarda 0,3 s (el hit-test recorre el trayecto) y cada cuadro de un arrastre 0,37 s, es
  decir, 2,7 cuadros por segundo. Un solo trayecto de 100 000 nodos todavía está en el suelo.
- Memoria de JavaScript: 65 MB (1 000 objetos), 113 MB (10 000), 210 MB (50 000), 650 MB (100 000) y
  914 MB (1 M de nodos).
- Ningún escenario se colgó ni dio error; el plazo por paso era de 120 s.

## Estimado contra simulado

Se compara la duración que el emulador GRBL (reloj acelerado, aceleración y velocidades máximas igualadas
al perfil: `$110/$111 = 12 000`, `$120/$121 = 1 000`) tarda en ejecutar el G-code contra
`preview.total_s`. Para los 115 trabajos de 20 s o más estimados que pasaron por el emulador:

| | |
|---|---|
| Mediana | +1,2 % (el simulado es un poco más largo; parte es el sesgo de sondeo de la medición) |
| Dentro de ±5 % | 93 de 115 |
| Dentro de ±10 % | 106 de 115 |
| Dentro de ±20 % | 111 de 115 |
| Extremos | +26,5 % (`wmc-outline-of-texas-detailed.svg`) y -40,4 % (`lbrnts-repro04-motor-controller.lbrn2`) |

Casos controlados (cuadrado de 150 mm, círculo de radio 60 mm, dos formas rellenas; línea y relleno):
entre -0,3 % y +2,7 %. Es decir, **el modelo de tiempo es correcto en los casos simples** y los
desvíos aparecen en archivos reales con:

- **Segmentos muy cortos tras aplanar curvas** (el simulado tarda más): `wmc-outline-of-texas-detailed.svg`
  +26,5 %, `wmc-blank-world-dymaxion.svg` +23,5 %, `wmc-mandala-border-01.svg` +10,6 %,
  `cncjs-arc-ijk.gcode` +9,5 % y `ugs-stress_test.gcode` +92 % (8,8 s estimados contra 16,9 s; 12 000
  líneas de ~0,5 mm). Es coherente con que el planificador de GRBL frena en cada unión según la
  desviación, y con más uniones por segundo el estimador resulta optimista.
- **Proyectos de LightBurn** (el estimado es más largo que el simulado, -13 % a -40 %):
  `lbrnts-repro04-motor-controller.lbrn2` -40,4 %, `stargatebadge-text-ring.lbrn2` -34,7 %,
  `mopa-jackolantern.lbrn2` -25,5 %, `lightburn-tester-example1/2.lbrn` -13 % y -12 %,
  `lbrnts-absolute-coordinate-grid.lbrn2` -13 %. Son capas de relleno e imagen con velocidades de hasta
  500 mm/s (30 000 mm/min, el doble de lo que admite el perfil). No se determinó la causa; candidatas:
  el estimador cuenta retardos o sobrecorte que el G-code no emite, o aplica velocidades distintas al
  tope de la máquina. Conviene revisarlo con un caso mínimo en la ola 6.
- La longitud quemada que dibuja el emulador coincide con la longitud de corte calculada con menos de
  1 % de diferencia en todos los trabajos.

## Prioridades para la ola 6

Ordenadas por impacto en el usuario.

1. **"Interior primero" cuadrático (alto).** Cualquier mapa o ilustración con miles de trayectos (o un
   PDF con trayectos largos) deja la interfaz 10 a 30 s calculando antes de poder previsualizar o
   enviar; es la opción activa por defecto. Apagarla baja 18 a 100 veces. Arreglo: prefiltrar por caja
   envolvente con un índice espacial (rejilla o árbol R) y limitar el muestreo por figura; o calcularla
   en segundo plano con progreso y cancelación. Criterio de aceptación: `amhara`, `cascadia` y `hooke` en
   menos de 2 s.
2. **Memoria de las imágenes grandes (alto).** 100 Mpx piden 3,4 a 4,8 GB; en un equipo de 8 GB la app
   caería. Arreglos: leer solo la cabecera para obtener el tamaño al importar (ahorra 2,3 GB), remuestrear
   al ppp que la cama y el intervalo necesitan (400 mm a 0,1 mm son 4 000 px) antes de tramar, y
   procesar por franjas.
3. **Conservar la causa cuando falla una importación (medio, fácil).** Devolver los avisos del
   importador junto con el error ("se omitieron 3 HATCH y 12 TEXT; conviértelos a contornos") en DXF y
   SVG; decir "la capa tiene la salida desactivada" en `.lbrn2`; ofrecer importar como imagen un SVG que
   solo trae una imagen incrustada.
4. **Ruida: 128 capas y 36 000 mm/min (medio).** En lugar de fallar al exportar, avisar al calcular,
   fusionar capas de igual ajuste al exportar y recortar la velocidad al máximo de la máquina con aviso.
   Hoy el usuario con un SVG de 129 colores no puede exportar a Ruida y no sabe qué hacer.
5. **Avisar si una capa pide más velocidad que la máquina (medio, fácil).** El cálculo y el G-code pasan
   en silencio con 48 000 mm/min sobre un perfil de 12 000; relacionado con la divergencia del estimado
   en los proyectos de LightBurn.
6. **Velocidad del cálculo de recorridos en vacío (medio).** `reduce_travel` es cuadrático con trayectos
   sueltos (1 M de `LINE` tarda 40 s contra 1 s sin él). Usar una rejilla para el vecino más cercano o un
   tope de tiempo con aviso.
7. **Tamaño del G-code y enlace (medio).** Un mapa llega a 4 M de líneas y a 80 MB. Fusionar segmentos
   colineales o más cortos que la resolución de la máquina (por ejemplo 0,005 mm) reduciría líneas y
   tiempo de envío; y avisar en la lista de comprobación cuando las líneas por segundo exigidas superen
   lo que permite el baudio (el mapa de Europa lo hace por encima de 1 400 mm/min).
8. **Revisar el estimador con segmentos cortos y proyectos de LightBurn (bajo-medio).** Hasta +26 % y
   -40 % de diferencia con el emulador en archivos reales; el modelo es exacto en los casos simples.
   Crear un caso mínimo de cada grupo y comprobar si falla el estimador o el emulador.
9. **TIFF (bajo).** Soportar YCbCr (o mensaje que sugiera guardar como PNG) y subir el límite de memoria
   del decodificador (rechaza 87 Mpx mientras un PNG de 100 Mpx carga).
10. **Lienzo con selecciones enormes o trayectos gigantes (bajo).** Solo se nota con 100 000 objetos o un
    trayecto de 1 M de nodos: confirmar un movimiento con `map` sobre todos los objetos (0,57 s) y el
    hit-test y el arrastre del trayecto único (0,3 a 0,37 s). Candidatos: hit-test con índice por trozos
    (ya existen en `geom.ts`) y arrastre con una transformación en el renderer sin reconstruir.

## Después de la ola 6B (2026-10-14)

Arreglos de la lista de prioridades, medidos con `tlaku-bench worker` (release) y `canvas-stress.mjs`:

| Punto | Antes | Después |
|---|---|---|
| 1. Interior primero (cálculo) | amhara 29,6 s · cascadia 14,4 s · hooke 11,2 s | 1,1–1,5 s · 0,3 s · 0,6 s; mismo orden, tiempo y recorrido |
| 2. Imágenes de 60–108 Mpx | importar 1,3–2,3 GB; calcular 17–28 s | importar < 75 MB; calcular 2,5–6 s |
| 3. Causa al fallar la importación | «no contiene trayectorias» | dice qué se omitió y qué hacer; SVG con solo imagen se importa |
| 4. Ruida con 129 colores / 48 000 mm/min | no exporta | fusiona capas vecinas iguales; recorta a 36 000 y avisa |
| 5. Capa más rápida que la máquina | en silencio | aviso al calcular |
| 6. `reduce_travel`, 1 M de líneas sueltas | 40 s | 4,7 s, mismo resultado |
| 7. G-code del mapa de Alemania | 4,29 M de líneas | 3,00 M (−30 %), largo −0,09 %; aviso si el enlace no da abasto |
| 8. Estimado | +26 % / −40 % contra el emulador | el estimador es correcto; el emulador calcula mal las uniones (pendiente en 6C) |
| 9. TIFF YCbCr y 87 Mpx | error | cargan |
| 10. Lienzo | clic con 1 M de nodos 280 ms; mover 10 de 100 000 objetos 215 ms | 47 ms; 48 ms. Mover todo con 100 000 sigue en ~400 ms |

## Cómo repetirlo

```text
scripts/run-stress.sh /tmp/tlaku-stress        # descarga, genera y mide (modo línea con emulador y modo relleno)
cd apps/desktop && node scripts/canvas-stress.mjs
```

Los resultados por archivo de la corrida de este informe están en
[pruebas-estres-resultados.csv](pruebas-estres-resultados.csv).
