import type { CameraPrefs, CameraSetup } from '@/api/types';
import type { SliceCreator } from './types';

/**
 * Imagen de la cámara dibujada bajo el diseño. `source` ya viene corregida de lente; `corners`
 * son sus cuatro esquinas en mm de documento (arriba-izq., arriba-der., abajo-der., abajo-izq.),
 * así el lienzo la dibuja con una homografía sin saber nada de la calibración.
 */
export interface CameraBackdrop {
  source: CanvasImageSource;
  width: number;
  height: number;
  corners: [[number, number], [number, number], [number, number], [number, number]];
  opacity: number;
  /** Cambia con cada cuadro nuevo para que el lienzo sepa que debe redibujar. */
  frame: number;
}

// Los tipos de la configuración viajan por LaserApi (`loadCameraSetup`/`saveCameraSetup`): viven en api/types.
export type { CameraAlignment, CameraPrefs, CameraSetup } from '@/api/types';

export type CameraStatus = 'off' | 'starting' | 'live';
export type CameraProblemKind = 'denied' | 'none' | 'busy' | 'unsupported' | 'unknown';

export interface CameraDevice {
  id: string;
  label: string;
}

export function emptyCameraSetup(): CameraSetup {
  return {
    version: 1,
    calibration: null,
    alignment: null,
    prefs: { device_id: null, device_label: null, width: null, height: null, opacity: 0.6, visible: false },
  };
}

export interface CameraSlice {
  cameraBackdrop: CameraBackdrop | null;
  setCameraBackdrop: (backdrop: CameraBackdrop | null) => void;

  /** Calibración, alineación y preferencias guardadas. */
  cameraSetup: CameraSetup;
  cameraSetupLoaded: boolean;
  cameraStatus: CameraStatus;
  cameraProblem: CameraProblemKind | null;
  cameraDevices: CameraDevice[];
  /** Tamaño real del video que entrega la cámara. */
  cameraStreamSize: [number, number] | null;
  cameraFrozen: boolean;
  /** Asistente de calibración de lente abierto. */
  cameraCalibrationOpen: boolean;

  setCameraSetup: (setup: CameraSetup, loaded?: boolean) => void;
  setCameraPrefs: (prefs: Partial<CameraPrefs>) => void;
  setCameraState: (state: Partial<Pick<CameraSlice, 'cameraStatus' | 'cameraProblem' | 'cameraDevices' | 'cameraStreamSize' | 'cameraFrozen'>>) => void;
  setCameraCalibrationOpen: (open: boolean) => void;
}

export const createCameraSlice: SliceCreator<CameraSlice> = (set) => ({
  cameraBackdrop: null,
  setCameraBackdrop: (cameraBackdrop) => set({ cameraBackdrop }),

  cameraSetup: emptyCameraSetup(),
  cameraSetupLoaded: false,
  cameraStatus: 'off',
  cameraProblem: null,
  cameraDevices: [],
  cameraStreamSize: null,
  cameraFrozen: false,
  cameraCalibrationOpen: false,

  setCameraSetup: (cameraSetup, loaded = true) => set({ cameraSetup, cameraSetupLoaded: loaded }),
  setCameraPrefs: (prefs) => set((s) => ({ cameraSetup: { ...s.cameraSetup, prefs: { ...s.cameraSetup.prefs, ...prefs } } })),
  setCameraState: (state) => set(state),
  setCameraCalibrationOpen: (cameraCalibrationOpen) => set({ cameraCalibrationOpen }),
});
