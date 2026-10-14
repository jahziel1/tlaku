# Contrato app-motor: Edición: curvas, grupos, texto, vectorizado y geometría

Parte de [el contrato app-motor](README.md). Curvas, grupos, texto, vectorizado, operaciones de geometría y de formas y arte; incluye los primeros comandos de cámara de la fase 3 (`detect_board`, `calibrate_lens`, marcas).

## Fase 3: curvas, grupos, texto, vectorizado y cámara

Versión de documento **2**. `load_project` migra la versión 1 (polilíneas) sin pérdida.

```ts
// tlaku_core::bezpath::PathSeg (coordenadas locales del objeto, mm)
type PathSeg =
  | { type: "move_to"; to: Point } | { type: "line_to"; to: Point }
  | { type: "quad_to"; ctrl: Point; to: Point }
  | { type: "cubic_to"; ctrl1: Point; ctrl2: Point; to: Point }
  | { type: "close" };
type BezPath = PathSeg[];

interface VectorObject { paths: BezPath[] }          // reemplaza PathData[] de la versión 1
interface DesignObject { /* ...lo anterior... */ group: string | null }  // grupos planos por id

interface TextObject {                // ObjectKind "text"
  text: string;                       // admite varias líneas
  font_family: string; bold: boolean; italic: boolean;
  size_mm: number;                    // tamaño del em (como en tipografía), no de la mayúscula
  letter_spacing_mm: number; line_spacing: number;     // interlineado en múltiplos del tamaño
  align: "left" | "center" | "right";
  on_path: { object_id: string; offset_mm: number; flip: boolean } | null;
  outline: BezPath[];                 // contorno ya generado por el motor; el pipeline usa este
}
```

