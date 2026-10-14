# Contrato entre la app (Tauri) y el motor

Fase 2. La interfaz (TypeScript) es dueña del documento que se edita; el motor (Rust) no
guarda estado del documento: recibe el documento completo y devuelve resultados. La única
excepción es la conexión con la máquina, que vive en Rust.

Los tipos se definen en Rust (crate `tlaku-doc`) con `serde` y se exportan a TypeScript con
`ts-rs` en `apps/desktop/src/bindings/` (`cargo test -p tlaku-doc` los regenera; la ruta sale de
`.cargo/config.toml`). Este archivo describe la forma; el código manda.
JSON con `snake_case`, unidades en mm, mm/min y %, origen abajo a la izquierda, Y hacia arriba.

## Documento

```ts
type Point = [number, number];

type BezPath = PathSeg[];   // curvas en coordenadas locales del objeto; ver "Fase 3"

interface DesignObject {
  id: string;                       // único en el documento
  layer: string;                    // color de capa "#rrggbb"
  transform: [number, number, number, number, number, number]; // afín [a b c d e f]: x' = a·x + c·y + e
  kind: ObjectKind;
  locked: boolean;
  hidden: boolean;
}
// el discriminante `type` vive en ObjectKind; VectorObject e ImageObject no lo llevan
type ObjectKind = ({ type: "vector" } & VectorObject) | ({ type: "image" } & ImageObject);
interface VectorObject { paths: BezPath[] }
interface ImageObject  { source_path: string; width_px: number; height_px: number;
                         size_mm: [number, number] }   // caja local [0,w]x[0,h]; el transform la coloca y escala

interface LayerSettings {
  color: string; name: string;
  mode: "line" | "fill" | "image";
  output: boolean;                  // se envía a la máquina
  visible: boolean;                 // se dibuja en el lienzo
  speed: number; power: number; min_power: number; passes: number; air: boolean;
  interval: number; angle: number; crosshatch: boolean; bidirectional: boolean;
  overscan: "auto" | number;
  kerf: number;                     // solo line; positivo = la pieza sale del tamaño dibujado
  image: ImageSettings;
  order: number | null;             // posición forzada; null = automático
}
interface ImageSettings { mode: "threshold" | "floyd" | "jarvis" | "stucki" | "atkinson" | "bayer4" | "grayscale";
                          threshold: number; brightness: number; contrast: number; gamma: number;
                          invert: boolean; dot_width: number }

interface Document { version: 2; objects: DesignObject[]; layers: LayerSettings[]; device_id: string | null }
```

Semántica que fija el motor (`tlaku-pipeline`):

- Los colores (`layer`, `color`) se normalizan a minúsculas al leerlos. `Document::from_json`
  rechaza versiones distintas de 1 con un error claro.
- `layers` es el "orden del archivo" (`layer_order: "file"`). Una capa sin objetos que salgan no
  cuenta. Un objeto cuya capa no existe en `layers` es un error.
- Sale a la máquina todo objeto no `hidden` cuya capa tenga `output` (`visible` solo afecta al
  lienzo; `locked` no influye). Un vector en una capa `image` (o al revés) se omite con una advertencia.
- Vectores: `transform` afín arbitrario. Imágenes: solo traslación, escala y reflejo (sin
  rotación ni sesgo): con rotación el objeto se omite con una advertencia.
- `order` es la posición (desde 1) en el orden final; no puede repetirse entre capas con salida.
- Una imagen importada mide `px * 25.4 / 96` mm (96 ppp) y va en la capa gris `#808080`
  (modo `image`, 3000 mm/min); los vectores importados van cada color a su capa `line` nueva.
- Capas `line` usan speed/power/passes/air/kerf; `fill` añade interval/angle/crosshatch/
  bidirectional/overscan; `image` usa speed/power (máxima)/min_power/passes/air/interval/
  bidirectional/overscan/`image`.

## Dispositivo y opciones de trabajo

```ts
interface DeviceProfile {
  id: string; name: string; kind: "grbl";
  bed: [number, number]; origin: "bottom_left" | "top_left" | "bottom_right" | "top_right";
  s_max: number; power_mode: "dynamic" | "constant";
  accel: [number, number]; max_rate: [number, number]; rapid_rate: number;
  baud: number; rx_buffer: number;
  start_gcode: string; end_gcode: string; return_to_origin: boolean;
}
interface JobOptions {
  start_from: "absolute" | "current_position" | "user_origin";
  job_origin: number;               // 0..8, fila por fila desde arriba a la izquierda
  user_origin: Point | null;       // coordenadas de la MÁQUINA (las que muestra GRBL)
  selection: string[] | null;       // ids; null = todo lo que tenga output
  layer_order: "auto" | "file";
}
```

- La posición actual de la cabeza y `user_origin` están en coordenadas de la máquina; el motor
  las pasa a coordenadas del documento según `DeviceProfile.origin`.
- `absolute`: coordenadas del documento tal cual. `current_position` y `user_origin`: el punto
  `job_origin` del rectángulo envolvente de lo que se envía (sin overscan) coincide con la
  posición actual / `user_origin` (0 = arriba-izquierda, 4 = centro, 6 = abajo-izquierda, como
  LightBurn). El motor no conoce la posición de la cabeza: la app la pasa a `compute_at`
  (posición de trabajo, la misma de `jog`); sin ella `current_position` es un error.
- `selection` con ids que no existen avisa; un objeto seleccionado pero `hidden` o de una capa
  sin `output` no sale.
- `DeviceProfile.origin` es la esquina donde la máquina tiene su X0 Y0: el G-code crece hacia
  dentro de la cama desde ella. El documento y la vista previa siempre usan origen abajo a la
  izquierda; solo `export_gcode`/`start_job` convierten (`top_left`: `y' = alto - y`, etc.).
- `start_gcode`/`end_gcode` vacíos significan "ninguno"; `power_mode` se aplica a todos los bloques.

## Comandos (`invoke`)

Los argumentos van como objeto con estas claves; los errores llegan como texto en español listo
para mostrar (la promesa se rechaza con un `string`). Los comandos pesados (`import_file`,
`compute_job`, `export_gcode`, `start_job`...) corren fuera del hilo principal.

