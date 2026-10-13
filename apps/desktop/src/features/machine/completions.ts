import type { I18nKey } from '@/i18n';
import { classify, type Risk } from './commandSafety';

export interface CommandHint {
  /** Texto que se inserta. */
  text: string;
  /** Explicación breve (diccionario). */
  desc: I18nKey;
}

/** Comandos de GRBL 1.1 que más se usan en una consola de láser, de solo lectura primero. */
export const COMMAND_CATALOG: readonly CommandHint[] = [
  { text: '$$', desc: 'machine.console.cmd.settings' },
  { text: '$#', desc: 'machine.console.cmd.offsets' },
  { text: '$G', desc: 'machine.console.cmd.parser' },
  { text: '$I', desc: 'machine.console.cmd.info' },
  { text: '$N', desc: 'machine.console.cmd.startup' },
  { text: 'status', desc: 'machine.console.cmd.status' },
  { text: 'hold', desc: 'machine.console.cmd.hold' },
  { text: 'resume', desc: 'machine.console.cmd.resume' },
  { text: 'reset', desc: 'machine.console.cmd.reset' },
  { text: 'M5', desc: 'machine.console.cmd.m5' },
  { text: '$X', desc: 'machine.console.cmd.unlock' },
  { text: '$H', desc: 'machine.console.cmd.home' },
  { text: '$C', desc: 'machine.console.cmd.check' },
  { text: '$J=G91 G21 X10 F1000', desc: 'machine.console.cmd.jog' },
  { text: '$32=1', desc: 'machine.console.cmd.laserMode' },
  { text: 'G0 X0 Y0', desc: 'machine.console.cmd.g0' },
  { text: 'G1 X10 Y10 S0 F1000', desc: 'machine.console.cmd.g1' },
  { text: 'G90', desc: 'machine.console.cmd.g90' },
  { text: 'G91', desc: 'machine.console.cmd.g91' },
  { text: 'G92 X0 Y0', desc: 'machine.console.cmd.g92' },
  { text: 'G21', desc: 'machine.console.cmd.g21' },
  { text: 'G54', desc: 'machine.console.cmd.g54' },
  { text: 'M3 S100', desc: 'machine.console.cmd.m3' },
  { text: 'M4 S100', desc: 'machine.console.cmd.m4' },
] as const;

const norm = (s: string) => s.replace(/\s+/g, '').toUpperCase();

/** Comandos del catálogo que empiezan como lo escrito (sin distinguir mayúsculas ni espacios). */
export function completions(typed: string, limit = 6): CommandHint[] {
  const q = norm(typed);
  if (q === '') return [];
  return COMMAND_CATALOG.filter((c) => norm(c.text).startsWith(q) && norm(c.text) !== q).slice(0, limit);
}

/** Prefijo común de varias sugerencias (para completar con Tab sin elegir una de ellas). */
export function commonPrefix(texts: readonly string[]): string {
  if (texts.length === 0) return '';
  let p = texts[0]!;
  for (const s of texts) {
    let i = 0;
    while (i < p.length && i < s.length && p[i]!.toUpperCase() === s[i]!.toUpperCase()) i++;
    p = p.slice(0, i);
  }
  return p;
}

/** Riesgo de una sugerencia, con la misma clasificación que se aplica al enviar. */
export function hintRisk(hint: CommandHint): Risk {
  if (['status', 'hold', 'resume', 'reset'].includes(hint.text)) return 'safe';
  return classify(norm(hint.text));
}
