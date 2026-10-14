import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi, type MockApi } from '@/api/mock';
import { MOCK_GALVO_ACK_REQUIRED } from '@/api/mock/galvo';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import type { GalvoState } from '@/api/types';
import { DEFAULT_GALVO_SETUP, GalvoSetupField, VerifyResult, applySetup, setupPort, verifySetup } from '@/features/devices/GalvoSetupField';
import { setLanguage } from '@/i18n';
import { FALLBACK_DEVICE } from '@/lib/device';
import { bindMachineEvents, createAppStore } from '@/store';
import { GalvoFields } from './GalvoFields';
import { ADJUST_STEPS_MM, FRAME_SHAPES, GalvoFrameControls, nudge, NO_ADJUST, parseLoops } from './GalvoFrame';
import { GalvoInterlockEditor, requirementOf, rulesOf } from './GalvoInterlock';
import { GalvoInterlockStatus } from './GalvoSection';
import { galvoDevice, ssr } from './galvoFixtures';
import { validateGalvoProfile } from './galvoProfile';

const noop = () => {};
const memory = new Map<string, string>();

beforeEach(() => {
  setLanguage('es');
  setApi(createMockApi({ latencyMs: 0 }));
});

describe('encuadre del galvo: formas, vueltas y ajuste en vivo (M42)', () => {
  it('ofrece rectángulo, trazos, casco convexo y objetos sueltos', () => {
    expect(FRAME_SHAPES).toEqual(['rect', 'contour', 'hull', 'objects']);
    const h = renderToString(<GalvoFrameControls state={{ framing: false }} loops={0} blocked={null} onFrame={noop} onAdjust={noop} />);
    for (const text of ['Rectángulo', 'Contorno', 'Casco convexo', 'Objetos sueltos', 'Vueltas', 'Encuadrar', 'Mover y escalar en vivo']) {
      expect(h).toContain(text);
    }
  });

  it('sin encuadre en curso el ajuste en vivo está apagado y lo dice', () => {
    const h = renderToString(<GalvoFrameControls state={{ framing: false }} loops={0} blocked={null} onFrame={noop} onAdjust={noop} />);
    expect(h).toContain('Se activa mientras hay un encuadre en curso.');
    expect(h.match(/<button[^>]*aria-label="(Izquierda|Derecha|Arriba|Abajo|Más grande|Más chico)"[^>]*disabled=""|<button[^>]*disabled=""[^>]*aria-label="(Izquierda|Derecha|Arriba|Abajo|Más grande|Más chico)"/g)?.length).toBe(6);
  });

  it('con un encuadre en curso: contador de vueltas y ajuste habilitado', () => {
    const h = renderToString(<GalvoFrameControls state={{ framing: true }} loops={3} blocked={null} onFrame={noop} onAdjust={noop} />);
    expect(h).toContain('Vuelta 3');
    expect(h).not.toContain('Se activa mientras hay un encuadre en curso.');
    // «Encuadrar» no se puede pulsar dos veces.
    expect(h).toMatch(/<button[^>]*btn--accent[^>]*disabled=""/);
    expect(h).not.toMatch(/aria-label="Izquierda"[^>]*disabled=""/);
  });

  it('si no se puede encuadrar, el botón lo dice y el ajuste no corre', () => {
    const h = renderToString(<GalvoFrameControls state={{ framing: true }} loops={1} blocked="No hay nada que enviar." onFrame={noop} onAdjust={noop} />);
    expect(h).toContain('title="No hay nada que enviar."');
    expect(h).toMatch(/aria-label="Izquierda"[^>]*disabled=""|disabled=""[^>]*aria-label="Izquierda"/);
  });

  it('el ajuste rechazado por salirse del campo se avisa', () => {
    const on = renderToString(<GalvoFrameControls state={{ framing: true }} loops={1} blocked={null} adjustRejected onFrame={noop} onAdjust={noop} />);
    expect(on).toContain('se descartó y sigue como estaba');
    const off = renderToString(<GalvoFrameControls state={{ framing: false }} loops={1} blocked={null} adjustRejected onFrame={noop} onAdjust={noop} />);
    expect(off).not.toContain('se descartó');
  });

  it('las vueltas: vacío es hasta detener; solo enteros de 1 a 9999', () => {
    expect(parseLoops('')).toBeNull();
    expect(parseLoops('  ')).toBeNull();
    expect(parseLoops('3')).toBe(3);
    for (const bad of ['0', '-1', '2.5', 'abc', '10000']) expect(parseLoops(bad)).toBe('invalid');
  });

  it('mover y escalar suman pasos, multiplican la escala y respetan los topes del motor', () => {
    expect(ADJUST_STEPS_MM).toContain(1);
    expect(nudge(NO_ADJUST, { sx: 1 }, 5)).toEqual({ dx: 5, dy: 0, scale: 1 });
    expect(nudge({ dx: 5, dy: 0, scale: 1 }, { sy: -1 }, 0.5)).toEqual({ dx: 5, dy: -0.5, scale: 1 });
    expect(nudge(NO_ADJUST, { factor: 1.1 }, 1)?.scale).toBe(1.1);
    expect(nudge({ dx: 0, dy: 0, scale: 0.05 }, { factor: 1 / 1.1 }, 1)).toBeNull();
    expect(nudge({ dx: 0, dy: 0, scale: 19.9 }, { factor: 1.1 }, 1)).toBeNull();
    expect(nudge({ dx: 999, dy: 0, scale: 1 }, { sx: 1 }, 5)).toBeNull();
  });
});