| Comando | Entrada | Salida |
|---|---|---|
| `import_file` | `{ path }` | `{ objects: DesignObject[], new_layers: LayerSettings[], warnings: string[] }`; error si el formato no se soporta o el archivo no tiene trayectorias. La ruta se vuelve absoluta (`source_path` de las imágenes) y se permite en el protocolo de assets |
| `compute_job` | `{ doc, device, options }` | `JobPreview`; error de texto si algo no cabe en la cama (con overscan) o un ajuste es inválido. Con máquina conectada usa su posición de trabajo para `current_position`; sin ella, ese modo es un error |
| `export_gcode` | `{ doc, device, options, path }` | `{ lines: number, estimate_s: number, warnings: string[] }`; escribe el archivo (atómico) |
| `save_project` / `load_project` | `{ path, doc }` / `{ path }` | `void` / `Document` (extensión `.tlaku`, JSON; `load_project` rechaza versiones desconocidas y permite las imágenes del documento en el protocolo de assets) |
| `list_devices` / `save_device` / `delete_device` | `{}` / `{ device }` / `{ id }` | perfiles guardados en `devices.json` dentro de la carpeta de configuración de la app (`app_config_dir`). Sin archivo, `list_devices` devuelve el perfil de fábrica (`DeviceProfile::default()`) sin guardarlo; `save_device` crea o reemplaza por `id` y valida lo básico (cama, S máx., aceleraciones, baudios); `delete_device` falla si el id no existe |
| `list_ports` | | `string[]` |
| `connect` / `disconnect` | `{ port, baud, device? }` / `{}` | `{ firmware: string }` (`Grbl 1.1h`, de `$I`) / `void`. `connect` abre el puerto (~2 s por el reinicio de la placa) y falla si hay otra conexión o la máquina no responde a `$I`; `disconnect` cancela un trabajo en curso antes de soltar el puerto y no falla si no hay conexión |
| `machine_command` | `{ line }` | `string[]`: líneas de respuesta, la última `ok`, `error:N: explicación` o `ALARM:N: explicación`. Rechaza `?`, `!`, `~` (usa `machine_realtime`) y falla con un trabajo en curso. `$H` espera hasta 60 s; el resto 5 s |
| `machine_realtime` | `{ cmd: "status" \| "hold" \| "resume" \| "reset" \| "jog_cancel" }` | `void`. Un byte (`?`, `!`, `~`, 0x18, 0x85). Durante un trabajo, `hold`/`resume` pausan y reanudan, `reset` lo cancela |
| `jog` | `{ dx, dy, speed }` | `void`: `$J=G91 G21 X.. Y.. F..` (mm, mm/min). Falla con un trabajo en curso o en alarma (con el consejo de `$X`/`$H`) |
| `start_job` | `{ doc, device, options }` | `void`. Genera el G-code, lo valida contra `rx_buffer` y aplica el mismo preflight que `tlaku send`: consulta `?` y **no transmite nada** si la máquina está en Alarm, Hold, Door, Run, Jog, Home, Check o Sleep; el error explica por qué y qué hacer (en la consola de la pestaña Máquina). La app nunca envía `$X`, `$H` ni `~` por su cuenta |
| `pause_job` / `resume_job` / `cancel_job` | | `void`. Pausa = feed hold; cancelar = feed hold y soft reset (láser apagado). Sin trabajo, `pause`/`resume` no hacen nada y `cancel_job` cancela el jog (0x85) |
| `frame_job` | `{ doc, device, options, speed }` | `void` (láser apagado, `G1 S0` bajo `M5`, rectángulo con overscan en coordenadas de máquina); mismo preflight que `start_job`. No emite `machine://job` ni `machine://progress` salvo error o cancelación |

```ts
interface JobPreview {
  layers: { color: string; mode: string; seconds: number; summary: string;
            travel_saved_pct: number | null; explanations: string[] }[];
  moves: number[];                     // plano: [kind, x, y, power] por movimiento; kind 0 = travel, 1 = cut (power 0 = overscan)
  move_times: number[];                // segundos acumulados al final de cada movimiento: move_times[i] <-> moves[4*i..4*i+4]
  total_s: number;                     // = último move_times
  bounds: [Point, Point];              // incluye overscan
  order_note: string | null;
  warnings: string[];
}
interface ExportResult  { lines: number; estimate_s: number; warnings: string[] }
interface ConnectResult { firmware: string }
type RealtimeCommand = "status" | "hold" | "resume" | "reset" | "jog_cancel";
```

Una entrada de `layers` por bloque en orden de ejecución (una capa de imagen con varias
imágenes da una por imagen); `explanations` tiene el motivo de cada figura de las capas `line`.
`move_times` sale de `tlaku-sim` (aceleración por eje y desviación de unión, el mismo modelo de
`total_s`): la vista previa lo usa para reproducir el trabajo en tiempo real (busca el primer
`i` con `move_times[i] >= t`); un movimiento de longitud cero repite el instante anterior.
En Rust (`tlaku-pipeline`): `compute`/`compute_at` verifican la cama; `build` calcula igual sin
verificarla (el CLI lo usa en `plan`); `gcode(&ComputedJob, &DeviceProfile)` genera el G-code y
`frame_gcode(&ComputedJob, &DeviceProfile, speed)` el del encuadre.

## Eventos (`listen`)

| Evento | Carga |
|---|---|
| `machine://status` | `StatusReport` |
| `machine://progress` | `{ acked, total }` (líneas de G-code confirmadas; como máximo cada 100 ms, el último siempre sale) |
| `machine://log` | `{ dir: "in" \| "out", text }`. Los reportes `?` no se registran; los `error:N` y `ALARM:N` llegan con su explicación |
| `machine://job` | `{ state: "started" \| "paused" \| "finished" \| "cancelled" \| "error", message }` |
| `machine://connection` | `{ connected: boolean, port: string \| null, error: string \| null }`. `error` solo se llena si la conexión se perdió (cable, silencio de más de 5 s, fallo de E/S) |

```ts
interface StatusReport {
  state: "idle" | "run" | "hold" | "jog" | "alarm" | "door" | "check" | "home" | "sleep";
  machine_pos: [number, number];   // mm
  work_pos: [number, number];      // mm; la misma de `jog` y de `current_position`
  feed: number;                    // F, mm/min
  spindle: number;                 // S, 0..s_max
  buffer: { planner: number; rx: number };  // bloques y bytes libres
}
```

`work_pos` = `machine_pos` - `WCO` (o al revés si GRBL reporta `WPos`); el `WCO` llega cada ~10
reportes y hasta entonces se asume 0. Sin `Bf:` en el reporte, `buffer` repite el último valor
(al inicio, la cola vacía: 15 bloques y 127 bytes). El estado se emite cada ~250 ms en reposo y
cada ~200 ms durante un trabajo (lo emite el propio streamer).

## Conexión con la máquina (concurrencia)

Un hilo dedicado es dueño del puerto serie; los comandos le llegan por un canal y esperan su
respuesta. En reposo sondea `?` y registra lo que llegue; al empezar un trabajo hace el preflight,
le entrega el puerto a un `Streamer` (conteo de caracteres) en otro hilo y traduce sus eventos;
al terminar recupera el puerto. Con un trabajo en curso solo se aceptan pausa, reanudación,
cancelación y bytes de tiempo real (`machine_command`, `jog` y otro `start_job` fallan con
"Hay un trabajo en curso..."). Cerrar la app cancela el trabajo (feed hold y soft reset).

## Imágenes

