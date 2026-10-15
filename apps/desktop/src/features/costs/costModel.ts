/**
 * Calculadora de costos y precio por pieza (mejora 3). Todo es aritmética pura: no depende de
 * React, del store ni de la red. Los datos del trabajo (tiempo, área) los trae `costMetrics.ts`.
 *
 * Fórmulas (todas por pieza; `n` = piezas que salen en una pasada del diseño, `q` = cantidad):
 *
 *   máquina        = (T / n) × tarifa_hora                       T = horas de la pasada
 *   desgaste       = (L / n) × (precio_tubo / vida_horas)         L = horas con el láser encendido
 *   electricidad   = (T / n) × (W_máquina + W_accesorios) / 1000 × precio_kWh
 *   material       = área_mm² / n × (1 + desperdicio) × precio_hoja / área_hoja_mm²   (modos área)
 *                  = precio_hoja / piezas_por_hoja                                     (modo hoja)
 *   acabado        = min_acabado / 60 × tarifa_mano_de_obra
 *   preparación    = min_preparación / 60 × tarifa_mano_de_obra / q     (se paga una vez por lote)
 *   consumibles    = costo fijo por pieza
 *   C (costo)      = suma de las líneas, cada una redondeada a la moneda
 *
 * Precio. La comisión de la plataforma `c` y el impuesto `t` se cobran sobre el PRECIO FINAL `P`
 * (lo que paga el cliente), no sobre el costo; por eso no basta sumarlos al costo. Lo que le queda a
 * quien vende es `P × (1 − c − t)`, y eso debe cubrir el costo más la ganancia:
 *
 *   margen sobre el costo (markup `m`):   P = C × (1 + m) / (1 − c − t)
 *   margen sobre el precio (`m`):         P = C / (1 − c − t − m)
 *
 * Si el denominador no es positivo, ningún precio cubre esos porcentajes (`impossible`). Después el
 * precio se redondea SIEMPRE hacia arriba al paso elegido (o al centavo de la moneda), así que la
 * ganancia real nunca queda por debajo de la pedida. Con un impuesto que se suma encima del precio
 * (IVA 16 %), escribe 16 / 1,16 = 13,79 % (es lo que pesa sobre el precio final).
 */

export type TimeSource = 'estimate' | 'calibrated' | 'lastReal';
export type MaterialMode = 'bbox' | 'area' | 'sheet';
export type MarginBase = 'cost' | 'price';

/** Valores de la máquina: se recuerdan por perfil de máquina. */
export interface MachineCost {
  /** Amortización o tarifa de la máquina por hora de trabajo (en la moneda elegida). */
  hourlyRate: number;
  /** Precio del tubo o del módulo de diodo. */
  tubePrice: number;
  /** Vida útil del tubo o diodo, en horas con el láser encendido. 0 = no cuenta el desgaste. */
  tubeLifeH: number;
  /** Consumo de la máquina, en W. */
  watts: number;
  /** Consumo de accesorios (extractor, compresor, enfriador), en W. */
  accessoryWatts: number;
}

/** Valores del material: se recuerdan con nombre (perfiles de material). */
export interface MaterialCost {
  /** Precio de una hoja completa. */
  sheetPrice: number;
  /** Tamaño de la hoja, en mm. */
  sheetW: number;
  sheetH: number;
  /** Desperdicio sobre el área usada, en %. */
  wastePct: number;
  /** Con el modo «por hoja»: cuántas piezas salen de una hoja con el acomodo. */
  piecesPerSheet: number;
}

/** Pasos de redondeo del precio (siempre hacia arriba). 0 = al centavo de la moneda. */
export const ROUND_STEPS = [0, 0.5, 1, 5, 10] as const;
export type RoundStep = (typeof ROUND_STEPS)[number];

/** Mano de obra, venta y moneda: se recuerdan para todas las máquinas. */
export interface SalesCost {
  /** Código ISO de la moneda (`MXN`, `USD`...). */
  currency: string;
  kwhPrice: number;
  /** Tarifa de la mano de obra por hora. */
  laborRate: number;
  prepMin: number;
  finishMin: number;
  /** Empaque y otros consumibles fijos por pieza. */
  consumables: number;
  /** Margen o ganancia deseada, en %. */
  margin: number;
  marginBase: MarginBase;
  /** Comisión de la plataforma sobre el precio final, en %. */
  commission: number;
  /** Impuestos sobre el precio final, en %. */
  tax: number;
  roundTo: RoundStep;
}

/** Lo que se sabe del trabajo ya resuelto a una sola pasada del diseño. */
export interface JobFigures {
  /** Duración de una pasada, en segundos (según la fuente de tiempo elegida). */
  runS: number;
  /** Segundos con el láser encendido dentro de esa pasada. */
  laserOnS: number;
  /** Área de la caja envolvente del diseño, en mm². */
  bboxMm2: number;
  /** Área real cubierta por las formas cerradas, en mm². */
  realMm2: number;
}

