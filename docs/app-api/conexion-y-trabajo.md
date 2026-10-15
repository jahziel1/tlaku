# Contrato app-motor: Conexión, trabajo, eventos e imágenes

Parte de [el contrato app-motor](README.md). Comandos de importar, calcular, exportar y enviar el trabajo; conexión con la máquina; eventos `machine://*`; imágenes.

## Comandos (`invoke`)

Los argumentos van como objeto con estas claves; los errores llegan como texto en español listo
para mostrar (la promesa se rechaza con un `string`). Los comandos pesados (`import_file`,
`compute_job`, `export_gcode`, `start_job`...) corren fuera del hilo principal.

| Comando | Entrada | Salida |
|---|---|---|
| `import_file` | `{ path }` | `{ objects: DesignObject[], new_layers: LayerSettings[], warnings: string[] }`; error si el formato no se soporta o el archivo no tiene trayectorias. La ruta se vuelve absoluta (`source_path` de las imágenes) y se permite en el protocolo de assets |
| `compute_job` | `{ doc, device, options }` | `JobPreview`; error de texto si algo no cabe en la cama (con overscan) o un ajuste es inválido. Con máquina conectada usa su posición de trabajo para `current_position`; sin ella, ese modo es un error |
| `job_link_warning` | `{ doc, device, options }` | `string \| null`: aviso (que **no bloquea**) de que el enlace serie del perfil no da abasto, calculado con el G-code real (`tlaku_pipeline::link_load`) y `device.baud`. `null` si alcanza o no aplica (conexión por red, Ruida, K40, galvo). Lo muestra el diálogo de inicio; al iniciar, el mismo aviso queda en el registro (`preflight_link_with_load`) |
| `export_gcode` | `{ doc, device, options, path }` | `{ lines: number, estimate_s: number, warnings: string[] }`; escribe el archivo (atómico) |
| `save_project` / `load_project` | `{ path, doc }` / `{ path }` | `void` / `Document` (extensión `.tlaku`, JSON; `load_project` rechaza versiones desconocidas y permite las imágenes del documento en el protocolo de assets; **fuentes de datos:** `save_project` escribe las rutas de `variable_text.sources` relativas a la carpeta del archivo cuando el CSV queda en ella o hasta 3 niveles arriba, y `load_project` las devuelve absolutas y migra el CSV único antiguo, `csv_path`, a una fuente principal) |
| `list_devices` / `save_device` / `delete_device` | `{}` / `{ device }` / `{ id }` | perfiles guardados en `devices.json` dentro de la carpeta de configuración de la app (`app_config_dir`). Sin archivo, `list_devices` devuelve el perfil de fábrica (`DeviceProfile::default()`) sin guardarlo; `save_device` crea o reemplaza por `id` y valida lo básico (cama, S máx., aceleraciones, baudios, desfase del puntero y U finitos). Si el perfil llega con `is_default`, se la quita a los demás (solo uno se activa al abrir); `delete_device` falla si el id no existe. El orden de la lista es el del archivo |
| `reorder_devices` | `{ ids: string[] }` | `void`: reescribe `devices.json` con los perfiles en el orden de `ids`; los ids que no existen se ignoran y los perfiles no nombrados quedan al final en su orden |
| `list_ports` | | `string[]` |
| `connect` / `disconnect` | `{ port, baud, device? }` / `{}` | `{ firmware: string }` (`Grbl 1.1h`, de `$I`) / `void`. `connect` abre el puerto (~2 s por el reinicio de la placa; con `device.no_dtr_reset` abre sin subir DTR y espera solo 0,3 s) y falla si hay otra conexión o la máquina no responde a `$I`; `disconnect` cancela un trabajo en curso antes de soltar el puerto y no falla si no hay conexión |
| `machine_command` | `{ line }` | `string[]`: líneas de respuesta, la última `ok`, `error:N: explicación` o `ALARM:N: explicación`. Rechaza `?`, `!`, `~` (usa `machine_realtime`) y falla con un trabajo en curso. `$H` espera hasta 60 s; el resto 5 s |
| `machine_realtime` | `{ cmd: "status" \| "hold" \| "resume" \| "reset" \| "jog_cancel" }` | `void`. Un byte (`?`, `!`, `~`, 0x18, 0x85). Durante un trabajo, `hold`/`resume` pausan y reanudan, `reset` lo cancela |
| `jog` | `{ dx, dy, speed }` | `void`: `$J=G91 G21 X.. Y.. F..` (mm, mm/min). Falla con un trabajo en curso o en alarma (con el consejo de `$X`/`$H`) |
| `start_job` | `{ doc, device, options, document? }` (`document` = ruta del proyecto, solo se anota en el punto de control) | `void`. Genera el G-code, lo valida contra `rx_buffer` y aplica el mismo preflight que `tlaku send`: consulta `?` y **no transmite nada** si la máquina está en Alarm, Hold, Door, Run, Jog, Home, Check o Sleep; el error explica por qué y qué hacer (en la consola de la pestaña Máquina).  La app nunca envía `$X`, `$H` ni `~` por su cuenta. En G-code guarda además el punto de control para «continuar un trabajo interrumpido» (ver [maquinas.md](maquinas.md)) |
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