`ImageObject.source_path` es una ruta absoluta del disco. La interfaz la muestra con
`imageUrl(path)` (de `src/api`): `convertFileSrc` (protocolo `asset://`) en Tauri y un
marcador de posición en el mock. El alcance estático del protocolo (`tauri.conf.json`) cubre el
home del usuario (sin `Library` ni carpetas ocultas), Documentos, Escritorio, Descargas,
Imágenes y los volúmenes montados; además `import_file` y `load_project` permiten las rutas
exactas de las imágenes que devuelven. La CSP ya tiene `img-src ... asset: http://asset.localhost`.

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
| `trace_image` | `{ path, settings: TraceSettings, size_mm: [number, number] }` | `{ paths: BezPath[], warnings: string[] }` |
| `vectors_to_bitmap` | `{ doc, selection: string[], dir: string \| null, stem, options: { dpi, filled, stroke_mm, invert } }` | `{ path, width_px, height_px, size_mm, origin_mm, in_temp_dir }` |
| `save_processed_image` | `{ image: ImageObject, layer: LayerSettings, path }` | `{ path, bytes, width_px, height_px }` |
| `art_import_svg` | `{ svg, target_layer: string \| null }` | `ImportResult` |
| `art_list_user` | — | `{ name, svg }[]` (SVG de `<configuración>/art/`) |
| `art_add_user` | `{ paths: string[] }` | `{ added: { name, svg }[], errors: string[] }` |
| `art_remove_user` | `{ name }` | — |
| `detect_board` | `{ image_png_base64: string, board: [number, number] }` | `[number, number][] \| null`: esquinas internas del tablero, fila por fila, en píxeles de la imagen; `null` si no se ve completo |
| `calibrate_lens` | `{ images_png_base64: string[], board: [number, number], square_mm: number }` | `CalibrationReport` |
| `load_camera_setup` | | `CameraSetup`; sin `camera.json`, una configuración vacía. Un archivo dañado se aparta como `camera.json.dañado` y se devuelve una vacía |
| `save_camera_setup` | `{ setup: CameraSetup }` | `CameraSetup`: valida, recalcula la homografía de la alineación y escribe `camera.json` (atómico) en la carpeta de configuración |
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

## Fase 4: taller (sin hardware)

Campos nuevos del contrato (todos con valor por defecto al leer documentos y perfiles viejos):

- `LayerSettings.fill_style`: `scan` (barrido) u `offset` (contornos concéntricos).
- `LayerSettings.tabs`: puentes de las capas line (`enabled`, `width`, `count`, `spacing`,
  `skip_inner`). Los huecos aparecen en la vista previa y en el G-code.
- `LayerSettings.z_offset` / `z_step`: profundidad del foco al empezar la capa y bajada por
  pasada, **relativas a la altura del cabezal al empezar el trabajo**. Positivo = foco más hacia
  dentro del material (el cabezal baja o la cama sube). Solo se emiten si `DeviceProfile.z_axis`
  es verdadero; si no, el trabajo avisa y los ignora.
- `LayerSettings.angle` ahora también es el ángulo de barrido de las capas image.
- `ImageMode`: `halftone`, `newsprint` y `sketch`; `ImageSettings.halftone_cell` (mm) y
  `halftone_angle` (grados).
- `DeviceProfile.rotary`: rotativo en lugar del eje Y (`roller` o `chuck`). El documento sigue en
  mm de superficie de la pieza; la conversión a mm de Y de la controladora se hace al generar el
  G-code y el encuadre.
- `DeviceProfile.scan_offsets`: corrección de ida y vuelta por velocidad (interpolada).
- `ToolpathBlock.z_mm` (motor): profundidad del foco respecto a la altura inicial al empezar el
  bloque (0 = no mover), `None` = no mover Z.

Campos nuevos de la revisión del taller (`docs/hardware/taller.md`, `dialectos-gcode.md`); un
perfil guardado sin ellos se lee con los valores indicados y produce el mismo G-code que antes
(salvo Z, que ahora es relativa, y el barrido, que ahora se reparte):

- `DeviceKind` pasa de `grbl` a `grbl | grbl_m3 | grbl_hal | fluid_nc | grbl_lpc | smoothie |
  marlin | custom_gcode`.
- `DeviceProfile.dialect: GcodeDialect`: cómo escribe el G-code el firmware (escala y decimales
  de S, línea de encendido `constant_on`/`dynamic_on` (`dynamic_on` nulo = no hay M4),
  `laser_off`, `s_inline` o línea de potencia aparte `power_cmd` con `{s}`, `g0_laser_off`,
  `travel_s0`, `feed_on_g0`, `air_on`/`air_off` (nulos = sin aire), `end_sync` (`M400`),
  `comment` (`semicolon` o `parentheses`), `max_line_len`). Si falta al leer, se toma el del
  `kind` (`GcodeDialect::for_kind`). La escala de S sigue siendo `DeviceProfile.s_max`. La interfaz
  debe aplicar `DeviceProfile::for_kind(kind)` (dialecto, `s_max`, `power_mode`, `baud`) al
  cambiar el tipo de un perfil nuevo y dejar editar los campos en `custom_gcode`.
- `DeviceProfile.z_invert` (por defecto `false`): solo afecta al G-code. Sin él, profundidad
  positiva sale como Z negativa (en GRBL +Z aleja el cabezal); con él, al revés. Prueba: con
  `z_offset = 1` el foco debe entrar 1 mm en el material.
- `DeviceProfile.scan_initial_offset` (mm, por defecto 0): corre todas las líneas de barrido
  (también las unidireccionales) en el sentido de las líneas de ida.
- `ScanOffset.offset` es la separación **medida** entre los extremos de ida y de vuelta; se reparte
  `-offset/2` (ida) y `+offset/2` (vuelta). Avisa si el overscan es menor que |offset|.
- `RotarySettings.mirror` (por defecto `false`): refleja la Y del trabajo dentro de su caja en el
  G-code. Los valores por defecto del rotativo pasan a 32 mm/vuelta y rodillo de 17 mm
  (`RotarySettings::preset(kind)` da 40 mm/vuelta para `chuck`); con `roller` no se valida
  `object_diameter`.
- Avisos nuevos en `JobPreview.warnings`: puentes frente al kerf, soft limits del rotativo
  (`ALARM:2`), overscan menor que el offset, y limitaciones del dialecto (sin aire, sin potencia
  dinámica, imágenes con S en línea aparte).
- Motor: `tlaku_pipeline::rotary_test(device, speed, power)` devuelve un `Document` con una línea
  de exactamente una circunferencia de la pieza (prueba de una vuelta); pasa por `compute` y
  `gcode` como cualquier diseño. Comando de Tauri: `rotary_test`.

Comandos nuevos:

| Comando | Argumentos | Resultado |
|---|---|---|
| `load_material_library` | — | `MaterialLibrary` (vacía si no hay archivo) |
| `load_factory_materials` | — | `MaterialLibrary` de fábrica (solo lectura; instala o actualiza su copia `materials-factory.json`, nunca toca `materials.json`) |
| `save_material_library` | `library` | — |
| `import_material_library` | `path` | `MaterialLibrary` del archivo `.tlakumat` |
| `export_material_library` | `path`, `library` | — |
| `material_test` | `params: MaterialTestParams` | `Document` con la cuadrícula |

Contrato de la biblioteca de materiales (ola 5.5): cada `MaterialPreset` puede traer, todos
opcionales para no romper bibliotecas guardadas, `role` (`cut`/`engrave`/`score`), `machine_class`
(`diode_5|10|20|40`, `co2_40|60|80|100`, `fiber_20|30|50`), `source` (dirección https de donde salen
los valores), `confidence` (`official`/`guide`/`forum`) y `warnings`. Los datos de fábrica viven en
`crates/tlaku-doc/data/materials-default.json` (formato compacto, validado por `check_factory`: todo
ajuste exige fuente y confianza) y `tlaku_doc::factory_library()` los expande. La interfaz elige el
ajuste de una máquina con `pickPreset` (`features/materials/pick.ts`): primero el de la máquina,
luego el de su clase, luego uno sin clase y al final la clase más cercana por potencia del mismo
tipo de láser, avisando que hay que probar.

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

La biblioteca vive en `materials.json` dentro de la carpeta de configuración de la app (escritura
atómica; un archivo dañado se aparta como `materials.json.dañado`, uno de versión desconocida da
error sin tocarlo). Los archivos `.tlakumat` son el mismo JSON de `MaterialLibrary`.

