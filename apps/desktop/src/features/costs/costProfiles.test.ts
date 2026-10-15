import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_MATERIAL } from './costModel';
import {
  COSTS_KEY,
  defaultStored,
  MATERIAL_PROFILES_MAX,
  machineFor,
  readCosts,
  removeMaterial,
  sanitizeMachine,
  sanitizeMaterial,
  sanitizeSales,
  sanitizeStored,
  upsertMaterial,
  writeCosts,
  type MaterialProfile,
} from './costProfiles';
import { resetCostStore, useCostStore } from './costStore';

const memory = new Map<string, string>();

beforeEach(() => {
  memory.clear();
  vi.stubGlobal('navigator', { language: 'es-MX' });
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
    removeItem: (k: string) => void memory.delete(k),
  });
  resetCostStore();
});

const stored = () => JSON.parse(memory.get(`tlaku.${COSTS_KEY}`) ?? 'null');

describe('saneado de lo guardado', () => {
  it('lo dañado cae en valores válidos', () => {
    expect(sanitizeMachine({ hourlyRate: -5, tubePrice: 'caro', tubeLifeH: 1e12, watts: Number.NaN })).toEqual({
      hourlyRate: 0,
      tubePrice: 0,
      tubeLifeH: 1e6,
      watts: 0,
      accessoryWatts: 0,
    });
    expect(sanitizeMaterial({ sheetPrice: 50, wastePct: 500, piecesPerSheet: 2.9 })).toMatchObject({ sheetPrice: 50, wastePct: 100, piecesPerSheet: 2, sheetW: 600, sheetH: 400 });
    expect(sanitizeMaterial(null)).toEqual({ ...DEFAULT_MATERIAL, sheetPrice: 0 });
  });

  it('la moneda desconocida se cambia por la de la región; las buenas se respetan', () => {
    expect(sanitizeSales({ currency: 'XXX' }, 'es', 'es-CO').currency).toBe('COP');
    expect(sanitizeSales({ currency: 'EUR' }, 'es', 'es-MX').currency).toBe('EUR');
    expect(sanitizeSales({}, 'en', null).currency).toBe('USD');
    expect(sanitizeSales({}, 'es', 'es-MX').currency).toBe('MXN');
  });

  it('porcentajes, redondeo y base del margen quedan dentro de sus opciones', () => {
    const s = sanitizeSales({ commission: 400, tax: -2, margin: 5000, marginBase: 'otro', roundTo: 7 }, 'es', null);
    expect(s).toMatchObject({ commission: 100, tax: 0, margin: 1000, marginBase: 'cost', roundTo: 0 });
    expect(sanitizeSales({ marginBase: 'price', roundTo: 5 }, 'es', null)).toMatchObject({ marginBase: 'price', roundTo: 5 });
  });

  it('cantidad y piezas son enteros de al menos 1', () => {
    const s = sanitizeStored({ quantity: 0, piecesInDesign: 3.7 }, 'es', null);
    expect(s.quantity).toBe(1);
    expect(s.piecesInDesign).toBe(3);
  });

  it('basura entera da los valores de fábrica', () => {
    for (const junk of [null, 5, 'x', [], { machines: 3, materials: 'no' }]) {
      const s = sanitizeStored(junk, 'es', 'es-MX');
      expect(s.machines).toEqual({});
      expect(s.materials).toEqual([]);
      expect(s.sales.currency).toBe('MXN');
      expect(s.materialMode).toBe('bbox');
    }
  });

  it('los perfiles de material repetidos o sin nombre se descartan y el elegido debe existir', () => {
    const s = sanitizeStored(
      {
        materials: [
          { name: ' Triplay 3 mm ', material: { sheetPrice: 90 }, mode: 'sheet' },
          { name: 'Triplay 3 mm', material: { sheetPrice: 1 } },
          { name: '   ' },
          { material: {} },
          'texto',
          { name: 'MDF', material: { sheetPrice: 70 }, mode: 'raro' },
        ],
        materialName: 'No existe',
      },
      'es',
      null,
    );
    expect(s.materials.map((p) => p.name)).toEqual(['Triplay 3 mm', 'MDF']);
    expect(s.materials[0]).toMatchObject({ mode: 'sheet', material: { sheetPrice: 90 } });
    expect(s.materials[1]!.mode).toBe('bbox');
    expect(s.materialName).toBeNull();
    expect(sanitizeStored({ materials: [{ name: 'MDF' }], materialName: 'MDF' }, 'es', null).materialName).toBe('MDF');
  });
});

