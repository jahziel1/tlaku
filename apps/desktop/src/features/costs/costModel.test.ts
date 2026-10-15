import { describe, expect, it } from 'vitest';
import {
  clampNum,
  computeCost,
  DEFAULT_MACHINE,
  DEFAULT_MATERIAL,
  DEFAULT_SALES,
  priceBeforeRounding,
  roundMoney,
  roundUpTo,
  type CostInputs,
} from './costModel';

/**
 * Caso a mano: una pasada de 1 h (30 min con láser), caja de 100 × 100 mm, hoja de 600 × 400 mm a 120.
 *   máquina 60 + desgaste (1200 / 2000 h × 0,5 h) 0,30 + electricidad (0,12 kW × 1 h × 2,5) 0,30
 *   + material (10 000 mm² ÷ 240 000 mm² × 120) 5 + acabado (6 min × 100/h) 10
 *   + preparación (30 min × 100/h = 50 ÷ 10 piezas) 5 + consumibles 2 = 82,60
 */
const BASE: CostInputs = {
  job: { runS: 3600, laserOnS: 1800, bboxMm2: 10_000, realMm2: 6_000 },
  machine: { hourlyRate: 60, tubePrice: 1200, tubeLifeH: 2000, watts: 100, accessoryWatts: 20 },
  material: { ...DEFAULT_MATERIAL, sheetPrice: 120, wastePct: 0 },
  materialMode: 'bbox',
  sales: { ...DEFAULT_SALES, kwhPrice: 2.5, laborRate: 100, prepMin: 30, finishMin: 6, consumables: 2, margin: 30, commission: 10, tax: 0 },
  piecesInDesign: 1,
  quantity: 10,
  timeUsed: 'calibrated',
};

const line = (r: ReturnType<typeof computeCost>, id: string) => r.lines.find((l) => l.id === id)!;

describe('redondeo', () => {
  it('la mitad sube aunque la coma flotante diga 1,00499999', () => {
    expect(roundMoney(1.005, 2)).toBe(1.01);
    expect(roundMoney(2.675, 2)).toBe(2.68);
    expect(roundMoney(0.125, 2)).toBe(0.13);
    expect(roundMoney(-1.005, 2)).toBe(-1.01);
  });

  it('monedas sin decimales redondean al entero', () => {
    expect(roundMoney(1234.5, 0)).toBe(1235);
    expect(roundMoney(1234.4, 0)).toBe(1234);
  });

  it('roundUpTo sube siempre al múltiplo y nunca baja el precio', () => {
    expect(roundUpTo(119.3111, 0, 2)).toBe(119.32);
    expect(roundUpTo(119.32, 0, 2)).toBe(119.32); // ya exacto: no sube otro centavo
    expect(roundUpTo(119.32, 0.5, 2)).toBe(119.5);
    expect(roundUpTo(119.5, 0.5, 2)).toBe(119.5);
    expect(roundUpTo(119.32, 5, 2)).toBe(120);
    expect(roundUpTo(120, 5, 2)).toBe(120);
    expect(roundUpTo(120.01, 10, 2)).toBe(130);
    expect(roundUpTo(1234.1, 0, 0)).toBe(1235); // CLP: al peso
  });

  it('clampNum acota y trata lo ilegible como el mínimo', () => {
    expect(clampNum(5, 0, 10)).toBe(5);
    expect(clampNum(-3, 0, 10)).toBe(0);
    expect(clampNum(99, 0, 10)).toBe(10);
    expect(clampNum(Number.NaN, 2, 10)).toBe(2);
    expect(clampNum('7', 0, 10)).toBe(0);
    expect(clampNum(Infinity, 0, 10)).toBe(0);
  });
});