`PreviewLayer.summary` puede incluir ", N puentes", ", pasada k/N" y ", Z x mm". Con `z_step != 0`
y varias pasadas, una capa produce una fila por pasada.

### Máquinas (fase 4, oleada C)

| Comando | Argumentos | Resultado |
|---|---|---|
| `device_defaults` | `kind` | `DeviceProfile` con el dialecto, S máxima, modo de potencia y baudios del tipo |
| `rotary_test` | `device`, `speed`, `power` | `Document` con una línea de exactamente una vuelta |
| `probe_port` | `port`, `baud` | `ProbeResult` (familia, firmware, banner, `$I`/`M115`, `$$`). Solo escribe `?`, `0x18`, `$I`, `$$` y `M115`; nunca mueve la máquina ni cambia ajustes |
| `simulate_fault` | `fault` | Inyecta una falla (llama, inclinación, alarma 3, desconexión). Solo con máquina virtual |

`list_ports` agrega al final los puertos virtuales `virtual:falcon2`, `virtual:grbl11`,
`virtual:grblhal`, `virtual:fluidnc`, `virtual:marlin` y `virtual:smoothie` (emulador
`tlaku-emu`). La velocidad del tiempo va en el id: `virtual:falcon2@5`.

El envío depende de la familia del perfil: GRBL con conteo de caracteres; Marlin y Smoothie línea a
línea, sin posición durante el trabajo, y cancelar siempre marca `JobEvent.position_lost`.
`MachineState` incluye `other` para estados que no conoce (por ejemplo `Tool` de grblHAL), con el
nombre original en `StatusReport.other_state`.

## Aplicación: preferencias, atajos y autoguardado

Código: `src-tauri/src/prefs.rs`, `src-tauri/src/autosave.rs`; en la interfaz `features/settings/`
y `features/recovery/`.

| Comando | Argumentos | Resultado |
|---|---|---|
| `load_preferences` | — | `PrefsFile` o `null` si no hay archivo. Uno dañado se aparta como `.dañado`; una versión desconocida es un error y no se toca |
| `save_preferences` | `data` (`PrefsFile`) | Escribe `preferences.json` de forma atómica |
| `export_preferences` | `path`, `bundle` | Escribe un paquete `.tlakuprefs` |
| `import_preferences` | `path` | `PrefsBundle` (valida `format` y `version`; máximo 1 MB) |
| `paths_exist` | `paths` | `bool[]`: si cada archivo existe (limpia «recientes») |
| `autosave_write` | `id`, `name`, `path`, `doc` | `RecoveryInfo`; reemplaza la copia `id` y recorta las más viejas |
| `autosave_list` | — | `RecoveryInfo[]` pendientes, de la más nueva a la más vieja; borra las superadas por un guardado posterior |
| `autosave_read` | `id` | `Document` |
| `autosave_discard` | `id` | Borra la copia (no es error que no exista) |

### Plantillas y ventanas (ola 3)

Código: `src-tauri/src/templates.rs` y `src-tauri/src/window.rs`; en la interfaz `features/shell/` (`templates.ts`, `TemplatesDialog.tsx`).

| Comando | Argumentos | Resultado |
|---|---|---|
| `list_templates` | — | `TemplateInfo[]` (`name`, `saved_at_ms`, `size_bytes`), por nombre |
| `save_template` | `name`, `doc` | `TemplateInfo`; reemplaza la plantilla del mismo nombre. El nombre es el del archivo `<nombre>.tlakutemplate` y no admite separadores de ruta ni caracteres reservados |
| `load_template` | `name` | `Document` |
| `delete_template` | `name` | Borra la plantilla (no es error que no exista) |
| `new_window` | — | Etiqueta de la ventana nueva (`doc-N`). Cada ventana tiene su propio documento; comparten la máquina y las preferencias |

Las plantillas viven en `<app_config_dir>/plantillas/`. La capacidad `capabilities/default.json` cubre las
ventanas `main` y `doc-*`.

- **Preferencias**: `preferences.json` en `app_config_dir()` (sobrevive a reinstalar y se puede copiar).
  Forma: `{ "version": 1, "prefs": { ... }, "shortcuts": { "<id de comando>": ["Mod+K", ...] } }`.
  El catálogo de claves, valores de fábrica y límites está en `src/store/prefs.ts` (`PREF_SPECS`); Rust
  no conoce cada clave, solo valida el sobre. `localStorage` guarda una copia para arrancar sin parpadeo.
- **Paquete `.tlakuprefs`**: el mismo JSON más `"format": "tlaku-prefs"`. Puede ser parcial (por ejemplo solo
  `shortcuts`): al importar, lo que falta se deja como está y lo que no se entiende se descarta con aviso.
- **Atajos**: `shortcuts` es el reemplazo completo de los atajos de un comando (`[]` = sin atajo); los
  comandos que no aparecen conservan los de fábrica. Se guardan con `Mod` (Cmd en macOS, Ctrl en el resto).
- **Recuperación**: `<datos de la app>/recuperacion/<id>.tlaku` y `<id>.meta.json`. Máximo 10 copias, 128 MB
  por copia y 512 MB en total. Una copia de un proyecto con ruta se descarta si el archivo tiene fecha igual o
  posterior; las de documentos sin guardar se ofrecen siempre.

## Conexión por red y automatización por UDP (ola 4)

### Conexión por red (GRBL, grblHAL, FluidNC)

`DeviceProfile.connection` elige el transporte: `{ "type": "serial" }` (por omisión; el puerto se
escoge al conectar), `{ "type": "tcp", "host", "port" }` o `{ "type": "web_socket", "url" }`
(`udp` es de Ruida). La interfaz entrega al motor, en el mismo argumento `port` de `connect` y
`probe_port`, un identificador por esquema; ningún puerto serie empieza así:

| Identificador | Qué abre |
|---|---|
| `tcp://host[:puerto]` | Telnet o TCP crudo; puerto 23 si falta (grblHAL Ethernet, servidor Telnet de FluidNC). Se rechaza y descarta la negociación Telnet si el servidor la manda |
| `ws://host[:puerto][/ruta]` | WebSocket de FluidNC y ESP3D. `wss://` no está soportado. Los mensajes de sesión de ESP3D (`currentID:`, `activeID:`, `PING:`) se descartan |
| `virtualnet:<modelo>[@<velocidad>]` | Máquina virtual por red: el emulador detrás de un socket TCP local en `127.0.0.1` (puerto libre) al que la app se conecta de verdad. Los modelos y la velocidad son los de `virtual:`. Al soltar la conexión se cierra el socket y se detiene el emulador. `list_ports` los ofrece al final |

Detalles:

- Por red no hay reinicio de la placa: tras conectar solo se espera y descarta un saludo breve (hasta 1,5 s)
  antes de `$I`. El `ok` puede llegar partido en varios paquetes; el lector de líneas trabaja byte a byte.
- Los errores de conexión llegan explicados en español: nombre no encontrado, conexión rechazada,
  tiempo agotado (5 s), equipo no alcanzable, conexión cerrada por la máquina.
- `probe_port` acepta esos identificadores («Probar conexión» del perfil): abre, lee el saludo, hace las
  mismas consultas de solo lectura de siempre y cierra, sin mover nada. Por red espera el saludo 0,8 s
  en lugar de 3,5 s.
