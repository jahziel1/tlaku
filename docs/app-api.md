# Contrato entre la app (Tauri) y el motor

Fase 2. La interfaz (TypeScript) es dueña del documento que se edita; el motor (Rust) no
guarda estado del documento: recibe el documento completo y devuelve resultados. La única
excepción es la conexión con la máquina, que vive en Rust.

Los tipos se definen en Rust (crate `laser-doc`) con `serde` y se exportan a TypeScript con
`ts-rs` en `apps/desktop/src/bindings/` (`cargo test -p laser-doc` los regenera; la ruta sale de
`.cargo/config.toml`). Este archivo describe la forma; el código manda.
JSON con `snake_case`, unidades en mm, mm/min y %, origen abajo a la izquierda, Y hacia arriba.

## Documento

```ts
type Point = [number, number];

interface PathData { points: Point[]; closed: boolean }        // coordenadas locales del objeto

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
interface VectorObject { paths: PathData[] }
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

interface Document { version: 1; objects: DesignObject[]; layers: LayerSettings[]; device_id: string | null }
```

Semántica que fija el motor (`laser-pipeline`):

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
| `save_project` / `load_project` | `{ path, doc }` / `{ path }` | `void` / `Document` (extensión `.lsproj`, JSON; `load_project` rechaza versiones desconocidas y permite las imágenes del documento en el protocolo de assets) |
| `list_devices` / `save_device` / `delete_device` | `{}` / `{ device }` / `{ id }` | perfiles guardados en `devices.json` dentro de la carpeta de configuración de la app (`app_config_dir`). Sin archivo, `list_devices` devuelve el perfil de fábrica (`DeviceProfile::default()`) sin guardarlo; `save_device` crea o reemplaza por `id` y valida lo básico (cama, S máx., aceleraciones, baudios); `delete_device` falla si el id no existe |
| `list_ports` | | `string[]` |
| `connect` / `disconnect` | `{ port, baud }` / `{}` | `{ firmware: string }` (`Grbl 1.1h`, de `$I`) / `void`. `connect` abre el puerto (~2 s por el reinicio de la placa) y falla si hay otra conexión o la máquina no responde a `$I`; `disconnect` cancela un trabajo en curso antes de soltar el puerto y no falla si no hay conexión |
| `machine_command` | `{ line }` | `string[]`: líneas de respuesta, la última `ok`, `error:N: explicación` o `ALARM:N: explicación`. Rechaza `?`, `!`, `~` (usa `machine_realtime`) y falla con un trabajo en curso. `$H` espera hasta 60 s; el resto 5 s |
| `machine_realtime` | `{ cmd: "status" \| "hold" \| "resume" \| "reset" \| "jog_cancel" }` | `void`. Un byte (`?`, `!`, `~`, 0x18, 0x85). Durante un trabajo, `hold`/`resume` pausan y reanudan, `reset` lo cancela |
| `jog` | `{ dx, dy, speed }` | `void`: `$J=G91 G21 X.. Y.. F..` (mm, mm/min). Falla con un trabajo en curso o en alarma (con el consejo de `$X`/`$H`) |
| `start_job` | `{ doc, device, options }` | `void`. Genera el G-code, lo valida contra `rx_buffer` y aplica el mismo preflight que `lasersoft send`: consulta `?` y **no transmite nada** si la máquina está en Alarm, Hold, Door, Run, Jog, Home, Check o Sleep; el error explica por qué y qué hacer (en la consola de la pestaña Máquina). La app nunca envía `$X`, `$H` ni `~` por su cuenta |
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
`move_times` sale de `laser-sim` (aceleración por eje y desviación de unión, el mismo modelo de
`total_s`): la vista previa lo usa para reproducir el trabajo en tiempo real (busca el primer
`i` con `move_times[i] >= t`); un movimiento de longitud cero repite el instante anterior.
En Rust (`laser-pipeline`): `compute`/`compute_at` verifican la cama; `build` calcula igual sin
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
