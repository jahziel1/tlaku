/** Captura de video: restricciones de getUserMedia y clasificación de errores. */

export type CameraProblem = 'denied' | 'none' | 'busy' | 'unsupported' | 'unknown';

export interface Resolution {
  width: number;
  height: number;
}

export const RESOLUTIONS: readonly Resolution[] = [
  { width: 640, height: 480 },
  { width: 1280, height: 720 },
  { width: 1920, height: 1080 },
];

export interface CaptureRequest {
  deviceId?: string | null;
  width?: number | null;
  height?: number | null;
}

/** Restricciones de getUserMedia. El tamaño es "ideal": la cámara puede dar otro cercano. */
export function buildConstraints(req: CaptureRequest): MediaStreamConstraints {
  const video: MediaTrackConstraints = {};
  if (req.deviceId) video.deviceId = { exact: req.deviceId };
  if (req.width && req.height) {
    video.width = { ideal: req.width };
    video.height = { ideal: req.height };
  }
  return { audio: false, video: Object.keys(video).length > 0 ? video : true };
}

/** Traduce el error de getUserMedia a un problema que la interfaz sabe explicar. */
export function classifyCameraError(e: unknown): CameraProblem {
  const name = typeof e === 'object' && e !== null && 'name' in e ? String((e as { name: unknown }).name) : '';
  switch (name) {
    case 'NotAllowedError':
    case 'PermissionDeniedError':
    case 'SecurityError':
      return 'denied';
    case 'NotFoundError':
    case 'DevicesNotFoundError':
      return 'none';
    case 'NotReadableError':
    case 'TrackStartError':
    case 'AbortError':
      return 'busy';
    case 'TypeError':
      return 'unsupported';
    default:
      return 'unknown';
  }
}

export interface CameraDeviceInfo {
  id: string;
  label: string;
}

/** Cámaras de la lista de dispositivos; sin permiso el navegador no da etiquetas ni ids. */
export function videoInputs(devices: readonly Pick<MediaDeviceInfo, 'kind' | 'deviceId' | 'label'>[]): CameraDeviceInfo[] {
  return devices
    .filter((d) => d.kind === 'videoinput')
    .map((d, i) => ({ id: d.deviceId, label: d.label || `#${i + 1}` }));
}

/** Elige la cámara guardada por id y, si cambió, por nombre. */
export function pickDevice(
  devices: readonly CameraDeviceInfo[],
  saved: { id?: string | null; label?: string | null },
): CameraDeviceInfo | null {
  return devices.find((d) => saved.id && d.id === saved.id) ?? devices.find((d) => saved.label && d.label === saved.label) ?? null;
}
