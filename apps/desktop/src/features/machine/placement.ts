import type { Document, JobOptions, Point, StartFrom } from '@/api/types';
import { unionBounds, type Bounds } from '@/lib/geometry';
import { objectBoundsWithCopies } from '@/lib/virtualArray';
import { workToDoc, type DeviceFrame } from '@/lib/device';

/**
 * Dónde quedará el trabajo respecto a la cama y al cabezal. Es la respuesta a la queja del foro
 * sobre el modelo mental de "Iniciar desde" x "Origen del trabajo": un único cálculo, mostrado
 * siempre antes de ejecutar. Todo en coordenadas de documento (mm, origen abajo a la izquierda,
 * Y hacia arriba); las posiciones de la máquina se convierten con el origen del perfil.
 */

export type PlacementStatus = 'ok' | 'noDesign' | 'needHead' | 'needUserOrigin';

export interface Placement {
  status: PlacementStatus;
  /** Rectángulo del trabajo ya colocado; null si no hay nada que enviar o falta el punto de anclaje. */
  rect: Bounds | null;
  /** Punto del rectángulo que coincide con el cabezal / origen de usuario (null en absolutas). */
  anchor: Point | null;
  /** Punto de la cama donde se ancla (cabezal u origen de usuario), en coordenadas de documento. */
  target: Point | null;
  /** Milímetros que el trabajo sobresale de la cama por cada lado (0 = dentro). */
  overflow: { left: number; right: number; top: number; bottom: number };
  /** El rectángulo cabe entero en la cama. */
  fits: boolean;
}

export interface PlacementInput {
  startFrom: StartFrom;
  /** 0..8, fila por fila desde arriba a la izquierda. */
  jobOrigin: number;
  /** Origen de usuario, en posición de trabajo de la máquina. */
  userOrigin: Point | null;
  /** Posición actual del cabezal, en posición de trabajo de la máquina; null si no se conoce. */
  head: Point | null;
  device: DeviceFrame;
  /** Rectángulo envolvente de lo que se envía (sin overscan); null si no hay nada. */
  design: Bounds | null;
  /**
   * Caja de la que sale el punto de anclaje del trabajo. Por omisión es `design`; al enviar solo
   * la selección sin "usar el origen de la selección" es la caja de todo el diseño, así lo
   * seleccionado queda donde estaría dentro del diseño completo.
   */
  originBox?: Bounds | null;
}

/** Posición del punto 0..8 dentro del rectángulo: `fx` de izquierda a derecha y `fy` de arriba abajo, 0..1. */
export function anchorFraction(index: number): [number, number] {
  const i = Number.isInteger(index) ? Math.min(8, Math.max(0, index)) : 4;
  return [(i % 3) / 2, Math.floor(i / 3) / 2];
}

/** Punto del rectángulo (coordenadas de documento, Y hacia arriba) que corresponde al índice 0..8. */
export function anchorPoint(b: Bounds, index: number): Point {
  const [fx, fy] = anchorFraction(index);
  const [[x0, y0], [x1, y1]] = b;
  return [x0 + fx * (x1 - x0), y1 - fy * (y1 - y0)];
}

const NO_OVERFLOW = { left: 0, right: 0, top: 0, bottom: 0 };
const EPS = 1e-6;

function overflowOf(rect: Bounds, bed: Point): Placement['overflow'] {
  const [[x0, y0], [x1, y1]] = rect;
  return {
    left: Math.max(0, -x0),
    bottom: Math.max(0, -y0),
    right: Math.max(0, x1 - bed[0]),
    top: Math.max(0, y1 - bed[1]),
  };
}

export function computeJobPlacement(input: PlacementInput): Placement {
  const { startFrom, jobOrigin, userOrigin, head, device, design } = input;
  const originBox = input.originBox ?? design;
  const empty = (status: PlacementStatus): Placement => ({
    status,
    rect: null,
    anchor: null,
    target: null,
    overflow: NO_OVERFLOW,
    fits: true,
  });
  if (!design) return empty('noDesign');

  const finish = (rect: Bounds, anchor: Point | null, target: Point | null): Placement => {
    const overflow = overflowOf(rect, device.bed);
    const fits = Object.values(overflow).every((v) => v <= EPS);
    return { status: 'ok', rect, anchor, target, overflow, fits };
  };

  if (startFrom === 'absolute') return finish(design, null, null);

  const workPoint = startFrom === 'current_position' ? head : userOrigin;
  if (!workPoint) return empty(startFrom === 'current_position' ? 'needHead' : 'needUserOrigin');

  const target = workToDoc(device, workPoint);
  const anchor = anchorPoint(originBox ?? design, jobOrigin);
  const dx = target[0] - anchor[0];
  const dy = target[1] - anchor[1];
  const rect: Bounds = [
    [design[0][0] + dx, design[0][1] + dy],
    [design[1][0] + dx, design[1][1] + dy],
  ];
  return finish(rect, target, target);
}

/** Rectángulo envolvente de lo que sale a la máquina (sin overscan, con las copias virtuales), respetando la selección. */
export function designBounds(doc: Pick<Document, 'objects' | 'layers'>, selection: JobOptions['selection']): Bounds | null {
  const outputs = new Set(doc.layers.filter((l) => l.output).map((l) => l.color.toLowerCase()));
  const only = selection ? new Set(selection) : null;
  return unionBounds(
    doc.objects
      .filter((o) => !o.hidden && outputs.has(o.layer.toLowerCase()) && (!only || only.has(o.id)))
      .map(objectBoundsWithCopies),
  );
}