describe('enclavamiento por entrada (M45)', () => {
  it('las reglas por entrada se convierten en máscara y valor, y vuelven', () => {
    expect(requirementOf(rulesOf(null))).toBeNull();
    const rules = rulesOf(null);
    rules[3] = 'high';
    rules[5] = 'low';
    const req = requirementOf(rules);
    expect(req).toEqual({ mask: 0b101000, value: 0b001000 });
    expect(rulesOf(req)[3]).toBe('high');
    expect(rulesOf(req)[5]).toBe('low');
    expect(rulesOf(req).filter((r) => r === 'any')).toHaveLength(14);
    // El valor nunca trae bits fuera de la máscara.
    expect(req && (req.value & ~req.mask) === 0).toBe(true);
  });

  it('el editor muestra las 16 entradas y el resumen de lo que se exige', () => {
    const empty = renderToString(<GalvoInterlockEditor value={null} onChange={noop} />);
    expect((empty.match(/<select/g) ?? []).length).toBe(16);
    expect(empty).toContain('Sin enclavamiento');
    expect(empty).not.toContain('Quitar el enclavamiento');
    const set = renderToString(<GalvoInterlockEditor value={{ mask: 8, value: 8 }} onChange={noop} />);
    expect(set).toContain('(entradas &amp; 0x0008) = 0x0008');
    expect(set).toContain('Quitar el enclavamiento');
    expect(set).toContain('Debe estar alta');
  });

  it('el panel dice si se cumple, si está abierto o que no hay', () => {
    const base = { pointer_offset: [0, 0] as [number, number], overscan_mm: 0 };
    expect(renderToString(<GalvoInterlockStatus state={{ ...base, interlock: { mask: 8, value: 8, met: true } }} />)).toContain('Cumplido');
    const open = renderToString(<GalvoInterlockStatus state={{ ...base, interlock: { mask: 8, value: 8, met: false } }} />);
    expect(open).toContain('ABIERTO');
    expect(open).toContain('role="alert"');
    expect(renderToString(<GalvoInterlockStatus state={{ ...base, interlock: null }} />)).toContain('no exige ninguna entrada');
  });

  it('el panel muestra el desfase del puntero y el overscan solo si valen algo', () => {
    const none = renderToString(<GalvoInterlockStatus state={{ interlock: null, pointer_offset: [0, 0], overscan_mm: 0 }} />);
    expect(none).not.toContain('Desfase del puntero');
    expect(none).not.toContain('Overscan');
    const some = renderToString(<GalvoInterlockStatus state={{ interlock: null, pointer_offset: [1.5, -2], overscan_mm: 2 } as Pick<GalvoState, 'interlock' | 'pointer_offset' | 'overscan_mm'>} />);
    expect(some).toContain('Desfase del puntero: X 1,50 · Y -2,00 mm');
    expect(some).toContain('Overscan de rellenos e imágenes: 2,00 mm');
  });
});

