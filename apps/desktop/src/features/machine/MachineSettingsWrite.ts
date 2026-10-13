import { getApi } from '@/api';
import type { I18nKey } from '@/i18n';
import { isJobBusy, useStore, type AppState } from '@/store';
import { errorMessage } from '@/lib/errors';
import { parseErrorLine } from './grblCodes';
import { sameValue, type RawSetting } from './MachineSettingsLogic';

/**
 * Escritura segura de ajustes `$n=valor`: UNA línea a la vez con "enviar y esperar" (jamás con
 * conteo de caracteres ni varias juntas), la respuesta tiene que terminar en `ok`, y al primer
 * fallo se para y se informa cuáles quedaron sin escribir.
 */

/** Envía una línea y devuelve la respuesta completa (la última línea es `ok`, `error:N` o `ALARM:N`). */
export type SendFn = (line: string) => Promise<string[]>;

export interface WriteFailure {
  /** La línea que falló (o la que no se llegó a enviar si el motivo es `blocked`). */
  line: string;
  /** `error`: la controladora dijo `error:N`; `reply`: contestó otra cosa que `ok`; `exception`: el envío falló; `blocked`: se perdió la conexión o empezó un trabajo. */
  kind: 'error' | 'reply' | 'exception' | 'blocked';
  /** El `N` de `error:N`, si lo hay. */
  code: number | null;
  /** Texto de la respuesta, del error o la clave del motivo del bloqueo. */
  text: string;
  blockedKey?: I18nKey;
}

export interface WriteOutcome {
  /** Líneas que la controladora confirmó con `ok`, en el orden en que se enviaron. */
  written: string[];
  failure: WriteFailure | null;
  /** Líneas que no se enviaron porque antes hubo un fallo (sin contar la que falló). */
  pending: string[];
}

export type ReplyKind = { kind: 'ok' } | { kind: 'error'; code: number | null; text: string } | { kind: 'reply'; text: string };

/** Interpreta la respuesta a una escritura: la última línea con texto manda y debe ser `ok`. */
export function classifyReply(reply: readonly string[]): ReplyKind {
  const last = [...reply].reverse().find((l) => l.trim() !== '')?.trim();
  if (last === undefined) return { kind: 'reply', text: '' };
  if (last.toLowerCase() === 'ok') return { kind: 'ok' };
  const code = parseErrorLine(last);
  if (code !== null || /^error\b/i.test(last)) return { kind: 'error', code, text: last };
  return { kind: 'reply', text: last };
}

/** Por qué no se puede escribir ahora (sin conexión o con un trabajo en curso); `null` si se puede. */
export function settingsBlockedReason(s: Pick<AppState, 'connection' | 'jobState'>): I18nKey | null {
  if (!s.connection.connected) return 'reason.needMachine';
  if (isJobBusy(s)) return 'reason.jobRunning';
  return null;
}

/** Igual que `settingsBlockedReason`, sobre el estado vigente (se consulta antes de cada línea). */
export function writeBlockedReason(): I18nKey | null {
  return settingsBlockedReason(useStore.getState());
}

const defaultSend: SendFn = (line) => getApi().machineCommand({ line });

let inFlight = false;

export interface WriteOptions {
  send?: SendFn;
  /** Se consulta antes de cada línea; si devuelve una clave, se detiene. */
  blocked?: () => I18nKey | null;
  onProgress?: (done: number, total: number) => void;
}

/**
 * Escribe las líneas en el orden dado, una por una. Se detiene en la primera que no conteste `ok`
 * y devuelve qué se escribió, qué falló y qué quedó pendiente. No se admiten dos escrituras a la vez.
 */
export async function writeSettingLines(lines: readonly string[], opts: WriteOptions = {}): Promise<WriteOutcome> {
  if (inFlight) throw new Error('Ya hay una escritura de ajustes en curso.');
  const send = opts.send ?? defaultSend;
  const blocked = opts.blocked ?? writeBlockedReason;
  const outcome: WriteOutcome = { written: [], failure: null, pending: [] };
  inFlight = true;
  try {
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i]!;
      const why = blocked();
      let failure: WriteFailure | null = why ? { line, kind: 'blocked', code: null, text: '', blockedKey: why } : null;
      if (!failure) {
        try {
          const verdict = classifyReply(await send(line));
          if (verdict.kind === 'error') failure = { line, kind: 'error', code: verdict.code, text: verdict.text };
          else if (verdict.kind === 'reply') failure = { line, kind: 'reply', code: null, text: verdict.text };
        } catch (e) {
          failure = { line, kind: 'exception', code: null, text: errorMessage(e) };
        }
      }
      if (failure) {
        outcome.failure = failure;
        outcome.pending = lines.slice(i + 1);
        return outcome;
      }
      outcome.written.push(line);
      opts.onProgress?.(outcome.written.length, lines.length);
    }
    return outcome;
  } finally {
    inFlight = false;
  }
}

export interface Mismatch {
  id: number;
  expected: string;
  /** Lo que informa la máquina ahora; `null` si el ajuste ya no aparece. */
  actual: string | null;
}

/** Compara lo que se escribió (`$n=valor`) con lo que informó `$$` después. Lista solo lo que no coincide. */
export function verifyWritten(written: readonly string[], reread: readonly RawSetting[]): Mismatch[] {
  const now = new Map(reread.map((s) => [s.id, s.raw]));
  const out: Mismatch[] = [];
  for (const line of written) {
    const m = /^\$(\d+)=(.*)$/.exec(line);
    if (!m) continue;
    const id = Number(m[1]);
    const actual = now.get(id) ?? null;
    if (actual === null || !sameValue(actual, m[2]!)) out.push({ id, expected: m[2]!, actual });
  }
  return out;
}
