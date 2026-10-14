import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { es } from '@/i18n';
import { applyKindChange, DEVICE_KINDS, FALLBACK_DEVICE, kindDefaults, validateDevice } from '@/lib/device';
import { CATALOG, findEntry } from './catalog';
import { rankCatalog } from './filter';
import { buildProfile } from './profile';
import { DetectStep, EntryDetail, SaveStep, type DetectStepProps } from './steps';

const noop = () => {};

const entry = findEntry('k40-m2nano')!;
const build = () =>
  buildProfile({ entry, kind: 'lhy_micro', name: 'Mi K40', takenIds: [], probedBaud: null, settings: null, useMachine: new Set() });

const detectProps = (patch: Partial<DetectStepProps> = {}): DetectStepProps => ({
  ports: ['/dev/ttyUSB0'],
  port: '/dev/ttyUSB0',
  baud: 115200,
  probe: { status: 'idle' },
  machineConnected: false,
  onPort: noop,
  onBaud: noop,
  onRefresh: noop,
  onDetect: noop,
  ...patch,
});

describe('Agregar máquina con un K40 (M2 Nano)', () => {
  it('el K40 se ofrece entre los tipos y en el catálogo, con la fuente de la cama', () => {
    expect(DEVICE_KINDS).toContain('lhy_micro');
    expect(entry.kind).toBe('lhy_micro');
    expect(entry.bed?.value).toEqual([300, 200]);
    expect(entry.bed?.source).toMatch(/^https:\/\//);
    // Las notas existen y dicen lo importante: potencia por perilla, posición por conteo, Moshiboard.
    for (const n of entry.notes) expect(es[n], n).toBeTypeOf('string');
    expect(entry.notes.map((n) => es[n]).join(' ')).toMatch(/perilla del K40/);
    expect(CATALOG.filter((e) => e.kind === 'lhy_micro')).toHaveLength(1);
  });

  it('se busca por K40, M2 Nano o Lihuiyu; con una máquina GRBL detectada no sale', () => {
    for (const q of ['k40 m2', 'nano', 'lihuiyu', 'LaserDRW']) {
      expect(rankCatalog({ query: q, detection: null }).some((r) => r.entry.id === 'k40-m2nano'), q).toBe(true);
    }
    const grbl = { family: 'grbl' as const, kind: 'grbl' as const, text: 'grbl 1.1h' };
    expect(rankCatalog({ query: '', detection: grbl }).some((r) => r.entry.id === 'k40-m2nano')).toBe(false);
    expect(rankCatalog({ query: '', detection: grbl, includeIncompatible: true }).some((r) => r.entry.id === 'k40-m2nano')).toBe(true);
  });

  it('el perfil nuevo es un K40 de serie: cama 300 x 200, home arriba a la izquierda, 140 mm/s', () => {
    const { profile, provenance } = build();
    expect(profile.kind).toBe('lhy_micro');
    expect(profile.bed).toEqual([300, 200]);
    expect(profile.origin).toBe('top_left');
    expect(profile.max_rate).toEqual([8400, 8400]);
    expect(profile.lhymicro).toEqual({ board: 'M2' });
    expect(profile.connection).toEqual({ type: 'serial' });
    // La cama tiene fuente; el home y la velocidad son valores de partida y el asistente lo marca.
    expect(provenance.bed).toBe('catalog');
    expect(provenance.origin).toBe('default');
    expect(provenance.max_rate).toBe('default');
    expect(Object.keys(validateDevice(profile))).toEqual([]);
  });

  it('detectar: ofrece el K40 y dice cuántas placas encontró por USB', () => {
    const none = renderToString(<DetectStep {...detectProps({ k40Boards: [], onPickK40: noop })} />);
    expect(none).toContain('K40 con M2 Nano por USB');
    expect(none).toContain('No se encontró ninguna placa de K40 por USB');
    expect(none).toContain('Usar K40 con M2 Nano');
    const found = renderToString(<DetectStep {...detectProps({ k40Boards: ['usb:k40:0'], onPickK40: noop })} />);
    expect(found).toContain('Se encontró 1 placa(s) de K40 por USB');
    expect(found).not.toContain('No se encontró ninguna placa');
    // Sin el gancho del K40 la parte no se muestra (los demás usos del paso no cambian).
    expect(renderToString(<DetectStep {...detectProps()} />)).not.toContain('K40 con M2 Nano por USB');
  });

  it('el modelo muestra la cama con su fuente y las notas', () => {
    const h = renderToString(<EntryDetail entry={entry} />);
    expect(h).toContain('K40 con M2 Nano (Lihuiyu)');
    expect(h).toContain('perilla del K40');
    expect(h).toContain('Moshiboard');
  });

  it('el último paso pide el modelo de placa y no muestra baudios ni S máxima', () => {
    const h = renderToString(<SaveStep name="Mi K40" onName={noop} built={build()} board="M2" onBoard={noop} />);
    expect(h).toContain('Modelo de placa del K40');
    expect(h).toContain('M2 (la de serie)');
    expect(h).toContain('Experimental');
    expect(h).toContain('valores de partida');
    expect(h).not.toContain('Baudios');
    expect(h).not.toContain('S máxima');
  });

  it('con otro tipo no hay selector de placa', () => {
    const grbl = buildProfile({ entry: null, kind: 'grbl', name: 'X', takenIds: [], probedBaud: null, settings: null, useMachine: new Set() });
    expect(renderToString(<SaveStep name="X" onName={noop} built={grbl} board="M2" onBoard={noop} />)).not.toContain('Modelo de placa del K40');
  });

  it('cambiar el tipo a K40 desde una Ruida suelta la IP', () => {
    const ruida = applyKindChange(FALLBACK_DEVICE, kindDefaults('ruida'), false);
    expect(ruida.connection.type).toBe('udp');
    expect(applyKindChange(ruida, kindDefaults('lhy_micro'), false).connection).toEqual({ type: 'serial' });
  });
});
