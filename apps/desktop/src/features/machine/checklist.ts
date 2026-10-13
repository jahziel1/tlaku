import type { I18nKey } from '@/i18n';

/**
 * Lista de comprobación del diálogo de inicio. Cada casilla es un bit de la preferencia
 * `jobChecklistItems` (ver `store/prefs.ts`): así la lista configurable cabe en una sola clave.
 */
export interface ChecklistItem {
  id: 'air' | 'focus' | 'material' | 'extractor' | 'lid';
  bit: number;
  label: I18nKey;
}

export const CHECKLIST_ITEMS: readonly ChecklistItem[] = [
  { id: 'air', bit: 1, label: 'machine.checklist.air' },
  { id: 'focus', bit: 2, label: 'machine.checklist.focus' },
  { id: 'material', bit: 4, label: 'machine.checklist.material' },
  { id: 'extractor', bit: 8, label: 'machine.checklist.extractor' },
  { id: 'lid', bit: 16, label: 'machine.checklist.lid' },
];

/** Todas las casillas: es el valor de fábrica de `jobChecklistItems`. */
export const CHECKLIST_ALL = CHECKLIST_ITEMS.reduce((m, i) => m | i.bit, 0);

/** Casillas que muestra la lista con esa configuración, en orden. */
export function visibleChecklist(mask: number): ChecklistItem[] {
  return CHECKLIST_ITEMS.filter((i) => (mask & i.bit) !== 0);
}

/** `mask` con la casilla `bit` incluida o no en la lista. */
export function withChecklistItem(mask: number, bit: number, shown: boolean): number {
  return (shown ? mask | bit : mask & ~bit) & CHECKLIST_ALL;
}

/** ¿Se puede iniciar? Sin la opción de exigirla, siempre; con ella, solo con todas las casillas visibles marcadas. */
export function checklistSatisfied(mask: number, required: boolean, ticked: ReadonlySet<ChecklistItem['id']>): boolean {
  return !required || visibleChecklist(mask).every((i) => ticked.has(i.id));
}

/** Las casillas visibles que faltan por marcar. */
export function checklistPending(mask: number, ticked: ReadonlySet<ChecklistItem['id']>): ChecklistItem[] {
  return visibleChecklist(mask).filter((i) => !ticked.has(i.id));
}
