import { applyH, homographyFrom4, invertH, type Mat3, type Quad } from '@/features/camera/geometry';
import { makeLayer } from '@/lib/layers';
import type { LaserApi } from '../api';
import type { BezPath, CalibrationReport, CameraAlignment, CameraSetup, DesignObject, Document, HeightModel, MarkResult, MarksReport } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

/** Ancho y alto de un PNG (cabecera IHDR) a partir de su base64; `null` si no es un PNG. */
export function pngSize(b64: string): [number, number] | null {
  const raw = b64.includes(',') ? b64.slice(b64.indexOf(',') + 1) : b64;
  try {
    const head = atob(raw.slice(0, 44));
    if (head.charCodeAt(1) !== 0x50 || head.slice(1, 4) !== 'PNG') return null;
    const u32 = (o: number) =>
      ((head.charCodeAt(o) << 24) | (head.charCodeAt(o + 1) << 16) | (head.charCodeAt(o + 2) << 8) | head.charCodeAt(o + 3)) >>> 0;
    return [u32(16), u32(20)];
  } catch {
    return null;
  }
}

export const STORAGE_KEY = 'tlaku.camera.setup';

/** Configuración vacía, igual a la que devuelve el motor cuando no hay `camera.json`. */
function blankSetup(): CameraSetup {
  return {
    version: 1,
    calibration: null,
    alignment: null,
    prefs: { device_id: null, device_label: null, width: null, height: null, opacity: 0.6, visible: false },
  };
}

// ---------------------------------------------------------------------------
// Cámara virtual para las marcas y la altura
// ---------------------------------------------------------------------------

/** Cuánto crece la imagen por mm de altura (la superficie elevada se ve más grande y más afuera). */
const MOCK_KAPPA = 0.0012;
/** Misma fracción de margen que el motor (`INSET` en tlaku-camera/src/marks.rs). */
const MOCK_INSET = 0.075;

/** Posiciones de las marcas en la cama, en el mismo orden que el motor (esquinas, bordes, centro). */
export function mockMarkPositions(bed: [number, number], count: 4 | 9): [number, number][] {
  const [x0, x1] = [MOCK_INSET * bed[0], (1 - MOCK_INSET) * bed[0]];
  const [y0, y1] = [MOCK_INSET * bed[1], (1 - MOCK_INSET) * bed[1]];
  const [xm, ym] = [(x0 + x1) / 2, (y0 + y1) / 2];
  const corners: [number, number][] = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
  return count === 4 ? corners : [...corners, [xm, y0], [x1, ym], [xm, y1], [x0, ym], [xm, ym]];
}

/**
 * Cama (mm) -> imagen (px) de la cámara virtual: el rectángulo de `box` (x0, y0, x1, y1 en mm)
 * ocupa casi todo el cuadro, con un poco de perspectiva.
 */
function mockBedToImage(size: [number, number], box: [number, number, number, number]): Mat3 | null {
  const [w, h] = size;
  const [x0, y0, x1, y1] = box;
  const bed: Quad = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
  const img: Quad = [[0.1 * w, 0.88 * h], [0.92 * w, 0.9 * h], [0.88 * w, 0.1 * h], [0.12 * w, 0.12 * h]];
  return homographyFrom4(bed, img);
}

/** Modelo de altura de la cámara virtual a partir de la matriz cama -> imagen a la altura `ref`. */
function mockHeightModel(bedToImage: Mat3, size: [number, number], ref: number, source: HeightModel['source'], min: number, max: number): HeightModel {
  const [cx, cy] = [size[0] / 2, size[1] / 2];
  const s = bedToImage[2][2];
  return {
    ref_height_mm: ref,
    bed_to_image: bedToImage,
    gradient: [-MOCK_KAPPA * cx * s, -MOCK_KAPPA * cy * s, -MOCK_KAPPA * s],
    source,
    min_height_mm: min,
    max_height_mm: max,
  };
}

/** Ruido determinista y reproducible (entre -0.5 y 0.5). */
function wiggle(seed: number, k: number): number {
  const v = Math.sin(seed * 12.9898 + k * 78.233) * 43758.5453;
  return v - Math.floor(v) - 0.5;
}

/**
 * Completa lo que calcula el motor al guardar: el modelo de altura (medido con dos alturas, o
 * estimado si hay lente) y el grosor del material acotado. En el mock el "modelo" sale de la
 * cámara virtual, no de las marcas.
 */