describe('costo por pieza (caso a mano)', () => {
  const r = computeCost(BASE);

  it('cada línea sale de su fórmula', () => {
    expect(line(r, 'machine').perPiece).toBe(60);
    expect(line(r, 'wear').perPiece).toBe(0.3);
    expect(line(r, 'power').perPiece).toBe(0.3);
    expect(line(r, 'material').perPiece).toBe(5);
    expect(line(r, 'finish').perPiece).toBe(10);
    expect(line(r, 'prep').perPiece).toBe(5);
    expect(line(r, 'consumables').perPiece).toBe(2);
    expect(r.cost).toBe(82.6);
  });

  it('el lote suma la preparación una sola vez', () => {
    expect(line(r, 'prep').batch).toBe(50);
    expect(line(r, 'machine').batch).toBe(600);
    expect(r.batchCost).toBe(826);
  });

  it('el desgaste solo cuenta las horas con láser encendido', () => {
    const off = computeCost({ ...BASE, job: { ...BASE.job, laserOnS: 0 } });
    expect(line(off, 'wear').perPiece).toBe(0);
    const full = computeCost({ ...BASE, job: { ...BASE.job, laserOnS: 99_999 } }); // no puede pasar del total
    expect(line(full, 'wear').perPiece).toBe(0.6);
    const noLife = computeCost({ ...BASE, machine: { ...BASE.machine, tubeLifeH: 0 } });
    expect(line(noLife, 'wear').perPiece).toBe(0);
  });

  it('piezas por pasada reparten tiempo, electricidad y material', () => {
    const r4 = computeCost({ ...BASE, piecesInDesign: 4 });
    expect(line(r4, 'machine').perPiece).toBe(15);
    expect(line(r4, 'power').perPiece).toBe(0.08); // 0,075 redondea a 0,08
    expect(line(r4, 'material').perPiece).toBe(1.25);
  });

  it('desperdicio sube el material en ese porcentaje', () => {
    const w = computeCost({ ...BASE, material: { ...BASE.material, wastePct: 20 } });
    expect(line(w, 'material').perPiece).toBe(6);
  });

  it('el área real cuesta menos que la caja envolvente', () => {
    const real = computeCost({ ...BASE, materialMode: 'area' });
    expect(line(real, 'material').perPiece).toBe(3); // 6 000 mm²
    expect(real.warnings).not.toContain('bboxArea');
    expect(r.warnings).toContain('bboxArea');
  });

  it('sin formas cerradas el área real cae en la caja y avisa', () => {
    const open = computeCost({ ...BASE, materialMode: 'area', job: { ...BASE.job, realMm2: 0 } });
    expect(line(open, 'material').perPiece).toBe(5);
    expect(open.warnings).toContain('noClosedShapes');
  });

  it('por hoja: precio ÷ piezas por hoja, y cuántas hojas hacen falta', () => {
    const sheet = computeCost({ ...BASE, materialMode: 'sheet', material: { ...BASE.material, piecesPerSheet: 6 } });
    expect(line(sheet, 'material').perPiece).toBe(20);
    expect(sheet.sheets).toBe(2); // 10 piezas / 6 por hoja
    expect(computeCost({ ...BASE, materialMode: 'sheet', material: { ...BASE.material, piecesPerSheet: 5 } }).sheets).toBe(2);
    expect(computeCost({ ...BASE, materialMode: 'sheet', material: { ...BASE.material, piecesPerSheet: 10 } }).sheets).toBe(1);
  });

  it('avisa cuando falta el tamaño de la hoja o las piezas por hoja', () => {
    expect(computeCost({ ...BASE, material: { ...BASE.material, sheetW: 0 } }).warnings).toContain('noSheetSize');
    expect(computeCost({ ...BASE, materialMode: 'sheet', material: { ...BASE.material, piecesPerSheet: 0 } }).warnings).toContain('noPiecesPerSheet');
  });

  it('sin trabajo calculado solo cuentan los gastos fijos y avisa', () => {
    const none = computeCost({ ...BASE, job: { runS: 0, laserOnS: 0, bboxMm2: 0, realMm2: 0 } });
    expect(none.warnings).toContain('noJob');
    expect(none.cost).toBe(17); // acabado 10 + preparación 5 + consumibles 2
  });

  it('avisa del tiempo sin calibrar y del aproximado', () => {
    expect(computeCost({ ...BASE, timeUsed: 'estimate' }).warnings).toContain('uncalibrated');
    expect(computeCost({ ...BASE, approximateTime: true }).warnings).toContain('approxTime');
    expect(r.warnings).not.toContain('uncalibrated');
  });
});