- El transporte (`tlaku_grbl::net::{TcpTransport, WebSocketTransport}`) implementa `Transport`, así que el
  `Streamer` y `Connection::start_with` no cambian. El cierre es ordenado (`shutdown` / trama Close).

### Ruida por UDP (EXPERIMENTAL)

Una controladora Ruida (CO2) se conecta con un perfil `kind = "ruida"`. Solo está probada contra el
emulador UDP de `tlaku-ruida`, no con una máquina real (ver `docs/hardware/ruida.md`, sección 15).

- `connect` recibe además `device` (el perfil activo). Con `kind = "ruida"` el motor ignora `baud` y usa
  `device.connection.host` (UDP, puertos 50200 y 40200), `device.ruida.magic` (`null` = detectarlo con el
  sondeo de solo lectura) y la cama y el origen del perfil. El `port` solo cuenta para `virtual:ruida[@<velocidad>]`,
  una máquina virtual que arranca el emulador UDP en `127.0.0.1` y se conecta a él por UDP real (`list_ports` la
  ofrece al final). `ConnectResult.firmware` es `Ruida RDC6442S · tarjeta 0x65106510`, y el evento de conexión lleva
  `port = "udp://<IP>"`. Un perfil Ruida sin IP solo conecta a la máquina virtual.
- Los eventos son los mismos `machine://*`. El estado llega cada ~500 ms (nunca durante una subida):
  `work_pos` y `machine_pos` son los mm desde el home hacia dentro de la cama, o sea las coordenadas del
  origen del perfil (`origin` = esquina del home); `feed`, `spindle` y `buffer` van en 0 y no hay `overrides`.
  Con un trabajo en marcha, un sondeo sin respuesta no es una conexión perdida (algunas controladoras ignoran
  la red mientras ejecutan); fuera de uno, tres seguidos sí.
- `start_job` genera el `.rd` (`tlaku_pipeline::export_rd_for_device`) y lo sube con progreso (`machine://progress`
  cuenta paquetes). Con `ruida.run_after_upload = false` (predeterminado) queda como documento `TLAKUnnn` en la
  memoria y no arranca: `machine://job` termina en `finished` con el aviso; con `true` se ejecuta y el hilo sigue el
  estado hasta que la máquina vuelve a reposo (`finished`). Si la subida falla antes del primer paquete, el comando
  devuelve el error y no hay eventos; si falla después, `machine://job` `error` (y `D8 01` si se ejecutaba). Se rechaza
  si la cama que informa la máquina no coincide con la del perfil.
- `pause_job` / `resume_job` / `cancel_job` son `D8 02` / `D8 03` / `D8 01` (cancelar también interrumpe una subida).
  `machine_realtime` traduce `hold`, `resume`, `reset` y `jog_cancel` a esos mismos; `status` no hace nada.
  `jog` y `jog_to` usan saltos `D9` con el láser apagado; `frame_job` y `frame_hull` recorren el rectángulo del trabajo
  con cinco saltos y no bloquean (ignoran `power_pct`: no hay encuadre con el láser encendido).
- Lo que es de GRBL (`machine_command`, `set_override`, ajustes `$`, macros, `jog_axis`, `fire_test`,
  `simulate_fault`) devuelve el error `Esta función es solo de GRBL...`.
- `ruida_info` → `RuidaInfo` y `ruida_files` → `RuidaFile[]`: solo lectura (M26 y M24).

**Despacho.** `AppState` guarda una sola conexión, `machine::active::ActiveMachine` (`Grbl`, `Ruida` o `Lhy`), y los
comandos pasan por `machine::active::Machine` (`is_closed`, `work_pos`, `jog`, `jog_to`, pausa, reanudar,
cancelar y `realtime`); lo exclusivo de GRBL usa `Machine::grbl()` (`AppState::handle`), lo de Ruida
`Machine::ruida()` y lo del K40 `Machine::lhy()`. Agregar un respaldo nuevo es una variante en cada enum: el compilador marca los
`match` que faltan. `start_job`, `frame_job` y `frame_hull` despachan por respaldo antes de la
ruta GRBL, porque cada respaldo calcula cosas distintas.

### K40 con placa M2 Nano (EXPERIMENTAL)

Un K40 con placa Lihuiyu M2 Nano (LHYMICRO-GL) se conecta con un perfil `kind = "lhy_micro"`. Solo está probado
contra el emulador de `tlaku-lhymicro`, no con una placa real (ver `docs/manual/13b-ruida-y-k40.md`). El código está en
`src-tauri/src/machine/lhymicro.rs`.

- **Conexión.** Es USB directo (CH341, `1A86:5512`), no un puerto serie. `list_ports` ofrece una entrada `usb:k40:<n>`
  por placa encontrada (`n` desde 0; la interfaz la muestra desde 1) y `virtual:k40[@<velocidad>]`, la placa simulada.
  `connect` recibe `device` y usa del perfil la placa (`device.lhymicro.board`: `A`, `B`, `B1`, `B2`, `M`, `M1`, `M2`
  —la de serie— o `M3`), la cama, el origen (esquina del home) y la velocidad máxima (el mayor de `max_rate`). Si el perfil cambia,
  hay que reconectar. Una Moshiboard comparte el USB pero no es compatible: `connect` lo dice en el error.
- **Posición.** La placa no informa dónde está el cabezal: la sesión la cuenta desde el home. Mientras no haya home, o
  tras detener o fallar un trabajo, la posición es desconocida y no se mueve nada. Empezar un trabajo sin posición
  conocida hace home primero.
- **Lo que reutiliza.** `start_job` (que bloquea en el hilo de la sesión mientras la placa ejecuta, y responde al salir el
  primer paquete), `pause_job`, `resume_job`, `cancel_job`, `jog`, `jog_to`, `frame_job` y `frame_hull` (siempre con el láser
  apagado y a la velocidad de salto de la placa) y los eventos `machine://*`. El progreso cuenta paquetes.
- **Detener sin confirmación.** Si la placa no confirma la detención, el mensaje del trabajo (`machine://job`) contiene
  el texto `NO hubo confirmación` (`UNCONFIRMED_MARK`); la interfaz lo busca para mostrar el aviso de paro de emergencia.
- **Lo que no existe** (devuelve `Esta función es solo de GRBL...`): `machine_command`, `set_override`, ajustes `$`,
  `jog_axis`, `fire_test`, Z, U y aire. La potencia de las capas se ignora y los grises se binarizan: lo avisa `lhy_preview`.

```ts
type LhyWarningKind = "power_ignored" | "gray_thresholded" | "air_assist_ignored" | "z_ignored" | "dwell_skipped";

interface LhyWarning {
  kind: LhyWarningKind;
  block: number;          // bloque del trabajo, desde 0
  layer: string | null;   // nombre de la capa del bloque, si se conoce
  value: number | null;   // potencia pedida (%) en power_ignored; milisegundos en dwell_skipped
  fires: boolean;         // solo en dwell_skipped: era un pulso (disparaba el láser), no una pausa
}

interface LhyPreview {
  board: string;
  warnings: LhyWarning[];
  packets: number;
  bytes: number;
  est_seconds: number;    // tiempo estimado por exceso (saltos a 40 mm/s)
  burn_mm: number;
  travel_mm: number;
  homes_first: boolean;   // no se conoce la posición: el envío hace home primero
}

interface LhyState {
  board: string;
  virtual_machine: boolean;
  position_known: boolean;  // contada desde el último home
  status_code: number;      // 206 lista, 238 ocupada, 236 terminó, 207 error de CRC, 239 sin energía, 204 terminó (M3); 0 sin lectura
}
```

