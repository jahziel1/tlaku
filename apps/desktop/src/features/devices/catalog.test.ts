import { describe, expect, it } from 'vitest';
import { es as dict } from '@/i18n';
import { CATALOG, entryMaxRate, entrySources, findEntry, type Fact } from './catalog';
import { DIODE_ENTRIES } from './catalog.diode';
import { RUIDA_ENTRIES } from './catalog.ruida';

const FACT_KEYS = ['detect', 'controller', 'bed', 'maxRate', 'maxRateY', 'accel', 'sMax', 'm4', 'air', 'baud', 'origin', 'homing', 'zAxis'] as const;

describe('catálogo: datos con fuente', () => {
  it('trae todos los modelos pedidos', () => {
    const ids = CATALOG.map((e) => e.id);
    expect(new Set(ids).size).toBe(ids.length);
    const names = CATALOG.map((e) => `${e.brand} ${e.model}`).join(' | ');
    for (const needle of [
      'Falcon2 22 W', 'Falcon2 40 W', 'Falcon2 Pro', 'Sculpfun S9', 'S30', 'A5 Pro', 'A10 Pro', 'X20 Pro',
      'Laser Master 2', 'Laser Master 3', 'xTool D1', 'D1 Pro', 'xTool S1', 'Ray5', 'Neje', 'Two Trees',
      'GRBL genérico', 'Cohesion3D', 'Marlin genérico', 'Smoothie genérico',
    ]) {
      expect(names, needle).toContain(needle);
    }
  });

  it('cada dato lleva una URL como fuente y un origen válido', () => {
    for (const e of CATALOG) {
      for (const key of FACT_KEYS) {
        const f = e[key] as Fact<unknown> | undefined;
        if (!f) continue;
        expect(f.source, `${e.id}.${key}`).toMatch(/^https:\/\/[^\s]+$/);
        expect(['oficial', 'tienda', 'foro', 'medido']).toContain(f.kind);
        if (f.note) expect(dict[f.note], `${e.id}.${key} nota`).toBeTypeOf('string');
      }
    }
  });

  it('las notas existen en el diccionario y los valores son plausibles', () => {
    for (const e of CATALOG) {
      for (const n of e.notes) expect(dict[n], `${e.id}: ${n}`).toBeTypeOf('string');
      if (e.bed) expect(e.bed.value.every((v) => v >= 100 && v <= 1500), e.id).toBe(true);
      if (e.maxRate) expect(e.maxRate.value).toBeGreaterThan(1000);
      if (e.sMax) expect(e.sMax.value).toBeGreaterThan(0);
    }
  });

  it('lo que no tiene fuente queda vacío (no se inventa)', () => {
    // Datos con cifras en conflicto o sin ficha: el área debe leerse de la máquina.
    for (const id of ['sculpfun-s30', 'ortur-lm3', 'xtool-d1', 'neje-master-2s']) {
      expect(findEntry(id)?.bed, id).toBeUndefined();
    }
    // Aceleración: solo hay cifras para la Falcon2 (contradictorias) y para la A1 Pro (el volcado de un usuario).
    expect(CATALOG.filter((e) => e.accel).map((e) => e.id)).toEqual(['creality-falcon2-22w', 'creality-falcon2-40w', 'creality-falcon2-pro', 'creality-falcon-a1-pro']);
  });

  it('los datos de la Falcon2 siguen a docs/hardware/falcon2.md', () => {
    const f = findEntry('creality-falcon2-22w')!;
    expect(f.kind).toBe('grbl');
    expect(f.bed?.value).toEqual([400, 415]);
    expect(f.maxRate?.value).toBe(25000);
    expect(f.sMax?.value).toBe(1000);
    expect(f.baud?.value).toBe(115200);
    expect(f.origin?.value).toBe('bottom_left');
    expect(f.zAxis?.value).toBe(false);
    expect(f.air?.value).toBe(true);
    expect(f.detect?.value).toContain('cv50');
    // Medido en una Pro 22 W real (docs/hardware/falcon2.md §10), no el volcado de foro de 50 000.
    const pro = findEntry('creality-falcon2-pro')!;
    expect(pro.maxRate?.value).toBe(25000);
    expect(pro.maxRate?.kind).toBe('medido');
    // $110=25000 (X) y $111=8000 (Y): el eje Y es más lento.
    expect(pro.maxRateY?.value).toBe(8000);
    expect(pro.maxRateY?.kind).toBe('medido');
    expect(entryMaxRate(pro)).toEqual([25000, 8000]);
    expect(entryMaxRate(f)).toEqual([25000, 25000]);
    expect(pro.accel?.value).toBe(1000);
  });

  it('las familias de firmware de las entradas genéricas coinciden con su tipo', () => {
    expect(findEntry('grbl-generic')?.kind).toBe('grbl');
    expect(findEntry('marlin-generic')?.kind).toBe('marlin');
    expect(findEntry('marlin-generic')?.baud?.value).toBe(250000);
    expect(findEntry('smoothie-generic')?.kind).toBe('smoothie');
    expect(findEntry('k40-cohesion3d')?.kind).toBe('grbl_lpc');
    expect(findEntry('ortur-lm3')?.kind).toBe('grbl_hal');
  });

  it('trae los modelos añadidos: CO2 Ruida y diodos GRBL', () => {
    const names = CATALOG.map((e) => `${e.brand} ${e.model}`).join(' | ');
    for (const needle of [
      'OMTech Maker 20 × 28', 'OMTech Maker 16 × 24', 'OMTech Pronto 40', 'OMTech Polar Lite', 'Monport CO2 20 × 28', 'Thunder Laser Nova 35',
      'Ruida genérico', 'Comgrow Z1', 'Longer B1', 'Two Trees TS2', 'Atomstack A20 Pro V2', 'Atomstack A20 Pro (original)', 'Atomstack A40 Pro',
      'Sculpfun SF-A9', 'Sculpfun iCube Pro', 'AlgoLaser Delta', 'AlgoLaser DIY Kit', 'Neje Max 4', 'Atezr P20 Plus', 'Monport 40 W Pro', 'Creality Falcon A1 Pro',
    ]) {
      expect(names, needle).toContain(needle);
    }
    expect(CATALOG.length).toBeGreaterThanOrEqual(40);
  });

  it('cada modelo añadido trae al menos una fuente y los valores de todo el catálogo están en rango', () => {
    for (const e of [...RUIDA_ENTRIES, ...DIODE_ENTRIES]) {
      if (!e.generic) expect(entrySources(e).length, e.id).toBeGreaterThan(0);
    }
    for (const e of CATALOG) {
      if (e.bed) expect(e.bed.value.every((v) => Number.isFinite(v) && v >= 100 && v <= 1500), `${e.id} cama`).toBe(true);
      if (e.maxRate) expect(e.maxRate.value, `${e.id} velocidad`).toBeLessThanOrEqual(200000);
      if (e.accel) expect(e.accel.value, `${e.id} aceleración`).toBeLessThanOrEqual(20000);
      if (e.baud) expect([9600, 19200, 38400, 57600, 115200, 230400, 250000, 460800], `${e.id} baudios`).toContain(e.baud.value);
      if (e.sMax) expect(Number.isInteger(e.sMax.value), `${e.id} S`).toBe(true);
      if (e.controller) expect(e.controller.value.trim(), `${e.id} controladora`).not.toBe('');
      for (const d of e.detect?.value ?? []) expect(d, `${e.id} detect`).toBe(d.toLowerCase());
    }
  });

  it('las máquinas Ruida llevan la controladora con su fuente y no traen datos de G-code', () => {
    const ruida = CATALOG.filter((e) => e.kind === 'ruida');
    expect(ruida.length).toBeGreaterThanOrEqual(6);
    for (const e of ruida) {
      // Ruida no usa S, baudios ni aceleración del catálogo: lo de G-code no aplica.
      expect(e.sMax, e.id).toBeUndefined();
      expect(e.baud, e.id).toBeUndefined();
      expect(e.accel, e.id).toBeUndefined();
      if (!e.generic) expect(e.controller?.value, e.id).toMatch(/Ruida|RDWorks/);
      expect(e.notes, e.id).toContain('catalog.note.ruida.network');
    }
  });

  it('los grblHAL del catálogo también aparecen al detectar GRBL (alsoKinds)', () => {
    for (const e of CATALOG.filter((x) => x.kind === 'grbl_hal')) expect(e.alsoKinds, e.id).toContain('grbl');
  });

  it('las cifras de las máquinas añadidas siguen a sus fuentes', () => {
    // Fichas oficiales (pulgadas ya convertidas a mm, ancho × alto).
    expect(findEntry('omtech-maker-2028')?.bed?.value).toEqual([700, 500]);
    expect(findEntry('omtech-maker-2028')?.maxRate?.value).toBe(36000);
    expect(findEntry('omtech-maker-1624')?.bed?.value).toEqual([600, 400]);
    expect(findEntry('thunder-nova-35')?.bed?.value).toEqual([900, 600]);
    expect(findEntry('thunder-nova-35')?.maxRate?.value).toBe(60000);
    expect(findEntry('sculpfun-sf-a9')?.maxRate?.value).toBe(36000);
    expect(findEntry('atomstack-a20-pro-v2')?.bed?.value).toEqual([400, 365]);
    expect(findEntry('atomstack-a20-pro-v2')?.maxRate?.value).toBe(24000);
    expect(findEntry('algolaser-delta')?.bed?.value).toEqual([440, 415]);
    expect(findEntry('longer-b1')?.baud?.value).toBe(115200);
    expect(findEntry('longer-b1')?.sMax?.value).toBe(1000);
    expect(findEntry('longer-b1')?.air?.value).toBe(true);
    expect(findEntry('comgrow-z1')?.sMax?.value).toBe(1000);
    // Sin cifra de aceleración con fuente: no se inventa.
    expect(findEntry('longer-b1')?.accel).toBeUndefined();
    // Lo que solo viene de un usuario del foro se rotula como tal.
    expect(findEntry('atomstack-a20-pro')?.sMax?.kind).toBe('foro');
    expect(findEntry('creality-falcon-a1-pro')?.accel?.kind).toBe('foro');
  });

  it('entrySources reúne las URL sin repetir', () => {
    const s = entrySources(findEntry('creality-falcon2-22w')!);
    expect(new Set(s).size).toBe(s.length);
    expect(s.length).toBeGreaterThan(3);
  });
});
