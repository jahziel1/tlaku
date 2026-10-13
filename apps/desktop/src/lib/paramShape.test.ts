import { beforeEach, describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import { replacePaths } from '@/features/canvas/nodeActions';
import { newVectorObject } from '@/features/canvas/shapes';
import { scaleAbout } from '@/features/canvas/transform';
import { useStore } from '@/store';
import { objectBounds } from './geometry';
import { clampShape, editShape, effectiveShape, newShapeObject, paramsToSpec, shapePaths, specToParams, type ShapeSpec } from './paramShape';

const empty = (): Document => ({ version: 2, objects: [], layers: [], device_id: null });
const size = (o: DesignObject) => {
  const b = objectBounds(o)!;
  return [b[1][0] - b[0][0], b[1][1] - b[0][1]];
};
const st = () => useStore.getState();
const obj = (id: string) => st().doc.objects.find((o) => o.id === id)!;
/** Lo que haría guardar y abrir el proyecto. */
const roundTrip = (doc: Document): Document => JSON.parse(JSON.stringify(doc)) as Document;

beforeEach(() => {
  st().replaceDocument(empty(), null);
});

describe('formas paramétricas persistidas en DesignObject.shape', () => {
  it('una forma nueva guarda sus parámetros en el contrato y está centrada en su origen local', () => {
    const o = newShapeObject('p', '#ff0000', { type: 'polygon', sides: 6, radius: 10, rounding: 0 }, [50, 40]);
    expect(o.shape).toEqual({ type: 'polygon', sides: 6, radius: 10, corner: 0 });
    expect(o.transform.slice(4)).toEqual([50, 40]);
    const b = objectBounds(o)!;
    expect((b[0][0] + b[1][0]) / 2).toBeCloseTo(50, 6);
  });

  it('los parámetros del contrato y los de edición se convierten sin pérdida', () => {
    const specs: ShapeSpec[] = [
      { type: 'rect', w: 10, h: 4, radius: 1 },
      { type: 'ellipse', w: 8, h: 6 },
      { type: 'polygon', sides: 7, radius: 5, rounding: 0.5 },
      { type: 'star', points: 5, outer: 10, ratio: 0.4, rounding: 0 },
    ];
    for (const s of specs) {
      const back = paramsToSpec(specToParams(s));
      expect(back).toMatchObject(s);
    }
  });

  it('ida y vuelta: crear, serializar, cargar y editar los parámetros', () => {
    st().addObjects([newShapeObject('r', '#ff0000', { type: 'rect', w: 40, h: 20, radius: 2 }, [100, 50])]);
    st().replaceDocument(roundTrip(st().doc), null);
    const loaded = obj('r');
    expect(loaded.shape).toEqual({ type: 'rect', width: 40, height: 20, radius: 2 });
    expect(effectiveShape(loaded)).toEqual({ type: 'rect', w: 40, h: 20, radius: 2 });

    const edited = editShape(loaded, { w: 60, radius: 5 })!;
    st().applyDocChange('canvas.history.shapeProps', (doc) => ({ ...doc, objects: doc.objects.map((o) => (o.id === 'r' ? edited : o)) }));
    const after = obj('r');
    expect(after.shape).toEqual({ type: 'rect', width: 60, height: 20, radius: 5 });
    expect(size(after)[0]).toBeCloseTo(60, 6);
    const b = objectBounds(after)!;
    expect((b[0][0] + b[1][0]) / 2).toBeCloseTo(100, 6); // conserva el centro

    st().undo();
    expect(obj('r').shape).toEqual({ type: 'rect', width: 40, height: 20, radius: 2 });
    st().redo();
    expect(obj('r').shape).toEqual({ type: 'rect', width: 60, height: 20, radius: 5 });
  });

  it('un documento sin shape no cambia al cargarlo ni al moverlo', () => {
    const plain = newVectorObject('x', '#ff0000', shapePaths({ type: 'rect', w: 10, h: 10, radius: 0 }));
    st().replaceDocument({ ...empty(), objects: [plain] }, null);
    expect(obj('x')).toBe(plain);
    expect(effectiveShape(obj('x'))).toBeNull();
    st().moveObjects(['x'], 5, 5);
    expect(obj('x').shape).toBeUndefined();
  });

  it('un documento que trae shape (aunque venga de otra herramienta) se edita', () => {
    const doc: Document = {
      ...empty(),
      objects: [{ ...newVectorObject('e', '#ff0000', shapePaths({ type: 'ellipse', w: 20, h: 10 })), shape: { type: 'ellipse', rx: 10, ry: 5 } }],
    };
    st().replaceDocument(doc, null);
    expect(effectiveShape(obj('e'))).toEqual({ type: 'ellipse', w: 20, h: 10 });
    expect(size(editShape(obj('e'), { h: 30 })!)[1]).toBeCloseTo(30, 6);
  });

  it('un shape que no coincide con los contornos se ignora', () => {
    const lying: DesignObject = { ...newShapeObject('r', '#ff0000', { type: 'rect', w: 10, h: 10, radius: 0 }, [0, 0]), shape: { type: 'rect', width: 99, height: 10, radius: 0 } };
    expect(effectiveShape(lying)).toBeNull();
  });

  it('mover y girar conservan la forma; copiar y deshacer también', () => {
    st().addObjects([newShapeObject('p', '#ff0000', { type: 'star', points: 5, outer: 10, ratio: 0.5, rounding: 0 }, [0, 0])]);
    const kind = obj('p').kind;
    st().moveObjects(['p'], 3, 4);
    expect(obj('p').kind).toBe(kind);
    st().transformObjects(['p'], [0, 1, -1, 0, 0, 0]); // 90 grados
    expect(obj('p').shape).not.toBeNull();
    const [copy] = st().duplicateObjects(['p']);
    expect(obj(copy!).shape).toEqual(obj('p').shape);
    st().undo();
    expect(st().doc.objects).toHaveLength(1);
  });

  it('escala uniforme: se hornea en los parámetros y la matriz queda solo con giro y traslación', () => {
    st().addObjects([newShapeObject('p', '#ff0000', { type: 'polygon', sides: 5, radius: 10, rounding: 1 }, [20, 20], Math.PI / 6)]);
    st().transformObjects(['p'], scaleAbout([20, 20], 2, 2));
    const o = obj('p');
    expect(o.shape).toEqual({ type: 'polygon', sides: 5, radius: 20, corner: 2 });
    expect(Math.hypot(o.transform[0], o.transform[1])).toBeCloseTo(1, 9);
    expect(Math.atan2(o.transform[1], o.transform[0])).toBeCloseTo(Math.PI / 6, 9);
    expect(o.transform[4]).toBeCloseTo(20, 9);
  });

  it('escala no uniforme: el rectángulo y la elipse cambian ancho y alto', () => {
    st().addObjects([
      newShapeObject('r', '#ff0000', { type: 'rect', w: 10, h: 10, radius: 0 }, [0, 0]),
      newShapeObject('e', '#ff0000', { type: 'ellipse', w: 10, h: 10 }, [30, 0], 0.3),
    ]);
    st().transformObjects(['r', 'e'], scaleAbout([0, 0], 3, 2));
    expect(obj('r').shape).toEqual({ type: 'rect', width: 30, height: 20, radius: 0 });
    // La elipse estaba girada: la escala del documento la sesga y ya no es una elipse de ejes propios.
    expect(obj('e').shape).toBeNull();
    st().undo();
    expect(obj('r').shape).toEqual({ type: 'rect', width: 10, height: 10, radius: 0 });
  });

  it('escala no uniforme: polígonos, estrellas y rectángulos redondeados dejan de ser paramétricos', () => {
    st().addObjects([
      newShapeObject('p', '#ff0000', { type: 'polygon', sides: 6, radius: 10, rounding: 0 }, [0, 0]),
      newShapeObject('s', '#ff0000', { type: 'star', points: 5, outer: 10, ratio: 0.5, rounding: 0 }, [30, 0]),
      newShapeObject('r', '#ff0000', { type: 'rect', w: 10, h: 10, radius: 2 }, [60, 0]),
    ]);
    st().transformObjects(['p', 's', 'r'], scaleAbout([0, 0], 2, 1));
    for (const id of ['p', 's', 'r']) expect(obj(id).shape, id).toBeNull();
    expect(size(obj('p'))[0]).toBeGreaterThan(size(newShapeObject('q', '#ff0000', { type: 'polygon', sides: 6, radius: 10, rounding: 0 }, [0, 0]))[0]);
  });

  it('un espejo no cambia la figura ni su orientación', () => {
    st().addObjects([newShapeObject('p', '#ff0000', { type: 'polygon', sides: 3, radius: 10, rounding: 0 }, [0, 0])]);
    const before = objectBounds(obj('p'))!;
    st().transformObjects(['p'], [-1, 0, 0, 1, 0, 0]);
    expect(obj('p').shape).not.toBeNull();
    const edited = editShape(obj('p'), { radius: 10 })!; // regenerar no la da la vuelta
    const after = objectBounds(edited)!;
    expect(after[0][1]).toBeCloseTo(before[0][1], 6);
    expect(after[1][1]).toBeCloseTo(before[1][1], 6);
  });

  it('editar nodos deja de ser paramétrico, también si el cambio no toca shape', () => {
    st().addObjects([newShapeObject('p', '#ff0000', { type: 'polygon', sides: 6, radius: 10, rounding: 0 }, [0, 0])]);
    const kind = obj('p').kind;
    const paths = kind.type === 'vector' ? [...kind.paths] : [];
    st().applyDocChange('canvas.history.shapeProps', (doc) => replacePaths(doc, new Map([['p', paths]])));
    expect(obj('p').shape).toBeNull();
    // Un cambio que reemplaza los contornos y olvida limpiar shape lo limpia el almacén.
    st().addObjects([newShapeObject('q', '#ff0000', { type: 'rect', w: 10, h: 10, radius: 0 }, [0, 0])]);
    st().applyDocChange('history.editObjects', (doc) => ({
      ...doc,
      objects: doc.objects.map((o) => (o.id === 'q' ? { ...o, kind: { type: 'vector' as const, paths: shapePaths({ type: 'ellipse', w: 3, h: 3 }) } } : o)),
    }));
    expect(obj('q').shape).toBeNull();
  });

  it('los grupos jerárquicos conservan su cadena y la forma', () => {
    st().addObjects([
      { ...newShapeObject('a', '#ff0000', { type: 'rect', w: 5, h: 5, radius: 0 }, [0, 0]), group: 'g2/g1' },
      { ...newShapeObject('b', '#ff0000', { type: 'rect', w: 5, h: 5, radius: 0 }, [10, 0]), group: 'g2' },
    ]);
    st().transformObjects(['a', 'b'], scaleAbout([0, 0], 2, 2));
    expect(obj('a').group).toBe('g2/g1');
    expect(obj('b').group).toBe('g2');
    expect(obj('a').shape).toMatchObject({ width: 10 });
  });

  it('los parámetros se limitan a valores válidos', () => {
    expect(clampShape({ type: 'polygon', sides: 1, radius: -5, rounding: -1 })).toEqual({ type: 'polygon', sides: 3, radius: 0.01, rounding: 0 });
    expect(clampShape({ type: 'star', points: 5, outer: 10, ratio: 2, rounding: 0 })).toMatchObject({ ratio: 0.95 });
    expect(clampShape({ type: 'rect', w: 10, h: 4, radius: 99 })).toMatchObject({ radius: 2 });
  });
});
