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
| `connect` / `disconnect` | `{ port, baud }` / `{}` | `{ firmware: string }` (`Grbl 1.1h`, de `$I`) / `void`. `connect` abre el puerto (~2 s por el reinicio de la placa) y falla si hay otra conexión o la máquina no responde a `$I`; `disconnect` cancela un trabajo en curso antes de soltar el puerto y no falla si no hay conexión |
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
| `trace_image` | `{ path, settings: TraceSettings, size_mm: [number, number] }` | `{ paths: BezPath[], warnings: string[] }` |
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
| `save_material_library` | `library` | — |
| `import_material_library` | `path` | `MaterialLibrary` del archivo `.tlakumat` |
| `export_material_library` | `path`, `library` | — |
| `material_test` | `params: MaterialTestParams` | `Document` con la cuadrícula |

`geometry_op` admite además `join`, `close` y `dedupe` con `tolerance` (mm; por defecto 0.05, 0.5
y 0.01). Solo aceptan objetos vector y nunca mezclan capas. Devuelven la lista completa de los
objetos de entrada en su orden: los que no cambian vuelven idénticos (mismo id); cada objeto que
cambia se sustituye por uno nuevo (`join-N`, `close-N`, `dedupe-N`); un objeto que queda sin trazo
no vuelve. La interfaz quita los ids que no volvieron y pone los nuevos en su lugar, en un solo
paso de deshacer.

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
