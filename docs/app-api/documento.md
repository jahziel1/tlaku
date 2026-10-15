# Contrato app-motor: Documento y perfil de dispositivo

Parte de [el contrato app-motor](README.md). El documento, los objetos, las capas, el perfil de dispositivo y las opciones de trabajo.

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
interface VectorObject {
  paths: BezPath[];
  // Esquinas que redondear/achaflanar sustituyó (D60): la punta original y los dos nodos entre los
  // que quedó la forma, en coordenadas locales. Opcional; solo vale mientras esos nodos no se editen.
  corners?: CornerMark[];
}
interface CornerMark { vertex: [number, number]; from: [number, number]; to: [number, number] }
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

### Datos variables (mejora 14)

Campos opcionales y retrocompatibles (un documento anterior no los lleva y se lee igual):

```ts
interface VariableText {          // Document.variable_text (opcional)
  serial: number; serial_step: number; serial_digits: number;
  csv_path: string | null;        // repite la ruta de sources[0]; es el CSV único de los documentos antiguos
  csv_row: number;                // fila actual de la fuente principal (0 = la primera de datos)
  advance_after_job: boolean;
  sources: DataSource[];          // fuentes con nombre; la primera es la principal. Vacía = documento antiguo
}
interface DataSource { name: string; path: string; on_short: "repeat" | "block" }
// name: letras, números, _ y -; único sin distinguir mayúsculas. path: absoluta en memoria; en el
// archivo es relativa a la carpeta del .tlaku si el CSV queda ahí o hasta 3 niveles arriba.
// on_short: qué hace una fuente secundaria con menos filas que la principal.
interface ImageObject  { /* ... */ variable?: { source: string; fit: "contain" | "cover" } }
interface VectorObject { /* ... */ code?: VariableCode }   // QR o código de barras con contenido variable
interface VariableCode { kind: string; data: string; ecc: string; size_mm: number; height_mm: number;
                         margin_mm: number; human_readable: boolean; invert: boolean; font_family: string }
```

- `Document::from_json` normaliza `variable_text` (`VariableText::normalize`): sin `sources` y con
  `csv_path` crea la fuente principal con el nombre del archivo; limpia y desduplica nombres; y deja
  `csv_path` igual a la ruta de la principal. Es idempotente.
- `source` de una imagen variable y `data` de un código son plantillas (`{csv:Logo}`,
  `{csv.productos:Sku}`) que la interfaz resuelve antes de calcular el trabajo (`resolveDocForOutput`):
  la imagen pasa a apuntar al archivo de su fila (ajustado a su caja) y el código se regenera con
  `generate_code`. El motor del trabajo nunca ve las plantillas.

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

### Piezas de un generador (mejora 2)

Campo opcional y retrocompatible: un documento anterior no lo lleva y se lee igual (`generators` vacío no se escribe).

```ts
interface Document { /* ... */ generators?: GeneratorRecord[] }
interface GeneratorRecord {
  group: string;            // id del grupo de más afuera (DesignObject.group) de las piezas
  generator: string;        // 'box' | 'hinge' | 'fit_test' ...
  params: { [clave: string]: unknown }; // los parámetros tal como los lee el generador
  cut_layer: string;        // capa de corte (#rrggbb)
  engrave_layer?: string | null; // capa de los rótulos
}
```

- El motor no interpreta el registro; la interfaz lo busca por la cadena de grupos del objeto seleccionado
  (`features/parts/partsModel.ts: recordFor`) y ofrece «Regenerar…», que reabre el diálogo con esos `params`
  (los campos que falten toman el valor de fábrica) y reemplaza las piezas del grupo **en el mismo lugar**.
- Si el usuario borra las piezas o las edita, el vínculo se rompe como con las formas paramétricas: regenerar
  vuelve a generar el conjunto completo (el diálogo avisa si el grupo ya no tiene las mismas piezas).

