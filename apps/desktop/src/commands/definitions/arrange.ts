import { moveDoc, alignMoves, bedBounds, centerOnBedMoves, distributeMoves, type AlignKind, type DistributeAxis } from '@/features/arrange/align';
import { centerForRadius, DEFAULT_RADIUS } from '@/features/arrange/arrays';
import { cleanupReason } from '@/features/arrange/cleanup';
import { analyzeSelection, geometryReason, runGeometry } from '@/features/arrange/geomOps';
import { useArrangeUi, type AlignReference, type CleanupOp } from '@/features/arrange/uiStore';
import { blockBounds, boundsCenter, selectionUnits } from '@/features/arrange/units';
import { equalizeDoc, type EqualizeAxis } from '@/features/arrange/equalize';
import { reorderObjects, type OrderOp } from '@/features/arrange/order';
import { flipMatrix, mirrorCopiesDoc, rotateSelectionMatrix, straightLineEnds } from '@/features/arrange/transformOps';
import { stepPriority } from '@/features/canvas/cutProps';
import { startPickMode } from '@/features/canvas/pickTool';
import { canRotate } from '@/features/canvas/transform';
import type { I18nKey } from '@/i18n';
import { reflectAcrossLine } from '@/lib/geometry';
import { selectActiveDevice, selectSelectedObjects } from '@/store';
import type { AppState } from '@/store';
import type { CommandDef } from '../types';

/** Motivo por el que no se puede mover nada, o `true` si hay al menos `min` unidades móviles. */
function movableReason(s: AppState, min: number, few: I18nKey): true | I18nKey {
  if (s.selectedIds.length === 0) return 'arrange.reason.noSelection';
  const { movable, units } = analyzeSelection(s);
  if (movable === 0) return units > 0 ? 'arrange.reason.locked' : 'arrange.reason.noSelection';
  return movable >= min ? true : few;
}

/** Algo seleccionado que se pueda transformar (no bloqueado ni oculto). */
function movableSelection(s: AppState): true | I18nKey {
  const picked = selectSelectedObjects(s);
  if (picked.length === 0) return 'arrange.reason.noSelection';
  return picked.some((o) => !o.locked && !o.hidden) ? true : 'arrange.reason.locked';
}

