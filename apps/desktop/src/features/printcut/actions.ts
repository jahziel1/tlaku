import type { Point } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { workToDoc } from '@/lib/device';
import { newLayerFor } from '@/lib/layers';
import { GUIDE_COLOR, isGuideColor } from '@/lib/palette';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore, type AppState } from '@/store';
import { jogBlockedReason } from '@/store/machine';
import { computeRegistration, nextMissing } from './flow';
import {
  designBox, MARK_COUNTS, marksFromSelection, MIN_MARK_DISTANCE_MM, registrationMarkObjects, type MarkCount, type MarkKind,
} from './marks';
import { usePrintCut } from './printcutStore';

const round3 = (n: number) => Math.round(n * 1000) / 1000;

/** Posición del láser en mm de la cama (coordenadas del documento), o la clave del motivo si no se puede leer. */
export function laserBedPosition(s: Pick<AppState, 'connection' | 'jobState' | 'status' | 'devices' | 'activeDeviceId'>): { point: Point } | { reason: I18nKey } {
  const why = jogBlockedReason(s);
  if (why) return { reason: why };
  const device = selectActiveDevice(s);
  if (device.rotary.enabled) return { reason: 'machine.position.reason.rotary' };
  const pos = s.status?.work_pos;
  if (!pos) return { reason: 'printcut.capture.noPosition' };
  const [x, y] = workToDoc(device, pos);
  return { point: [round3(x), round3(y)] };
}

/** Anota la posición medida de la marca `index` y pasa a la siguiente sin medir. */
function storePhysical(index: number, point: Point): void {
  const ui = usePrintCut.getState();
  ui.setPhysical(index, point);
  const physical = usePrintCut.getState().physical;
  usePrintCut.setState({ current: nextMissing(physical, index), picking: false });
}

/** Paso 2, con el láser: guarda dónde está el cabezal como posición de la marca en curso. */
export function saveLaserPosition(): boolean {
  const s = useStore.getState();
  const r = laserBedPosition(s);
  if ('reason' in r) {
    s.notify('warning', t(r.reason));
    return false;
  }
  const { current } = usePrintCut.getState();
  storePhysical(current, r.point);
  return true;
}

/** Paso 2, con la cámara: el clic sobre la imagen alineada da la posición de la marca en curso. */
export function savePickedPoint(point: Point): void {
  storePhysical(usePrintCut.getState().current, [round3(point[0]), round3(point[1])]);
}

/** Lleva el cabezal a donde estaría la marca si la impresión estuviera exactamente en su sitio. */
export async function goToCurrentMark(): Promise<boolean> {
  const { marks, current } = usePrintCut.getState();
  const mark = marks[current];
  if (!mark) return false;
  return useStore.getState().goToDocPoint(mark.point[0], mark.point[1]);
}

/** Paso 1: toma las marcas de los objetos seleccionados. Avisa si no sirven. */
export function takeSelectionAsMarks(): boolean {
  const s = useStore.getState();
  const r = marksFromSelection(s.doc, s.selectedIds);
  if ('error' in r) {
    s.notify('warning', t(r.error === 'count' ? 'printcut.marks.badCount' : 'printcut.marks.tooClose', { min: MIN_MARK_DISTANCE_MM }));
    return false;
  }
  usePrintCut.getState().setMarks(r.marks);
  return true;
}

/** Paso 1: inserta cruces o círculos en las esquinas del diseño (capa de guías) y los toma como marcas. */
export function addRegistrationMarks(kind: MarkKind, count: MarkCount): boolean {
  const s = useStore.getState();
  const box = designBox(s.doc);
  if (!box) {
    s.notify('warning', t('printcut.marks.noDesign'));
    return false;
  }
  const { objects, marks } = registrationMarkObjects(s.doc, box, kind, count);
  // Un solo paso de historial; la capa de guías se crea sin salida si no existía.
  s.applyDocChange('printcut.history.addMarks', (doc) => ({
    ...doc,
    layers: doc.layers.some((l) => isGuideColor(l.color)) ? doc.layers : [...doc.layers, newLayerFor(GUIDE_COLOR)],
    objects: [...doc.objects, ...objects],
  }));
  s.select(objects.map((o) => o.id));
  usePrintCut.getState().setMarks(marks);
  const bed = selectActiveDevice(useStore.getState()).bed;
  if (marks.some((m) => m.point[0] < 0 || m.point[1] < 0 || m.point[0] > bed[0] || m.point[1] > bed[1])) {
    s.notify('warning', t('printcut.marks.outsideBed'));
  }
  return true;
}

/** Cantidad de marcas válida (2 a 4). */
export function sanitizeCount(n: number): MarkCount {
  return (MARK_COUNTS as readonly number[]).includes(n) ? (n as MarkCount) : 2;
}

/** Paso 3: guarda la transformación en las opciones del trabajo; la vista previa se recalcula sola. */
export async function acceptRegistration(): Promise<boolean> {
  const ui = usePrintCut.getState();
  const result = await computeRegistration(ui.marks, ui.physical, ui.mode);
  const s = useStore.getState();
  if (!result?.ok) return false;
  const { fit } = result;
  // Print & Cut trabaja con coordenadas absolutas de la cama: no tiene sentido con otro origen del trabajo.
  const switched = s.jobOptions.start_from !== 'absolute';
  s.setJobOptions({ transform: fit.transform, ...(switched ? { start_from: 'absolute' as const } : {}) });
  usePrintCut.setState({
    applied: {
      transform: fit.transform,
      marks: ui.marks.length,
      rmsMm: fit.residualRms,
      maxMm: fit.residualMax,
      rotationDeg: fit.rotationDeg,
      scale: fit.scale,
      warnings: fit.warnings,
    },
  });
  s.notify('success', t('printcut.accepted', { rms: fmt(fit.residualRms, 2), deg: fmt(fit.rotationDeg, 2) }));
  if (switched) s.notifyKey('info', 'printcut.switchedAbsolute');
  return true;
}

/** Quita la transformación del trabajo (vuelve a cortar donde dice el diseño). */
export function resetRegistration(): void {
  const s = useStore.getState();
  s.setJobOptions({ transform: null });
  usePrintCut.setState({ applied: null });
  usePrintCut.getState().restart();
}