| Comando | Argumentos | Devuelve | Qué hace |
|---|---|---|---|
| `lhy_preview` | `{ doc, device, options }` (los mismos de `JobArgs`) | `LhyPreview` | Calcula y codifica el trabajo **sin enviarlo** y devuelve sus avisos y el resumen. No necesita conexión; si la hay y se conoce la posición, parte de ella, y si no, `homes_first` es `true` |
| `lhy_home` | — | `void` | Lleva el cabezal al home (finales de carrera, orden `IPP`) y vuelve al terminar. Desde ahí se cuenta la posición. Falla si la máquina está ocupada o la conexión no es un K40 |
| `lhy_state` | — | `LhyState` | Placa, si la máquina es la virtual, si se conoce la posición y último código de estado. Falla si no hay conexión o no es un K40 |

En la API de TypeScript son `lhyPreview(args)`, `lhyHome()` y `lhyState()` (`src/api/api.ts`); los tipos están en `src/api/types.ts`.
Cada `LhyWarningKind` tiene su texto en la interfaz (`lhymicro.warn.<kind>`): el motor entrega datos, no frases. En macOS la app
declara el permiso `com.apple.security.device.usb` en `Entitlements.plist` para poder abrir la placa con el runtime reforzado.

### Automatización por UDP (M54)

Un servidor local opcional para que otros programas o scripts manejen la app con comandos de texto.

- **Apagado por omisión.** Se activa en Preferencias ▸ Automatización (`automationEnabled`), con el puerto
  `automationPort` (1024 a 65535, 47801 por omisión) y la casilla `automationAllowStart`.
- **Solo escucha en `127.0.0.1`.** Ningún otro equipo de la red puede enviarle nada.
- Comando `automation_configure` · `{ config: { enabled, port, allow_start } }` → `{ running, port, error }`
  (la interfaz lo llama al arrancar y cada vez que cambian esas preferencias). Un puerto ocupado o menor
  de 1024 no es un error del comando: vuelve `running: false` con el motivo en `error`.
- Eventos hacia la ventana: `automation://load` `{ path }` y `automation://start` `{ confirmed }`. La interfaz
  es dueña del documento y de las opciones del trabajo, así que `LOAD` y `START` se piden a ella.

**Protocolo.** Un comando por datagrama UDP, texto UTF-8 en una sola línea (máximo 4096 bytes; un `\n` o
`\r\n` final se ignora). La palabra no distingue mayúsculas. La respuesta es un datagrama de una línea
terminada en `\n`, enviado al origen.

| Comando | Respuesta | Qué hace |
|---|---|---|
| `PING` | `PONG` | Comprueba que la app escucha |
| `LOAD <ruta>` | `OK loading` | Pide abrir un proyecto `.tlaku` o importar el diseño, como si se soltara sobre la ventana. La ruta debe ser **absoluta** y de un archivo que existe (el resto de la línea es la ruta, con espacios). El resultado de la importación se avisa en la interfaz |
| `START` | `OK starting` o `PENDING confirm_in_app: ...` | Ver seguridad |
| `STATUS` | `STATUS connected=0` o `STATUS connected=1 state=<idle\|run\|hold\|alarm...> x=<mm> y=<mm>` | Estado de la máquina y posición de trabajo |
| `STOP` | `OK` | Detiene el trabajo (feed hold y soft reset; sin trabajo, cancela el jog). Siempre se atiende si hay máquina conectada |

Errores: `ERR <código>: <texto en español>`. Códigos: `not_connected`, `busy`, `not_found`, `bad_argument`,
`bad_text`, `too_long`, `empty`, `unknown_command`, `failed`.

**Seguridad de `START`:**

1. Exige máquina conectada y en reposo (`idle`); si no, `ERR not_connected` o `ERR busy`, y no pasa nada.
2. Por omisión **no inicia**: abre el diálogo de inicio de la interfaz y responde `PENDING confirm_in_app`;
   el usuario revisa y confirma allí. Se puede consultar el avance con `STATUS`.
3. Solo con la preferencia explícita «permitir iniciar desde automatización» (`automationAllowStart`,
   apagada por omisión) responde `OK starting` y la interfaz inicia sin preguntar, y aun así, si la lista de
   comprobación del diálogo de inicio es obligatoria (`jobChecklistRequired`), se pregunta en el diálogo.
   La interfaz vuelve a comprobar que haya algo que enviar y que la máquina esté lista.

Ejemplo desde una terminal: `echo PING | nc -u -w1 127.0.0.1 47801`.

## Control remoto desde el celular (ola 5.5)

Un servidor HTTP opcional (`apps/desktop/src-tauri/src/remote/`) deja ver y controlar el trabajo desde el
navegador del celular, sin instalar nada. La PC muestra un código QR; el celular lo abre y carga **una sola
página** (`page.html`, embebida en el ejecutable con `include_str!`, sin CDN ni dependencia de Vite) que habla
con la API de abajo. La página está en español e inglés según el idioma del navegador.

### Seguridad

- **Apagado de fábrica** (`remoteEnabled`, Preferencias ▸ Control remoto). Un paquete `.tlakuprefs` o
  `.tlakubundle` importado nunca lo enciende (`NEVER_IMPORTED` en `features/settings/bundle.ts`).
- **Solo red local, nunca internet.** Escucha en **una** dirección IPv4 privada (RFC 1918 o enlace local):
  la que se elige en Preferencias (`remoteIp`) o, sin elección, la recomendada (192.168.x antes que 10.x antes que
  172.16/12; las interfaces de contenedores, VPN y máquinas virtuales van al final). Jamás en `0.0.0.0`.
  Además cada petición se rechaza con 403 si el par que la manda no es una dirección local (un reenvío de
  puertos del router, por ejemplo). El encabezado `Host` debe ser una IP (defensa contra el DNS rebinding).
- **Clave aleatoria de 128 bits** (`getrandom`), distinta en cada arranque del servidor, que viaja en el
  **fragmento** de la URL del QR (`http://<ip>:<puerto>/#<clave>`): el navegador nunca manda el fragmento al
  servidor ni lo deja en el historial (la página lo borra de la barra al leerlo). La página la entrega una vez a
  `POST /api/login` y recibe una **cookie de sesión** (`tlaku_session`, otro valor aleatorio, `HttpOnly`,
  `SameSite=Strict`). Alternativa para scripts: la clave en cada petición con `Authorization: Bearer <clave>` o
  `X-Tlaku-Token` (esas peticiones comparten una sesión por dirección y aparato).
- **Autenticación en cada petición** salvo la página estática (`GET /`) y `POST /api/login`. Los secretos se
  comparan en **tiempo constante** (`auth::ct_eq`) y la búsqueda de sesión recorre todas.
- **Límite de intentos:** 5 claves incorrectas en 60 s desde una dirección la bloquean 120 s (429 con
  `Retry-After`), incluso si luego manda la correcta. El bloqueo también alcanza a la clave por encabezado.