function rotatableSelection(s: AppState): true | I18nKey {
  const base = movableSelection(s);
  if (base !== true) return base;
  return canRotate(selectSelectedObjects(s).filter((o) => !o.locked)) ? true : 'canvas.bar.rotateImage';
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

const baseArrangeCommands: CommandDef[] = [
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

// ---- voltear, espejo, rotar, orden, bloqueo, igualar y interruptores -------------------------------------

/** Voltea la selección sobre el centro de su caja. */
const flipCommands: CommandDef[] = (['h', 'v'] as const).map((axis) => ({
  id: `arrange.flip.${axis}`,
  label: `cmd.arrange.flip.${axis}` as I18nKey,
  category: 'arrange',
  // Mayús+Mod+V ya es "pegar en sitio": el vertical usa Alt.
  shortcuts: axis === 'h' ? ['Alt+H', 'Mod+Shift+H'] : ['Alt+V'],
  enabled: movableSelection,
  run: (s) => {
    const m = flipMatrix(selectSelectedObjects(s), axis);
    if (m) s.transformObjects(s.selectedIds, m, undefined, `arrange.history.flip.${axis}` as I18nKey);
  },
}));

/** Giros rápidos: 90 grados, el paso configurable (15 por omisión) y 5 grados; `[` antihorario, `]` horario. */
const ROTATIONS = [
  { id: 'ccw90', key: '[', deg: () => 90 },
  { id: 'cw90', key: ']', deg: () => -90 },
  { id: 'ccwStep', key: 'Shift+[', deg: (s: AppState) => s.prefs.rotateStep },
  { id: 'cwStep', key: 'Shift+]', deg: (s: AppState) => -s.prefs.rotateStep },
  { id: 'ccw5', key: 'Alt+[', deg: () => 5 },
  { id: 'cw5', key: 'Alt+]', deg: () => -5 },
] as const;

const rotateCommands: CommandDef[] = ROTATIONS.map((r) => ({
  id: `arrange.rotate.${r.id}`,
  label: `cmd.arrange.rotate.${r.id}` as I18nKey,
  category: 'arrange',
  shortcuts: [r.key],
  repeatable: true,
  enabled: rotatableSelection,
  run: (s) => {
    const m = rotateSelectionMatrix(selectSelectedObjects(s), r.deg(s));
    if (m) s.transformObjects(s.selectedIds, m, undefined, 'arrange.history.rotate');
  },
}));

const ORDER_KEYS: Record<OrderOp, string[]> = {
  front: ['Mod+PageUp', 'Mod+Shift+]'],
  back: ['Mod+PageDown', 'Mod+Shift+['],
  forward: ['PageUp', 'Mod+]'],
  backward: ['PageDown', 'Mod+['],
};

/** Orden de dibujo: lo último de la lista se dibuja encima. */
const orderCommands: CommandDef[] = (Object.keys(ORDER_KEYS) as OrderOp[]).map((op) => ({
  id: `arrange.order.${op}`,
  label: `cmd.arrange.order.${op}` as I18nKey,
  category: 'arrange',
  shortcuts: ORDER_KEYS[op],
  enabled: (s) => {
    const picked = new Set(s.selectedIds);
    if (picked.size === 0) return 'arrange.reason.noSelection';
    return reorderObjects(s.doc.objects, picked, op) !== s.doc.objects ? true : 'arrange.reason.orderNoChange';
  },
  run: (s) => {
    const picked = new Set(s.selectedIds);
    s.applyDocChange(`arrange.history.order.${op}` as I18nKey, (doc) => {
      const objects = reorderObjects(doc.objects, picked, op);
      return objects === doc.objects ? doc : { ...doc, objects: [...objects] };
    });
  },
}));

/**
 * Prioridad de corte de los objetos: -1 corta antes, +1 corta después (solo dentro de su capa y si
 * la optimización respeta la prioridad). Sin prioridad cuenta como 0.
 */
const priorityCommands: CommandDef[] = (
  [
    { id: 'arrange.priority.earlier', label: 'canvas.cmd.priorityEarlier', keys: ['Alt+Shift+['], delta: -1 },
    { id: 'arrange.priority.later', label: 'canvas.cmd.priorityLater', keys: ['Alt+Shift+]'], delta: 1 },
  ] as const
).map<CommandDef>(({ id, label, keys, delta }) => ({
  id,
  label,
  category: 'arrange',
  shortcuts: [...keys],
  enabled: (s) => (s.selectedIds.length === 0 ? 'arrange.reason.noSelection' : true),
  run: (s) => stepPriority(s, s.selectedIds, delta),
}));

/** Igualar ancho o alto con el de la primera unidad seleccionada (con `Keep` conserva la proporción). */
const equalizeCommands: CommandDef[] = (['width', 'height'] as EqualizeAxis[]).flatMap((axis) =>
  [false, true].map<CommandDef>((keep) => ({
    id: `arrange.same.${axis}${keep ? 'Keep' : ''}`,
    label: `cmd.arrange.same.${axis}${keep ? 'Keep' : ''}` as I18nKey,
    category: 'arrange',
    enabled: (s) => movableReason(s, 2, 'arrange.reason.alignNeedTwo'),
    run: (s) => {
      const units = selectionUnits(s.doc, s.selectedIds);
      s.applyDocChange(`arrange.history.same.${axis}` as I18nKey, (doc) => equalizeDoc(doc, units, axis, keep));
    },
  })),
);

const transformCommands: CommandDef[] = [
  ...flipCommands,
  {
    // Con los objetos y, de último, una recta de dos puntos: copia reflejada sobre ella. Si no, se
    // dibuja la línea del espejo en el lienzo.
    id: 'arrange.mirrorLine',
    label: 'cmd.arrange.mirrorLine',
    category: 'arrange',
    shortcuts: ['Alt+Shift+M'],
    enabled: movableSelection,
    run: (s) => {
      const picked = selectSelectedObjects(s);
      const lineObj = picked.length >= 2 ? picked.find((o) => o.id === s.selectedIds[s.selectedIds.length - 1]) : undefined;
      const ends = lineObj ? straightLineEnds(lineObj) : null;
      const m = ends ? reflectAcrossLine(ends[0], ends[1]) : null;
      if (lineObj && m) {
        let created: string[] = [];
        const rest = s.selectedIds.filter((id) => id !== lineObj.id);
        s.applyDocChange('arrange.history.mirror', (doc) => {
          const r = mirrorCopiesDoc(doc, rest, m);
          created = r.created;
          return r.doc;
        });
        if (created.length) s.select(created);
        return;
      }
      s.setTool('select');
      startPickMode('mirror');
      s.notifyKey('info', 'canvas.pick.mirror.help');
    },
  },
  {
    id: 'arrange.twoPoint',
    label: 'cmd.arrange.twoPoint',
    category: 'arrange',
    shortcuts: ['Alt+T'],
    enabled: rotatableSelection,
    run: (s) => {
      s.setTool('select');
      startPickMode('twoPoint');
      s.notifyKey('info', 'canvas.pick.twoPoint.help');
    },
  },
  ...rotateCommands,
  ...orderCommands,
  ...priorityCommands,
  {
    id: 'arrange.lock',
    label: 'cmd.arrange.lock',
    category: 'arrange',
    shortcuts: ['Mod+L'],
    enabled: (s) => (s.selectedIds.length > 0 ? true : 'arrange.reason.noSelection'),
    run: (s) => {
      const ids = new Set(s.selectedIds);
      s.applyDocChange('arrange.history.lock', (doc) => ({ ...doc, objects: doc.objects.map((o) => (ids.has(o.id) && !o.locked ? { ...o, locked: true } : o)) }));
    },
  },
  {
    // Lo bloqueado no se puede seleccionar con el ratón: se desbloquea todo de una vez.
    id: 'arrange.unlockAll',
    label: 'cmd.arrange.unlockAll',
    category: 'arrange',
    shortcuts: ['Mod+Shift+L'],
    enabled: (s) => (s.doc.objects.some((o) => o.locked) ? true : 'arrange.reason.noLocked'),
    run: (s) => {
      const freed = s.doc.objects.filter((o) => o.locked && !o.hidden).map((o) => o.id);
      s.applyDocChange('arrange.history.unlock', (doc) => ({ ...doc, objects: doc.objects.map((o) => (o.locked ? { ...o, locked: false } : o)) }));
      // Los desbloqueados quedan seleccionados para que se vea cuáles eran.
      s.select(freed);
    },
  },
  ...equalizeCommands,
  // Interruptores de transformación: evitan mover, escalar o girar por accidente.
  ...(['move', 'scale', 'rotate'] as const).map<CommandDef>((kind) => ({
    id: `arrange.xform.${kind}`,
    label: `cmd.arrange.xform.${kind}` as I18nKey,
    category: 'arrange',
    checked: (s) => s.transformAllow[kind],
    run: (s) => s.setTransformAllow({ [kind]: !s.transformAllow[kind] }),
  })),
  {
    id: 'arrange.xform.center',
    label: 'cmd.arrange.xform.center',
    category: 'arrange',
    checked: (s) => s.scaleFromCenter,
    run: (s) => s.setScaleFromCenter(!s.scaleFromCenter),
  },
  {
    id: 'arrange.xform.ratio',
    label: 'cmd.arrange.xform.ratio',
    category: 'arrange',
    checked: (s) => s.lockRatio,
    run: (s) => s.setLockRatio(!s.lockRatio),
  },
];

export const arrangeCommands: CommandDef[] = [...baseArrangeCommands, ...transformCommands];
