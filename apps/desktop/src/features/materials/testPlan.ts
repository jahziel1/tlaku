/**
 * Lógica pura de la prueba de materiales: el formulario del diálogo, su paso a `MaterialTestParams`,
 * las validaciones previas (las mismas reglas que el motor) y la geometría de la vista previa.
 */
import type { LayerMode, LayerSettings, MaterialTestParams, TestAxis, TestParam } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { makeLayer, normalizeLayer } from '@/lib/layers';

export const TEST_PARAMS: TestParam[] = ['speed', 'power', 'passes', 'interval'];
export const MAX_COUNT = 30;
/** Esquina inferior izquierda de la cuadrícula en el documento nuevo. */
export const TEST_ORIGIN: [number, number] = [5, 5];
/** Pasadas máximas que acepta el motor. */
export const MAX_PASSES = 1000;

/** Rango inicial al elegir un parámetro para un eje. */
export const DEFAULT_RANGE: Record<TestParam, { from: number; to: number; count: number }> = {
  speed: { from: 500, to: 3000, count: 5 },
  power: { from: 10, to: 100, count: 5 },
  passes: { from: 1, to: 4, count: 4 },
  interval: { from: 0.05, to: 0.25, count: 5 },
};

export interface FixedValues {
  speed: number;
  power: number;
  passes: number;
  interval: number;
}

/** Lo que el usuario edita en el diálogo. */
export interface TestForm {
  mode: Extract<LayerMode, 'line' | 'fill'>;
  rows: TestAxis;
  cols: TestAxis;
  cell: [number, number];
  gap: number;
  labels: boolean;
  labelSpeed: number;
  labelPower: number;
  /** Parte de los ajustes de la capa activa (aire, ángulo, overscan...) en vez de los de fábrica. */
  fromActiveLayer: boolean;
  /** Valores de los parámetros que ningún eje varía. */
  fixed: FixedValues;
}

const axisOf = (param: TestParam): TestAxis => ({ param, ...DEFAULT_RANGE[param] });

export function fixedFrom(layer: LayerSettings | null): FixedValues {
  const l = layer ?? makeLayer('#000000', '');
  return { speed: l.speed, power: l.power, passes: l.passes, interval: l.interval };
}

export function defaultForm(active: LayerSettings | null): TestForm {
  return {
    mode: active?.mode === 'fill' ? 'fill' : 'line',
    rows: axisOf('power'),
    cols: axisOf('speed'),
    cell: [10, 10],
    gap: 3,
    labels: true,
    labelSpeed: 1500,
    labelPower: 15,
    fromActiveLayer: active !== null,
    fixed: fixedFrom(active),
  };
}

/** Ajustes base de todas las celdas: la capa activa (o los de fábrica) con los valores fijos. */
export function baseLayer(form: TestForm, active: LayerSettings | null): LayerSettings {
  const source = form.fromActiveLayer && active ? normalizeLayer(active) : makeLayer('#000000', '');
  return {
    ...source,
    mode: form.mode,
    speed: form.fixed.speed,
    power: form.fixed.power,
    passes: form.fixed.passes,
    interval: form.fixed.interval,
    // El motor reasigna color, nombre y orden de cada celda.
    order: null,
    output: true,
    visible: true,
  };
}

export function buildParams(form: TestForm, active: LayerSettings | null, origin: [number, number] = TEST_ORIGIN): MaterialTestParams {
  return {
    mode: form.mode,
    rows: form.rows,
    cols: form.cols,
    cell: form.cell,
    gap: form.gap,
    base: baseLayer(form, active),
    labels: form.labels,
    label_speed: form.labelSpeed,
    label_power: form.labelPower,
    origin,
  };
}