export interface CostInputs {
  job: JobFigures;
  machine: MachineCost;
  material: MaterialCost;
  materialMode: MaterialMode;
  sales: SalesCost;
  /** Piezas que salen en una pasada del diseño (≥ 1). */
  piecesInDesign: number;
  /** Piezas que se van a hacer y vender (≥ 1). */
  quantity: number;
  /** Fuente de tiempo que se usó de verdad (si no hay historial, la elegida cae en el estimado). */
  timeUsed?: TimeSource;
  /** El tiempo viene de un cálculo por distancia, no de los tiempos exactos del motor. */
  approximateTime?: boolean;
}

export type CostLineId = 'machine' | 'wear' | 'power' | 'material' | 'finish' | 'prep' | 'consumables';

export interface CostLine {
  id: CostLineId;
  /** Monto por pieza, redondeado a la moneda. */
  perPiece: number;
  /** Monto del lote (por pieza × cantidad; la preparación suma exacto una vez). */
  batch: number;
}

export type CostWarning =
  /** No hay una vista previa: el tiempo y el área valen 0. */
  | 'noJob'
  /** El tiempo salió de un cálculo por distancia, sin los tiempos exactos del motor. */
  | 'approxTime'
  /** El tiempo es el estimado sin calibrar con trabajos reales. */
  | 'uncalibrated'
  /** El área usada es la caja envolvente; con un acomodo apretado sobra material. */
  | 'bboxArea'
  /** El diseño no tiene formas cerradas: el área real es 0 y se usó la caja envolvente. */
  | 'noClosedShapes'
  /** El modo de área necesita el tamaño de la hoja. */
  | 'noSheetSize'
  /** El modo «por hoja» necesita las piezas por hoja. */
  | 'noPiecesPerSheet'
  /** Comisión + impuesto (+ margen sobre el precio) llegan al 100 %: no hay precio posible. */
  | 'impossible';

export interface CostResult {
  lines: CostLine[];
  /** Costo por pieza (suma de las líneas por pieza). */
  cost: number;
  /** Precio final por pieza, redondeado hacia arriba; `null` si es imposible. */
  price: number | null;
  /** Desglose del precio por pieza. */
  commission: number;
  tax: number;
  profit: number;
  /** Ganancia real sobre el costo y sobre el precio, en % (tras redondear). */
  profitOnCostPct: number | null;
  profitOnPricePct: number | null;
  /** Totales del lote. */
  batchCost: number;
  batchPrice: number | null;
  batchProfit: number | null;
  /** Hojas que hacen falta (solo con el modo «por hoja»). */
  sheets: number | null;
  warnings: CostWarning[];
}

// ---------------------------------------------------------------------------
// Valores de fábrica y límites
// ---------------------------------------------------------------------------

export const DEFAULT_MACHINE: MachineCost = { hourlyRate: 0, tubePrice: 0, tubeLifeH: 0, watts: 0, accessoryWatts: 0 };

export const DEFAULT_MATERIAL: MaterialCost = { sheetPrice: 0, sheetW: 600, sheetH: 400, wastePct: 10, piecesPerSheet: 1 };

export const DEFAULT_SALES: SalesCost = {
  currency: 'MXN',
  kwhPrice: 0,
  laborRate: 0,
  prepMin: 0,
  finishMin: 0,
  consumables: 0,
  margin: 30,
  marginBase: 'cost',
  commission: 0,
  tax: 0,
  roundTo: 0,
};

/** Límites de cada campo numérico (se usan al validar lo escrito y lo leído del disco). */
export const LIMITS = {
  money: { min: 0, max: 1e9 },
  hours: { min: 0, max: 1e6 },
  watts: { min: 0, max: 1e6 },
  minutes: { min: 0, max: 1e5 },
  mm: { min: 0, max: 1e5 },
  pct: { min: 0, max: 100 },
  margin: { min: 0, max: 1000 },
  count: { min: 1, max: 1e6 },
} as const;

/** Acota `v` a [min, max]; lo que no es un número finito cae en `min`. */
export function clampNum(v: unknown, min: number, max: number): number {
  const n = typeof v === 'number' ? v : Number.NaN;
  if (!Number.isFinite(n)) return min;
  return Math.min(max, Math.max(min, n));
}

// ---------------------------------------------------------------------------
// Redondeo
// ---------------------------------------------------------------------------

/** Tolerancia contra el ruido de coma flotante (1,005 × 100 = 100,49999...). */
const EPS = 1e-9;

/** Redondea al centavo de la moneda (`digits` decimales), con la mitad hacia arriba. */
export function roundMoney(value: number, digits: number): number {
  if (!Number.isFinite(value)) return 0;
  const f = 10 ** digits;
  const r = Math.round((Math.abs(value) + EPS) * f) / f;
  return value < 0 ? -r : r;
}

/** Redondea hacia arriba al múltiplo `step` (y luego al centavo, para quitar el ruido). */
export function roundUpTo(value: number, step: number, digits: number): number {
  if (!Number.isFinite(value)) return 0;
  const s = step > 0 ? step : 10 ** -digits;
  return roundMoney(Math.ceil(value / s - EPS) * s, digits);
}

