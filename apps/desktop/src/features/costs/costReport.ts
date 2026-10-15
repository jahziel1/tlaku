import { t, type I18nKey } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import type { CostInputs, CostLine, CostResult, CostWarning, TimeSource } from './costModel';
import { formatMoney, plainNumber } from './currency';

/** Datos que necesita el desglose para escribirse en texto o CSV. */
export interface ReportContext {
  inputs: CostInputs;
  result: CostResult;
  /** Nombre del proyecto y de la máquina; `null` si no se sabe. */
  project: string | null;
  machine: string;
  /** Fuente de tiempo que se usó. */
  timeUsed: TimeSource;
  /** Largo de corte del trabajo, en mm. */
  cutMm: number;
  /** Decimales de la moneda y etiqueta regional (`es-MX`). */
  digits: number;
  tag: string;
  /** Fecha ISO (`2026-10-15`); opcional para que las pruebas no dependan del reloj. */
  date?: string;
}

export interface ReportRow {
  /** Cuenta como título de sección (sin valor). */
  heading?: boolean;
  label: string;
  /** Texto para leer, con formato regional. */
  display: string;
  /** Valor para hojas de cálculo: número con punto decimal, o texto. */
  raw: string;
  unit: string;
}

const LINE_KEYS: Record<CostLine['id'], I18nKey> = {
  machine: 'costs.line.machine',
  wear: 'costs.line.wear',
  power: 'costs.line.power',
  material: 'costs.line.material',
  finish: 'costs.line.finish',
  prep: 'costs.line.prep',
  consumables: 'costs.line.consumables',
};

export const WARNING_KEYS: Record<CostWarning, I18nKey> = {
  noJob: 'costs.warn.noJob',
  approxTime: 'costs.warn.approxTime',
  uncalibrated: 'costs.warn.uncalibrated',
  bboxArea: 'costs.warn.bboxArea',
  noClosedShapes: 'costs.warn.noClosedShapes',
  noSheetSize: 'costs.warn.noSheetSize',
  noPiecesPerSheet: 'costs.warn.noPiecesPerSheet',
  impossible: 'costs.warn.impossible',
};

const SOURCE_KEYS: Record<TimeSource, I18nKey> = {
  estimate: 'costs.time.estimate',
  calibrated: 'costs.time.calibrated',
  lastReal: 'costs.time.lastReal',
};

const MODE_KEYS = {
  bbox: 'costs.mat.bbox',
  area: 'costs.mat.area',
  sheet: 'costs.mat.sheet',
} as const satisfies Record<CostInputs['materialMode'], I18nKey>;

