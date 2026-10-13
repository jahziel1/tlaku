import type { I18nKey } from '@/i18n';
import { alarmKey, crealityKey, errorKey, isStatusReport, parseAlarmLine, parseCrealityError, parseErrorLine, type FirmwareFamily } from './grblCodes';

export type LineKind = 'alarm' | 'error' | 'ok' | 'status' | 'plain';

export interface LineInfo {
  /** Cómo se pinta la línea. */
  kind: LineKind;
  /** Textos que se agregan debajo, en orden (significado y consejo). */
  explain: I18nKey[];
}

/** `ALARM:9` o `error:20` sin texto detrás: el motor suele explicarlos, pero no siempre. */
const BARE_GRBL = /^\s*(ALARM|error):\s*\d+\s*$/i;
/** `ERROR:05` o `ERROR:06.` sin texto detrás. */
const BARE_CREALITY = /^\s*ERROR:\s*\d{1,3}\.?\s*$/;

/**
 * Clasifica una línea del registro y decide qué explicar. Los códigos se leen según la familia del
 * firmware (`ALARM:10` y los `error:N` extendidos cambian de significado). Las fallas `ERROR:NN` de
 * Creality (mayúsculas) no son `error:N` de GRBL: llevan su propio significado (llama, inclinación,
 * tapa...) y un consejo. Solo las líneas recibidas (`in`) se explican.
 */
export function classifyLine(dir: 'in' | 'out', text: string, family: FirmwareFamily): LineInfo {
  if (dir !== 'in') return { kind: 'plain', explain: [] };
  const alarm = parseAlarmLine(text);
  if (alarm !== null) return { kind: 'alarm', explain: BARE_GRBL.test(text) ? [alarmKey(alarm, family)] : [] };
  const error = parseErrorLine(text);
  if (error !== null) return { kind: 'error', explain: BARE_GRBL.test(text) ? [errorKey(error, family)] : [] };
  const creality = parseCrealityError(text);
  if (creality !== null) {
    // Si el motor ya puso el significado después del código, solo falta el consejo.
    const meaning = BARE_CREALITY.test(text) ? [crealityKey(creality)] : [];
    return { kind: 'alarm', explain: [...meaning, 'machine.creality.recovery'] };
  }
  if (/^ok\b/i.test(text)) return { kind: 'ok', explain: [] };
  return { kind: isStatusReport(text) ? 'status' : 'plain', explain: [] };
}