| Comando | Entrada | Salida |
|---|---|---|
| `list_fonts` | | `{ family: string; styles: string[] }[]` |
| `text_outline` | `{ text: TextObject, path: BezPath \| null }` | `{ paths: BezPath[], warnings: string[] }` (un `BezPath` por glifo, en coordenadas locales del texto; las advertencias son sustituciones de fuente, caracteres sin glifo o texto que no cabe en la trayectoria) |
| `geometry_op` | `{ op: "union" \| "difference" \| "intersection" \| "xor" \| "weld" \| "offset", objects: DesignObject[], distance?: number, join?: "round" \| "miter" \| "bevel" }` | `DesignObject[]` (resultado en coordenadas de documento, transform identidad) |
| `shape_op` | `{ op: "trim" \| "break_apart" \| "optimize" \| "fillet" \| "cut_shapes" \| "resize_slots", objects: DesignObject[], params?: ShapeParams }` | `{ objects: DesignObject[], stats: ShapeStats }` (misma convención de resultado que `join`/`close`/`dedupe`) |
| `review_design` | `{ doc: Document, bed: [number, number], options?: ReviewOptions }` | `Finding[]` (lo grave primero; solo lee, no cambia nada) |
| `trace_image` | `{ path, settings: TraceSettings, size_mm: [number, number] }` | `{ paths: BezPath[], warnings: string[] }` |
| `vectors_to_bitmap` | `{ doc, selection: string[], dir: string \| null, stem, options: { dpi, filled, stroke_mm, invert } }` | `{ path, width_px, height_px, size_mm, origin_mm, in_temp_dir }` |
| `save_processed_image` | `{ image: ImageObject, layer: LayerSettings, path }` | `{ path, bytes, width_px, height_px }` |
| `preview_image` | `{ image: ImageObject, layer: LayerSettings, region_mm: [x0, y0, x1, y1] \| null, max_px: number }` | `{ png_base64, width_px, height_px, covered_mm, interval_mm, exact }`: lo que se grabaría de la imagen con los ajustes de `layer` (C51), sin calcular el trabajo. `region_mm` null = imagen entera en a lo sumo `max_px` puntos por lado (32 a 4096; con un intervalo más grueso que el de la capa si no cabe, y entonces `exact` es `false`); con rectángulo (mm locales de la imagen, Y hacia arriba) devuelve ese trozo a la rejilla real del trabajo. `covered_mm` es lo realmente cubierto |
| `fit_registration` | `{ pairs: { design: [x, y], physical: [x, y] }[], mode: 'auto' \| 'rigid' \| 'similarity' \| 'affine' }` | `{ ok: true, fit: RegistrationFit } \| { ok: false, error }`: Print & Cut (M37). `fit` trae `transform`, `mode` (el que se usó: `auto` y una afín imposible se resuelven aquí), `rotationDeg`, `scale`, `scaleXY`, `residuals`, `residualRms`, `residualMax` y `warnings` (`mirrored`, `scale {pct}`, `anisotropy {xPct, yPct}`). `error`: `few {need, have}`, `nan`, `close {a, b}` o `collinear`. Un ajuste imposible no es un error del comando |
| `plan_tiles` | `{ design: [[x0, y0], [x1, y1]], size: [w, h], overlap, order: 'rows' \| 'serpentine', margin }` | `{ ok: true, plan } \| { ok: false, error }`: tiling (M39). `plan` trae `tiles` (`{ spec: { rect, offset }, row, col }`, con `margin` ya sumado al `offset`), `cols`, `rows`, `colRanges`, `rowRanges` y `marks` (segmentos de las cruces de alineación, dos por cruz). `error`: `numbers`, `empty`, `size`, `overlapNegative`, `overlap {overlap, tile}` o `tooMany {count}` |
| `art_import_svg` | `{ svg, target_layer: string \| null }` | `ImportResult` |
| `art_list_user` | — | `{ name, svg }[]` (SVG de `<configuración>/art/`) |
| `art_add_user` | `{ paths: string[] }` | `{ added: { name, svg }[], errors: string[] }` |
| `art_remove_user` | `{ name }` | — |
| `art_save_user` | `{ items: { name, svg }[] }` | `{ added: { name, svg }[], errors: string[] }`: guarda dibujos que llegan con su texto (importar un `.tlakubundle`). Nunca pisa uno existente (`nombre`, `nombre-2`...); el nombre pierde la extensión y cualquier ruta; cada SVG debe leerse como dibujo (2 MB como máximo) |
| `detect_board` | `{ image_png_base64: string, board: [number, number] }` | `[number, number][] \| null`: esquinas internas del tablero, fila por fila, en píxeles de la imagen; `null` si no se ve completo |
| `calibrate_lens` | `{ images_png_base64: string[], board: [number, number], square_mm: number }` | `CalibrationReport` |
| `load_camera_setup` | | `CameraSetup`; sin `camera.json`, una configuración vacía. Un archivo dañado se aparta como `camera.json.dañado` y se devuelve una vacía |
| `save_camera_setup` | `{ setup: CameraSetup }` | `CameraSetup`: valida, recalcula la homografía de la alineación y escribe `camera.json` (atómico) en la carpeta de configuración |
| `fetch_camera_frame` | `{ url: string }` | `{ mime, data_base64 }`: un cuadro de una cámara de red (ver [camara.md](camara.md#cámaras-de-red)) |
| `save_board_file` | `{ path: string, content_base64: string }` | `void`: escribe el tablero de calibración (solo `.svg` o `.pdf`) en la ruta que eligió el usuario |
| `camera_marks` | `{ bed: [number, number], count: 4 \| 9 }` | `Document`: `count` cruces en posiciones conocidas de la cama (coordenadas absolutas, 85 % del ancho y alto), un objeto por marca, en una sola capa `line` de baja potencia (3000 mm/min, 10 %). La marca 1 (esquina inferior izquierda) lleva además un círculo que la rodea. La interfaz lo abre como documento nuevo |
| `detect_camera_marks` | `{ image_png_base64: string, bed: [number, number], count: 4 \| 9, prior_to_bed: Mat3 \| null }` | `MarksReport`: busca las marcas en una foto **ya corregida de lente**, las asigna a sus posiciones, ajusta la homografía con RANSAC y mide la precisión con validación cruzada. `prior_to_bed` (imagen a mm) es una alineación provisional, p. ej. la manual; sin ella la orientación la fija el círculo de la marca 1 |

Todos corren fuera del hilo principal y los argumentos van en `snake_case` (`image_png_base64`, `size_mm`...).

```ts
interface TraceSettings {
  threshold: number; invert: boolean; blur_px: number; speckle_px: number;
  corner_threshold_deg: number; smoothing: number; mode: "spline" | "polygon";
}

interface LensCalibration {            // modelo de Brown-Conrady
  width: number; height: number;       // tamaño de las imágenes con que se calibró
  fx: number; fy: number; cx: number; cy: number;
  k1: number; k2: number; p1: number; p2: number; k3: number;
  rms_error_px: number;                // error de reproyección rms, en píxeles
  views_used: number;
}
interface CalibrationReport {
  calibration: LensCalibration;
  warnings: string[];                  // vistas descartadas, pocas vistas, error alto...
  used_views: number[];                // índices (de `images_png_base64`) de las vistas usadas
  view_rms_px: number[];               // error rms de cada vista usada, en el mismo orden
}
type Mat3 = [[number, number, number], [number, number, number], [number, number, number]];

// Alineación entre la imagen ya corregida y la cama: manual de 4 puntos o con marcas grabadas.
interface CameraAlignment {
  image_size: [number, number];
  image_px: [Point, Point, Point, Point];   // píxeles de la imagen corregida (con marcas: las esquinas del rectángulo de marcas, proyectadas)
  bed_mm: [Point, Point, Point, Point];     // mm de documento, mismo orden
  to_bed: Mat3;                             // homografía px -> mm a la altura `reference_height_mm`
  rms_mm: number;
  // Campos nuevos (todos con valor por defecto al leer un camera.json anterior):
  method: "manual" | "marks";
  reference_height_mm: number;              // altura (mm sobre la cama) del plano de `to_bed`; 0 por defecto
  samples: AlignmentSample[];               // marcas por altura; vacío en la manual
  height: HeightModel | null;               // lo calcula el motor al guardar
}
interface AlignmentSample {                 // marcas usadas a una altura, en el orden de `bed_mm`
  height_mm: number; image_px: Point[]; bed_mm: Point[];
  loo_rms_mm: number | null; loo_max_mm: number | null;   // validación cruzada al detectar; informativo
}
// H(h) = [H1 H2 (H3 + (h - ref)·G)] de la cama (mm) a la imagen (px)
interface HeightModel {
  ref_height_mm: number;
  bed_to_image: Mat3;                       // a `ref_height_mm`, sin normalizar la escala
  gradient: [number, number, number];       // G: cambio de la tercera columna por mm de altura
  source: "measured" | "modelled";          // dos o más alturas | una sola alineación y la matriz de la cámara
  min_height_mm: number; max_height_mm: number;  // rango medido (con "modelled", solo `ref_height_mm`)
}
interface CameraPrefs { device_id: string | null; device_label: string | null;
                        width: number | null; height: number | null;
                        opacity: number /* 0..1 */; visible: boolean;
                        material_height_mm: number /* grosor del material, 0..300; 0 por defecto */ }
interface CameraSetup { version: 1; calibration: LensCalibration | null;
                        alignment: CameraAlignment | null; prefs: CameraPrefs }

interface MarkResult {
  index: number;                            // 0..count-1; las 4 primeras son las esquinas (inf. izq., inf. der., sup. der., sup. izq.)
  bed: Point;                               // posición teórica (mm)
  image: Point | null;                      // centro detectado (px); null si no se encontró
  confidence: number;                       // 0..1 (simetría de punto de la cruz)
  used: boolean;                            // participa en el ajuste final (RANSAC)
  residual_mm: number | null;               // error del ajuste en esa marca
  loo_mm: number | null;                    // error de validación cruzada (ajuste sin esta marca)
}
interface MarksReport {
  marks: MarkResult[];
  candidates: Point[];                      // todo lo que tenía forma de cruz (px), para dibujarlo
  to_bed: Mat3;                             // homografía imagen -> cama del ajuste
  expected: number; found: number; used: number;
  rms_mm: number; max_residual_mm: number;  // sobre las marcas usadas
  loo_rms_mm: number | null; loo_max_mm: number | null;  // null con menos de 5 marcas usadas
  scale_px_per_mm: number;
  used_prior: boolean;
  warnings: string[];
}
```

Convenciones que fija el motor:

- **Trayecto del texto en coordenadas locales.** `text_outline` devuelve el contorno en coordenadas
  locales del texto, y `path` (la curva guía de un texto sobre trayectoria) también debe ir en
  coordenadas LOCALES del texto: la interfaz lo calcula aplicando la inversa del `transform` del
  texto a la guía (`transform_texto⁻¹ · transform_guía`). No va en coordenadas de documento. El
  contorno ya generado (`outline`) no interviene: es justo lo que se recalcula. El pipeline usa el
  `outline` guardado en el documento, así que abrir un proyecto no necesita el motor de texto.
- **`geometry_op`.** Los objetos llegan con sus curvas y su `transform` (coordenadas de documento) y
  el resultado vuelve como objetos vectoriales nuevos con transform identidad. En las booleanas
  (`union`, `difference`, `intersection`, `xor`) el **primer objeto de la lista es A** y todos los
  demás forman el conjunto B: `difference` es A menos B, `intersection` es A con la unión de B y
  `xor` es A contra la unión de B. Quien llama decide cuál es A (la interfaz deja elegirlo; el
  orden de selección nunca es implícito). Un grupo se manda como un único objeto con todas sus
  curvas. `weld` une todos en una región (conserva agujeros e islas). `offset` desplaza cada objeto
  por separado y **conserva la capa de cada objeto**; una booleana o `weld` dejan el resultado en la
  capa del primer operando. `distance` positiva = hacia afuera. Las trayectorias abiertas no
  participan: se rechazan diciendo qué objetos son.
- **Cámara: las imágenes viajan como PNG en base64, no como rutas.** `detect_board` y
  `calibrate_lens` reciben fotogramas sueltos que la interfaz captura del video (con o sin prefijo
  `data:image/png;base64,`); el motor no lee archivos del disco para esto. `calibrate_lens` exige al
  menos 3 vistas con el tablero visible (omite con un aviso las que no lo tienen).
- **No hay `undistort_frame`.** El video nunca pasa por el motor: la corrección de la lente en vivo
  se hace en WebGL en el frontend con la `LensCalibration` guardada, y la imagen ya corregida se
  dibuja bajo el diseño con la homografía de `CameraAlignment`. El motor solo detecta el tablero,
  calibra y guarda la configuración.
- **`camera.json`** vive en la carpeta de configuración de la app junto a `devices.json`; en modo
  mock (navegador) la misma configuración se guarda en memoria y `localStorage`. Los campos nuevos de
  `CameraAlignment` y `CameraPrefs` llevan valor por defecto: un `camera.json` anterior carga igual
  (alineación manual, altura de referencia 0, sin capturas, grosor 0).
- **Coordenadas de píxel de la alineación: el origen es el borde de la imagen** (la esquina superior
  izquierda es `(0, 0)` y el centro del primer píxel `(0.5, 0.5)`), igual que `backdropCorners`. Lo
  respetan los puntos de `detect_camera_marks` (entrada `prior_to_bed` y salida) aunque el motor trabaje
  internamente con el centro del píxel en `(i, j)`.
- **Marcas grabadas.** `detect_camera_marks` busca cruces con el tensor de estructura de los gradientes
  y las valida por simetría de punto; las asigna a sus posiciones (con la alineación provisional, por
  cercanía; sin ella, por una búsqueda de hipótesis que descarta espejos y fija el giro con el círculo de
  la marca 1, así funciona aunque la cámara esté girada 180 grados o falte alguna marca); refina cada
  centro ajustando las rectas de los dos brazos y cortándolas (subpíxel); ajusta la homografía con RANSAC.
  **Precisión real**: con 5 o más marcas usadas, `loo_*` es la validación cruzada de dejar una fuera, en mm
  de cama, y es conservadora (la marca dejada fuera se extrapola); con 4 marcas el ajuste es exacto y
  `loo_*` es `null`. Un error mayor a 0,3 mm en alguna marca se señala en `warnings`. Falla con un mensaje
  accionable si hay menos de 4 cruces medibles.
- **Corrección por altura.** `save_camera_setup` recalcula todo lo derivado de la alineación. Con
  `samples` de dos o más alturas (a ≥ 2 mm de diferencia) ajusta `H(h) = [H1 H2 (H3 + (h - ref)·G)]` con
  una DLT conjunta (sin conocer la matriz de la cámara, `source = "measured"`); `to_bed` pasa a ser la
  homografía a la altura más baja (`reference_height_mm`). Una captura a la misma altura (± 0,25 mm)
  reemplaza a la anterior. Con una sola altura (o con la alineación manual) y la lente calibrada, el
  modelo se estima con la matriz de la cámara (`source = "modelled"`, aproximado); sin lente, `height`
  es `null` y el grosor no corrige. El fondo del lienzo usa la inversa de `H(prefs.material_height_mm)`;
  fuera del rango medido la corrección extrapola (la interfaz lo avisa).

`geometry_op` admite además `join`, `close` y `dedupe` con `tolerance` (mm; por defecto 0.05, 0.5
y 0.01). Solo aceptan objetos vector y nunca mezclan capas. Devuelven la lista completa de los
objetos de entrada en su orden: los que no cambian vuelven idénticos (mismo id); cada objeto que
cambia se sustituye por uno nuevo (`join-N`, `close-N`, `dedupe-N`); un objeto que queda sin trazo
no vuelve. La interfaz quita los ids que no volvieron y pone los nuevos en su lugar, en un solo
paso de deshacer.

`shape_op` (recortar, separar, optimizar, redondear esquinas, cortar formas y redimensionar ranuras)
sigue la misma convención de resultado y devuelve además `stats` (cuántas cosas se hicieron, nodos
antes y después, anchos de ranura detectados, puntos del tramo que se borraría). Cada operación lee
solo sus campos de `params` (ver `edit/shape_ops.rs`), así que la interfaz puede llamarla sin aplicar
el resultado para mostrar una vista previa. Solo acepta objetos vectoriales.

`review_design` («Revisar diseño», sin IA) analiza el documento y devuelve hallazgos; los arreglos los
aplica la interfaz con `geometry_op` (`join`/`close`/`dedupe`) y `shape_op` (`optimize`), cada uno como
un solo paso de deshacer. `bed` es `[ancho, alto]` en mm. Cada `Finding` trae `key` (estable para el
mismo contenido), `kind` (`near_closed`, `overlap`, `joinable`, `tiny`, `extra_nodes`, `outside_all`,
`outside_part`, `text`, `silent_layer`, `hidden_layer`, `image_dpi_high`, `image_dpi_low`), `severity`
(`serious`, `warning`, `info`), `layer` (`#rrggbb` o `null`), `object_ids`, `fix_ids` (solo si la operación
necesita más objetos que los afectados: quitar duplicados), `count`, `value`, `detail` y `tolerance` (mm,
la misma con la que se arregla). `options` (todo opcional): `close_mm` 0,5, `join_mm` 0,1, `dedupe_mm` 0,01,
`tiny_mm` 0,2, `node_mm` 0,02, `dpi_high` 2 y `dpi_low` 0,5. Los ocultos, las guías y las capas sin salida
no entran en el análisis geométrico, y los bloqueados tampoco en lo que se arregla editando su trazo.
