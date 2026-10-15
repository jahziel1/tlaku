import type { MachineState, ResumeReadiness, ResumeView } from '@/api/types';
import type { I18nKey } from '@/i18n';

/** Dónde se reanudará: la línea del G-code, el punto dentro de ella (si cae a mitad de un corte) y su movimiento. */
export interface RestartChoice {
  line: number;
  at: [number, number] | null;
  /** Índice en `ResumeView.moves` del movimiento con el que sigue el trabajo. */
  moveIndex: number;
  percent: number;
  /** Lo movió el usuario con el control (no es el que propuso el motor). */
  custom: boolean;
}

/** Cuántos movimientos tiene el recorrido (el primero es el punto de partida). */
export function moveCount(view: Pick<ResumeView, 'move_lines'>): number {
  return view.move_lines.length;
}

/** El punto que propone el motor. */
export function suggestedChoice(view: ResumeView): RestartChoice {
  const s = view.suggested;
  return { line: s.line, at: s.at, moveIndex: s.move_index, percent: s.percent, custom: false };
}

/** El punto que empieza en el movimiento `moveIndex` (siempre al inicio de su línea). */
export function choiceAt(view: ResumeView, moveIndex: number): RestartChoice {
  const n = moveCount(view);
  const i = Math.min(Math.max(1, Math.round(moveIndex)), Math.max(1, n - 1));
  return { line: view.move_lines[i] ?? 0, at: null, moveIndex: i, percent: n > 1 ? (i / (n - 1)) * 100 : 0, custom: true };
}

/** Movimiento del recorrido más cercano a (x, y), útil para elegir el punto desde la vista previa. */
export function nearestMove(view: Pick<ResumeView, 'moves'>, x: number, y: number): number {
  let best = 1;
  let bestD = Infinity;
  for (let i = 4; i + 3 < view.moves.length; i += 4) {
    const d = (view.moves[i + 1]! - x) ** 2 + (view.moves[i + 2]! - y) ** 2;
    if (d < bestD) {
      bestD = d;
      best = i / 4;
    }
  }
  return best;
}

/**
 * Qué paso de seguridad pide esta máquina antes de reanudar. `busy` (encuadre o movimiento en curso) y `noReply`
 * (no contestó a `$$`) no son «sin homing»: no hay nada que confirmar hasta que la máquina responda.
 */
export function homeMode(ready: ResumeReadiness | null): 'checking' | 'busy' | 'noReply' | 'home' | 'confirm' {
  if (!ready) return 'checking';
  if (ready.state !== 'idle' && ready.state !== 'alarm') return 'busy';
  if (!ready.settings_read) return 'noReply';
  return ready.homing === true ? 'home' : 'confirm';
}

export interface ResumeGate {
  connected: boolean;
  /** El perfil activo es el del trabajo interrumpido. */
  sameMachine: boolean;
  jobBusy: boolean;
  state: MachineState | undefined;
  ready: ResumeReadiness | null;
  /** Hay un punto de reinicio elegido. */
  hasPoint: boolean;
  unmoved: boolean;
  framedRun: boolean;
  framedOk: boolean;
  checklistOk: boolean;
}

/**
 * `null` si se puede reanudar; si no, la clave del primer motivo. Es la misma lista de condiciones que exige el
 * motor (`continue_job`), que las vuelve a comprobar: esto solo avisa antes.
 */
export function resumeBlockedReason(g: ResumeGate): I18nKey | null {
  if (!g.connected) return 'machine.resume.blocked.connect';
  if (!g.sameMachine) return 'machine.resume.blocked.wrong';
  if (g.jobBusy || (g.state !== undefined && g.state !== 'idle')) return 'machine.resume.blocked.busy';
  if (!g.hasPoint) return 'machine.resume.blocked.point';
  const mode = homeMode(g.ready);
  if (mode === 'checking') return 'machine.resume.blocked.checking';
  if (mode === 'busy') return 'machine.resume.blocked.busy';
  if (mode === 'noReply') return 'machine.resume.blocked.noReply';
  if (mode === 'home' && !g.ready?.homed) return 'machine.resume.blocked.home';
  if (mode === 'confirm' && !g.unmoved) return 'machine.resume.blocked.confirm';
  if (!g.framedRun) return 'machine.resume.blocked.frame';
  if (!g.framedOk) return 'machine.resume.blocked.frameOk';
  if (!g.checklistOk) return 'machine.resume.blocked.checklist';
  return null;
}
