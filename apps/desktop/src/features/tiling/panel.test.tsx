import type { ReactElement } from 'react';
import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { tilingEnabled } from '@/commands/definitions/tiling';
import { commands } from '@/commands';
import { DeviceFeederFields } from '@/features/machine/DeviceFeeder';
import { rectObject } from '@/features/nest/testUtils';
import { FALLBACK_DEVICE } from '@/lib/device';
import { factoryLayer } from '@/lib/layers';
import { setLanguage } from '@/i18n';
import { selectActiveDevice, useStore } from '@/store';
import { TilingHost } from './index';
import { selectSceneTiles } from './overlay';
import { newSession, mark } from './session';
import { defaultConfig, openTiling, requestPlan, useTilingUi } from './store';
import { TilingOffer } from './TilingOffer';
import { TilingPanel } from './TilingPanel';

const BED: [number, number] = [60, 40];

/** Espera a que el motor (aquí la maqueta) responda con el plan de las opciones de ahora. */
const planned = () => vi.waitFor(() => expect(useTilingUi.getState().planning).toBe(false));

/**
 * `renderToString` lee el estado inicial de los stores (zustand usa `getInitialState` en el
 * servidor), no el actual: se copia el estado de ahora a ese objeto mientras se pinta.
 */
function ssr(node: ReactElement): string {
  const stores = [useStore, useTilingUi] as unknown as { getInitialState: () => Record<string, unknown>; getState: () => Record<string, unknown> }[];
  const saved = stores.map((s) => ({ ...s.getInitialState() }));
  stores.forEach((s) => Object.assign(s.getInitialState(), s.getState()));
  try {
    return renderToString(node);
  } finally {
    stores.forEach((s, i) => Object.assign(s.getInitialState(), saved[i]));
  }
}

function setDoc(w: number, h: number, feeder = false) {
  const device = { ...selectActiveDevice(useStore.getState()), bed: BED, feeder: { enabled: feeder, feed_gcode: 'G91\nG0 Y{mm}\nG90', feed_mm: 300 } };
  useStore.setState({
    devices: [device],
    activeDeviceId: device.id,
    doc: { version: 2, device_id: null, layers: [factoryLayer('#ff0000', 'Rojo')], objects: w > 0 ? [rectObject('a', w, h)] : [] },
    cutSelectionOnly: false,
    selectedIds: [],
  });
}

beforeEach(() => {
  setLanguage('es');
  useTilingUi.setState({ open: false, phase: 'config', session: null, saved: null, message: null, plan: null, planning: false, design: null });
});

describe('aviso «no cabe» y comando', () => {
  it('ofrece dividir solo si el diseño es más grande que la cama', () => {
    setDoc(100, 30);
    const out = ssr(<TilingOffer />);
    expect(out).toContain('no cabe de una vez');
    expect(out).toContain('Dividir en tramos…');
    setDoc(50, 30);
    expect(ssr(<TilingOffer />)).toBe('');
  });

  it('el comando existe, está en el menú Máquina y exige algo que enviar', () => {
    setDoc(0, 0);
    expect(tilingEnabled(useStore.getState())).toBe('tiling.reason.nothing');
    setDoc(100, 30);
    expect(tilingEnabled(useStore.getState())).toBe(true);
    expect(commands.get('machine.tiling')).toBeDefined();
  });
});