describe('perfil galvo: overscan, desfase del puntero y enclavamiento', () => {
  it('el perfil de partida es válido', () => {
    expect(validateGalvoProfile(galvoDevice())).toEqual({});
  });

  it('el overscan, el desfase y el enclavamiento se validan como el motor', () => {
    const d = galvoDevice();
    expect(validateGalvoProfile({ ...d, galvo: { ...d.galvo, overscan_mm: 20 } })).toEqual({});
    expect(validateGalvoProfile({ ...d, galvo: { ...d.galvo, overscan_mm: 21 } }).overscan).toBe('galvo.err.overscan');
    expect(validateGalvoProfile({ ...d, galvo: { ...d.galvo, overscan_mm: -1 } }).overscan).toBe('galvo.err.overscan');
    expect(validateGalvoProfile({ ...d, pointer_offset: [50, -50] })).toEqual({});
    expect(validateGalvoProfile({ ...d, pointer_offset: [51, 0] }).pointer_offset).toBe('galvo.err.pointerOffset');
    expect(validateGalvoProfile({ ...d, pointer_offset: [Number.NaN, 0] }).pointer_offset).toBe('galvo.err.pointerOffset');
    expect(validateGalvoProfile({ ...d, galvo: { ...d.galvo, require_input: { mask: 1, value: 1 } } })).toEqual({});
    expect(validateGalvoProfile({ ...d, galvo: { ...d.galvo, require_input: { mask: 1, value: 2 } } }).interlock).toBe('galvo.err.interlock');
  });

  it('los campos del perfil traen overscan, desfase del puntero y enclavamiento', () => {
    const d = galvoDevice({ pointer_offset: [1, 2] });
    const h = ssr(<GalvoFields draft={d} edit={noop} issues={{}} />);
    for (const text of ['Overscan de rellenos e imágenes', 'Desfase del puntero rojo', 'Enclavamiento por entrada', 'Entrada 15']) {
      expect(h).toContain(text);
    }
    const bad = ssr(<GalvoFields draft={d} edit={noop} issues={{ overscan: 'galvo.err.overscan', interlock: 'galvo.err.interlock' }} />);
    expect(bad).toContain('entre 0 y 20 mm');
    expect(bad).toContain('bits dentro de su máscara');
  });

  it('en inglés', () => {
    setLanguage('en');
    const h = ssr(<GalvoFields draft={galvoDevice()} edit={noop} issues={{}} />);
    expect(h).toContain('Fill and image overscan');
    expect(h).toContain('Red pointer offset');
    expect(h).toContain('Input interlock');
    const f = renderToString(<GalvoFrameControls state={{ framing: false }} loops={0} blocked={null} onFrame={noop} onAdjust={noop} />);
    expect(f).toContain('Convex hull');
    expect(f).toContain('Loose objects');
    expect(f).not.toContain('Casco convexo');
  });
});

