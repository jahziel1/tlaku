/**
 * Calibración del desfase cámara-láser de la cámara del cabezal: se graba una marca en una
 * posición conocida, se ve con la cámara desde varias posiciones del cabezal (que se mueve unos
 * milímetros con los comandos de jog) y se calcula el desfase y la escala (ver `head.ts`).
 */
import type { DesignObject, Document, HeadCalibration } from '@/api/types';
import { ensureSaved } from '@/features/history/files';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { makeLayer } from '@/lib/layers';
import { selectActiveDevice, useStore } from '@/store';
import { selectHeadDoc } from '@/store/machine';
import { applyHeadCalibration } from './actions';
import type { Vec2 } from './geometry';
import { fitHeadCalibration, type HeadFit, type HeadFitError } from './head';
import { HEAD_CAL_MIN_OBS, HEAD_CAL_STEP_MM, useHeadCal } from './headState';
import { machineDeps, SETTLE_MS } from './headScan';
import { correctedSpace } from './session';

const app = () => useStore.getState();

/** Brazo (mm) de la cruz de la marca. */
export const HEAD_MARK_ARM_MM = 8;
const MARK_COLOR = '#0066ff';

/** Desplazamientos del cabezal entre una observación y la siguiente (múltiplos del paso). */
const PATTERN: Vec2[] = [[0, 0], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0]];

/** Marca por omisión: el centro de la cama (en coordenadas absolutas). */
export function defaultMark(): Vec2 {
  const bed = selectActiveDevice(app()).bed;
  return [Math.round(bed[0] / 2), Math.round(bed[1] / 2)];
}

/** Documento con una cruz en `mark` (capa de línea de baja potencia, que no corta). */
export function headMarkDocument(mark: Vec2): Document {
  const [x, y] = mark;
  const a = HEAD_MARK_ARM_MM;
  const obj: DesignObject = {
    id: 'head-mark',
    layer: MARK_COLOR,
    transform: [1, 0, 0, 1, 0, 0],
    kind: {
      type: 'vector',
      paths: [
        [{ type: 'move_to', to: [x - a, y] }, { type: 'line_to', to: [x + a, y] }],
        [{ type: 'move_to', to: [x, y - a] }, { type: 'line_to', to: [x, y + a] }],
      ],
    },
    locked: false,
    hidden: false,
    group: null,
  };
  return { version: 2, objects: [obj], layers: [makeLayer(MARK_COLOR, t('camera.head.cal.layer'), { mode: 'line', speed: 3000, power: 10 })], device_id: null };
}

/** Abre la marca como documento nuevo (pregunta antes si hay cambios sin guardar). */
export async function openHeadMarkAsDocument(): Promise<boolean> {
  const mark = useHeadCal.getState().mark ?? defaultMark();
  useHeadCal.getState().setMark(mark);
  if (!(await ensureSaved('new'))) return false;
  const s = app();
  s.replaceDocument({ ...headMarkDocument(mark), device_id: s.doc.device_id }, null);
  s.fitContent();
  void s.computePreview();
  s.notify('success', t('camera.head.cal.opened', { x: mark[0], y: mark[1] }));
  return true;
}

/** Empieza una medición nueva: borra las observaciones y atiende los clics sobre la imagen. */
export function startMeasuring(): void {
  const cal = useHeadCal.getState();
  if (!cal.mark) cal.setMark(defaultMark());
  cal.reset();
  cal.setMeasuring(true);
}

export type ObservationProblem = 'noPosition' | 'noSpace';

/**
 * Registra el clic sobre la cruz: la posición del láser en este instante y el píxel marcado
 * (en el espacio de la imagen corregida).
 */
export function addObservation(px: Vec2): ObservationProblem | null {
  const laser = selectHeadDoc(app());
  if (!laser) return 'noPosition';
  if (!correctedSpace()) return 'noSpace';
  useHeadCal.getState().addObs({ laser, px });
  return null;
}

/** Lleva el cabezal a la posición de la siguiente observación. Devuelve si pudo moverlo. */
export async function moveToNextObservation(): Promise<boolean> {
  const { obs } = useHeadCal.getState();
  const first = obs[0];
  if (!first) return false;
  const [dx, dy] = PATTERN[obs.length % PATTERN.length]!;
  const target: Vec2 = [first.laser[0] + dx * HEAD_CAL_STEP_MM, first.laser[1] + dy * HEAD_CAL_STEP_MM];
  try {
    await machineDeps().moveTo(target);
    await new Promise((r) => setTimeout(r, SETTLE_MS));
    return true;
  } catch (e) {
    app().notify('error', errorMessage(e));
    return false;
  }
}

/** Resultado del ajuste con las observaciones actuales. */
export function currentFit(): HeadFit | { error: HeadFitError | 'noMark' | 'noSpace' } {
  const { obs, mark } = useHeadCal.getState();
  const space = correctedSpace();
  if (!mark) return { error: 'noMark' };
  if (!space) return { error: 'noSpace' };
  if (obs.length < HEAD_CAL_MIN_OBS) return { error: 'tooFew' };
  return fitHeadCalibration(obs, mark, space);
}

/** Guarda la calibración, enciende el modo de cámara en el cabezal y cierra la medición. */
export function saveHeadCalibration(cal: HeadCalibration): void {
  applyHeadCalibration(cal);
  useHeadCal.getState().reset();
  app().notify('success', t('camera.head.cal.saved', { x: cal.offset_mm[0].toFixed(2), y: cal.offset_mm[1].toFixed(2) }));
}
