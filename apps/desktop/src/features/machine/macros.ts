import type { Macro } from '@/api/types';
import { needsConfirmation, parseCommand, isDestructive, type CommandError, type Risk } from './commandSafety';

/** Máximo de macros por perfil (cada una puede tener un atajo propio: `machine.macro1` a `machine.macro10`). */
export const MAX_MACROS = 10;
/** Máximo de líneas útiles por macro (una macro es un atajo, no un programa). */
export const MAX_MACRO_LINES = 50;

/** Una línea ya validada de una macro, tal como se va a enviar. */
export interface MacroLine {
  /** Línea normalizada (mayúsculas, sin espacios ni comentarios). */
  text: string;
  /** Línea tal cual la escribió el usuario, sin comentarios: para firmwares que necesitan espacios (Marlin). */
  raw: string;
  risk: Risk;
  destructive: boolean;
}

export type MacroPlan =
  | {
      ok: true;
      lines: MacroLine[];
      /** El riesgo más alto de sus líneas. */
      risk: Risk;
      /** Hay que mostrar las líneas y pedir confirmación: lo marcó el usuario o alguna línea no es de solo lectura. */
      confirm: boolean;
    }
  | { ok: false; error: { code: 'empty' } | { code: 'tooMany'; max: number } | { code: 'realtime'; line: number; text: string } | { code: 'line'; line: number; error: CommandError } };

const RANK: Record<Risk, number> = { safe: 0, other: 1, setting: 2, unlock: 2, home: 3, moves: 3, laser: 4 };

/** Quita comentarios `;…` para firmwares que reciben la línea con espacios. */
function stripComment(line: string): string {
  const i = line.indexOf(';');
  return (i >= 0 ? line.slice(0, i) : line).trim();
}

/**
 * Valida una macro con las mismas reglas de la consola (`parseCommand`): sin caracteres de tiempo
 * real, cada línea cabe en el búfer, y se calcula el riesgo de cada una. Las líneas vacías o de
 * solo comentario se omiten. Una macro no puede mandar bytes de tiempo real (`hold`, `reset`...).
 */
export function planMacro(macro: Pick<Macro, 'gcode' | 'confirm'>, rxBuffer: number): MacroPlan {
  const lines: MacroLine[] = [];
  const rows = macro.gcode.split(/\r?\n/);
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i]!;
    const raw = stripComment(row);
    if (raw === '') continue;
    const parsed = parseCommand(row, rxBuffer);
    if (!parsed.ok) {
      // Una línea que solo trae comentario queda vacía: se omite como arriba. Lo demás es un error real.
      if (parsed.error.code === 'empty') continue;
      return { ok: false, error: { code: 'line', line: i + 1, error: parsed.error } };
    }
    if (parsed.cmd.kind === 'realtime') return { ok: false, error: { code: 'realtime', line: i + 1, text: row.trim() } };
    lines.push({ text: parsed.cmd.text, raw, risk: parsed.cmd.risk, destructive: isDestructive(parsed.cmd.text) });
  }
  if (lines.length === 0) return { ok: false, error: { code: 'empty' } };
  if (lines.length > MAX_MACRO_LINES) return { ok: false, error: { code: 'tooMany', max: MAX_MACRO_LINES } };
  const risk = lines.reduce<Risk>((worst, l) => (RANK[l.risk] > RANK[worst] ? l.risk : worst), 'safe');
  return { ok: true, lines, risk, confirm: macro.confirm || lines.some((l) => needsConfirmation(l.risk)) };
}

/** Nombre y G-code válidos para guardar: nombre no vacío y que el G-code se pueda validar. */
export function macroProblem(macro: Macro, rxBuffer: number): 'name' | 'gcode' | null {
  if (macro.name.trim() === '') return 'name';
  return planMacro(macro, rxBuffer).ok ? null : 'gcode';
}

/** ¿La última línea de la respuesta es un `ok`? Cualquier otra cosa (`error:N`, `ALARM:N`) es un rechazo. */
export function replyOk(reply: readonly string[]): boolean {
  const last = reply[reply.length - 1];
  return last !== undefined && last.startsWith('ok');
}