describe('asistente de alta del galvo (M41)', () => {
  const profile = galvoDevice();

  it('tiene cuatro pasos: lente, campo, tarjeta y verificación sin láser', () => {
    const h = renderToString(<GalvoSetupField setup={DEFAULT_GALVO_SETUP} onSetup={noop} profile={profile} boards={['usb:galvo:0']} />);
    for (const text of ['1 · Lente y fuente', '2 · Campo de la lente', '3 · Tarjeta', '4 · Verificación sin láser', '1064 nm', 'Galvo · tarjeta USB 1 · 9588:9899', 'Tarjeta simulada (sin hardware)', 'Verificar sin láser']) {
      expect(h).toContain(text);
    }
    expect(h).toContain('No mueve los espejos');
    // Sin el perfil (o sin tarjetas USB) la parte sigue: la verificación no se puede lanzar sin perfil.
    expect(renderToString(<GalvoSetupField setup={DEFAULT_GALVO_SETUP} onSetup={noop} />)).toMatch(/<button[^>]*disabled=""[^>]*>Verificar sin láser/);
    expect(renderToString(<GalvoSetupField setup={DEFAULT_GALVO_SETUP} onSetup={noop} profile={profile} />)).toContain('No se encontró ninguna tarjeta por USB');
  });

  it('la tarjeta elegida: la que se pidió, si no la primera USB y si no la simulada', () => {
    expect(setupPort(DEFAULT_GALVO_SETUP, [])).toBe('virtual:galvo');
    expect(setupPort(DEFAULT_GALVO_SETUP, ['usb:galvo:1'])).toBe('usb:galvo:1');
    expect(setupPort({ ...DEFAULT_GALVO_SETUP, port: 'virtual:galvo' }, ['usb:galvo:1'])).toBe('virtual:galvo');
    // Una tarjeta que ya no está no se usa.
    expect(setupPort({ ...DEFAULT_GALVO_SETUP, port: 'usb:galvo:7' }, ['usb:galvo:1'])).toBe('usb:galvo:1');
  });

  it('el perfil verificado lleva la lente, la fuente y la cama del asistente', () => {
    const p = applySetup(profile, { field_mm: 150, source: 'uv' });
    expect(p.bed).toEqual([150, 150]);
    expect(p.galvo.field_mm).toBe(150);
    expect(p.galvo.source).toBe('uv');
  });

  it('con la tarjeta simulada verifica sin pedir la confirmación de seguridad', async () => {
    const requestAck = vi.fn(async () => null);
    const r = await verifySetup(profile, DEFAULT_GALVO_SETUP, [], requestAck);
    expect(requestAck).not.toHaveBeenCalled();
    expect(r.status).toBe('done');
    if (r.status === 'done') {
      expect(r.report.checks.every((c) => c.ok)).toBe(true);
      expect(r.report.virtual_machine).toBe(true);
    }
  });

  it('una tarjeta USB pide la confirmación en CADA verificación y, si se cancela, no se abre', async () => {
    const api = createMockApi({ latencyMs: 0 });
    setApi(api);
    const spy = vi.spyOn(api, 'galvoVerify');
    const cancelled = await verifySetup(profile, DEFAULT_GALVO_SETUP, ['usb:galvo:0'], async () => null);
    expect(cancelled.status).toBe('cancelled');
    expect(spy).not.toHaveBeenCalled();
    const full = { eyewear: true, enclosure: true, emergency_stop: true };
    const ask = vi.fn(async () => full);
    const a = await verifySetup(profile, DEFAULT_GALVO_SETUP, ['usb:galvo:0'], ask);
    const b = await verifySetup(profile, DEFAULT_GALVO_SETUP, ['usb:galvo:0'], ask);
    expect([a.status, b.status]).toEqual(['done', 'done']);
    expect(ask).toHaveBeenCalledTimes(2);
    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy.mock.calls[0]![0].galvo_ack).toEqual(full);
  });

  it('el motor (simulado) rechaza una tarjeta USB sin las tres confirmaciones', async () => {
    const api = createMockApi({ latencyMs: 0 });
    await expect(api.galvoVerify({ device: profile, port: 'usb:galvo:0', galvo_ack: null })).rejects.toThrow(MOCK_GALVO_ACK_REQUIRED);
    await expect(api.galvoVerify({ device: profile, port: 'usb:galvo:0', galvo_ack: { eyewear: true, enclosure: true, emergency_stop: false } })).rejects.toThrow(MOCK_GALVO_ACK_REQUIRED);
    await expect(api.galvoVerify({ device: profile, port: '/dev/ttyUSB0' })).rejects.toThrow(/no es un puerto galvo/);
    const r = await verifySetup(profile, { ...DEFAULT_GALVO_SETUP, port: 'virtual:galvo' }, [], async () => null);
    expect(r.status).toBe('done');
  });

  it('el resultado dice cada comprobación y si todo salió bien', () => {
    const ok = renderToString(
      <VerifyResult
        report={{
          board: 'LMCV4-FIBER-M',
          vendor_id: 0x9588,
          product_id: 0x9899,
          virtual_machine: true,
          serial: '0001-0002-0003-0004',
          version: '0000-0102-0000-0020',
          status_raw: 0x20,
          busy: false,
          ready: true,
          inputs: 0,
          interlock: null,
          checks: [{ id: 'board', ok: true, detail: 'LMCV4-FIBER-M (USB 9588:9899)' }],
        }}
      />,
    );
    expect(ok).toContain('Tarjeta reconocida');
    expect(ok).toContain('9588:9899');
    expect(ok).toContain('nada se movió ni se encendió');
    const bad = renderToString(
      <VerifyResult
        report={{ board: 'X', vendor_id: 0x9588, product_id: 0x9899, virtual_machine: false, serial: 's', version: 'v', status_raw: 0, busy: false, ready: true, inputs: 0, interlock: { mask: 1, value: 1, met: false }, checks: [{ id: 'interlock', ok: false, detail: 'abierto' }] }}
      />,
    );
    expect(bad).toContain('Enclavamiento por entrada');
    expect(bad).toContain('Hay comprobaciones fallidas');
    expect(bad).toContain('role="alert"');
  });
});

