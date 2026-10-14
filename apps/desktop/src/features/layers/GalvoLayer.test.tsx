import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { setLanguage } from '@/i18n';
import { makeLayer } from '@/lib/layers';
import { FIELD_SPECS, validateField } from './fields';
import { freqOutOfRange, mmMinToMmS, mmSToMmMin, sourceHasPulse, type GalvoUiProfile } from './GalvoLayer';
import { LayerFieldsView, type FieldsDevice } from './LayerFields';

const PROFILE: GalvoUiProfile = { source: 'fiber_q', freqMinKhz: 20, freqMaxKhz: 80, pulseWidthsNs: [] };
const device = (galvo: GalvoUiProfile | undefined = PROFILE): FieldsDevice => ({ maxRate: 25000, powerMode: 'constant', hasZ: false, galvo });
const GRBL: FieldsDevice = { maxRate: 25000, powerMode: 'dynamic', hasZ: false };

/** El editor de campos de una capa con la máquina dada. */
const fields = (patch: Parameters<typeof makeLayer>[2] = {}, dev: FieldsDevice = device()) => {
  const layer = makeLayer('#ff0000', 'Marca', patch);
  return renderToString(<LayerFieldsView layer={layer} target={{ color: layer.color, sub: null }} device={dev} />);
};

const g = (over: Partial<ReturnType<typeof makeLayer>['galvo']> = {}) => ({ ...makeLayer('#f00', 'x').galvo, ...over });

beforeEach(() => setLanguage('es'));

describe('sección Galvo del editor de capa', () => {
  it('solo aparece con una máquina galvo', () => {
    expect(fields({}, GRBL)).not.toContain('Frecuencia');
    expect(fields({}, GRBL)).not.toContain('Retardos de la capa');
    const h = fields();
    expect(h).toContain('>Galvo<');
    expect(h).toContain('Frecuencia');
    expect(h).toContain('Repeticiones en la tarjeta');
  });

  it('con overscan en el perfil, las capas de relleno e imagen dicen que lo usan; las de línea, no', () => {
    const withOverscan = device({ ...PROFILE, overscanMm: 2 });
    expect(fields({ mode: 'fill' }, withOverscan)).toContain('se usa el del perfil (2 mm)');
    expect(fields({ mode: 'image' }, withOverscan)).toContain('se usa el del perfil (2 mm)');
    expect(fields({ mode: 'line' }, withOverscan)).not.toContain('se usa el del perfil');
    // Sin overscan en el perfil no hay nada que decir.
    expect(fields({ mode: 'fill' })).not.toContain('se usa el del perfil');
    expect(fields({ mode: 'fill' }, device({ ...PROFILE, overscanMm: 0 }))).not.toContain('se usa el del perfil');
  });

  it('la velocidad se edita en mm/s y se guarda en mm/min', () => {
    // 60000 mm/min son 1000 mm/s.
    const h = fields({ speed: 60000 });
    expect(h).toContain('<span class="lyr-field__unit">mm/s</span>');
    expect(h).toContain('value="1000"');
    expect(h).not.toContain('<span class="lyr-field__unit">mm/min</span>');
    // Con GRBL sigue en mm/min.
    const grbl = fields({ speed: 60000 }, GRBL);
    expect(grbl).toContain('<span class="lyr-field__unit">mm/min</span>');
    expect(grbl).toContain('value="60000"');
    expect(mmMinToMmS(60000)).toBe(1000);
    expect(mmSToMmMin(1000)).toBe(60000);
    expect(mmSToMmMin(33.3)).toBe(1998);
    expect(mmSToMmMin(mmMinToMmS(12345))).toBe(12345);
  });

  it('la frecuencia fuera del rango de la fuente avisa', () => {
    expect(fields({ galvo: g({ frequency_khz: 30 }) })).not.toContain('Fuera del rango de la fuente');
    const h = fields({ galvo: g({ frequency_khz: 200 }) });
    expect(h).toContain('Fuera del rango de la fuente (20 a 80 kHz)');
    expect(freqOutOfRange(10, PROFILE)).toEqual({ min: 20, max: 80 });
    expect(freqOutOfRange(50, PROFILE)).toBeNull();
    // Sin rango en el perfil no se avisa.
    expect(freqOutOfRange(10, { ...PROFILE, freqMaxKhz: 0 })).toBeNull();
  });

  it('el ancho de pulso sale de la lista del perfil; la fibra Q-switched y el CO2 no lo tienen', () => {
    expect(fields()).toContain('no tiene ancho de pulso ajustable');
    expect(fields({}, device({ ...PROFILE, source: 'co2' }))).toContain('no tiene ancho de pulso ajustable');
    const mopa = device({ ...PROFILE, source: 'fiber_mopa', pulseWidthsNs: [4, 9, 13] });
    const h = fields({ galvo: g({ pulse_width_ns: 9 }) }, mopa);
    expect(h).toContain('Ancho de pulso');
    for (const ns of ['4 ns', '9 ns', '13 ns']) expect(h).toContain(ns);
    expect(h).not.toContain('Elige un ancho de pulso');
    expect(sourceHasPulse('fiber_mopa') && sourceHasPulse('uv')).toBe(true);
    expect(sourceHasPulse('fiber_q') || sourceHasPulse('co2')).toBe(false);
  });

  it('MOPA sin ancho de pulso lo exige; UV lo deja opcional', () => {
    const mopa = device({ ...PROFILE, source: 'fiber_mopa', pulseWidthsNs: [4, 9] });
    expect(fields({}, mopa)).toContain('Elige un ancho de pulso: la fuente MOPA lo exige.');
    const uv = device({ ...PROFILE, source: 'uv', pulseWidthsNs: [10, 20] });
    const h = fields({}, uv);
    expect(h).toContain('Ancho de pulso');
    expect(h).not.toContain('Elige un ancho de pulso');
    expect(h).toContain('Opcional en UV');
  });

  it('un pulso guardado que ya no está en la lista se sigue viendo y se avisa', () => {
    const mopa = device({ ...PROFILE, source: 'fiber_mopa', pulseWidthsNs: [4, 9] });
    const h = fields({ galvo: g({ pulse_width_ns: 20 }) }, mopa);
    expect(h).toContain('20 ns');
    expect(h).toContain('El pulso de 20 ns no está en la lista del perfil');
  });

  it('«Sobrescribir retardos» muestra sus campos solo encendido', () => {
    const off = fields();
    expect(off).toContain('Sobrescribir retardos');
    expect(off).not.toContain('Retardo de encendido');
    const on = fields({ galvo: g({ override_timings: true }) });
    for (const label of ['Retardo de encendido', 'Retardo de apagado', 'Retardo de fin de trazo', 'Retardo de esquina', 'Velocidad de salto', 'Retardo de salto mínimo', 'Retardo de salto máximo', 'Distancia límite de salto']) {
      expect(on, label).toContain(label);
    }
    // Encendido, la sección arranca abierta.
    expect(on).toMatch(/<details class="lyr-fold" open="">\s*<summary class="lyr-fold__title"><span>Retardos de la capa/);
  });

  it('el bamboleo solo en capas de línea', () => {
    expect(fields()).toContain('Bamboleo');
    expect(fields({ mode: 'fill' })).not.toContain('Bamboleo');
    expect(fields({ galvo: g({ wobble: { enabled: false, size: 0.2, step: 0.05 } }) })).not.toContain('Ancho cubierto');
    const on = fields({ galvo: g({ wobble: { enabled: true, size: 0.2, step: 0.05 } }) });
    expect(on).toContain('Ancho cubierto');
    expect(on).toContain('Avance por vuelta');
  });

  it('pausas y PPI; el PPI avisa que no tiene efecto', () => {
    const h = fields();
    for (const label of ['Pausa antes', 'Pausa después', 'Pulsos por pulgada']) expect(h).toContain(label);
    expect(h).not.toContain('no tiene efecto en el galvo');
    expect(fields({ galvo: g({ ppi: 100 }) })).toContain('no tiene efecto en el galvo');
  });

  it('el grabado 3D por cortes solo en capas de imagen', () => {
    expect(fields()).not.toContain('Grabado 3D por cortes');
    expect(fields({ mode: 'fill' })).not.toContain('Grabado 3D por cortes');
    const img = fields({ mode: 'image' });
    expect(img).toContain('Grabado 3D por cortes');
    expect(img).not.toContain('Ángulo por corte');
    const on = fields({ mode: 'image', galvo: g({ slice3d: { enabled: true, slices: 16, angle_step_deg: 0 } }) });
    expect(on).toContain('Cortes');
    expect(on).toContain('Ángulo por corte');
  });

  it('en inglés', () => {
    setLanguage('en');
    const h = fields({ mode: 'image' });
    expect(h).toContain('Repeats on the board');
    expect(h).toContain('3D slice engraving');
    expect(h).toContain('Layer delays');
  });
});

