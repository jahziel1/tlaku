import type { CodeResult, DesignObject } from '@/api/types';
import { objectBounds } from '@/lib/geometry';
import type { VariableCodeObject } from './varObjects';

/**
 * El código variable con las formas de `result` (el código de esta fila) en lugar de las que
 * tenía. Se queda centrado donde estaba y conserva giro y escala: un código de barras cambia de
 * ancho con el contenido, y así no se corre hacia un lado.
 */
export function placeCodeResult(o: VariableCodeObject, result: CodeResult): DesignObject {
  const [a, b, c, d, e, f] = o.transform;
  const next: VariableCodeObject = { ...o, kind: { ...o.kind, paths: result.paths } };
  const before = objectBounds(o);
  const local = objectBounds({ ...next, transform: [1, 0, 0, 1, 0, 0] });
  if (!before || !local) return { ...next, transform: [a, b, c, d, e, f] };
  const cx = (before[0][0] + before[1][0]) / 2;
  const cy = (before[0][1] + before[1][1]) / 2;
  const lx = (local[0][0] + local[1][0]) / 2;
  const ly = (local[0][1] + local[1][1]) / 2;
  return { ...next, transform: [a, b, c, d, cx - (a * lx + c * ly), cy - (b * lx + d * ly)] };
}
