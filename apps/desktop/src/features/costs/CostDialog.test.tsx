// @vitest-environment jsdom
import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DesignObject, JobPreview } from '@/api/types';
import { commands } from '@/commands';
import { setLanguage } from '@/i18n';
import { polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { selectActiveDevice, useStore } from '@/store';
import { audit, click, cleanup, describeViolations, mount } from '@/test/dom';
import { CostDialog } from './CostDialog';
import { resetCostStore, useCostStore } from './costStore';

const RED = '#ff3b4a';

const rect = (id: string, w: number, h: number): DesignObject => ({
  id,
  layer: RED,
  transform: [1, 0, 0, 1, 0, 0],
  locked: false,
  hidden: false,
  group: null,
  kind: { type: 'vector', paths: [polylineBez([[0, 0], [w, 0], [w, h], [0, h]], true)] },
});

const preview = (totalS: number): JobPreview => ({
  layers: [{ color: RED, mode: 'line', seconds: totalS, summary: 'Rojo', travel_saved_pct: null, explanations: [] }],
  moves: [0, 0, 0, 0, 1, 100, 0, 80, 1, 100, 100, 80, 1, 0, 100, 80],
  move_times: [0, totalS / 3, (2 * totalS) / 3, totalS],
  total_s: totalS,
  bounds: [[0, 0], [100, 100]],
  order_note: null,
  warnings: [],
});

/** Deja en el store de la app un diseño y una vista previa ya calculada (así la calculadora no pide otra). */
function setDesign(w: number, h: number, totalS: number) {
  act(() => {
    useStore.setState({
      doc: { version: 2, objects: [rect('a', w, h)], layers: [makeLayer(RED, 'Rojo')], device_id: null },
      preview: preview(totalS),
      previewStatus: 'ready',
      previewStale: false,
    });
  });
}

const memory = new Map<string, string>();

beforeEach(() => {
  memory.clear();
  vi.stubGlobal('localStorage', {
    getItem: (k: string) => memory.get(k) ?? null,
    setItem: (k: string, v: string) => void memory.set(k, v),
    removeItem: (k: string) => void memory.delete(k),
  });
  Object.defineProperty(navigator, 'language', { value: 'es-MX', configurable: true });
  setLanguage('es');
  resetCostStore();
  setDesign(100, 100, 3600);
});

afterEach(async () => {
  await cleanup();
  act(() => useCostStore.getState().closeDialog());
  vi.unstubAllGlobals();
});

const open = async () => {
  const m = await mount(<CostDialog />);
  await act(async () => commands.run('costs.open'));
  return m;
};

const dialog = () => document.querySelector<HTMLElement>('[role="dialog"]')!;
const byLabel = (text: string): HTMLInputElement | HTMLSelectElement => {
  const label = [...dialog().querySelectorAll('label')].find((l) => l.textContent === text);
  if (!label) throw new Error(`No hay un campo «${text}»`);
  return dialog().querySelector<HTMLInputElement>(`#${CSS.escape(label.htmlFor)}`)!;
};
const price = () => dialog().querySelector('[data-testid="cost-price"]')!.textContent;
const cost = () => dialog().querySelector('[data-testid="cost-cost"]')!.textContent;

async function type(label: string, value: string) {
  const input = byLabel(label) as HTMLInputElement;
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await act(async () => {
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  });
}

async function choose(label: string, value: string) {
  const select = byLabel(label) as HTMLSelectElement;
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value')!.set!.call(select, value);
    select.dispatchEvent(new Event('change', { bubbles: true }));
  });
}