describe('asistente', () => {
  it('cerrado no pinta nada; abierto muestra las opciones y el plan', async () => {
    setDoc(100, 60);
    expect(ssr(<TilingHost />)).toBe('');
    openTiling();
    await planned();
    useTilingUi.getState().patchConfig({ size: [55, 35], overlap: 10, margin: 0 });
    await planned();
    const out = ssr(<TilingHost />);
    for (const s of ['Dividir en tramos', 'Ancho del tramo', 'Alto del tramo', 'Solape', 'Margen a la cama', 'Por filas', 'Serpentina', 'Marcas de alineación en el solape', '4 tramos (2 columnas × 2 filas)', 'Empezar']) {
      expect(out).toContain(s);
    }
    expect(out.match(/til-preview__tile--preview/g)).toHaveLength(4);
  });

  it('el tamaño por defecto es la cama menos el margen', async () => {
    setDoc(100, 60);
    openTiling();
    await planned();
    const { config } = useTilingUi.getState();
    expect(config.size).toEqual([BED[0] - 2 * config.margin, BED[1] - 2 * config.margin]);
  });

  it('un tramo que no cabe en la cama bloquea el inicio con su motivo', async () => {
    setDoc(100, 60);
    openTiling();
    await planned();
    useTilingUi.getState().patchConfig({ size: [80, 35] });
    await planned();
    const out = ssr(<TilingPanel />);
    expect(out).toContain('no cabe en la cama');
    expect(out).toMatch(/<button[^>]*disabled=""[^>]*>Empezar/);
  });

  it('un solape mayor que el tramo se explica', async () => {
    setDoc(100, 60);
    openTiling();
    await planned();
    useTilingUi.getState().patchConfig({ size: [50, 30], overlap: 50 });
    await planned();
    expect(ssr(<TilingPanel />)).toContain('El solape debe ser menor que el tramo.');
  });

  it('con alimentador y una columna anuncia el avance automático; con varias columnas, no', async () => {
    setDoc(50, 150, true);
    openTiling();
    await planned();
    useTilingUi.getState().patchConfig({ size: [55, 35], overlap: 10, margin: 0 });
    await planned();
    expect(ssr(<TilingPanel />)).toContain('el material avanza solo (25');
    setDoc(100, 60, true);
    openTiling();
    await planned();
    useTilingUi.getState().patchConfig({ size: [55, 35], overlap: 10, margin: 0 });
    await planned();
    expect(ssr(<TilingPanel />)).toContain('solo se usa cuando los tramos van en una sola dirección');
  });

  it('ofrece retomar un avance guardado', async () => {
    setDoc(100, 60);
    openTiling();
    await planned();
    const { plan, design, config } = useTilingUi.getState();
    if (!plan?.ok || !design) throw new Error('sin plan');
    const specs = plan.plan.tiles.map((p) => p.spec);
    const saved = mark(newSession(specs, design, config, 'dev', false), 0, 'done');
    useTilingUi.setState({ saved });
    const out = ssr(<TilingPanel />);
    expect(out).toContain('trabajo por tramos sin terminar');
    expect(out).toContain('Retomar');
  });
});

describe('pasos del envío', async () => {
  async function session(phase: 'place' | 'sending' | 'feeding' | 'failed' | 'finished', over = {}) {
    setDoc(100, 60);
    const config = { ...defaultConfig(BED), size: [55, 35] as [number, number], overlap: 10, margin: 0 };
    const design: [[number, number], [number, number]] = [[0, 0], [100, 60]];
    const plan = await requestPlan(design, config);
    if (!plan.ok) throw new Error('sin plan');
    const s = newSession(plan.plan.tiles.map((p) => p.spec), design, config, 'dev', false);
    useTilingUi.setState({ open: true, phase, config, design, plan, session: { ...s, ...over }, message: null });
  }

  it('place: «Coloca el material así…» con el tramo, enviar y saltar', async () => {
    await session('place');
    const out = ssr(<TilingPanel />);
    expect(out).toContain('Tramo 1 de 4: coloca el material así…');
    expect(out).toContain('Enviar tramo 1');
    expect(out).toContain('Saltar tramo 1');
    expect(out).toContain('0 enviados, 0 saltados, 4 en total');
  });

  it('place tras un tramo hecho recuerda alinear las cruces', async () => {
    await session('place');
    const s0 = useTilingUi.getState().session!;
    await session('place', { status: mark(s0, 0, 'done').status, current: 1 });
    expect(ssr(<TilingPanel />)).toContain('Alinea las cruces');
  });

  it('sending, feeding, failed y finished', async () => {
    await session('sending');
    expect(ssr(<TilingPanel />)).toContain('Enviando tramo 1 de 4…');
    await session('feeding');
    expect(ssr(<TilingPanel />)).toContain('Avanzando el material…');
    await session('failed');
    useTilingUi.setState({ message: 'alarma activa' });
    const failed = ssr(<TilingPanel />);
    expect(failed).toContain('El tramo 1 no salió');
    expect(failed).toContain('alarma activa');
    expect(failed).toContain('Reintentar tramo 1');
    await session('finished', { status: ['done', 'done', 'skipped', 'done'] });
    expect(ssr(<TilingPanel />)).toContain('Se enviaron 3 tramos y se saltaron 1.');
  });

  it('en inglés se traduce', async () => {
    setLanguage('en');
    await session('place');
    const out = ssr(<TilingPanel />);
    expect(out).toContain('Tile 1 of 4: place the material like this…');
    expect(out).toContain('Send tile 1');
  });
});