describe('lectura y escritura en el almacenamiento local', () => {
  it('ida y vuelta', () => {
    const data = { ...defaultStored('es', 'es-MX'), quantity: 25, machines: { falcon: sanitizeMachine({ hourlyRate: 80, watts: 150 }) } };
    writeCosts(data);
    expect(readCosts('es', 'es-MX')).toEqual(data);
  });

  it('un JSON roto o ausente no rompe: valores de fábrica', () => {
    expect(readCosts('es', 'es-MX')).toEqual(defaultStored('es', 'es-MX'));
    memory.set(`tlaku.${COSTS_KEY}`, '{no es json');
    expect(readCosts('en', null).sales.currency).toBe('USD');
  });

  it('sin almacenamiento disponible tampoco se rompe', () => {
    vi.stubGlobal('localStorage', {
      getItem: () => {
        throw new Error('bloqueado');
      },
      setItem: () => {
        throw new Error('bloqueado');
      },
    });
    expect(readCosts('es', null).quantity).toBe(1);
    expect(() => writeCosts(defaultStored('es', null))).not.toThrow();
  });
});

describe('perfiles de material', () => {
  const profile = (name: string, price = 10): MaterialProfile => ({ name, material: sanitizeMaterial({ sheetPrice: price }), mode: 'area' });

  it('guardar agrega y volver a guardar con el mismo nombre reemplaza', () => {
    let list = upsertMaterial([], profile('MDF', 70));
    list = upsertMaterial(list, profile('Acrílico', 300));
    list = upsertMaterial(list, profile('MDF', 75));
    expect(list.map((p) => [p.name, p.material.sheetPrice])).toEqual([['MDF', 75], ['Acrílico', 300]]);
  });

  it('un nombre vacío no guarda y el tope se respeta', () => {
    expect(upsertMaterial([], profile('  '))).toEqual([]);
    let list: MaterialProfile[] = [];
    for (let i = 0; i < MATERIAL_PROFILES_MAX + 5; i++) list = upsertMaterial(list, profile(`m${i}`));
    expect(list).toHaveLength(MATERIAL_PROFILES_MAX);
  });

  it('eliminar quita solo ese', () => {
    const list = [profile('a'), profile('b')];
    expect(removeMaterial(list, 'a').map((p) => p.name)).toEqual(['b']);
    expect(removeMaterial(list, 'zzz')).toHaveLength(2);
  });
});

describe('store de la calculadora', () => {
  it('los valores de cada máquina se recuerdan por separado y de fábrica son ceros', () => {
    const st = () => useCostStore.getState();
    expect(machineFor(st(), 'falcon').hourlyRate).toBe(0);
    st().setMachine('falcon', { hourlyRate: 80, watts: 150 });
    st().setMachine('k40', { hourlyRate: 20, tubeLifeH: 1500 });
    st().setMachine('falcon', { tubePrice: 900 });
    expect(machineFor(st(), 'falcon')).toMatchObject({ hourlyRate: 80, watts: 150, tubePrice: 900 });
    expect(machineFor(st(), 'k40')).toMatchObject({ hourlyRate: 20, tubeLifeH: 1500, watts: 0 });
    expect(stored().machines.falcon.hourlyRate).toBe(80);
  });

  it('lo escrito se sanea antes de guardarse', () => {
    const st = () => useCostStore.getState();
    st().setMachine('x', { hourlyRate: -10 });
    st().setSales({ commission: 250, currency: 'XXX' });
    st().setQuantity(0);
    st().setPiecesInDesign(2.8);
    expect(machineFor(st(), 'x').hourlyRate).toBe(0);
    expect(st().sales.commission).toBe(100);
    expect(st().sales.currency).toBe('MXN'); // la inválida cae en la de la región (es-MX)
    expect(st().quantity).toBe(1);
    expect(st().piecesInDesign).toBe(2);
  });

  it('guardar, cargar y eliminar un perfil de material', () => {
    const st = () => useCostStore.getState();
    st().setMaterial({ sheetPrice: 90, sheetW: 1220, sheetH: 610, wastePct: 15 });
    st().setMaterialMode('area');
    expect(st().saveMaterialProfile('Triplay 3 mm')).toBe(true);
    st().setMaterial({ sheetPrice: 5 });
    st().setMaterialMode('bbox');
    st().loadMaterialProfile('Triplay 3 mm');
    expect(st().material).toMatchObject({ sheetPrice: 90, sheetW: 1220, wastePct: 15 });
    expect(st().materialMode).toBe('area');
    expect(st().materialName).toBe('Triplay 3 mm');
    expect(stored().materials).toHaveLength(1);
    expect(st().saveMaterialProfile('   ')).toBe(false);
    st().deleteMaterialProfile('Triplay 3 mm');
    expect(st().materials).toEqual([]);
    expect(st().materialName).toBeNull();
    expect(stored().materials).toEqual([]);
  });

  it('al abrir lee lo guardado una sola vez', () => {
    writeCosts({ ...defaultStored('es', null), quantity: 12 });
    const st = () => useCostStore.getState();
    st().openDialog();
    expect(st().open).toBe(true);
    expect(st().quantity).toBe(12);
    st().setQuantity(30);
    st().closeDialog();
    st().openDialog(); // no vuelve a leer: conserva lo cambiado
    expect(st().quantity).toBe(30);
  });
});
