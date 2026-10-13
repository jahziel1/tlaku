import { t } from '@/i18n';
import { errorKey, type FirmwareFamily } from './grblCodes';
import type { Mismatch, WriteFailure, WriteOutcome } from './MachineSettingsWrite';

/** Aviso que se muestra tras una escritura (de uno o de varios ajustes). */
export interface SettingsNotice {
  tone: 'ok' | 'warn' | 'error';
  text: string;
  /** Líneas de detalle: lo pendiente, o lo que no coincidió al releer. */
  details?: string[];
}

/** Texto de por qué falló una línea: el `error:N` con su explicación, o lo que contestó la máquina. */
export function failureText(f: WriteFailure, family: FirmwareFamily): string {
  switch (f.kind) {
    case 'error':
      return f.code === null ? f.text : `error:${f.code}: ${t(errorKey(f.code, family))}`;
    case 'reply':
      return f.text === '' ? t('mset.result.noReply') : t('mset.result.unexpected', { reply: f.text });
    case 'blocked':
      return f.blockedKey ? t(f.blockedKey) : '';
    case 'exception':
      return f.text;
  }
}

/**
 * Resumen de una escritura: si todo salió bien (y la relectura coincide) es un aviso verde; si hubo
 * un fallo, dice cuál línea falló, por qué y cuáles quedaron sin escribir; si la relectura no
 * coincide, lo avisa sin llamar fallo a lo que la máquina aceptó.
 */
export function outcomeNotice(outcome: WriteOutcome, family: FirmwareFamily, mismatches: readonly Mismatch[] | null): SettingsNotice {
  const f = outcome.failure;
  if (f) {
    const notWritten = [f.line, ...outcome.pending];
    return {
      tone: 'error',
      text: t(f.kind === 'blocked' ? 'mset.result.stoppedBlocked' : 'mset.result.failed', { line: f.line, error: failureText(f, family), done: outcome.written.length }),
      details: [t('mset.result.pending', { n: notWritten.length }), ...notWritten],
    };
  }
  if (mismatches === null) {
    return { tone: 'warn', text: t('mset.result.noVerify', { n: outcome.written.length }) };
  }
  if (mismatches.length > 0) {
    return {
      tone: 'warn',
      text: t('mset.result.mismatch', { n: outcome.written.length }),
      details: mismatches.map((m) => t(m.actual === null ? 'mset.result.mismatchGone' : 'mset.result.mismatchRow', { n: m.id, expected: m.expected, actual: m.actual ?? '' })),
    };
  }
  return { tone: 'ok', text: t('mset.result.ok', { n: outcome.written.length }) };
}
