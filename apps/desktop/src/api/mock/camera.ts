import type { LaserApi } from '../api';
import type { CalibrationReport, CameraSetup } from '../types';

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
      memory = structuredClone(setup);
      try {
        storage?.setItem(STORAGE_KEY, JSON.stringify(setup));
      } catch {
        // sin almacenamiento: queda en memoria
      }
      return structuredClone(setup);
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
 * tablero ligeramente en perspectiva en el centro del cuadro y `calibrateLens` una calibración
 * plausible para el tamaño de la primera imagen. Sirve para desarrollar el asistente sin motor.
 */
export function mockCamera(
  lag: Lag,
  setups: SetupStore = createSetupStore(browserStorage()),
): Pick<LaserApi, 'detectBoard' | 'calibrateLens' | 'loadCameraSetup' | 'saveCameraSetup' | 'saveBoardFile'> {
  return {
    loadCameraSetup: () => lag(() => setups.load()),
    saveCameraSetup: ({ setup }) => lag(() => setups.save(setup)),
    // No hay disco: solo se valida la extensión como hace el motor (el navegador descarga el archivo aparte).
    saveBoardFile: ({ path }) =>
      lag(() => {
        if (!/\.(svg|pdf)$/i.test(path)) throw new Error('El tablero solo se guarda como .svg o .pdf.');
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