// ---------------------------------------------------------------------------
// Cálculo
// ---------------------------------------------------------------------------

/** Área usada por la pasada según el modo (mm²) y los avisos que implica. */
function usedArea(inputs: CostInputs, warnings: CostWarning[]): number {
  const { job, materialMode } = inputs;
  if (materialMode === 'bbox') {
    warnings.push('bboxArea');
    return job.bboxMm2;
  }
  if (job.realMm2 > 0) return job.realMm2;
  if (job.bboxMm2 > 0) warnings.push('noClosedShapes');
  return job.bboxMm2;
}

/**
 * Precio antes de redondear, o `null` si ningún precio cubre la comisión, los impuestos y el margen
 * (el denominador no es positivo). Ver la fórmula en la cabecera del archivo.
 */
export function priceBeforeRounding(cost: number, sales: Pick<SalesCost, 'commission' | 'tax' | 'margin' | 'marginBase'>): number | null {
  const c = sales.commission / 100;
  const t = sales.tax / 100;
  const m = sales.margin / 100;
  const den = sales.marginBase === 'price' ? 1 - c - t - m : 1 - c - t;
  if (!(den > 1e-6)) return null;
  return sales.marginBase === 'price' ? cost / den : (cost * (1 + m)) / den;
}

/** Calcula el costo, el precio y el desglose. `digits` = decimales de la moneda. */
export function computeCost(inputs: CostInputs, digits = 2): CostResult {
  const { job, machine, material, materialMode, sales } = inputs;
  const n = Math.max(1, Math.floor(inputs.piecesInDesign) || 1);
  const q = Math.max(1, Math.floor(inputs.quantity) || 1);
  const warnings: CostWarning[] = [];
  if (job.runS <= 0 && job.bboxMm2 <= 0) warnings.push('noJob');
  else if (job.runS > 0) {
    if (inputs.timeUsed === 'estimate') warnings.push('uncalibrated');
    if (inputs.approximateTime) warnings.push('approxTime');
  }

  const runH = Math.max(0, job.runS) / 3600;
  const onH = Math.min(Math.max(0, job.laserOnS), Math.max(0, job.runS)) / 3600;

  const machinePer = (runH / n) * machine.hourlyRate;
  const wearPer = machine.tubeLifeH > 0 ? (onH / n) * (machine.tubePrice / machine.tubeLifeH) : 0;
  const powerPer = (runH / n) * ((machine.watts + machine.accessoryWatts) / 1000) * sales.kwhPrice;

  let materialPer = 0;
  let sheets: number | null = null;
  if (materialMode === 'sheet') {
    if (material.piecesPerSheet >= 1) {
      materialPer = material.sheetPrice / material.piecesPerSheet;
      sheets = Math.ceil(q / material.piecesPerSheet - EPS);
    } else {
      warnings.push('noPiecesPerSheet');
    }
  } else {
    const sheetArea = material.sheetW * material.sheetH;
    const area = usedArea(inputs, warnings);
    if (sheetArea > 0) materialPer = ((area / n) * (1 + material.wastePct / 100) * material.sheetPrice) / sheetArea;
    else warnings.push('noSheetSize');
  }

  const rate = sales.laborRate;
  const finishPer = (sales.finishMin / 60) * rate;
  const prepBatch = (sales.prepMin / 60) * rate;

  const r = (v: number) => roundMoney(v, digits);
  const prepPer = r(prepBatch / q);
  const parts: [CostLineId, number, number | null][] = [
    ['machine', r(machinePer), null],
    ['wear', r(wearPer), null],
    ['power', r(powerPer), null],
    ['material', r(materialPer), null],
    ['finish', r(finishPer), null],
    ['prep', prepPer, r(prepBatch)],
    ['consumables', r(sales.consumables), null],
  ];
  const lines: CostLine[] = parts.map(([id, perPiece, batchFixed]) => ({ id, perPiece, batch: batchFixed ?? r(perPiece * q) }));
  const cost = r(lines.reduce((s, l) => s + l.perPiece, 0));
  const batchCost = r(lines.reduce((s, l) => s + l.batch, 0));

  const raw = priceBeforeRounding(cost, sales);
  let price: number | null = null;
  if (raw !== null) price = roundUpTo(raw, sales.roundTo, digits);
  else warnings.push('impossible');

  const commission = price === null ? 0 : r((price * sales.commission) / 100);
  const tax = price === null ? 0 : r((price * sales.tax) / 100);
  const profit = price === null ? 0 : r(price - commission - tax - cost);
  return {
    lines,
    cost,
    price,
    commission,
    tax,
    profit,
    profitOnCostPct: price !== null && cost > 0 ? (profit / cost) * 100 : null,
    profitOnPricePct: price !== null && price > 0 ? (profit / price) * 100 : null,
    batchCost,
    batchPrice: price === null ? null : r(price * q),
    batchProfit: price === null ? null : r(r(price * q) - r(commission * q) - r(tax * q) - batchCost),
    sheets,
    warnings,
  };
}
