import { describe, expect, it } from 'vitest';
import { falconProProbe, genericGrblProbe } from '@/api/mock/probe';
import type { ProbeResult } from '@/api/api';
import { CATALOG } from './catalog';
import { detectionFrom, fold, rankCatalog } from './filter';

const ids = (r: ReturnType<typeof rankCatalog>) => r.map((x) => x.entry.id);
const probe = (patch: Partial<ProbeResult>): ProbeResult => ({ ...genericGrblProbe(), ...patch });

describe('fold', () => {
  it('quita acentos, signos y mayúsculas', () => {
    expect(fold('Falcon-2  Pró')).toBe('falcon 2 pro');
  });
});

describe('rankCatalog sin detección', () => {
  it('muestra todo, ordenado por marca, con las genéricas al final', () => {
    const r = rankCatalog({ query: '', detection: null });
    expect(r).toHaveLength(CATALOG.length);
    const firstGeneric = r.findIndex((x) => x.entry.generic);
    expect(r.slice(firstGeneric).every((x) => x.entry.generic)).toBe(true);
    // Las marcas salen en orden alfabético (las genéricas, al final).
    const brands = r.slice(0, firstGeneric).map((x) => x.entry.brand);
    expect(brands).toEqual([...brands].sort((a, b) => a.localeCompare(b, 'es')));
    expect(brands[0]).toBe('AlgoLaser');
  });

  it('la búsqueda exige todas las palabras en marca, modelo o alias', () => {
    expect(ids(rankCatalog({ query: 'falcon', detection: null }))).toEqual(['creality-falcon-a1-pro', 'creality-falcon2-22w', 'creality-falcon2-40w', 'creality-falcon2-pro']);
    expect(ids(rankCatalog({ query: 'falcon 2 pro', detection: null }))).toEqual(['creality-falcon2-pro']);
    expect(ids(rankCatalog({ query: 'LM3', detection: null }))).toEqual(['ortur-lm3']);
    expect(ids(rankCatalog({ query: 'cohesion', detection: null }))).toEqual(['k40-cohesion3d']);
    expect(ids(rankCatalog({ query: 'xtool pro', detection: null }))).toEqual(['xtool-d1-pro']);
    expect(rankCatalog({ query: 'no existe esto', detection: null })).toEqual([]);
  });
});

describe('rankCatalog con detección', () => {
  it('una Falcon2 se reconoce por su saludo y sale primera', () => {
    const d = detectionFrom(falconProProbe());
    const r = rankCatalog({ query: '', detection: d });
    expect(r[0]!.entry.id.startsWith('creality-falcon2')).toBe(true);
    expect(r[0]!.byBanner).toBe(true);
    expect(r.filter((x) => x.byBanner)).toHaveLength(3);
    expect(r[3]!.entry.id).toBe('grbl-generic');
    // Solo firmwares GRBL: sin Marlin, Smoothie, grblHAL ni GRBL-LPC.
    expect(r.every((x) => ['grbl', 'grbl_m3', 'custom_gcode'].includes(x.entry.kind) || (x.entry.alsoKinds ?? []).includes('grbl'))).toBe(true);
    expect(ids(r)).not.toContain('marlin-generic');
    expect(ids(r)).toContain('ortur-lm3'); // grblHAL con compatibilidad 1 dice Grbl
  });

  it('otro GRBL cualquiera: ningún modelo se marca como reconocido y va primero el GRBL genérico', () => {
    const r = rankCatalog({ query: '', detection: detectionFrom(genericGrblProbe()) });
    expect(r.some((x) => x.byBanner)).toBe(false);
    expect(r[0]!.entry.id).toBe('grbl-generic');
  });

  it('grblHAL con banner de la Ortur LM3', () => {
    const d = detectionFrom(probe({ family: 'grbl_hal', suggested_kind: 'grbl_hal', banner: "Ortur Laser Master 3 Ready!\nGrblHAL 1.1f ['$' or '$HELP' for help]" }));
    const r = rankCatalog({ query: '', detection: d });
    // Otros grblHAL del catálogo (AlgoLaser Delta, Falcon A1 Pro) también salen, pero solo la LM3 se reconoce.
    expect(r[0]!.entry.id).toBe('ortur-lm3');
    expect(r[0]!.byBanner).toBe(true);
    expect(r.filter((x) => x.byBanner)).toHaveLength(1);
    expect(r.every((x) => x.entry.kind === 'grbl_hal' || x.entry.generic)).toBe(true);
  });

  it('Marlin y Smoothie solo muestran su familia', () => {
    const marlin = detectionFrom(probe({ family: 'marlin', suggested_kind: 'marlin', banner: 'start\necho:Marlin 2.1.2.5', settings: null }));
    expect(ids(rankCatalog({ query: '', detection: marlin }))).toEqual(['marlin-generic']);
    const smoothie = detectionFrom(probe({ family: 'smoothie', suggested_kind: 'smoothie', banner: 'Smoothie', settings: null }));
    expect(ids(rankCatalog({ query: '', detection: smoothie }))).toEqual(['smoothie-generic']);
  });

  it('"mostrar otras familias" devuelve todo el catálogo, con las compatibles primero', () => {
    const d = detectionFrom(probe({ family: 'marlin', suggested_kind: 'marlin', banner: 'start' }));
    const all = rankCatalog({ query: '', detection: d, includeIncompatible: true });
    expect(all).toHaveLength(CATALOG.length);
    expect(all[0]!.entry.id).toBe('marlin-generic');
    expect(all[0]!.compatible).toBe(true);
    expect(all.at(-1)!.compatible).toBe(false);
  });

  it('familia desconocida: no se filtra por firmware', () => {
    const d = detectionFrom(probe({ family: 'unknown', suggested_kind: 'custom_gcode', banner: '', info: [], settings: null }));
    expect(rankCatalog({ query: '', detection: d })).toHaveLength(CATALOG.length);
  });
});
