import { describe, expect, it } from 'vitest';
import { makeLayer } from '@/lib/layers';
import { activeConflicts, advancedInUse, layerRules, minPowerApplies } from './rules';

const layer = (patch = {}) => makeLayer('#ff0000', 'Capa', patch);
const on = <T extends object>(o: T): T & { enabled: true } => ({ ...o, enabled: true });

describe('combinaciones incompatibles', () => {
  it('sin nada encendido no se bloquea nada', () => {
    expect(layerRules(layer())).toEqual({ perforation: null, dotMode: null, ramp: null, angleIncrement: null, imageProcessing: null });
  });

  it('el modo punto bloquea la rampa y la perforación, con su motivo', () => {
    const l = layer({ dot_mode: on(layer().dot_mode) });
    const r = layerRules(l);
    expect(r.ramp).toBe('layers.reason.rampDot');
    expect(r.perforation).toBe('layers.reason.perfDot');
    expect(r.dotMode).toBeNull();
  });

  it('la perforación bloquea el modo punto', () => {
    expect(layerRules(layer({ perforation: on(layer().perforation) })).dotMode).toBe('layers.reason.dotPerf');
  });

  it('un control ya encendido nunca se bloquea, para poder arreglar la combinación', () => {
    const l = layer({ dot_mode: on(layer().dot_mode), ramp: on(layer().ramp), perforation: on(layer().perforation) });
    const r = layerRules(l);
    expect(r).toMatchObject({ ramp: null, perforation: null, dotMode: null });
    expect(activeConflicts(l)).toEqual(['layers.warn.dotPerf', 'layers.warn.dotRamp']);
  });

  it('pass-through bloquea el tramado y los ajustes de imagen', () => {
    const l = layer({ mode: 'image', image: { ...layer().image, pass_through: true } });
    expect(layerRules(l).imageProcessing).toBe('layers.reason.passThrough');
    expect(layerRules(layer({ mode: 'image' })).imageProcessing).toBeNull();
  });

  it('el relleno por contornos bloquea el incremento de ángulo; por regiones no', () => {
    expect(layerRules(layer({ mode: 'fill', fill_style: 'offset' })).angleIncrement).toBe('layers.reason.offsetAngle');
    expect(layerRules(layer({ mode: 'fill', fill_style: 'flood' })).angleIncrement).toBeNull();
  });
});

describe('potencia mínima', () => {
  it('en línea solo aplica con potencia dinámica (M4); en imagen siempre; en relleno nunca', () => {
    expect(minPowerApplies({ mode: 'line' }, 'dynamic')).toBe(true);
    expect(minPowerApplies({ mode: 'line' }, 'constant')).toBe(false);
    expect(minPowerApplies({ mode: 'image' }, 'constant')).toBe(true);
    expect(minPowerApplies({ mode: 'fill' }, 'dynamic')).toBe(false);
  });
});

describe('ajustes avanzados en uso (modo principiante)', () => {
  it('una capa de fábrica no usa ninguno', () => {
    for (const mode of ['line', 'fill', 'image'] as const) expect(advancedInUse(layer({ mode }))).toEqual([]);
  });

  it('solo cuenta lo que el modo de la capa muestra', () => {
    const l = layer({ mode: 'fill', perforation: on(layer().perforation), overcut: 2 });
    expect(advancedInUse(l)).toEqual([]);
    expect(advancedInUse({ ...l, mode: 'line' })).toEqual(['perforation', 'overcut']);
  });

  it('detecta línea, relleno, imagen, ángulo y operaciones adicionales', () => {
    const base = layer();
    expect(advancedInUse(layer({ start_delay_ms: 5, ramp: on(base.ramp), min_power: 10 }))).toEqual(['delays', 'ramp', 'min_power']);
    expect(advancedInUse(layer({ min_power: 10 }), 'constant')).toEqual([]);
    expect(advancedInUse(layer({ mode: 'fill', fill_style: 'flood', fill_grouping: 'by_shape', angle_increment: 5 }))).toEqual(['fill_style', 'fill_grouping', 'angle_increment']);
    const image = { ...base.image, pass_through: true, enhance_radius: 1, enhance_amount: 0.5, denoise: 2 };
    expect(advancedInUse(layer({ mode: 'image', image }))).toEqual(['pass_through', 'enhance', 'denoise']);
    expect(advancedInUse(layer({ sub_layers: [base] }))).toEqual(['sub_layers']);
  });

  it('el realce necesita radio y fuerza a la vez', () => {
    const image = { ...layer().image, enhance_radius: 1, enhance_amount: 0 };
    expect(advancedInUse(layer({ mode: 'image', image }))).toEqual([]);
  });
});
