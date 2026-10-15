import { describe, expect, it } from 'vitest';
import { falconProProbe, genericGrblProbe } from '@/api/mock/probe';
import type { ProbeResult } from '@/api/api';
import { CATALOG, findEntry, type CatalogEntry } from './catalog';
import { compareWithMachine, defaultMachineChoices } from './compare';
import { identifyModel, probeLines } from './signature';

/** Líneas reales de una Falcon2 Pro 22 W (docs/hardware/falcon2.md §10; crates/tlaku-grbl/src/serial.rs). */
const realFalcon = (): ProbeResult => ({
  ...falconProProbe(),
  settings: { ...falconProProbe().settings, $110: '25000.000', $111: '8000.000', $120: '1000.000', $121: '1000.000', $130: '400.000', $131: '415.000' },
});

describe('identifyModel: firma del firmware', () => {
  it('la Falcon2 Pro real se reconoce por la línea de $I y por la cama 400 × 415', () => {
    const g = identifyModel(realFalcon());
    expect(g.best?.entry.id).toBe('creality-falcon2-pro');
    expect(g.candidates).toHaveLength(1);
    expect(g.best?.signature.kind).toBe('medido');
    expect(g.best?.bedMatches).toBe(true);
    // Evidencia: la primera línea que coincidió (la del saludo) y la de la potencia (extra), tal como las dijo la placa.
    expect(g.best?.evidence[0]).toBe('[OK]system start-up CV50-Pro-MASTER-Release V1.0.1 20240325-1042');
    expect(g.best?.evidence.some((l) => l.startsWith('inquire_info->power:22W'))).toBe(true);
  });

  it('también funciona si la cadena solo está en el saludo, sin importar mayúsculas', () => {
    const probe: ProbeResult = { ...genericGrblProbe(), banner: '[OK]system start-up CV50-PRO-MASTER-Release V1.0.1 20240325-1042', info: ['[VER:1.1f.20220810:]'] };
    expect(identifyModel(probe).best?.entry.id).toBe('creality-falcon2-pro');
  });

  it('un GRBL cualquiera, un resultado vacío o una familia desconocida no preseleccionan nada', () => {
    expect(identifyModel(genericGrblProbe()).candidates).toEqual([]);
    expect(identifyModel(null).best).toBeNull();
    expect(identifyModel({ ...realFalcon(), family: 'unknown' }).best).toBeNull();
  });

  it('solo cuentan los modelos del mismo firmware que se detectó', () => {
    expect(identifyModel({ ...realFalcon(), family: 'marlin', suggested_kind: 'marlin' }).best).toBeNull();
  });

  it('la Ortur LM3 se reconoce por su saludo grblHAL', () => {
    const probe: ProbeResult = {
      ...genericGrblProbe(),
      family: 'grbl_hal',
      suggested_kind: 'grbl_hal',
      banner: "Ortur Laser Master 3 Ready!\nGrblHAL 1.1f ['$' or '$HELP' for help]",
      info: [],
    };
    expect(identifyModel(probe).best?.entry.id).toBe('ortur-lm3');
  });

  describe('varias firmas a la vez', () => {
    const sig = (contains: string[], bed?: [number, number]) => ({ contains, bed, source: 'https://example.com/x', kind: 'foro' as const });
    const fake = (id: string, signatures: CatalogEntry['signatures']): CatalogEntry => ({ id, brand: 'X', model: id, kind: 'grbl', signatures, notes: [] });

    it('desempata por la cama leída', () => {
      const a = fake('a', [sig(['cv50-pro-master'], [400, 400])]);
      const b = fake('b', [sig(['cv50-pro-master'], [400, 415])]);
      const g = identifyModel(realFalcon(), [a, b]);
      expect(g.candidates.map((c) => c.entry.id)).toEqual(['b', 'a']);
      expect(g.best?.entry.id).toBe('b');
    });

    it('si nada desempata, no hay preselección y se devuelven todas las candidatas', () => {
      const a = fake('a', [sig(['cv50-pro-master'])]);
      const b = fake('b', [sig(['cv50-pro-master'])]);
      const g = identifyModel(realFalcon(), [a, b]);
      expect(g.best).toBeNull();
      expect(g.candidates.map((c) => c.entry.id).sort()).toEqual(['a', 'b']);
    });

    it('la firma más específica (más cadenas) gana a la general', () => {
      const general = fake('general', [sig(['cv50-pro-master'])]);
      const specific = fake('specific', [sig(['cv50-pro-master', 'cv50-laser-22w'])]);
      expect(identifyModel(realFalcon(), [general, specific]).best?.entry.id).toBe('specific');
    });

    it('exige todas las cadenas de la firma', () => {
      expect(identifyModel(realFalcon(), [fake('c', [sig(['cv50-pro-master', 'cv50-laser-40w'])])]).candidates).toEqual([]);
    });
  });
});

describe('firmas en el catálogo', () => {
  it('cada firma tiene cadenas en minúsculas, fuente https y nivel válido', () => {
    for (const e of CATALOG) {
      for (const s of e.signatures ?? []) {
        expect(s.contains.length, e.id).toBeGreaterThan(0);
        for (const c of [...s.contains, ...(s.show ?? [])]) expect(c, e.id).toBe(c.toLowerCase());
        expect(s.source, e.id).toMatch(/^https:\/\/\S+$/);
        expect(['oficial', 'tienda', 'foro', 'medido']).toContain(s.kind);
      }
    }
  });

  it('la firma de la Falcon2 Pro es medida y no hay firma para la clásica (sin salida publicada)', () => {
    expect(findEntry('creality-falcon2-pro')?.signatures?.[0]?.kind).toBe('medido');
    expect(findEntry('creality-falcon2-22w')?.signatures).toBeUndefined();
  });

  it('probeLines separa el saludo en líneas y añade las de $I', () => {
    const lines = probeLines(realFalcon());
    expect(lines[0]).toBe('usb connect succeed');
    expect(lines).toContain('[OPT:VHL,127,65536]');
  });
});

describe('revisar tras preseleccionar: lo de la máquina manda por defecto', () => {
  it('velocidad y aceleración de la máquina se proponen cuando difieren del catálogo', () => {
    const probe = realFalcon();
    const pro = findEntry('creality-falcon2-pro')!;
    // El catálogo de la Pro trae lo medido: se fuerza otra cifra para comprobar que la de la máquina gana.
    const entry: CatalogEntry = { ...pro, maxRate: { ...pro.maxRate!, value: 50000 }, accel: { ...pro.accel!, value: 5000 } };
    const cmp = compareWithMachine(entry, probe.settings);
    expect(cmp.rows.find((r) => r.field === 'max_rate')?.status).toBe('differs');
    expect(cmp.rows.find((r) => r.field === 'accel')?.status).toBe('differs');
    const choices = defaultMachineChoices(cmp);
    expect(choices.has('max_rate')).toBe(true);
    expect(choices.has('accel')).toBe(true);
    // Lo que coincide ($30, cama 400 × 415) se queda con el catálogo.
    expect(choices.has('s_max')).toBe(false);
    expect(choices.has('bed')).toBe(false);
  });
});
