import { runTrimAt, shapeReason, trimReason } from '@/features/arrange/shapeOps';
import { useShapeOps, type ShapeDialogOp } from '@/features/arrange/shapeOpsUi';
import type { ShapeOp } from '@/api/types';
import type { CommandDef } from '../types';

/** Abre el diálogo de una operación de forma (parámetros, vista previa y aplicar). */
function dialogCommand(id: string, label: CommandDef['label'], op: ShapeOp & ShapeDialogOp, shortcuts?: string[]): CommandDef {
  return {
    id,
    label,
    category: 'arrange',
    ...(shortcuts ? { shortcuts } : {}),
    enabled: (s) => shapeReason(s, op),
    run: (s) => {
      if (shapeReason(s, op) === true) useShapeOps.getState().open(op);
    },
  };
}

/**
 * Operaciones de forma de la ola 3: recortar, separar, optimizar, redondear esquinas, cortar formas
 * y redimensionar ranuras. Recortar es una herramienta del lienzo (`features/canvas/trimTool.ts`, que
 * llama a `runTrimAt` con el punto de cada clic); este comando, para quien lo lanza con el
 * atajo o el menú, recorta el tramo del último punto donde se hizo clic (`useShapeOps.setLastPoint`).
 */
export const shapeCommands: CommandDef[] = [
  {
    id: 'arrange.trim',
    label: 'shapes.cmd.trim',
    category: 'arrange',
    shortcuts: ['Mod+K'],
    enabled: trimReason,
    run: async (s) => {
      const point = useShapeOps.getState().lastPoint;
      if (point && trimReason(s) === true) await runTrimAt(s, point);
    },
  },
  dialogCommand('arrange.breakApart', 'shapes.cmd.breakApart', 'break_apart', ['Alt+B']),
  dialogCommand('arrange.optimize', 'shapes.cmd.optimize', 'optimize', ['Alt+Shift+O']),
  dialogCommand('arrange.fillet', 'shapes.cmd.fillet', 'fillet'),
  dialogCommand('arrange.cutShapes', 'shapes.cmd.cutShapes', 'cut_shapes', ['Alt+Shift+C']),
  dialogCommand('arrange.resizeSlots', 'shapes.cmd.resizeSlots', 'resize_slots'),
];