describe('precio: comisión e impuestos sobre el precio final', () => {
  it('margen sobre el costo: P = C (1 + m) / (1 − c − t)', () => {
    const r = computeCost(BASE);
    expect(priceBeforeRounding(82.6, BASE.sales)).toBeCloseTo(119.3111, 4);
    expect(r.price).toBe(119.32);
    expect(r.commission).toBe(11.93);
    expect(r.tax).toBe(0);
    expect(r.profit).toBe(24.79);
    expect(r.batchPrice).toBe(1193.2);
  });

  it('lo que queda tras la comisión y el impuesto cubre costo + ganancia pedida', () => {
    for (const [commission, tax, margin] of [[10, 0, 30], [12.5, 16, 40], [6.5, 13.79, 25], [0, 0, 0], [30, 10, 100]] as const) {
      const r = computeCost({ ...BASE, sales: { ...BASE.sales, commission, tax, margin } });
      const kept = r.price! - r.commission - r.tax; // lo que le queda a quien vende
      expect(kept).toBeGreaterThanOrEqual(r.cost * (1 + margin / 100) - 0.011);
    }
  });

  it('sumar la comisión al costo se quedaría corto: el precio la cubre sobre el precio final', () => {
    const r = computeCost(BASE);
    const naive = r.cost * 1.3 * 1.1; // «30 % + 10 %» sumados: 118,18
    expect(r.price!).toBeGreaterThan(naive);
    expect(r.price! * 0.9).toBeGreaterThanOrEqual(r.cost * 1.3 - 0.01);
  });

  it('margen sobre el precio: P = C / (1 − c − t − m)', () => {
    const sales = { ...BASE.sales, marginBase: 'price' as const, margin: 20, commission: 10, tax: 5 };
    expect(priceBeforeRounding(100, sales)).toBeCloseTo(153.846, 3);
    const r = computeCost({ ...BASE, sales: { ...sales, laborRate: 0, consumables: 100, prepMin: 0, finishMin: 0 }, job: { runS: 0, laserOnS: 0, bboxMm2: 0, realMm2: 0 } });
    expect(r.cost).toBe(100);
    expect(r.price).toBe(153.85);
    expect(r.profitOnPricePct!).toBeGreaterThanOrEqual(19.99);
  });

  it('con 100 % o más entre comisión, impuesto y margen no hay precio', () => {
    const r = computeCost({ ...BASE, sales: { ...BASE.sales, marginBase: 'price', margin: 60, commission: 30, tax: 10 } });
    expect(r.price).toBeNull();
    expect(r.batchPrice).toBeNull();
    expect(r.warnings).toContain('impossible');
    expect(computeCost({ ...BASE, sales: { ...BASE.sales, commission: 70, tax: 30 } }).price).toBeNull();
  });

  it('el redondeo hacia arriba nunca baja la ganancia', () => {
    const exact = computeCost(BASE);
    for (const roundTo of [0.5, 1, 5, 10] as const) {
      const r = computeCost({ ...BASE, sales: { ...BASE.sales, roundTo } });
      expect(r.price! % roundTo === 0 || Math.abs((r.price! / roundTo) % 1) < 1e-9).toBe(true);
      expect(r.price!).toBeGreaterThanOrEqual(exact.price!);
      expect(r.profit).toBeGreaterThanOrEqual(exact.profit);
    }
    expect(computeCost({ ...BASE, sales: { ...BASE.sales, roundTo: 5 } }).price).toBe(120);
  });

  it('moneda sin decimales (CLP): todo en pesos enteros', () => {
    const r = computeCost({ ...BASE, machine: DEFAULT_MACHINE, sales: { ...BASE.sales, laborRate: 3333, consumables: 1000 } }, 0);
    for (const l of r.lines) expect(Number.isInteger(l.perPiece)).toBe(true);
    expect(Number.isInteger(r.price!)).toBe(true);
    expect(Number.isInteger(r.commission)).toBe(true);
  });

  it('el desglose cuadra: precio = costo + comisión + impuesto + ganancia', () => {
    const r = computeCost({ ...BASE, sales: { ...BASE.sales, commission: 12.5, tax: 13.79, margin: 35 } });
    expect(roundMoney(r.cost + r.commission + r.tax + r.profit, 2)).toBe(r.price);
    expect(roundMoney(r.lines.reduce((s, l) => s + l.perPiece, 0), 2)).toBe(r.cost);
  });
});

describe('propiedades: el precio crece de forma monótona', () => {
  const price = (patch: Partial<CostInputs>) => computeCost({ ...BASE, ...patch }).price!;

  it('con el tiempo', () => {
    let prev = 0;
    for (const runS of [0, 600, 1800, 3600, 7200, 36_000]) {
      const p = price({ job: { ...BASE.job, runS, laserOnS: runS / 2 } });
      expect(p).toBeGreaterThanOrEqual(prev);
      prev = p;
    }
  });

  it('con el área', () => {
    let prev = 0;
    for (const bboxMm2 of [0, 1000, 10_000, 100_000]) {
      const p = price({ job: { ...BASE.job, bboxMm2 } });
      expect(p).toBeGreaterThanOrEqual(prev);
      prev = p;
    }
  });

  it('con el margen, la comisión y los impuestos', () => {
    for (const key of ['margin', 'commission', 'tax'] as const) {
      let prev = 0;
      for (const v of [0, 5, 10, 25, 40]) {
        const p = price({ sales: { ...BASE.sales, [key]: v } });
        expect(p, `${key}=${v}`).toBeGreaterThanOrEqual(prev);
        prev = p;
      }
    }
  });

  it('el lote cuesta más con la cantidad y cada pieza cuesta menos (la preparación se reparte)', () => {
    let prevBatch = 0;
    let prevPiece = Infinity;
    for (const quantity of [1, 2, 5, 10, 50, 500]) {
      const r = computeCost({ ...BASE, quantity });
      expect(r.batchPrice!).toBeGreaterThan(prevBatch);
      expect(r.price!).toBeLessThanOrEqual(prevPiece);
      prevBatch = r.batchPrice!;
      prevPiece = r.price!;
    }
  });
});
