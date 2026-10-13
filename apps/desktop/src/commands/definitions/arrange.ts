import { moveDoc, alignMoves, bedBounds, centerOnBedMoves, distributeMoves, type AlignKind, type DistributeAxis } from '@/features/arrange/align';
import { centerForRadius, DEFAULT_RADIUS } from '@/features/arrange/arrays';
import { cleanupReason } from '@/features/arrange/cleanup';
import { analyzeSelection, geometryReason, runGeometry } from '@/features/arrange/geomOps';
import { useArrangeUi, type AlignReference, type CleanupOp } from '@/features/arrange/uiStore';
import { blockBounds, boundsCenter, selectionUnits } from '@/features/arrange/units';
import type { I18nKey } from '@/i18n';
import { selectActiveDevice } from '@/store';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

/** Motivo por el que no se puede mover nada, o `true` si hay al menos `min` unidades móviles. */
function movableReason(s: AppState, min: number, few: I18nKey): true | I18nKey {
  if (s.selectedIds.length === 0) return 'arrange.reason.noSelection';
  const { movable, units } = analyzeSelection(s);
  if (movable === 0) return units > 0 ? 'arrange.reason.locked' : 'arrange.reason.noSelection';
  return movable >= min ? true : few;
}

const ALIGN_KINDS: AlignKind[] = ['left', 'hcenter', 'right', 'top', 'vcenter', 'bottom'];

/** Respecto a qué se alinea ahora y, si es un objeto, cuál (por defecto el primero seleccionado). */
function alignTarget(s: AppState, units: ReturnType<typeof selectionUnits>) {
  const ui = useArrangeUi.getState();
  const bed = bedBounds(selectActiveDevice(s).bed);
  if (ui.alignReference === 'bed') return { ref: bed, fixedId: undefined };
  if (ui.alignReference === 'object') {
    const key = units.find((u) => u.ids.includes(ui.alignObjectId ?? '')) ?? units[0];
    return key ? { ref: key.bounds, fixedId: key.id } : null;
  }
  const block = blockBounds(units);
  return block ? { ref: block, fixedId: undefined } : null;
}

const alignReason = (s: AppState): true | I18nKey => {
  const ref = useArrangeUi.getState().alignReference;
  return movableReason(s, ref === 'bed' ? 1 : 2, 'arrange.reason.alignNeedTwo');
};

const alignCommands: CommandDef[] = ALIGN_KINDS.map((kind) => ({
  id: `arrange.align.${kind}`,
  label: `arrange.cmd.align.${kind}` as I18nKey,
  category: 'arrange',
  enabled: alignReason,
  run: (s) => {
    const units = selectionUnits(s.doc, s.selectedIds);
    const target = alignTarget(s, units);
    if (!target) return;
    const moves = alignMoves(units, kind, target.ref, target.fixedId);
    s.applyDocChange(`arrange.history.align.${kind}` as I18nKey, (doc) => moveDoc(doc, moves));
  },
}));

const distributeCommands: CommandDef[] = (['h', 'v'] satisfies DistributeAxis[]).map((axis) => ({
  id: `arrange.distribute.${axis}`,
  label: `arrange.cmd.distribute.${axis}` as I18nKey,
  category: 'arrange',
  enabled: (s) => movableReason(s, 3, 'arrange.reason.distributeNeedThree'),
  run: (s) => {
    const moves = distributeMoves(selectionUnits(s.doc, s.selectedIds), axis);
    s.applyDocChange(`arrange.history.distribute.${axis}` as I18nKey, (doc) => moveDoc(doc, moves));
  },
}));

/** Unir, cerrar y quitar duplicados: abren un diálogo pequeño con la tolerancia. */
const cleanupCommands: CommandDef[] = (['join', 'close', 'dedupe'] satisfies CleanupOp[]).map((op) => ({
  id: `arrange.${op}`,
  label: `arrange.cmd.${op}` as I18nKey,
  category: 'arrange',
  enabled: (s) => cleanupReason(s, op),
  run: () => useArrangeUi.getState().open({ kind: 'cleanup', op }),
}));

const referenceCommands: CommandDef[] = (['selection', 'object', 'bed'] satisfies AlignReference[]).map((ref) => ({
  id: `arrange.alignTo.${ref}`,
  label: `arrange.cmd.alignTo.${ref}` as I18nKey,
  category: 'arrange',
  checked: () => useArrangeUi.getState().alignReference === ref,
  run: (s) => {
    // "Un objeto": se fija el primero de la selección; el diálogo de alinear deja elegir otro.
    const first = selectionUnits(s.doc, s.selectedIds)[0]?.id ?? null;
    useArrangeUi.getState().setAlign(ref, ref === 'object' ? first : undefined);
  },
}));

