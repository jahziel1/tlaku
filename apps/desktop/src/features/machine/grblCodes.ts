import type { I18nKey } from '@/i18n';

/**
 * Familia del firmware. Los `error:N` y `ALARM:N` extendidos significan cosas distintas según
 * la familia (p. ej. `ALARM:10` es autoescuadre en GRBL 1.1 y paro de emergencia en grblHAL),
 * así que el texto depende de ella. Sin saberla se usa un texto genérico.
 */
export type FirmwareFamily = 'grbl' | 'grblhal' | 'fluidnc' | 'unknown';

/** Familia a partir del nombre que devuelve la conexión (`Grbl 1.1h`, `FluidNC v3.9.7`, `grblHAL 1.1f`). */
export function firmwareFamily(name: string | null | undefined): FirmwareFamily {
  const n = (name ?? '').toLowerCase();
  if (n.includes('fluidnc')) return 'fluidnc';
  if (n.includes('grblhal')) return 'grblhal';
  return n.startsWith('grbl') ? 'grbl' : 'unknown';
}

/** Códigos de `ALARM:N` con texto propio: 1 a 9 y 10 de GRBL 1.1, y 11 a 22 de grblHAL. */
export const ALARM_CODES = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22] as const;

/** Códigos de `error:N` de GRBL 1.1 con texto propio en el diccionario (válidos en cualquier firmware). */
export const ERROR_CODES = [
  1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34,
  35, 36, 37, 38,
] as const;

/** `error:N` extendidos de grblHAL con texto individual (más los grupos 60 a 92, ver `halGroup`). */
export const HAL_ERROR_CODES = [18, 19, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58] as const;

/** Códigos de falla propios de Creality (`ERROR:NN`) con texto en el diccionario. */
export const CREALITY_CODES = [1, 2, 4, 5, 6, 7, 13, 14, 24, 25] as const;

export function alarmKey(code: number | null, family: FirmwareFamily = 'unknown'): I18nKey {
  if (code === null || !(ALARM_CODES as readonly number[]).includes(code)) return 'machine.alarm.unknown';
  // El 10 cambia de significado según el firmware; sin saberlo, el texto lo dice.
  if (code === 10 && family !== 'unknown') return `machine.alarm.10.${family}` as I18nKey;
  return `machine.alarm.${code}` as I18nKey;
}

/** Grupo de errores de grblHAL (SD, expresiones...) al que pertenece un código, si lo hay. */
function halGroup(code: number): string | null {
  if (code >= 60 && code <= 66) return 'sd';
  if (code === 70) return 'bt';
  if (code >= 71 && code <= 76) return 'expr';
  if (code >= 77 && code <= 79) return 'auth';
  if (code >= 80 && code <= 83) return 'flow';
  if (code >= 84 && code <= 89) return 'files';
  if (code >= 90 && code <= 92) return 'cutter';
  return null;
}

/** Errores extendidos de FluidNC (lista de terceros) agrupados por significado. */
function fluidncGroup(code: number): string | null {
  if (code === 19) return '19';
  if (code === 39) return '39';
  if (code >= 60 && code <= 69) return 'sd';
  if (code === 70 || code === 71) return 'radio';
  if (code === 80 || code === 81) return 'setting';
  if (code === 90) return 'message';
  if (code === 100 || code === 101) return 'save';
  if (code === 110) return 'auth';
  if (code === 120) return 'busy';
  if (code === 152) return 'config';
  return null;
}

/**
 * Texto de un `error:N` de GRBL. Los de GRBL 1.1 valen en cualquier firmware; los extendidos
 * (18, 19 y desde el 39) solo se interpretan si se conoce la familia (grblHAL o FluidNC);
 * si no, se dice que dependen del firmware. No hay una tabla única: los números chocan.
 */
export function errorKey(code: number, family: FirmwareFamily = 'unknown'): I18nKey {
  if ((ERROR_CODES as readonly number[]).includes(code)) return `machine.error.${code}` as I18nKey;
  if (family === 'grblhal') {
    if ((HAL_ERROR_CODES as readonly number[]).includes(code)) return `machine.error.hal.${code}` as I18nKey;
    const g = halGroup(code);
    if (g) return `machine.error.hal.${g}` as I18nKey;
  } else if (family === 'fluidnc') {
    const g = fluidncGroup(code);
    if (g) return `machine.error.fnc.${g}` as I18nKey;
  }
  const extended = code === 18 || code === 19 || code >= 39;
  return extended && family !== 'grbl' ? 'machine.error.extended' : 'machine.error.unknown';
}