describe('campos numéricos del galvo', () => {
  const ok = (field: Parameters<typeof validateField>[0], text: string) => validateField(field, text).ok;

  it('frecuencia mayor que 0; repeticiones enteras de 1 a 100000', () => {
    expect(ok('galvo_freq', '30')).toBe(true);
    expect(ok('galvo_freq', '0')).toBe(false);
    expect(ok('galvo_repeat', '1')).toBe(true);
    expect(ok('galvo_repeat', '0')).toBe(false);
    expect(ok('galvo_repeat', '2.5')).toBe(false);
    expect(ok('galvo_repeat', '100001')).toBe(false);
  });

  it('los retardos pueden ser negativos; las pausas no', () => {
    expect(ok('galvo_delay', '-120')).toBe(true);
    expect(ok('galvo_pause', '-1')).toBe(false);
    expect(ok('galvo_pause', '250')).toBe(true);
    expect(ok('galvo_jump_delay', '-1')).toBe(false);
  });

  it('bamboleo con tamaño y paso mayores que 0; cortes de 2 a 256', () => {
    expect(ok('galvo_wobble_size', '0')).toBe(false);
    expect(ok('galvo_wobble_step', '0')).toBe(false);
    expect(ok('galvo_wobble_size', '0.2')).toBe(true);
    expect(ok('galvo_slices', '1')).toBe(false);
    expect(ok('galvo_slices', '257')).toBe(false);
    expect(ok('galvo_slices', '16')).toBe(true);
    expect(FIELD_SPECS.galvo_speed.unit).toBe('mm/s');
    expect(ok('galvo_speed', '1000')).toBe(true);
  });
});