- **Regenerar la clave** (botón en la PC) invalida el QR anterior y **expulsa a todos los clientes**. La PC
  lista quién está conectado (dirección, aparato, actividad) y puede **desconectar** a uno (`remote_kick`).
  Las sesiones sin actividad caducan a los 30 min; máximo 8 sesiones y 16 conexiones en vivo.
- **Sin CORS:** no se emite ningún encabezado `Access-Control-*`. Las acciones (`POST`) exigen la marca
  `X-Requested-With` (o la clave en encabezado) y, si el navegador manda `Origin`, que sea este mismo servidor.
- Respuestas con `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
  `X-Frame-Options: DENY` y una política de contenido (`default-src 'none'`; solo el script y los estilos
  de la propia página).
- **Pausar, reanudar y detener: siempre.** Van directo a la máquina (`Machine::pause/resume/cancel`), sin
  pasar por la ventana: si la ventana se congela, detener sigue funcionando. Detener no mira el estado,
  el encuadre ni el checklist. El galvo no tiene pausa (`caps.pause = false`), solo detener.
- **Iniciar** exige todo esto a la vez: el checklist completo del celular (*estoy junto a la máquina*, *área
  despejada*, *aire o extractor encendidos*); máquina conectada y en reposo; sin trabajo en curso; la ventana
  de la PC informando (si calla más de 12 s, `pc_offline`); algo que enviar; **que el trabajo se haya
  encuadrado en la PC después del último cambio** del documento, de la máquina o de las opciones del trabajo
  (`selectFramed` en `store/machine.ts`: la marca guarda las referencias de `doc`, `jobOptions`, el
  dispositivo activo y el evento de conexión, más la selección si el trabajo es «solo la selección»; el
  store es inmutable, así que cualquier cambio la invalida; reconectar también). Un galvo (haz invisible)
  nunca se inicia desde el celular. El servidor **no inicia nada**: pide a la ventana (`remote://start`) que
  inicie con su ruta de siempre (`startJob`, con el preflight), y la ventana vuelve a comprobar el
  encuadre antes de responder (`remote_start_result`); así la lógica no se duplica. Si falla, el celular
  recibe el motivo. Si la ventana no contesta en 60 s, `timeout`.
- **No hay HTTPS.** En una red local no se puede conseguir un certificado de confianza para una IP privada;
  usar uno autofirmado haría que el navegador del celular mostrara una advertencia que los usuarios
  aprenden a ignorar. Por eso la clave va en el fragmento de la URL y la sesión en una cookie aparte, pero
  quien pueda **espiar la red local** (un Wi-Fi sin cifrar, por ejemplo) puede ver el tráfico y la cookie:
  úsalo solo en una red de confianza. La cookie no lleva `Secure` por la misma razón.

### API HTTP

Cuerpos y respuestas en JSON UTF-8. Errores: `{ "ok": false, "code": "...", "message": "..." }` (`message` en
español; la página traduce por `code`). Todo lo de `/api/*` excepto `login` exige credenciales (401 si faltan).

| Ruta | Respuesta |
|---|---|
| `GET /` | La página del celular (pública, estática). |
| `POST /api/login` · `{ "token": "<clave>" }` | 200 `{ ok: true }` + `Set-Cookie`; 401 `bad_token`; 429 `locked`. |
| `GET /api/state` | Estado completo (abajo). |
| `GET /api/events` | **SSE** (`text/event-stream`): un evento `state` (el mismo JSON de `/api/state`) al conectar, en cada cambio (con un pequeño agrupado de 150 ms) y cada 10 s como latido. Se cierra sola si el cliente es expulsado. |
| `GET /api/route` | `{ version, bed: [w, h] \| null, d }`: recorrido del trabajo como trazado SVG en mm del documento (Y arriba). |
| `GET /api/camera.jpg` | Último cuadro de la cámara de la PC (JPEG); 404 `no_camera` si no hay uno de los últimos 20 s. |
| `POST /api/pause`, `/api/resume`, `/api/stop` | 200 `{ ok: true }`; 409 con `not_connected`, `no_pause` (galvo) o `failed` (la máquina lo rechazó). |
| `POST /api/start` · `{ "checklist": { "nearby": true, "clear": true, "air": true } }` | 200 si la ventana inició el trabajo; 409 con `checklist`, `pc_offline`, `not_connected`, `job_active`, `not_idle`, `nothing_to_send`, `galvo`, `not_framed`, `busy_request` o `failed` (el motivo del preflight va en `message`); 504 `timeout`. |

Estado (`/api/state` y eventos `state`):

```ts
interface RemoteState {
  v: 1;
  pc_online: boolean;                       // la ventana informó hace menos de 12 s
  machine: { connected: boolean; kind?: 'grbl'|'ruida'|'lhy'|'galvo'; virtual?: boolean; state?: string };  // del motor
  device: string; doc: string;
  job: { phase: 'idle'|'running'|'paused'|'finished'|'cancelled'|'error'; message: string;
         progress: number /* 0..1 */; elapsed_s: number; remaining_s: number | null; total_s: number | null;
         layer: { index: number; count: number; color: string; mode: string } | null;
         finished_seq: number; error_seq: number };   // contadores: el aviso al terminar se dispara al subir
  head: [number, number] | null; bed: [number, number] | null;   // mm del documento
  caps: { pause: boolean; pause_reason: string | null; stop: boolean };
  start: { allowed: boolean; reason: string | null; message: string | null };
  route_v: number; camera_v: number;        // 0 = no hay; la página los compara para volver a pedir
}
```

### Lo que informa la ventana

El estado del trabajo sale de los eventos `machine://*` que el store (`store/machine.ts`) ya sigue; la ventana
lo resume y se lo manda al servidor (`features/remote/`), como mucho 4 veces por segundo y solo si cambió
(más un latido cada 4 s). Si la ventana se cierra, el servidor lo nota (`pc_online: false`) y bloquea iniciar.

| Comando (`invoke`; todos con `rename_all = "snake_case"`) | Argumentos | Resultado |
|---|---|---|
| `remote_configure` | `config: { enabled, port, ip: string \| null }` | `RemoteStatus` |
| `remote_status` | — | `RemoteStatus` |
| `remote_regenerate` | — | `RemoteStatus` con el QR nuevo; los clientes quedan expulsados |
| `remote_kick` | `id` | `RemoteStatus` |
| `remote_update` | `snapshot: RemoteSnapshot` | — |
| `remote_set_route` | `route: { bed, d } \| null` | — (error si pasa de 400 KB o trae caracteres que no son de un trazado) |
| `remote_camera_frame` | `jpeg_b64` (vacío = cámara apagada) | `boolean`: hay algún celular esperándolo. Si es `false` la ventana no manda más hasta que haya clientes |
| `remote_start_result` | `result: { id, ok, code?, message? }` | — |

`RemoteStatus = { running, port, ip, url, full_url, qr_svg, error, addrs: [{ ip, name, recommended }], clients: [{ id, ip, agent, since_ms, idle_s, connected }] }`.
`full_url` lleva la clave (es la del QR); `qr_svg` es el QR de `full_url` como SVG (lo genera `tlaku-codes`;
las formas vienen con Y hacia arriba y se voltean para dibujarlas). `RemoteSnapshot` está en `api/api.ts`; el
servidor limpia y acota todo lo que recibe (texto de 120 caracteres, números finitos, etc.).