/** `ALARM:9` -> 9; otra línea -> null. */
export function parseAlarmLine(text: string): number | null {
  const m = /^\s*ALARM:\s*(\d+)/i.exec(text);
  return m ? Number(m[1]) : null;
}

/**
 * `error:20` -> 20 (el `error:N` de GRBL); otra línea -> null. Solo minúsculas: el
 * `ERROR:05` de Creality (mayúsculas) es otra cosa, ver `parseCrealityError`.
 */
export function parseErrorLine(text: string): number | null {
  const m = /^\s*error:\s*(\d+)/.exec(text);
  return m ? Number(m[1]) : null;
}

/**
 * `ERROR:05` -> 5 (falla propia de Creality: mayúsculas y dos dígitos); `ERROR:06.` y
 * `ERROR:06: texto` también. No es el `error:5` de GRBL ("ajuste desactivado") y no se debe
 * leer con su tabla. Otra línea -> null.
 */
export function parseCrealityError(text: string): number | null {
  const m = /^\s*ERROR:\s*(\d{1,3})(?![0-9A-Za-z\u00C0-\uFFFF])/.exec(text);
  return m ? Number(m[1]) : null;
}

/** Texto de una falla de Creality; los códigos sin descripción conocida caen en uno genérico. */
export function crealityKey(code: number): I18nKey {
  if (code >= 8 && code <= 12) return 'machine.creality.sensors';
  return (CREALITY_CODES as readonly number[]).includes(code)
    ? (`machine.creality.${code}` as I18nKey)
    : 'machine.creality.unknown';
}

/** ¿Es un reporte de estado (`<Idle|MPos:...>`)? La consola puede ocultarlos. */
export function isStatusReport(text: string): boolean {
  const s = text.trim();
  return s.startsWith('<') && s.endsWith('>');
}

// ---------------------------------------------------------------------------
// Qué sugerir ante una alarma (mismas reglas que `alarm_advice` del CLI)
// ---------------------------------------------------------------------------

export type AlarmAction = 'unlock' | 'home';

export type AlarmTip =
  | 'moveAway'
  | 'moveAwayThenUnlock'
  | 'homeAfter'
  | 'checkLimits'
  | 'homingOff'
  | 'homingOn'
  | 'ifLimitSwitches'
  | 'ifNoLimitSwitches'
  | 'positionLost';

export interface AlarmAdvice {
  tips: AlarmTip[];
  /** Acciones que se ofrecen, en orden de preferencia. Cada una pide confirmación. */
  actions: AlarmAction[];
  /** La que suele corresponder; null si depende de si la máquina tiene finales de carrera. */
  recommended: AlarmAction | null;
}

/**
 * `code` es el `ALARM:N` recibido (si lo hubo) y `homing` el valor de `$22 != 0`
 * (si se pudo leer). Nunca se ejecuta nada por su cuenta: solo se sugiere.
 */
export function alarmAdvice(code: number | null, homing: boolean | null): AlarmAdvice {
  const lost: AlarmTip[] = code === 3 ? ['positionLost'] : [];
  if (code === 4 || code === 5) return { tips: [], actions: ['unlock'], recommended: 'unlock' };
  // grblHAL: 11 pide homing. 10 es autoescuadre en GRBL 1.1 y paro de emergencia en grblHAL:
  // no se recomienda nada (el texto de la alarma explica la diferencia).
  if (code === 11) return { tips: ['homingOn'], actions: ['home', 'unlock'], recommended: 'home' };
  if (code === 10) return { tips: ['checkLimits'], actions: ['unlock', 'home'], recommended: null };
  if (code !== null && code >= 6 && code <= 9) {
    return { tips: ['checkLimits'], actions: ['home', 'unlock'], recommended: 'home' };
  }
  if (code === 1 || code === 2) {
    return homing === false
      ? { tips: ['moveAway'], actions: ['unlock'], recommended: 'unlock' }
      : { tips: ['moveAwayThenUnlock', 'homeAfter'], actions: ['unlock', 'home'], recommended: 'unlock' };
  }
  if (homing === false) return { tips: [...lost, 'homingOff'], actions: ['unlock'], recommended: 'unlock' };
  if (homing === true) return { tips: [...lost, 'homingOn'], actions: ['home', 'unlock'], recommended: 'home' };
  return { tips: [...lost, 'ifLimitSwitches', 'ifNoLimitSwitches'], actions: ['home', 'unlock'], recommended: null };
}
