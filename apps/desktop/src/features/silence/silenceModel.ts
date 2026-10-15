/** Lógica pura del aviso de «la máquina no contesta» (sin React ni store). */

/** Desde cuántos milisegundos de silencio se muestra el aviso (el motor avisa a partir de ~2 s). */
export const SILENCE_SHOW_MS = 2000;

/** Pasado este silencio EN PLENO TRABAJO el aviso se vuelve más serio (la conexión sigue abierta). */
export const SILENCE_SERIOUS_MS = 60_000;

export interface SilenceInput {
  connected: boolean;
  /** `silenceMs` del store: lo que la placa lleva sin contestar; 0 = contesta. */
  silenceMs: number;
  /** Hay un trabajo corriendo o en pausa. */
  jobBusy: boolean;
}

export interface SilenceNotice {
  /** Segundos enteros sin respuesta (para el texto «desde hace N s»). */
  seconds: number;
  /** El silencio ocurre con un trabajo en marcha. */
  duringJob: boolean;
  /** Más de un minuto sin respuesta durante un trabajo: revisar la máquina y, si hace falta, el paro de emergencia. */
  serious: boolean;
}

/**
 * El aviso de «la máquina no contesta desde hace N s…», o `null` si no toca mostrarlo. Nunca implica
 * desconectar: Tlaku espera (como LightBurn) y el aviso se quita solo cuando la placa vuelve a contestar.
 */
export function silenceNotice(i: SilenceInput): SilenceNotice | null {
  if (!i.connected || i.silenceMs < SILENCE_SHOW_MS) return null;
  return { seconds: Math.floor(i.silenceMs / 1000), duringJob: i.jobBusy, serious: i.jobBusy && i.silenceMs >= SILENCE_SERIOUS_MS };
}