describe('diálogo «Costos y precio»', () => {
  it('el comando lo abre, con título y moneda de la región', async () => {
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    await open();
    expect(dialog().textContent).toContain('Costos y precio');
    expect(byLabel('Moneda').value).toBe('MXN');
    expect(price()).toBe('$0.00');
  });

  it('calcula con los datos escritos y se actualiza al cambiar el diseño', async () => {
    await open();
    await type('Costo de la máquina por hora', '60');
    await type('Precio de la hoja', '120');
    await type('Desperdicio', '0');
    await type('Margen de ganancia', '30');
    await type('Comisión de la plataforma', '10');
    // 1 h de máquina (60) + caja de 100 × 100 mm sobre una hoja de 600 × 400 mm a 120 (5) = 65
    expect(cost()).toBe('$65.00');
    expect(price()).toBe('$93.89'); // 65 × 1,3 ÷ 0,9 = 93,888...

    setDesign(200, 100, 7200); // el doble de tiempo y de área
    expect(cost()).toBe('$130.00'); // 120 + 10
    expect(price()).toBe('$187.78');
  });

  it('la cantidad reparte la preparación entre las piezas', async () => {
    await open();
    await type('Tarifa de mano de obra por hora', '120');
    await type('Preparación del lote', '60');
    expect(cost()).toBe('$120.00');
    await type('Cantidad a vender', '10');
    expect(cost()).toBe('$12.00');
  });

  it('el margen sobre el precio y un redondeo hacia arriba', async () => {
    await open();
    await type('Costo de la máquina por hora', '100');
    await type('Margen de ganancia', '20');
    await type('Comisión de la plataforma', '10');
    await choose('El margen se calcula sobre', 'price');
    expect(price()).toBe('$142.86'); // 100 ÷ 0,7
    await choose('Redondear el precio hacia arriba a', '5');
    expect(price()).toBe('$145.00');
  });

  it('avisa cuando comisión y margen no dejan precio posible', async () => {
    await open();
    await type('Comisión de la plataforma', '70');
    await type('Impuestos', '30');
    expect(price()).toBe('No hay precio posible');
    expect(dialog().textContent).toContain('suman 100 % o más');
  });

  it('muestra la fórmula con los números de ahora', async () => {
    await open();
    await type('Costo de la máquina por hora', '60');
    await type('Margen de ganancia', '30');
    await type('Comisión de la plataforma', '10');
    expect(dialog().querySelector('.cost__formula')!.textContent).toContain('$60.00 × (1 + 0.3) ÷ (1 − 0.1 − 0) = 86.6667 → $86.67');
  });

  it('los valores de la máquina se recuerdan: cerrar y abrir los conserva', async () => {
    await open();
    await type('Costo de la máquina por hora', '75');
    await type('Consumo de la máquina', '150');
    await act(async () => useCostStore.getState().closeDialog());
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    resetCostStore(); // como si fuera otra sesión: se vuelve a leer del almacenamiento
    await act(async () => commands.run('costs.open'));
    expect((byLabel('Costo de la máquina por hora') as HTMLInputElement).value).toBe('75');
    expect((byLabel('Consumo de la máquina') as HTMLInputElement).value).toBe('150');
  });

  it('los valores son por máquina: otra máquina empieza en cero', async () => {
    await open();
    await type('Costo de la máquina por hora', '75');
    const first = selectActiveDevice(useStore.getState());
    const other = { ...first, id: 'otra-maquina', name: 'Otra' };
    await act(async () => useStore.setState({ devices: [other], activeDeviceId: other.id }));
    expect((byLabel('Costo de la máquina por hora') as HTMLInputElement).value).toBe('0');
    await act(async () => useStore.setState({ devices: [first], activeDeviceId: first.id }));
    expect((byLabel('Costo de la máquina por hora') as HTMLInputElement).value).toBe('75');
  });

  it('guarda un perfil de material y lo recupera', async () => {
    await open();
    await type('Precio de la hoja', '90');
    await type('Nombre para guardar', 'Triplay 3 mm');
    const save = [...dialog().querySelectorAll('button')].find((b) => b.textContent === 'Guardar perfil')!;
    await click(save);
    expect([...(byLabel('Perfil de material') as HTMLSelectElement).options].map((o) => o.value)).toEqual(['', 'Triplay 3 mm']);
    await type('Precio de la hoja', '5');
    await choose('Perfil de material', '');
    await choose('Perfil de material', 'Triplay 3 mm');
    expect((byLabel('Precio de la hoja') as HTMLInputElement).value).toBe('90');
  });

  it('copia el desglose al portapapeles', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { ...navigator, language: 'es-MX', clipboard: { writeText } });
    await open();
    await type('Costo de la máquina por hora', '60');
    const copy = [...dialog().querySelectorAll('button')].find((b) => b.textContent === 'Copiar desglose')!;
    await click(copy);
    expect(writeText).toHaveBeenCalledOnce();
    expect(writeText.mock.calls[0]![0]).toContain('Costo por pieza');
    const csv = [...dialog().querySelectorAll('button')].find((b) => b.textContent === 'Copiar CSV')!;
    await click(csv);
    expect(writeText.mock.calls[1]![0]).toMatch(/^Sección,Concepto,Valor,Unidad\r\n/);
  });

  it('en inglés los textos y la moneda cambian', async () => {
    Object.defineProperty(navigator, 'language', { value: 'en-US', configurable: true });
    setLanguage('en');
    resetCostStore();
    await open();
    expect(dialog().textContent).toContain('Cost and price');
    expect(byLabel('Currency').value).toBe('USD');
    expect(price()).toBe('$0.00');
  });

  it('pasa la auditoría de accesibilidad (axe) en español e inglés', async () => {
    await open();
    await type('Costo de la máquina por hora', '60');
    await type('Comisión de la plataforma', '70');
    await type('Impuestos', '40'); // con el aviso de precio imposible visible
    let res = await audit(dialog());
    expect(res.violations, describeViolations(res).join('\n')).toEqual([]);

    await type('Comisión de la plataforma', '10');
    await type('Impuestos', '0');
    res = await audit(dialog());
    expect(res.violations, describeViolations(res).join('\n')).toEqual([]);

    await act(async () => useCostStore.getState().closeDialog());
    setLanguage('en');
    await act(async () => commands.run('costs.open'));
    res = await audit(dialog());
    expect(res.violations, describeViolations(res).join('\n')).toEqual([]);
  });
});