/** Filas del desglose, en el orden en que se leen. */
export function reportRows(ctx: ReportContext): ReportRow[] {
  const { inputs, result, digits, tag } = ctx;
  const code = inputs.sales.currency;
  const money = (v: number) => formatMoney(v, code, tag);
  const num = (v: number, d: number) => new Intl.NumberFormat(tag, { minimumFractionDigits: d, maximumFractionDigits: d }).format(v);
  const rows: ReportRow[] = [];
  const head = (label: string) => rows.push({ heading: true, label, display: '', raw: '', unit: '' });
  const text = (label: string, value: string) => rows.push({ label, display: value, raw: value, unit: '' });
  const quantity = (label: string, value: number, d: number, unit: string) =>
    rows.push({ label, display: unit ? `${num(value, d)} ${unit}` : num(value, d), raw: plainNumber(value, d), unit });
  const cash = (label: string, value: number | null) =>
    rows.push({ label, display: value === null ? '—' : money(value), raw: value === null ? '' : plainNumber(value, digits), unit: code });
  const pct = (label: string, value: number | null) =>
    rows.push({ label, display: value === null ? '—' : `${num(value, 1)} %`, raw: value === null ? '' : plainNumber(value, 2), unit: '%' });

  const job = inputs.job;
  head(t('costs.report.section.job'));
  if (ctx.project) text(t('costs.report.project'), ctx.project);
  if (ctx.date) text(t('costs.report.date'), ctx.date);
  text(t('costs.report.machine'), ctx.machine);
  text(t('costs.report.timeSource'), t(SOURCE_KEYS[ctx.timeUsed]));
  rows.push({ label: t('costs.report.time'), display: fmtClock(job.runS), raw: plainNumber(job.runS, 0), unit: 's' });
  rows.push({ label: t('costs.report.laserOn'), display: fmtClock(job.laserOnS), raw: plainNumber(job.laserOnS, 0), unit: 's' });
  quantity(t('costs.report.cutLength'), ctx.cutMm / 10, 1, 'cm');
  text(t('costs.report.materialMode'), t(MODE_KEYS[inputs.materialMode]));
  if (inputs.materialMode !== 'sheet') {
    const area = inputs.materialMode === 'bbox' || job.realMm2 <= 0 ? job.bboxMm2 : job.realMm2;
    quantity(t('costs.report.area'), area / 100, 1, 'cm²');
  }
  quantity(t('costs.report.piecesInDesign'), inputs.piecesInDesign, 0, '');
  quantity(t('costs.report.quantity'), inputs.quantity, 0, '');

  head(t('costs.report.section.cost'));
  for (const l of result.lines) if (l.perPiece !== 0) cash(t(LINE_KEYS[l.id]), l.perPiece);
  cash(t('costs.report.costTotal'), result.cost);

  head(t('costs.report.section.price'));
  const base = inputs.sales.marginBase === 'price' ? t('costs.margin.onPrice') : t('costs.margin.onCost');
  pct(`${t('costs.report.margin')} (${base})`, inputs.sales.margin);
  pct(t('costs.report.commissionPct'), inputs.sales.commission);
  pct(t('costs.report.taxPct'), inputs.sales.tax);
  cash(t('costs.report.commission'), result.commission);
  cash(t('costs.report.tax'), result.tax);
  cash(t('costs.report.profit'), result.profit);
  pct(t('costs.report.profitOnCost'), result.profitOnCostPct);
  cash(t('costs.report.price'), result.price);

  head(t('costs.report.section.batch'));
  quantity(t('costs.report.quantity'), inputs.quantity, 0, '');
  cash(t('costs.report.batchCost'), result.batchCost);
  cash(t('costs.report.batchPrice'), result.batchPrice);
  cash(t('costs.report.batchProfit'), result.batchProfit);
  if (result.sheets !== null) quantity(t('costs.report.sheets'), result.sheets, 0, '');
  return rows;
}

/** Desglose para pegar en un mensaje, una cotización o una nota. */
export function reportText(ctx: ReportContext): string {
  const lines: string[] = [t('costs.report.title')];
  for (const r of reportRows(ctx)) {
    if (r.heading) lines.push('', r.label.toUpperCase());
    else lines.push(`${r.label}: ${r.display}`);
  }
  if (ctx.result.warnings.length > 0) {
    lines.push('', t('costs.report.section.notes').toUpperCase());
    for (const w of ctx.result.warnings) lines.push(`- ${t(WARNING_KEYS[w])}`);
  }
  return lines.join('\n');
}

/** Un campo del CSV entre comillas si hace falta (comas, comillas o saltos de línea). */
export function csvField(value: string): string {
  const safe = /^[=+\-@\t\r]/.test(value) && !/^-?\d+(\.\d+)?$/.test(value) ? `'${value}` : value;
  return /[",\n\r;]/.test(safe) ? `"${safe.replace(/"/g, '""')}"` : safe;
}

/**
 * El mismo desglose en CSV: `seccion,concepto,valor,unidad`. Los números llevan punto decimal y no
 * separador de miles, para que cualquier hoja de cálculo los lea; el texto sale en el idioma de la interfaz.
 */
export function reportCsv(ctx: ReportContext): string {
  const out = [[t('costs.csv.section'), t('costs.csv.concept'), t('costs.csv.value'), t('costs.csv.unit')].map(csvField).join(',')];
  let section = '';
  for (const r of reportRows(ctx)) {
    if (r.heading) {
      section = r.label;
      continue;
    }
    out.push([section, r.label, r.raw, r.unit].map(csvField).join(','));
  }
  for (const w of ctx.result.warnings) out.push([t('costs.report.section.notes'), t(WARNING_KEYS[w]), '', ''].map(csvField).join(','));
  return out.join('\r\n') + '\r\n';
}