/** Parámetros que ningún eje varía (los que el usuario fija a mano). */
export function fixedParams(form: Pick<TestForm, 'mode' | 'rows' | 'cols'>): TestParam[] {
  const varied = new Set<TestParam>([form.rows.param, form.cols.param]);
  return TEST_PARAMS.filter((p) => !varied.has(p) && (p !== 'interval' || form.mode === 'fill'));
}

/** Parámetros que se pueden elegir en un modo (el intervalo solo existe en relleno). */
export function paramsFor(mode: TestForm['mode']): TestParam[] {
  return mode === 'fill' ? TEST_PARAMS : TEST_PARAMS.filter((p) => p !== 'interval');
}

/** Cambia el parámetro de un eje; si el otro eje ya lo usaba, el otro toma el que dejas libre. */
export function setAxisParam(form: TestForm, which: 'rows' | 'cols', param: TestParam): TestForm {
  const other = which === 'rows' ? 'cols' : 'rows';
  const previous = form[which].param;
  if (previous === param) return form;
  const next = { ...form, [which]: axisOf(param) };
  if (form[other].param === param) next[other] = axisOf(previous);
  return next;
}

/** Cambia el modo; si un eje usaba el intervalo, que solo existe en relleno, toma otro parámetro libre. */
export function setMode(form: TestForm, mode: TestForm['mode']): TestForm {
  const next = { ...form, mode };
  if (mode === 'fill') return next;
  for (const which of ['rows', 'cols'] as const) {
    if (next[which].param !== 'interval') continue;
    const taken = new Set([next.rows.param, next.cols.param]);
    const free = paramsFor('line').find((p) => !taken.has(p)) ?? 'passes';
    next[which] = axisOf(free);
  }
  return next;
}

/** Valores del eje: interpolación lineal inclusive; una sola celda usa `from`; las pasadas se redondean a entero >= 1. */
export function axisValues(axis: TestAxis): number[] {
  return Array.from({ length: Math.max(0, Math.floor(axis.count)) }, (_, i) => {
    const v = axis.count === 1 ? axis.from : axis.from + ((axis.to - axis.from) * i) / (axis.count - 1);
    return axis.param === 'passes' ? Math.max(1, Math.round(v)) : v;
  });
}

export interface TestProblem {
  key: I18nKey;
  params?: Record<string, string | number>;
}

/** Primer problema del formulario, con las mismas reglas que el motor; `null` si está bien. */
export function validateForm(form: TestForm): TestProblem | null {
  if (form.rows.param === form.cols.param) return { key: 'materials.test.err.sameParam' };
  for (const which of ['rows', 'cols'] as const) {
    const axis = form[which];
    const axisName = t(which === 'rows' ? 'materials.test.axisRows' : 'materials.test.axisCols');
    if (!Number.isInteger(axis.count) || axis.count < 1 || axis.count > MAX_COUNT) {
      return { key: 'materials.test.err.count', params: { axis: axisName, max: MAX_COUNT } };
    }
    if (!Number.isFinite(axis.from) || !Number.isFinite(axis.to)) return { key: 'materials.test.err.range', params: { axis: axisName } };
    if (axis.param === 'interval' && form.mode === 'line') return { key: 'materials.test.err.intervalLine' };
    for (const v of [axis.from, axis.to]) {
      if (axis.param === 'power' && (v < 0 || v > 100)) return { key: 'materials.test.err.power', params: { axis: axisName } };
      if ((axis.param === 'speed' || axis.param === 'interval') && v <= 0) return { key: 'materials.test.err.positive', params: { axis: axisName } };
      if (axis.param === 'passes' && Math.round(v) > MAX_PASSES) return { key: 'materials.test.err.passes', params: { max: MAX_PASSES } };
    }
  }
  if (!(form.cell[0] > 0 && form.cell[1] > 0)) return { key: 'materials.test.err.cell' };
  if (!(form.gap >= 0)) return { key: 'materials.test.err.gap' };
  const f = form.fixed;
  if (!(f.speed > 0)) return { key: 'materials.test.err.fixedSpeed' };
  if (!(f.power >= 0 && f.power <= 100)) return { key: 'materials.test.err.fixedPower' };
  if (!(Number.isInteger(f.passes) && f.passes >= 1 && f.passes <= MAX_PASSES)) return { key: 'materials.test.err.fixedPasses', params: { max: MAX_PASSES } };
  if (form.mode === 'fill' && !(f.interval > 0)) return { key: 'materials.test.err.fixedInterval' };
  if (form.labels) {
    if (!(form.labelSpeed > 0)) return { key: 'materials.test.err.labelSpeed' };
    if (!(form.labelPower >= 0 && form.labelPower <= 100)) return { key: 'materials.test.err.labelPower' };
    const need = labelCellMin(form);
    if (need.width > form.cell[0] + form.gap) return { key: 'materials.test.err.labelWidth', params: { mm: Math.ceil((need.width - form.gap) * 10) / 10 } };
    if (form.cell[1] + form.gap < LABEL_HEIGHT + 0.5) return { key: 'materials.test.err.labelHeight', params: { mm: Math.ceil((LABEL_HEIGHT + 0.5 - form.gap) * 10) / 10 } };
  }
  return null;
}

