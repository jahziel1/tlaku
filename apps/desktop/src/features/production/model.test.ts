import { describe, expect, it } from 'vitest';
import type { ConnectionEvent, MaterialPreset } from '@/api/types';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { makeLayer } from '@/lib/layers';
import { canon, fullReason, layersKey, makeSnapshot, materialKey, originKey, type SkipInput, type StartChecks } from './model';

const connection: ConnectionEvent = { connected: true, port: 'COM3', error: null };
const doc = { layers: sampleLayers(), objects: sampleObjects() };
const options = { start_from: 'absolute', job_origin: 0, user_origin: null, use_selection_origin: false, selection: null, transform: null, tile: null } as const;

const snap = (over: Partial<Parameters<typeof makeSnapshot>[0]> = {}) =>
  makeSnapshot({ connection, deviceId: 'falcon', doc, options, presets: [], ...over });

const CLEAN: StartChecks = { previewWarnings: [], linkWarning: null, seriousFindings: 0, placementOk: true, failed: false };

/** Todo en orden: la única entrada que permite omitir la lista. */
function ok(over: Partial<SkipInput> = {}): SkipInput {
  return {
    enabled: true,
    fromUser: true,
    deviceKind: 'grbl',
    connected: true,
    baseline: snap(),
    now: snap(),
    positionDoubtful: false,
    repeating: false,
    checks: CLEAN,
    ...over,
  };
}

describe('modo producción: cuándo se puede omitir la lista de comprobación', () => {
  it('con TODO en orden se puede omitir', () => {
    expect(fullReason(ok())).toBeNull();
  });

  // Cada condición, por separado, obliga a mostrar el diálogo completo.
  it('apagado de fábrica: sin la preferencia se muestra el completo', () => {
    expect(fullReason(ok({ enabled: false }))).toBe('pref');
  });

  it('si no lo abrió una persona con Iniciar (cola guiada, automatización, modo sencillo), se muestra el completo', () => {
    expect(fullReason(ok({ fromUser: false }))).toBe('notFromUser');
  });

  it('sin un trabajo anterior confirmado con la lista completa y terminado bien, se muestra el completo', () => {
    expect(fullReason(ok({ baseline: null }))).toBe('noBaseline');
  });

  it('sin máquina conectada, se muestra el completo', () => {
    expect(fullReason(ok({ connected: false }))).toBe('noMachine');
  });

  it('otra conexión (reconectar crea otro evento), se muestra el completo', () => {
    expect(fullReason(ok({ now: snap({ connection: { ...connection } }) }))).toBe('connection');
  });

  it('otro puerto, se muestra el completo', () => {
    const other = { ...connection, port: 'COM4' };
    expect(fullReason(ok({ baseline: snap({ connection: other }), now: snap({ connection: other }) }))).toBeNull(); // misma conexión nueva en ambos: ok
    expect(fullReason(ok({ now: snap({ connection: { ...connection, port: 'COM4' } }) }))).toBe('connection');
  });

  it('otra máquina (perfil) con la misma conexión, se muestra el completo', () => {
    expect(fullReason(ok({ now: snap({ deviceId: 'otra' }) }))).toBe('connection');
  });

  it('un ajuste de una capa con salida cambió, se muestra el completo', () => {
    const layers = doc.layers.map((l, i) => (i === 0 ? { ...l, speed: l.speed + 1 } : l));
    expect(fullReason(ok({ now: snap({ doc: { ...doc, layers } }) }))).toBe('layers');
  });

  it('se apagó o se encendió la salida de una capa que se usa, se muestra el completo', () => {
    const layers = doc.layers.map((l, i) => (i === 0 ? { ...l, output: !l.output } : l));
    expect(fullReason(ok({ now: snap({ doc: { ...doc, layers } }) }))).toBe('layers');
  });

  it('el origen del trabajo cambió, se muestra el completo', () => {
    for (const patch of [{ start_from: 'user_origin' as const }, { job_origin: 4 }, { user_origin: [1, 2] as [number, number] }, { use_selection_origin: true, selection: ['a'] }]) {
      expect(fullReason(ok({ now: snap({ options: { ...options, ...patch } }) }))).toBe('origin');
    }
  });

  it('pasar de todo el diseño a «Solo la selección» se cuenta como cambio de origen, se muestra el completo', () => {
    expect(fullReason(ok({ now: snap({ options: { ...options, selection: ['a'] } }) }))).toBe('origin');
  });

  it('el material de la biblioteca que reconoce una capa cambió, se muestra el completo', () => {
    const layer = doc.layers.find((l) => l.output)!;
    const preset = { id: 'm1', material: 'mdf', thickness: 3, title: 'Corte', notes: '', device_id: null, settings: layer } as unknown as MaterialPreset;
    expect(fullReason(ok({ baseline: snap({ presets: [] }), now: snap({ presets: [preset] }) }))).toBe('material');
    expect(fullReason(ok({ baseline: snap({ presets: [preset] }), now: snap({ presets: [] }) }))).toBe('material');
  });

  it('posición de la máquina dudosa, se muestra el completo', () => {
    expect(fullReason(ok({ positionDoubtful: true }))).toBe('positionLost');
  });

  it('con «Repetir con el alimentador», se muestra el completo', () => {
    expect(fullReason(ok({ repeating: true }))).toBe('repeat');
  });

  it('K40 y galvo nunca omiten la lista', () => {
    expect(fullReason(ok({ deviceKind: 'galvo' }))).toBe('device');
    expect(fullReason(ok({ deviceKind: 'lhy_micro' }))).toBe('device');
  });

  it('mientras no terminan las comprobaciones no se omite (pendiente)', () => {
    expect(fullReason(ok({ checks: null }))).toBe('pending');
  });

  it('cualquier aviso de la revisión previa obliga a mostrar el completo', () => {
    expect(fullReason(ok({ checks: { ...CLEAN, previewWarnings: ['Algo'] } }))).toBe('warnings');
    expect(fullReason(ok({ checks: { ...CLEAN, linkWarning: 'Enlace saturado' } }))).toBe('warnings');
    expect(fullReason(ok({ checks: { ...CLEAN, seriousFindings: 1 } }))).toBe('review');
    expect(fullReason(ok({ checks: { ...CLEAN, placementOk: false } }))).toBe('placement');
  });

  it('si la revisión falló, no se puede asegurar que no hay avisos: se muestra el completo', () => {
    expect(fullReason(ok({ checks: { ...CLEAN, failed: true } }))).toBe('review');
  });

  it('lo barato se decide sin esperar a la revisión (cheap = «pending» solo si todo lo demás cuadra)', () => {
    expect(fullReason(ok({ checks: null, enabled: false }))).toBe('pref');
    expect(fullReason(ok({ checks: null, baseline: null }))).toBe('noBaseline');
  });
});