export const arrangeCommands: CommandDef[] = [
  // Booleanas
  {
    id: 'arrange.union',
    label: 'arrange.cmd.union',
    category: 'arrange',
    shortcuts: ['Alt+U'],
    enabled: (s) => geometryReason(s, 'union'),
    run: async (s) => void (await runGeometry(s, { op: 'union' })),
  },
  {
    id: 'arrange.difference',
    label: 'arrange.cmd.difference',
    category: 'arrange',
    shortcuts: ['Alt+D'],
    enabled: (s) => geometryReason(s, 'difference'),
    // La forma principal (A) se elige en un diálogo pequeño: nunca depende de un orden invisible.
    run: (s) => {
      const ui = useArrangeUi.getState();
      ui.setPrimary(selectionUnits(s.doc, s.selectedIds)[0]?.id ?? null);
      ui.open({ kind: 'boolean', op: 'difference' });
    },
  },
  {
    id: 'arrange.intersection',
    label: 'arrange.cmd.intersection',
    category: 'arrange',
    enabled: (s) => geometryReason(s, 'intersection'),
    run: (s) => runSymmetric(s, 'intersection'),
  },
  {
    id: 'arrange.xor',
    label: 'arrange.cmd.xor',
    category: 'arrange',
    enabled: (s) => geometryReason(s, 'xor'),
    run: (s) => runSymmetric(s, 'xor'),
  },
  {
    id: 'arrange.weld',
    label: 'arrange.cmd.weld',
    category: 'arrange',
    shortcuts: ['Alt+W'],
    enabled: (s) => geometryReason(s, 'weld'),
    run: async (s) => void (await runGeometry(s, { op: 'weld' })),
  },
  {
    id: 'arrange.offset',
    label: 'arrange.cmd.offset',
    category: 'arrange',
    shortcuts: ['Alt+O'],
    enabled: (s) => geometryReason(s, 'offset'),
    run: () => useArrangeUi.getState().open({ kind: 'offset' }),
  },
  ...cleanupCommands,
  // Alinear, distribuir y centrar
  ...alignCommands,
  ...distributeCommands,
  {
    id: 'arrange.centerOnBed',
    label: 'arrange.cmd.centerOnBed',
    category: 'arrange',
    shortcuts: ['Alt+C'],
    enabled: (s) => movableReason(s, 1, 'arrange.reason.noSelection'),
    run: (s) => {
      const units = selectionUnits(s.doc, s.selectedIds);
      const block = blockBounds(units);
      if (!block) return;
      const moves = centerOnBedMoves(units, block, bedBounds(selectActiveDevice(s).bed));
      s.applyDocChange('arrange.history.centerOnBed', (doc) => moveDoc(doc, moves));
    },
  },
  {
    id: 'arrange.alignDialog',
    label: 'arrange.cmd.alignDialog',
    category: 'arrange',
    enabled: (s) => movableReason(s, 1, 'arrange.reason.noSelection'),
    run: () => useArrangeUi.getState().open({ kind: 'align' }),
  },
  ...referenceCommands,
  // Arreglos
  {
    id: 'arrange.grid',
    label: 'arrange.cmd.grid',
    category: 'arrange',
    enabled: (s) => movableReason(s, 1, 'arrange.reason.noSelection'),
    run: () => useArrangeUi.getState().open({ kind: 'grid' }),
  },
  {
    id: 'arrange.circular',
    label: 'arrange.cmd.circular',
    category: 'arrange',
    enabled: (s) => movableReason(s, 1, 'arrange.reason.noSelection'),
    run: (s) => {
      // El centro de giro propuesto queda a `DEFAULT_RADIUS` mm debajo del bloque (la pieza arriba, como las 12 de un reloj).
      const block = blockBounds(selectionUnits(s.doc, s.selectedIds));
      const ui = useArrangeUi.getState();
      if (block) {
        const [bx, by] = boundsCenter(block);
        const [centerX, centerY] = centerForRadius(block, { centerX: bx, centerY: by }, DEFAULT_RADIUS);
        ui.patchCircular({ centerX, centerY });
      }
      ui.open({ kind: 'circular' });
    },
  },
];

/** Intersección y xor son simétricas con dos formas; con más, hay que decir cuál es A. */
async function runSymmetric(s: AppState, op: 'intersection' | 'xor'): Promise<void> {
  const units = selectionUnits(s.doc, s.selectedIds);
  if (units.length === 2) {
    await runGeometry(s, { op });
    return;
  }
  const ui = useArrangeUi.getState();
  ui.setPrimary(units[0]?.id ?? null);
  ui.open({ kind: 'boolean', op });
}
