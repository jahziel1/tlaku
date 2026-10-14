/**
 * Cámara de red (M35): bucle que pide un cuadro tras otro al motor (`fetch_camera_frame`) y se lo
 * pasa a la sesión. Va aparte de `session.ts` para probarlo sin cámara ni WebGL: las dependencias
 * (pedir, decodificar, esperar) se inyectan.
 *
 * Un cuadro por llamada: el ritmo real lo marca la cámara (a menudo 3 a 8 cuadros por segundo).
 * Si falla el primer cuadro se avisa de inmediato (URL mala, clave, fuera de la red local); una vez
 * que ya hubo imagen, se toleran unos fallos seguidos antes de darla por perdida.
 */
import type { NetworkFrame } from '@/api/types';

export interface NetworkDeps {
  fetchFrame(url: string): Promise<NetworkFrame>;
  /** Dibuja el cuadro en el canvas de origen de la sesión y devuelve su tamaño. */
  decode(frame: NetworkFrame): Promise<[number, number]>;
  sleep(ms: number): Promise<void>;
}

export interface NetworkHandlers {
  /** Llegó un cuadro ya dibujado; `first` en el primero de la sesión. */
  onFrame(size: [number, number], first: boolean): void;
  /** La cámara se dio por perdida (el bucle termina). */
  onFail(message: string): void;
}

export interface NetworkOptions {
  /** Pausa mínima entre peticiones (ms). */
  intervalMs?: number;
  /** Fallos seguidos tras el primer cuadro antes de rendirse. */
  maxFailures?: number;
  /** Espera tras un fallo (ms). */
  retryMs?: number;
}

export const NET_INTERVAL_MS = 120;
export const NET_MAX_FAILURES = 5;
export const NET_RETRY_MS = 500;

const messageOf = (e: unknown): string => (e instanceof Error ? e.message : typeof e === 'string' ? e : String(e));

/** Corre hasta que `current()` deje de ser cierto o la cámara se pierda. */
export async function runNetworkLoop(
  url: string,
  deps: NetworkDeps,
  handlers: NetworkHandlers,
  current: () => boolean,
  opts: NetworkOptions = {},
): Promise<void> {
  const interval = opts.intervalMs ?? NET_INTERVAL_MS;
  const maxFailures = opts.maxFailures ?? NET_MAX_FAILURES;
  const retry = opts.retryMs ?? NET_RETRY_MS;
  let first = true;
  let failures = 0;
  const t0 = () => Date.now();
  while (current()) {
    const started = t0();
    try {
      const frame = await deps.fetchFrame(url);
      if (!current()) return;
      const size = await deps.decode(frame);
      if (!current()) return;
      handlers.onFrame(size, first);
      first = false;
      failures = 0;
    } catch (e) {
      if (!current()) return;
      failures++;
      if (first || failures >= maxFailures) {
        handlers.onFail(messageOf(e));
        return;
      }
      await deps.sleep(retry);
      continue;
    }
    const spent = t0() - started;
    if (spent < interval) await deps.sleep(interval - spent);
  }
}

/** Bytes de un texto en base64. */
export function base64ToBytes(b64: string): Uint8Array<ArrayBuffer> {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Decodifica el cuadro y lo dibuja en `canvas` (solo en el navegador). Devuelve su tamaño. */
export async function drawNetworkFrame(frame: NetworkFrame, canvas: HTMLCanvasElement): Promise<[number, number]> {
  const bitmap = await createImageBitmap(new Blob([base64ToBytes(frame.data_base64)], { type: frame.mime }));
  try {
    if (canvas.width !== bitmap.width || canvas.height !== bitmap.height) {
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
    }
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('No se pudo dibujar el cuadro de la cámara de red.');
    ctx.drawImage(bitmap, 0, 0);
    return [bitmap.width, bitmap.height];
  } finally {
    bitmap.close();
  }
}
