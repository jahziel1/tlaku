# Contrato app-motor: Cámara

Parte de [el contrato app-motor](README.md). Comandos de la cámara (ola 4).

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