describe('acciones del almacén con el galvo simulado', () => {
  async function connected() {
    memory.clear();
    vi.stubGlobal('localStorage', {
      getItem: (k: string) => memory.get(k) ?? null,
      setItem: (k: string, v: string) => void memory.set(k, v),
    });
    const api: MockApi = createMockApi({ latencyMs: 0, statusIntervalMs: 20, minJobMs: 5_000, jobSpeedup: 1 });
    setApi(api);
    const store = createAppStore();
    const device = { ...structuredClone(FALLBACK_DEVICE), id: 'galvo-1', name: 'Mi galvo', kind: 'galvo' as const, bed: [110, 110] as [number, number], origin: 'bottom_left' as const };
    await store.getState().loadDevices();
    await store.getState().saveDevice(device);
    store.getState().setActiveDevice(device.id);
    await bindMachineEvents(api, store);
    await store.getState().refreshPorts();
    store.getState().applyImport({ objects: sampleObjects(), new_layers: sampleLayers(), warnings: [] });
    store.getState().setSelectedPort('virtual:galvo');
    await store.getState().connect();
    await vi.waitFor(() => expect(store.getState().connection.connected).toBe(true));
    await vi.waitFor(() => expect(store.getState().status).not.toBeNull());
    return { api, store };
  }

  it('encuadra la forma pedida con sus vueltas y deja mover y escalar en vivo', async () => {
    const { api, store } = await connected();
    const spy = vi.spyOn(api, 'galvoFrame');
    // Sin encuadre en curso no hay nada que ajustar.
    expect(await store.getState().adjustGalvoFrame({ dx: 1, dy: 0, scale: 1 })).toBe(false);
    await store.getState().galvoFrame('hull', 3);
    expect(spy.mock.calls[0]![0]).toMatchObject({ shape: 'hull', loops: 3 });
    await vi.waitFor(async () => expect((await api.galvoState()).framing).toBe(true));
    expect(store.getState().framedMark).not.toBeNull();
    expect(await store.getState().adjustGalvoFrame({ dx: 5, dy: -2, scale: 1.1 })).toBe(true);
    // Fuera de los topes del motor: se rechaza y no cambia nada.
    expect(await store.getState().adjustGalvoFrame({ dx: 5, dy: -2, scale: 100 })).toBe(false);
    expect(await store.getState().adjustGalvoFrame({ dx: Number.NaN, dy: 0, scale: 1 })).toBe(false);
  });

  it('una holgura o unas vueltas inválidas llegan como error, no como encuadre', async () => {
    const { api, store } = await connected();
    await expect(api.galvoFrame({ ...(await import('@/store/selectors')).buildJobArgs(store.getState()), shape: 'objects', gap_mm: -1 })).rejects.toThrow(/holgura/);
    await expect(api.galvoFrame({ ...(await import('@/store/selectors')).buildJobArgs(store.getState()), shape: 'rect', loops: 0 })).rejects.toThrow(/al menos 1/);
  });

  it('los eventos de vuelta actualizan el contador y el aviso de ajuste descartado', async () => {
    const { store } = await connected();
    store.getState().onGalvo({ kind: 'light', loops: 4, adjust_gen: 2, adjust_rejected: false });
    expect(store.getState()).toMatchObject({ galvoLoops: 4, galvoAdjustGen: 2, galvoAdjustRejected: false });
    store.getState().onGalvo({ kind: 'light', loops: 5, adjust_gen: 2, adjust_rejected: true });
    expect(store.getState().galvoAdjustRejected).toBe(true);
    store.getState().onGalvoConnection({ connected: false, port: null, error: null });
    expect(store.getState()).toMatchObject({ galvoLoops: 0, galvoAdjustGen: 0, galvoAdjustRejected: false });
  });

  it('el estado del galvo simulado trae el enclavamiento del perfil con el que se conectó', async () => {
    memory.clear();
    const api = createMockApi({ latencyMs: 0 });
    setApi(api);
    const device = { ...galvoDevice(), galvo: { ...galvoDevice().galvo, require_input: { mask: 8, value: 8 }, overscan_mm: 2 }, pointer_offset: [1, -1] as [number, number] };
    await api.connect({ port: 'virtual:galvo', baud: 115200, device });
    const st = await api.galvoState();
    expect(st.interlock).toEqual({ mask: 8, value: 8, met: false });
    expect(st.overscan_mm).toBe(2);
    expect(st.pointer_offset).toEqual([1, -1]);
    expect(st.framing).toBe(false);
  });
});