Eventos hacia la ventana: `remote://clients` `{ clients }` (entró, salió, caducó o lo desconectaron),
`remote://start` `{ id, client }` (un celular pidió iniciar con su checklist completo) y `remote://action`
`{ action: 'pause' | 'resume' | 'stop', client }` (para avisar en la PC y cortar el bucle de encuadre).

### Cámara

La cámara vive en el webview de la PC. Mientras haya clientes conectados y la cámara esté encendida
(`cameraStatus === 'live'`), la ventana manda cada 2,5 s un JPEG reducido (640 px de ancho, calidad 0,6; unos
30 a 60 KB) tomado del lienzo ya corregido de lente, con `remote_camera_frame`. Sin clientes no se manda nada
y el servidor ni siquiera guarda lo que reciba. La página muestra la imagen y la recarga cuando sube `camera_v`.

### Aviso al terminar

La página muestra un aviso destacado a pantalla completa («¡Trabajo terminado!» o «El trabajo falló»),
vibra (`navigator.vibrate`, donde el navegador lo permita; Safari de iOS no), suena un pitido (Web Audio,
una vez que el usuario tocó la página), parpadea el título de la pestaña y mantiene la pantalla encendida
mientras trabaja (`wakeLock`). El aviso se dispara con los contadores `finished_seq`/`error_seq`, que sube el
servidor al ver el paso de «en curso» a «terminado»/«falló»: no depende de que el celular estuviera
mirando en ese instante. Al entrar a una sesión ya empezada no se avisa de lo que pasó antes.

### Probarlo con un celular

1. Preferencias ▸ Control remoto ▸ activar. Debe decir «Escuchando en 192.168.x.x:47802».
2. Con el celular en la misma red Wi-Fi, escanear el QR con la cámara y abrir el enlace.
3. Conectar una máquina (o una `virtual:*`), cargar un diseño y **encuadrar** (Máquina ▸ Encuadrar).
4. En el celular: Iniciar trabajo ▸ marcar las tres casillas ▸ Iniciar. Probar Pausar, Reanudar y Detener
   (toque doble). Cambiar algo en el diseño en la PC: Iniciar vuelve a quedar deshabilitado con el motivo.
5. «Regenerar clave» en la PC: el celular pasa a «Te desconectaron». «Desconectar» quita solo a uno.

## Cámara (ola 4)

Ojo de pez, imagen de la cama, cámara en el cabezal y buscador de centro. `camera.json` gana campos
nuevos y **todos** llevan valor por defecto al leer: un archivo anterior carga igual.

| Comando | Argumentos | Resultado |
|---|---|---|
| `calibrate_lens` | `{ images_png_base64, board, square_mm, model?: "auto" \| "brown_conrady" \| "fisheye" }` | `CalibrationReport`. Sin `model` es `auto`: se ajustan Brown-Conrady y el ojo de pez de Kannala-Brandt y se elige por validación cruzada con vistas reservadas (el ojo de pez solo gana si mejora ≥ 15 % y ≥ 0,1 px). `model_scores` trae el error de ajuste y de vistas reservadas de cada modelo |
| `compose_bed_image` | `{ tiles: { image_png_base64, to_bed: Mat3 }[], bed, px_per_mm: number \| null, grayscale, feather_px: number \| null, crop_px: number \| null, rect: [x0, y0, x1, y1] \| null, dir: string \| null, stem, save }` | `BedImage { path, width_px, height_px, size_mm, origin_mm, in_temp_dir, coverage, px_per_mm, preview_png_base64 }`. Compone fotos **ya corregidas de lente**, cada una con su `to_bed` (píxel con origen en el borde -> mm de la cama), en una imagen alineada con los ejes de la cama; la fila de arriba es la `Y` mayor y `origin_mm` es su esquina inferior izquierda, así que colocada con `transform = [1,0,0,1,origin]` queda donde estaba. Mezcla los traslapes con una rampa de `feather_px` y descarta `crop_px` de cada borde. Con `save` escribe `<stem>-camara-<n>.png` en `dir` (o en la carpeta temporal); la vista previa se reduce a 1600 px de ancho |
| `detect_circle_in_image` | `{ image_png_base64, hint?: { center: [x, y], radius?: number } }` | `CircleFit { center, radius, rms_px, inliers, arc_coverage, points }` en píxeles de la imagen. Hough con la dirección del gradiente, radio por histograma y ajuste de Kåsa; devuelve hasta 48 puntos del borde para que quien llama los pase a mm y ajuste ahí (la perspectiva vuelve elipse al círculo). Falla si se ve menos del 45 % del contorno |

```ts
type LensModel = "brown_conrady" | "fisheye";
interface LensCalibration {           // campos nuevos, opcionales al leer
  model?: LensModel;                  // por defecto brown_conrady
  k4?: number;                        // 4.º coeficiente de Kannala-Brandt
  view_zoom?: number;                 // focal de la imagen corregida / focal de la calibración; 1 por defecto
}
interface CameraPrefs { grayscale?: boolean; update_on_job_finish?: boolean }
interface HeadCalibration {
  image_size: [number, number]; ref_px: [number, number];
  px_to_mm: [[number, number], [number, number]];   // píxeles -> mm (con la inversión del eje Y de la imagen)
  offset_mm: [number, number]; rms_mm: number; observations: number;
}
interface HeadCamera { enabled: boolean; calibration: HeadCalibration | null }
interface CameraSetup { /* ... */ head?: HeadCamera }
```

- **Ojo de pez (Kannala-Brandt).** `θ = atan(r)`, `θd = θ(1 + k1 θ² + k2 θ⁴ + k3 θ⁶ + k4 θ⁸)` y el píxel es
  `f · θd · (x, y) / r`. No usa `p1`/`p2`. El ajuste (LM) parte de varias focales supuestas (la inicialización
  de Zhang no sirve) y refina primero focal y `k1` y luego todo; `k3`/`k4` solo si el error sigue alto. La
  proyección rectilínea no puede mostrar 180°, así que la imagen corregida usa `view_zoom < 1`
  (el borde horizontal de la imagen cruda hasta 60° del eje): `UndistortMap`, el shader WebGL
  (`lens.ts`, `uFisheye`/`uZoom`) y el modelo de altura de `save_camera_setup` usan la misma focal
  `fx · view_zoom`. Las calibraciones guardadas antes (sin `model`) siguen siendo Brown-Conrady con zoom 1.
- **Cámara en el cabezal.** El píxel `p` ve el punto `L + offset + A (p - ref)` (mm de documento, `L` = posición
  del láser). El desfase y `A` salen de mínimos cuadrados con ≥ 3 observaciones de una marca grabada en una
  posición conocida, vista desde posiciones distintas del cabezal. El recorrido (`planScan`) cubre la cama o la
  selección con un 20 % de traslape sobre la parte útil de cada foto (se descarta el 6 % de cada borde) en
  orden de serpiente; el láser nunca sale de la cama y, si el desfase lo obliga, se avisa que quedan zonas sin ver.
  Mueve la máquina con `jog_to` (láser apagado), espera a que llegue y esté en reposo, deja 0,5 s de reposo y fotografía.
  Pide confirmación antes y se puede cancelar entre fotos.
- **Buscador de centro.** Con 3 puntos: circuncírculo en mm; con la detección automática, ajuste por
  mínimos cuadrados a los puntos de borde pasados a mm. Mover al centro usa `jog_to`; fijar el origen escribe
  `user_origin` (coordenadas de trabajo, como «Origen de usuario aquí»).