export function normalizeMockSetup(setup: CameraSetup): CameraSetup {
  const out = structuredClone(setup);
  const h = out.prefs.material_height_mm ?? 0;
  out.prefs.material_height_mm = Number.isFinite(h) ? Math.min(300, Math.max(0, h)) : 0;
  const a: CameraAlignment | null = out.alignment;
  if (!a) return out;
  const samples = a.samples ?? [];
  a.method = samples.length > 0 ? 'marks' : 'manual';
  a.samples = samples;
  a.reference_height_mm = samples.length > 0 ? Math.min(...samples.map((x) => x.height_mm)) : (a.reference_height_mm ?? 0);
  const inv = invertH(a.to_bed);
  const heights = samples.map((x) => x.height_mm);
  if (!inv) {
    a.height = null;
  } else if (samples.length >= 2) {
    const [lo, hi] = [Math.min(...heights), Math.max(...heights)];
    a.height = mockHeightModel(inv, a.image_size, lo, 'measured', lo, hi);
  } else {
    a.height = out.calibration ? mockHeightModel(inv, a.image_size, a.reference_height_mm, 'modelled', a.reference_height_mm, a.reference_height_mm) : null;
  }
  return out;
}

export interface SetupStore {
  load(): CameraSetup;
  save(setup: CameraSetup): CameraSetup;
}

/**
 * Persistencia simulada de `camera.json`: en memoria y, si el navegador lo permite, en
 * `localStorage` (la configuración sobrevive a recargar la página en modo mock). Un JSON dañado o
 * la falta de almacenamiento se tratan como "sin archivo", como hace el motor real.
 */
export function createSetupStore(storage: Pick<Storage, 'getItem' | 'setItem'> | null): SetupStore {
  let memory: CameraSetup | null = null;
  return {
    load() {
      try {
        const text = storage?.getItem(STORAGE_KEY);
        if (text) return JSON.parse(text) as CameraSetup;
      } catch {
        // dañado o sin permiso: se cae a la copia en memoria
      }
      return structuredClone(memory ?? blankSetup());
    },
    save(setup) {
      const saved = normalizeMockSetup(setup);
      memory = structuredClone(saved);
      try {
        storage?.setItem(STORAGE_KEY, JSON.stringify(saved));
      } catch {
        // sin almacenamiento: queda en memoria
      }
      return structuredClone(saved);
    },
  };
}

function browserStorage(): Storage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

/**
 * Métodos simulados del área `camera`. No analizan la imagen: `detectBoard` devuelve siempre un
 * tablero ligeramente en perspectiva en el centro del cuadro, `calibrateLens` una calibración
 * plausible para el tamaño de la primera imagen y `detectCameraMarks` las marcas de una cámara
 * virtual (con ruido de unos 0.1 mm y error de validación cruzada plausible). Sirve para
 * desarrollar los asistentes sin motor.
 */
