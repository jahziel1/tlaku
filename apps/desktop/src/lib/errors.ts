/** Texto crudo de cualquier error (los comandos de Tauri rechazan con un string). */
function rawMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === 'string') return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

/**
 * Los errores del motor que la interfaz debe reconocer llevan un código estable al inicio:
 * `[gcode_zero_power] a.nc: ...` (`tlaku_i18n::coded` en Rust). El texto cambia con el idioma; el
 * código no. Nunca compares el texto de un error del motor: pide un código en Rust y compáralo aquí.
 */
const CODE_RE = /^\s*\[([a-z][a-z0-9_]*)\]\s*/;

/** Códigos de error del motor que la interfaz reconoce. */
export const ENGINE_ERROR = {
  /** G-code cuyos cortes llevan todos potencia 0 (`M3 S0`). */
  gcodeZeroPower: 'gcode_zero_power',
  /** El trabajo (con su overscan) o el encuadre se sale de la cama. */
  bedOverflow: 'bed_overflow',
  /** El diseño está vacío o ningún objeto produce salida: no hay nada que enviar. */
  nothingToSend: 'nothing_to_send',
  /** Galvo: detener NO se confirmó; la fuente puede seguir habilitada (`tlaku_galvo::session::STOP_UNCONFIRMED_CODE`). */
  galvoStopUnconfirmed: 'galvo_stop_unconfirmed',
  /** K40: detener NO se confirmó; la placa puede seguir en marcha o con el láser encendido (`machine::lhymicro::STOP_UNCONFIRMED`). */
  lhyStopUnconfirmed: 'lhymicro_stop_unconfirmed',
  /** Línea del registro de la consola: terminó el recorrido de un encuadre. */
  frameDone: 'frame_done',
  /** Línea del registro de la consola: se canceló el recorrido de un encuadre. */
  frameCancelled: 'frame_cancelled',
} as const;

/** Código estable del error del motor (`gcode_zero_power`), o `null` si no trae uno. */
export function errorCode(e: unknown): string | null {
  const m = CODE_RE.exec(rawMessage(e));
  return m ? m[1] : null;
}

/** Texto legible de cualquier error, sin el código del motor (que no es para el usuario). */
export function errorMessage(e: unknown): string {
  return rawMessage(e).replace(CODE_RE, '');
}

/**
 * ¿El mensaje o error del motor dice que detener NO se confirmó (galvo o K40)? Se reconoce solo por
 * el código estable, nunca por el texto, que cambia con el idioma.
 */
export function isStopUnconfirmed(e: unknown): boolean {
  const code = errorCode(e);
  return code === ENGINE_ERROR.galvoStopUnconfirmed || code === ENGINE_ERROR.lhyStopUnconfirmed;
}
