import type { I18nKey } from '@/i18n';

/** Códigos de `ALARM:N` de GRBL 1.1 con texto propio en el diccionario. */
export const ALARM_CODES = [1, 2, 3, 4, 5, 6, 7, 8, 9] as const;

/** Códigos de `error:N` de GRBL 1.1 con texto propio en el diccionario. */
export const ERROR_CODES = [
  1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34,
  35, 36, 37, 38,
] as const;

export function alarmKey(code: number | null): I18nKey {
  return code !== null && (ALARM_CODES as readonly number[]).includes(code)
    ? (`machine.alarm.${code}` as I18nKey)
    : 'machine.alarm.unknown';
}

export function errorKey(code: number): I18nKey {
  return (ERROR_CODES as readonly number[]).includes(code) ? (`machine.error.${code}` as I18nKey) : 'machine.error.unknown';
}

/** `ALARM:9` -> 9; `error:20` -> 20; otra línea -> null. */
export function parseAlarmLine(text: string): number | null {
  const m = /^\s*ALARM:\s*(\d+)/i.exec(text);
  return m ? Number(m[1]) : null;
}

export function parseErrorLine(text: string): number | null {
  const m = /^\s*error:\s*(\d+)/i.exec(text);
  return m ? Number(m[1]) : null;
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