describe('huellas', () => {
  it('canon no depende del orden de las llaves', () => {
    expect(canon({ a: 1, b: { c: [1, 2], d: null } })).toBe(canon({ b: { d: null, c: [1, 2] }, a: 1 }));
  });

  it('layersKey ignora lo que no sale (sin salida, sin objetos) y la visibilidad, pero ve cualquier ajuste', () => {
    const base = layersKey(doc);
    const extra = makeLayer('#123456', 'Vacía');
    expect(layersKey({ ...doc, layers: [...doc.layers, extra] })).toBe(base); // capa sin objetos
    expect(layersKey({ ...doc, layers: doc.layers.map((l) => ({ ...l, visible: !l.visible })) })).toBe(base);
    expect(layersKey({ ...doc, layers: doc.layers.map((l) => (l.output ? { ...l, power: l.power + 1 } : l)) })).not.toBe(base);
    expect(layersKey({ ...doc, layers: doc.layers.map((l) => (l.output ? { ...l, passes: l.passes + 1 } : l)) })).not.toBe(base);
  });

  it('originKey distingue el trabajo entero de la selección, pero no QUÉ objetos se eligen', () => {
    expect(originKey({ ...options, selection: ['a'] })).toBe(originKey({ ...options, selection: ['b', 'c'] }));
    expect(originKey({ ...options, selection: ['a'] })).not.toBe(originKey(options));
  });

  it('materialKey es estable sin biblioteca', () => {
    expect(materialKey(doc, [])).toBe(materialKey(doc, []));
  });
});
