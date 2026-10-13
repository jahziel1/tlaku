import type { I18nKey } from '@/i18n';
import type { HistoryEntry } from '@/store/document';

export interface TimelineRow {
  /** Punto del historial al que lleva esta fila (ver `jumpTo`): 0 = estado inicial. */
  position: number;
  label: I18nKey;
  /** Hora de la acción; null en el estado inicial. */
  at: number | null;
  /** `applied` = ya hecho, `current` = estado actual, `undone` = deshecho (se puede rehacer). */
  status: 'applied' | 'current' | 'undone';
}

/** Línea de tiempo completa, del estado inicial al último paso rehacible (la más antigua primero). */
export function buildTimeline(past: readonly HistoryEntry[], future: readonly HistoryEntry[]): TimelineRow[] {
  const rows: TimelineRow[] = [{ position: 0, label: 'history.initial', at: null, status: past.length === 0 ? 'current' : 'applied' }];
  [...past, ...future].forEach((e, i) => {
    const position = i + 1;
    rows.push({
      position,
      label: e.label,
      at: e.at,
      status: position < past.length ? 'applied' : position === past.length ? 'current' : 'undone',
    });
  });
  return rows;
}

/** Hora local `HH:MM:SS` con dos cifras en cada parte. */
export function formatTime(at: number): string {
  const d = new Date(at);
  return [d.getHours(), d.getMinutes(), d.getSeconds()].map((n) => String(n).padStart(2, '0')).join(':');
}