export function mockCamera(
  lag: Lag,
  setups: SetupStore = createSetupStore(browserStorage()),
): Pick<LaserApi, 'detectBoard' | 'calibrateLens' | 'loadCameraSetup' | 'saveCameraSetup' | 'saveBoardFile' | 'cameraMarks' | 'detectCameraMarks'> {
  return {
    loadCameraSetup: () => lag(() => setups.load()),
    saveCameraSetup: ({ setup }) => lag(() => setups.save(setup)),
    // No hay disco: solo se valida la extensión como hace el motor (el navegador descarga el archivo aparte).
    saveBoardFile: ({ path }) =>
      lag(() => {
        if (!/\.(svg|pdf)$/i.test(path)) throw new Error('El tablero solo se guarda como .svg o .pdf.');
      }),

    cameraMarks: ({ bed, count }) =>
      lag((): Document => {
        if ((count !== 4 && count !== 9) || !(bed[0] >= 60) || !(bed[1] >= 60)) throw new Error('Cama o número de marcas no válidos.');
        const arm = Math.min(10, Math.max(4, 0.04 * Math.min(bed[0], bed[1])));
        const color = '#0066ff';
        const objects: DesignObject[] = mockMarkPositions(bed, count).map(([x, y], i) => {
          const paths: BezPath[] = [
            [{ type: 'move_to', to: [x - arm, y] }, { type: 'line_to', to: [x + arm, y] }],
            [{ type: 'move_to', to: [x, y - arm] }, { type: 'line_to', to: [x, y + arm] }],
          ];
          if (i === 0) {
            const r = 1.25 * arm;
            const ring: BezPath = Array.from({ length: 48 }, (_, k) => ({
              type: k === 0 ? ('move_to' as const) : ('line_to' as const),
              to: [x + r * Math.cos((2 * Math.PI * k) / 48), y + r * Math.sin((2 * Math.PI * k) / 48)] as [number, number],
            }));
            paths.push([...ring, { type: 'close' }]);
          }
          return { id: `mark-${i + 1}`, layer: color, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths }, locked: false, hidden: false, group: null };
        });
        return { version: 2, objects, layers: [makeLayer(color, 'Marcas de cámara', { mode: 'line', speed: 3000, power: 10 })], device_id: null };
      }),

    detectCameraMarks: ({ image_png_base64, bed, count }) =>
      lag((): MarksReport => {
        const [w, h] = pngSize(image_png_base64) ?? [640, 480];
        const pos = mockMarkPositions(bed, count);
        const xs = pos.map((p) => p[0]);
        const ys = pos.map((p) => p[1]);
        const p0 = mockBedToImage([w, h], [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)]);
        const toBed = p0 && invertH(p0);
        if (!p0 || !toBed) throw new Error('No se encontraron las marcas en la imagen.');
        const seed = image_png_base64.length % 997;
        const marks: MarkResult[] = pos.map((bedPos, i) => {
          const [u, v] = applyH(p0, bedPos);
          const noise = 0.18 * Math.hypot(wiggle(seed, 2 * i), wiggle(seed, 2 * i + 1));
          const residual = 0.04 + 0.12 * Math.abs(wiggle(seed, 40 + i)) * 2;
          return {
            index: i,
            bed: bedPos,
            image: [u + noise * Math.sign(wiggle(seed, 3 * i)), v + noise * Math.sign(wiggle(seed, 5 * i + 1))],
            confidence: 0.9 + 0.09 * Math.abs(wiggle(seed, 70 + i)) * 2,
            used: true,
            residual_mm: residual,
            loo_mm: count === 9 ? residual * 1.7 : null,
          };
        });
        const res = marks.map((m) => m.residual_mm!);
        const loo = marks.map((m) => m.loo_mm).filter((x): x is number => x !== null);
        const rms = (xs2: number[]) => Math.sqrt(xs2.reduce((a, b) => a + b * b, 0) / xs2.length);
        const spurious: [number, number][] = [[0.5 * w, 0.03 * h], [0.97 * w, 0.5 * h]];
        return {
          marks,
          candidates: [...marks.map((m) => m.image!), ...spurious],
          to_bed: toBed,
          expected: count,
          found: count,
          used: count,
          rms_mm: rms(res),
          max_residual_mm: Math.max(...res),
          loo_rms_mm: loo.length ? rms(loo) : null,
          loo_max_mm: loo.length ? Math.max(...loo) : null,
          scale_px_per_mm: Math.hypot(applyH(p0, [10, 0])[0] - applyH(p0, [0, 0])[0], applyH(p0, [10, 0])[1] - applyH(p0, [0, 0])[1]) / 10,
          used_prior: false,
          warnings: [],
        };
      }),

    detectBoard: ({ image_png_base64, board }) =>
      lag(() => {
        const [w, h] = pngSize(image_png_base64) ?? [640, 480];
        const [cols, rows] = board;
        const pts: [number, number][] = [];
        for (let j = 0; j < rows; j++) {
          for (let i = 0; i < cols; i++) {
            const u = i / (cols - 1);
            const v = j / (rows - 1);
            // Trapecio: la fila de abajo es un poco más ancha, como un tablero inclinado.
            const k = 0.78 + 0.1 * v;
            pts.push([w * (0.5 + (u - 0.5) * 0.6 * k), h * (0.3 + v * 0.4)]);
          }
        }
        return pts;
      }),

    calibrateLens: ({ images_png_base64 }) =>
      lag((): CalibrationReport => {
        if (images_png_base64.length < 3) throw new Error('Se necesitan al menos 3 vistas con el tablero visible.');
        const [w, h] = pngSize(images_png_base64[0]!) ?? [640, 480];
        const n = images_png_base64.length;
        return {
          calibration: {
            width: w,
            height: h,
            fx: w * 0.95,
            fy: w * 0.95,
            cx: w / 2,
            cy: h / 2,
            k1: -0.18,
            k2: 0.04,
            p1: 0,
            p2: 0,
            k3: 0,
            rms_error_px: 0.34,
            views_used: n,
          },
          warnings: [],
          used_views: images_png_base64.map((_, i) => i),
          view_rms_px: images_png_base64.map((_, i) => 0.28 + 0.03 * (i % 4)),
        };
      }),
  };
}