describe('lienzo', () => {
  it('con el asistente cerrado no hay tramos; abierto, los del plan numerados', async () => {
    setDoc(100, 60);
    expect(selectSceneTiles()).toBeNull();
    openTiling();
    await planned();
    useTilingUi.getState().patchConfig({ size: [55, 35], overlap: 10, margin: 0 });
    await planned();
    const scene = selectSceneTiles();
    expect(scene?.tiles.map((t) => [t.label, t.state])).toEqual([[1, 'preview'], [2, 'preview'], [3, 'preview'], [4, 'preview']]);
    // Memoizado: sin cambios devuelve el mismo objeto.
    expect(selectSceneTiles()).toBe(scene);
  });

  it('durante la sesión marca el actual y el avance', async () => {
    setDoc(100, 60);
    openTiling();
    await planned();
    useTilingUi.getState().patchConfig({ size: [55, 35], overlap: 10, margin: 0 });
    await planned();
    const { plan, design, config } = useTilingUi.getState();
    if (!plan?.ok || !design) throw new Error('sin plan');
    const s = mark(newSession(plan.plan.tiles.map((p) => p.spec), design, config, 'dev', false), 0, 'done');
    useTilingUi.setState({ phase: 'place', session: { ...s, current: 1 } });
    const scene = selectSceneTiles();
    expect(scene?.tiles.map((t) => [t.state, t.current])).toEqual([['done', false], ['pending', true], ['pending', false], ['pending', false]]);
  });
});

describe('sección del alimentador en el perfil', async () => {
  const draft = (feeder: Partial<typeof FALLBACK_DEVICE.feeder>) => ({ ...FALLBACK_DEVICE, feeder: { ...FALLBACK_DEVICE.feeder, ...feeder } });
  const noop = () => {};

  it('apagado solo muestra la casilla con su ayuda', () => {
    const html = renderToString(<DeviceFeederFields draft={draft({ enabled: false })} edit={noop} />);
    expect(html).toContain('Tengo un alimentador automático');
    expect(html).toContain('Cinta o rodillos');
    expect(html).not.toContain('G-code de avance');
  });

  it('activo muestra el G-code con {mm}, el avance por defecto y una vista previa', () => {
    const html = renderToString(<DeviceFeederFields draft={draft({ enabled: true, feed_mm: 120 })} edit={noop} />);
    for (const s of ['G-code de avance', 'Avance por defecto', 'se sustituye por la distancia', 'G91 / G0 Y120 / G90']) expect(html).toContain(s);
  });

  it('los errores se muestran junto a cada campo', () => {
    const html = renderToString(<DeviceFeederFields draft={draft({ enabled: true, feed_gcode: 'G0 Y10', feed_mm: 0 })} edit={noop} />);
    expect(html).toContain('Falta {mm}');
    expect(html).toContain('El avance debe ser un número mayor que cero.');
  });
});
