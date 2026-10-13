import { describe, expect, it } from 'vitest';
import { es as dict } from '@/i18n';
import { CATALOG, entrySources, findEntry, type Fact } from './catalog';

const FACT_KEYS = ['detect', 'bed', 'maxRate', 'accel', 'sMax', 'm4', 'air', 'baud', 'origin', 'homing', 'zAxis'] as const;

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
        expect(['oficial', 'tienda', 'foro']).toContain(f.kind);
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
    // Aceleración: solo hay cifras (contradictorias) para la Falcon2.
    expect(CATALOG.filter((e) => e.accel).map((e) => e.id)).toEqual(['creality-falcon2-22w', 'creality-falcon2-40w', 'creality-falcon2-pro']);
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
    expect(findEntry('creality-falcon2-pro')?.maxRate?.value).toBe(50000);
  });

  it('las familias de firmware de las entradas genéricas coinciden con su tipo', () => {
    expect(findEntry('grbl-generic')?.kind).toBe('grbl');
    expect(findEntry('marlin-generic')?.kind).toBe('marlin');
    expect(findEntry('marlin-generic')?.baud?.value).toBe(250000);
    expect(findEntry('smoothie-generic')?.kind).toBe('smoothie');
    expect(findEntry('k40-cohesion3d')?.kind).toBe('grbl_lpc');
    expect(findEntry('ortur-lm3')?.kind).toBe('grbl_hal');
  });

  it('entrySources reúne las URL sin repetir', () => {
    const s = entrySources(findEntry('creality-falcon2-22w')!);
    expect(new Set(s).size).toBe(s.length);
    expect(s.length).toBeGreaterThan(3);
  });
});
