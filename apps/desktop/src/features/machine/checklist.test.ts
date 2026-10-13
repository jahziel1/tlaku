import { describe, expect, it } from 'vitest';
import { CHECKLIST_ALL, checklistPending, checklistSatisfied, visibleChecklist, withChecklistItem, type ChecklistItem } from './checklist';

const ticked = (...ids: ChecklistItem['id'][]) => new Set(ids);

describe('lista de comprobación', () => {
  it('por omisión muestra las cinco casillas', () => {
    expect(visibleChecklist(CHECKLIST_ALL).map((i) => i.id)).toEqual(['air', 'focus', 'material', 'extractor', 'lid']);
    expect(CHECKLIST_ALL).toBe(31);
  });

  it('cada casilla se enciende y se apaga sin tocar las demás', () => {
    const noAir = withChecklistItem(CHECKLIST_ALL, 1, false);
    expect(visibleChecklist(noAir).map((i) => i.id)).toEqual(['focus', 'material', 'extractor', 'lid']);
    expect(withChecklistItem(noAir, 1, true)).toBe(CHECKLIST_ALL);
    expect(visibleChecklist(0)).toEqual([]);
  });

  it('sin exigirla, siempre se puede iniciar', () => {
    expect(checklistSatisfied(CHECKLIST_ALL, false, ticked())).toBe(true);
  });

  it('exigida, hacen falta todas las casillas visibles y solo ellas', () => {
    expect(checklistSatisfied(CHECKLIST_ALL, true, ticked('air', 'focus', 'material', 'extractor'))).toBe(false);
    expect(checklistSatisfied(CHECKLIST_ALL, true, ticked('air', 'focus', 'material', 'extractor', 'lid'))).toBe(true);
    // Una casilla oculta no se exige.
    const noLid = withChecklistItem(CHECKLIST_ALL, 16, false);
    expect(checklistSatisfied(noLid, true, ticked('air', 'focus', 'material', 'extractor'))).toBe(true);
    // Lista vacía: nada que exigir.
    expect(checklistSatisfied(0, true, ticked())).toBe(true);
  });

  it('dice cuáles faltan', () => {
    expect(checklistPending(CHECKLIST_ALL, ticked('air', 'lid')).map((i) => i.id)).toEqual(['focus', 'material', 'extractor']);
  });
});