// ---------------------------------------------------------------------------
// Geometría de la vista previa (aproxima la del motor)
// ---------------------------------------------------------------------------

export const LABEL_HEIGHT = 3;
const LABEL_GAP = 2;

/** Ancho en mm de un rótulo de la fuente de trazo del motor (dígitos 2 unidades, punto 0,6, espacio 1). */
export function labelWidth(text: string): number {
  const unit = LABEL_HEIGHT / 4;
  let units = 0;
  for (const ch of text) {
    if (units > 0) units += 1;
    units += ch === '.' ? 0.6 : ch === ' ' ? 1 : 2;
  }
  return units * unit;
}

const HEADER: Record<TestParam, string> = { speed: 'V MM/MIN', power: 'P %', passes: 'PAS', interval: 'INT MM' };

/** Texto de un valor en un rótulo y en la vista previa. */
export function valueText(param: TestParam, v: number): string {
  const decimals = param === 'power' ? 1 : param === 'interval' ? 3 : 0;
  const s = v.toFixed(decimals);
  return s.includes('.') ? s.replace(/0+$/, '').replace(/\.$/, '') : s;
}

function labelCellMin(form: TestForm): { width: number } {
  const widest = Math.max(0, ...axisValues(form.cols).map((v) => labelWidth(valueText(form.cols.param, v))));
  return { width: widest + 0.5 };
}

export interface GridLayout {
  /** Esquina inferior izquierda de la cuadrícula de celdas, en mm relativos al origen. */
  gridX: number;
  /** Tamaño de todo el conjunto con los rótulos. */
  width: number;
  height: number;
  rowValues: number[];
  colValues: number[];
}

export function gridLayout(form: TestForm): GridLayout {
  const rowValues = axisValues(form.rows);
  const colValues = axisValues(form.cols);
  const rows = Math.max(rowValues.length, 1);
  const cols = Math.max(colValues.length, 1);
  const gridW = cols * form.cell[0] + (cols - 1) * form.gap;
  const gridH = rows * form.cell[1] + (rows - 1) * form.gap;
  let left = 0;
  let top = 0;
  if (form.labels) {
    const rowTexts = rowValues.map((v) => valueText(form.rows.param, v));
    left = Math.max(0, ...rowTexts.map(labelWidth), labelWidth(HEADER[form.rows.param])) + LABEL_GAP;
    top = LABEL_GAP + LABEL_HEIGHT + LABEL_GAP + LABEL_HEIGHT;
  }
  return { gridX: left, width: left + gridW, height: gridH + top, rowValues, colValues };
}

/** Texto corto del parámetro para encabezados de la vista previa. */
export function paramUnit(param: TestParam): string {
  return HEADER[param];
}
