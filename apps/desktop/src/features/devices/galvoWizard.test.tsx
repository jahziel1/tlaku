import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { es, setLanguage } from '@/i18n';
import { validateGalvoProfile } from '@/features/machine/galvoProfile';
import { applyKindChange, DEVICE_KINDS, FALLBACK_DEVICE, kindDefaults, validateDevice } from '@/lib/device';
import { CATALOG, findEntry } from './catalog';
import { rankCatalog } from './filter';
import { DEFAULT_GALVO_SETUP, GalvoSetupField } from './GalvoSetupField';
import { buildProfile } from './profile';
import { DetectStep, EntryDetail, SaveStep, type DetectStepProps } from './steps';

const noop = () => {};

const entry = findEntry('galvo-ezcad2')!;
const build = () => buildProfile({ entry, kind: 'galvo', name: 'Mi galvo', takenIds: [], probedBaud: null, settings: null, useMachine: new Set() });

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

beforeEach(() => setLanguage('es'));

describe('Agregar máquina con un galvo', () => {
  it('se ofrece entre los tipos y en el catálogo como «Galvo (fibra o UV, EZCAD2)»', () => {
    expect(DEVICE_KINDS).toContain('galvo');
    expect(entry.kind).toBe('galvo');
    expect(entry.model).toBe('Galvo (fibra o UV, EZCAD2)');
    expect(CATALOG.filter((e) => e.kind === 'galvo')).toHaveLength(1);
    expect(entry.notes.map((n) => es[n]).join(' ')).toMatch(/EXPERIMENTAL/);
    // La cama no tiene fuente: el catálogo no inventa una.
    expect(entry.bed).toBeUndefined();
  });

  it('se busca por EZCAD2, JCZ, fibra o UV; con una máquina GRBL detectada no sale', () => {
    for (const q of ['galvo', 'ezcad', 'jcz', 'bjjcz', 'fibra', 'uv', 'mopa']) {
      expect(rankCatalog({ query: q, detection: null }).some((r) => r.entry.id === 'galvo-ezcad2'), q).toBe(true);
    }
    const grbl = { family: 'grbl' as const, kind: 'grbl' as const, text: 'grbl 1.1h' };
    expect(rankCatalog({ query: '', detection: grbl }).some((r) => r.entry.id === 'galvo-ezcad2')).toBe(false);
    expect(rankCatalog({ query: '', detection: grbl, includeIncompatible: true }).some((r) => r.entry.id === 'galvo-ezcad2')).toBe(true);
  });

  it('el perfil nuevo: el campo es la cama, origen abajo a la izquierda, sin G-code ni baudios', () => {
    const { profile, provenance } = build();
    expect(profile.kind).toBe('galvo');
    expect(profile.bed).toEqual([110, 110]);
    expect(profile.origin).toBe('bottom_left');
    expect(profile.galvo.field_mm).toBe(110);
    expect(provenance.bed).toBe('default');
    expect(Object.keys(validateDevice(profile))).toEqual([]);
    // Con el campo y la fuente de partida el perfil ya es válido para el motor.
    expect(validateGalvoProfile(profile)).toEqual({});
  });

  it('detectar: ofrece el galvo y dice cuántas tarjetas 9588:9899 encontró por USB', () => {
    const none = renderToString(<DetectStep {...detectProps({ galvoBoards: [], onPickGalvo: noop })} />);
    expect(none).toContain('Galvo (fibra o UV, EZCAD2) por USB');
    expect(none).toContain('9588:9899');
    expect(none).toContain('No se encontró ninguna tarjeta BJJCZ por USB');
    expect(none).toContain('Usar galvo');
    const found = renderToString(<DetectStep {...detectProps({ galvoBoards: ['usb:galvo:0', 'usb:galvo:1'], onPickGalvo: noop })} />);
    expect(found).toContain('Se encontró 2 tarjeta(s) BJJCZ por USB');
    expect(found).not.toContain('No se encontró ninguna tarjeta');
    expect(found).toContain('btn btn--accent');
    // Sin el gancho del galvo la parte no se muestra.
    expect(renderToString(<DetectStep {...detectProps()} />)).not.toContain('dev-galvo');
  });

  it('el modelo muestra las notas: experimental, el campo y el origen', () => {
    const h = renderToString(<EntryDetail entry={entry} />);
    expect(h).toContain('Galvo (fibra o UV, EZCAD2)');
    expect(h).toContain('EXPERIMENTAL');
    expect(h).toContain('campo de la lente');
    expect(h).toContain('abajo a la izquierda');
  });

  it('el último paso pide la lente y la fuente, y no muestra baudios, S máxima ni velocidades', () => {
    const h = renderToString(<SaveStep name="Mi galvo" onName={noop} built={build()} galvo={DEFAULT_GALVO_SETUP} onGalvo={noop} />);
    expect(h).toContain('Lente y fuente del galvo');
    expect(h).toContain('Campo de la lente');
    expect(h).toContain('110 × 110 mm');
    expect(h).toContain('300 × 300 mm');
    expect(h).toContain('Fibra Q-switched');
    expect(h).toContain('Fibra MOPA');
    expect(h).toContain('UV');
    expect(h).toContain('Experimental');
    expect(h).toContain('valores de partida');
    for (const hidden of ['Baudios', 'S máxima', 'Velocidad máxima', 'Aceleración']) expect(h).not.toContain(hidden);
  });

  it('con MOPA avisa que falta la lista de anchos de pulso', () => {
    const h = renderToString(<GalvoSetupField setup={{ field_mm: 150, source: 'fiber_mopa' }} onSetup={noop} />);
    expect(h).toContain('necesita la lista de anchos de pulso');
    expect(renderToString(<GalvoSetupField setup={DEFAULT_GALVO_SETUP} onSetup={noop} />)).not.toContain('necesita la lista de anchos de pulso');
    // Un campo fuera de la lista se sigue viendo.
    expect(renderToString(<GalvoSetupField setup={{ field_mm: 123, source: 'uv' }} onSetup={noop} />)).toContain('123 × 123 mm');
  });

  it('con otro tipo no hay selector de lente', () => {
    const grbl = buildProfile({ entry: null, kind: 'grbl', name: 'X', takenIds: [], probedBaud: null, settings: null, useMachine: new Set() });
    expect(renderToString(<SaveStep name="X" onName={noop} built={grbl} galvo={DEFAULT_GALVO_SETUP} onGalvo={noop} />)).not.toContain('Lente y fuente del galvo');
  });

  it('cambiar el tipo a galvo desde una Ruida suelta la IP', () => {
    const ruida = applyKindChange(FALLBACK_DEVICE, kindDefaults('ruida'), false);
    expect(ruida.connection.type).toBe('udp');
    expect(applyKindChange(ruida, kindDefaults('galvo'), false).connection).toEqual({ type: 'serial' });
    // Y el origen pasa a abajo a la izquierda, el único que vale para el galvo.
    expect(applyKindChange({ ...ruida, origin: 'top_left' }, kindDefaults('galvo'), false).origin).toBe('bottom_left');
    expect(applyKindChange({ ...ruida, origin: 'top_left' }, kindDefaults('grbl'), false).origin).toBe('top_left');
  });

  it('en inglés', () => {
    setLanguage('en');
    const h = renderToString(<DetectStep {...detectProps({ galvoBoards: [], onPickGalvo: noop })} />);
    expect(h).toContain('No BJJCZ board found over USB');
    expect(renderToString(<GalvoSetupField setup={DEFAULT_GALVO_SETUP} onSetup={noop} />)).toContain('Galvo lens and source');
  });
});
