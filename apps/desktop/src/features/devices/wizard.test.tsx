import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { falconProProbe, genericGrblProbe, MOCK_FALCON_PORT, MOCK_GRBL_PORT } from '@/api/mock/probe';
import type { ProbeResult } from '@/api/api';
import { commands } from '@/commands';
import { es } from '@/i18n';
import { validateDevice } from '@/lib/device';
import { selectActiveDevice, useStore } from '@/store';
import { AddMachineWizard, StepList } from './AddMachineWizard';
import { CATALOG, findEntry } from './catalog';
import { compareWithMachine, defaultMachineChoices } from './compare';
import { detectionFrom, rankCatalog } from './filter';
import { buildProfile } from './profile';
import { DetectStep, EntryDetail, LaserModeNotice, ModelStep, OTHER_ID, ReviewStep, SaveStep, type DetectStepProps } from './steps';
import { useDevicesWizard } from './wizardStore';

const noop = () => {};

function detectProps(patch: Partial<DetectStepProps> = {}): DetectStepProps {
  return {
    ports: [MOCK_FALCON_PORT, MOCK_GRBL_PORT, 'virtual:falcon2'],
    port: MOCK_FALCON_PORT,
    baud: 115200,
    probe: { status: 'idle' },
    machineConnected: false,
    onPort: noop,
    onBaud: noop,
    onRefresh: noop,
    onDetect: noop,
    ...patch,
  };
}

beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }));
  useDevicesWizard.setState({ open: false });
});

describe('textos', () => {
  it('toda clave catalog.* usada por el catálogo existe', () => {
    for (const e of CATALOG) for (const n of e.notes) expect(es[n]).toBeTypeOf('string');
  });
});

describe('mock de probePort', () => {
  it('Falcon2 Pro en el primer puerto, GRBL genérico en el segundo, error en uno inexistente', async () => {
    const api = createMockApi({ latencyMs: 0 });
    const ports = await api.listPorts();
    expect(ports[0]).toBe(MOCK_FALCON_PORT);
    expect(ports[1]).toBe(MOCK_GRBL_PORT);
    const falcon = await api.probePort({ port: MOCK_FALCON_PORT, baud: 115200 });
    expect(falcon.banner).toContain('CV50-Pro-MASTER');
    expect(falcon.settings?.$22).toBe('1');
    expect(falcon.settings?.$30).toBe('1000');
    const grbl = await api.probePort({ port: MOCK_GRBL_PORT, baud: 115200 });
    expect(grbl.firmware).toBe('Grbl 1.1h (20190825)');
    expect((await api.probePort({ port: 'virtual:falcon2', baud: 115200 })).banner).toContain('CV50');
    await expect(api.probePort({ port: '/dev/nada', baud: 115200 })).rejects.toMatch(/No se pudo abrir/);
    await expect(api.probePort({ port: MOCK_GRBL_PORT, baud: 0 })).rejects.toMatch(/baudios/);
  });
});

describe('pasos del asistente (render)', () => {
  it('lista de pasos con el actual marcado', () => {
    const h = renderToString(<StepList current="review" />);
    for (const s of ['Detectar', 'Modelo', 'Revisar', 'Guardar']) expect(h).toContain(s);
    expect(h.match(/aria-current="step"/g)).toHaveLength(1);
  });

  it('detectar: puertos (virtual marcado), baudios, botones, nota de red y "próximamente"', () => {
    const h = renderToString(<DetectStep {...detectProps()} />);
    expect(h).toContain(MOCK_FALCON_PORT);
    expect(h).toContain('virtual:falcon2 (virtual)');
    expect(h).toContain('Detectar');
    expect(h).toContain('Ruida');
    expect(h).toContain('Otro / genérico');
    expect(h).toContain('nunca mueve la máquina');
  });

  it('detectar: sin puertos, conectada, detectando, error y resultado', () => {
    expect(renderToString(<DetectStep {...detectProps({ ports: [], port: '' })} />)).toContain('No se encontraron puertos');
    expect(renderToString(<DetectStep {...detectProps({ machineConnected: true })} />)).toContain('Desconéctala para detectar otra');
    expect(renderToString(<DetectStep {...detectProps({ probe: { status: 'probing' } })} />)).toContain('Detectando…');
    const err = renderToString(<DetectStep {...detectProps({ probe: { status: 'error', message: 'No se pudo abrir /dev/x' } })} />);
    expect(err).toContain('No se pudo detectar: No se pudo abrir /dev/x');
    const done = renderToString(<DetectStep {...detectProps({ probe: { status: 'done', result: falconProProbe() } })} />);
    expect(done).toContain('Grbl 1.1f (20220810)');
    expect(done).toContain('31 ajustes');
    expect(done).toContain('CV50-Pro-MASTER');
  });

  it('detectar: familia desconocida avisa', () => {
    const unknown: ProbeResult = { family: 'unknown', firmware: '', banner: '', info: [], settings: null, suggested_kind: 'custom_gcode' };
    const h = renderToString(<DetectStep {...detectProps({ probe: { status: 'done', result: unknown } })} />);
    expect(h).toContain('No se reconoció');
    expect(h).toContain('Otro / genérico');
  });

  it('modelo: la Falcon2 detectada sale marcada como reconocida y la lista incluye "Otro / genérico"', () => {
    const probe = falconProProbe();
    const ranked = rankCatalog({ query: '', detection: detectionFrom(probe) });
    const h = renderToString(
      <ModelStep ranked={ranked} query="" onQuery={noop} selectedId={null} onSelect={noop} probe={probe} showAll={false} onShowAll={noop} otherKind="grbl" onOtherKind={noop} />,
    );
    expect(h).toContain('Falcon2 22 W');
    expect(h).toContain('Reconocida por su saludo');
    expect(h).toContain('Otro / genérico');
    expect(h).toContain('Mostrar también modelos de otras familias');
    expect(h).toContain('Elige un modelo');
    expect(h).not.toContain('Marlin genérico');
  });

  it('modelo: sin detección se avisa y sin resultados también', () => {
    const h = renderToString(
      <ModelStep ranked={rankCatalog({ query: 'zzz', detection: null })} query="zzz" onQuery={noop} selectedId={null} onSelect={noop} probe={null} showAll={false} onShowAll={noop} otherKind="grbl" onOtherKind={noop} />,
    );
    expect(h).toContain('Sin detección');
    expect(h).toContain('Ningún modelo coincide');
  });

  it('modelo: "Otro" muestra el tipo de firmware', () => {
    const h = renderToString(
      <ModelStep ranked={[]} query="" onQuery={noop} selectedId={OTHER_ID} onSelect={noop} probe={null} showAll={false} onShowAll={noop} otherKind="marlin" onOtherKind={noop} />,
    );
    expect(h).toContain('Tipo de firmware');
    expect(h).toContain('Marlin 2.x');
  });

  it('detalle de un modelo: cada dato con su fuente, lo desconocido avisa que se lea de la máquina', () => {
    const f = renderToString(<EntryDetail entry={findEntry('creality-falcon2-22w')!} />);
    expect(f).toContain('https://crealityfalcon.com/blogs/product-tutorials-and-support/the-creality-falcon2-series-comparison');
    expect(f).toContain('400 × 415 mm');
    expect(f).toContain('Oficial');
    const s30 = renderToString(<EntryDetail entry={findEntry('sculpfun-s30')!} />);
    expect(s30).toContain('Sin dato en el catálogo (se leerá de la máquina)');
    expect(s30).toContain('Cama');
    expect(s30).toContain('léela de la máquina');
  });

  it('revisar: tabla de diferencias con casillas "usar lo de la máquina"', () => {
    const entry = findEntry('creality-falcon2-22w')!;
    const comparison = compareWithMachine(entry, falconProProbe().settings);
    const h = renderToString(<ReviewStep comparison={comparison} useMachine={defaultMachineChoices(comparison)} onToggle={noop} kindMismatch={null} oldGrbl={false} />);
    expect(h).toContain('Catálogo frente a la máquina');
    expect(h).toContain('400 × 415');
    expect(h).toContain('400 × 400');
    expect(h).toContain('Coincide');
    expect(h).toContain('Distinto');
    expect(h).toContain('Usar lo de la máquina para Cama (mm)');
    expect(h).toContain('Modo láser activado');
    expect(h).not.toContain('role="alert"');
  });

  it('revisar: $32=0 avisa en rojo (alerta) y $32 distinto de 1 también', () => {
    const settings = { ...genericGrblProbe().settings!, $32: '0' };
    const comparison = compareWithMachine(findEntry('grbl-generic')!, settings);
    const h = renderToString(<ReviewStep comparison={comparison} useMachine={new Set()} onToggle={noop} kindMismatch={null} oldGrbl={false} />);
    expect(h).toContain('role="alert"');
    expect(h).toContain('El modo láser está desactivado ($32=0)');
    expect(renderToString(<LaserModeNotice mode="other" />)).toContain('role="alert"');
    expect(renderToString(<LaserModeNotice mode="unknown" />)).not.toContain('role="alert"');
  });

  it('revisar: sin ajustes explica que no se puede comparar; avisa de tipo distinto y de GRBL viejo', () => {
    const h = renderToString(<ReviewStep comparison={null} useMachine={new Set()} onToggle={noop} kindMismatch={{ detected: 'grbl_hal', chosen: 'grbl' }} oldGrbl />);
    expect(h).toContain('No hay ajustes leídos de la máquina');
    expect(h).toContain('grblHAL');
    expect(h).toContain('anterior a 1.1f');
  });

  it('guardar: nombre, valores y su origen, con aviso de lo que quedó con valor de partida', () => {
    const built = buildProfile({ entry: findEntry('sculpfun-s30')!, kind: 'grbl', name: 'Sculpfun S30', takenIds: [], probedBaud: null, settings: null, useMachine: new Set() });
    const h = renderToString(<SaveStep name="Sculpfun S30" onName={noop} built={built} />);
    expect(h).toContain('value="Sculpfun S30"');
    expect(h).toContain('Valor de partida: léelo de la máquina');
    expect(h).toContain('Hay datos que ni el catálogo ni la máquina aportaron');
    expect(renderToString(<SaveStep name="  " onName={noop} built={built} />)).toContain('Escribe un nombre.');
  });

  it('el asistente completo se pinta con sus pasos, su pie y la opción de saltar', () => {
    const h = renderToString(<AddMachineWizard />);
    expect(h).toContain('Agregar máquina');
    expect(h).toContain('Saltar la detección');
    expect(h).toContain('Cancelar');
    expect(h).toContain('Siguiente');
  });
});

describe('buildProfile', () => {
  const build = (id: string | null, probe: ProbeResult | null, useMachine?: Set<'bed' | 's_max' | 'max_rate' | 'accel'>, name = 'Mi láser') => {
    const entry = id ? findEntry(id)! : null;
    const settings = probe?.settings ?? null;
    return buildProfile({
      entry,
      kind: entry?.kind ?? probe?.suggested_kind ?? 'grbl',
      name,
      takenIds: ['otro'],
      probedBaud: probe ? 115200 : null,
      settings,
      useMachine: useMachine ?? defaultMachineChoices(compareWithMachine(entry, settings)),
    });
  };

  it('Falcon2 22 W sin conexión: usa solo el catálogo y es válido', () => {
    const { profile: p, provenance } = build('creality-falcon2-22w', null, new Set());
    expect(validateDevice(p)).toEqual({});
    expect(p).toMatchObject({ kind: 'grbl', bed: [400, 415], max_rate: [25000, 25000], rapid_rate: 25000, accel: [1000, 1000], s_max: 1000, baud: 115200, origin: 'bottom_left', z_axis: false, power_mode: 'dynamic' });
    expect(provenance).toMatchObject({ bed: 'catalog', max_rate: 'catalog', s_max: 'catalog', baud: 'catalog', origin: 'catalog' });
    expect(p.id).toBe('mi-laser');
  });

  it('con la máquina conectada: lo marcado "usar de la máquina" pisa al catálogo', () => {
    const probe = falconProProbe();
    const { profile: p, provenance } = build('creality-falcon2-22w', probe);
    expect(p.bed).toEqual([400, 400]);
    expect(p.max_rate).toEqual([50000, 50000]);
    expect(p.accel).toEqual([5000, 5000]);
    expect(p.rapid_rate).toBe(50000);
    expect(provenance).toMatchObject({ bed: 'machine', max_rate: 'machine', accel: 'machine', s_max: 'catalog' });
    expect(validateDevice(p)).toEqual({});
    // Si el usuario deja la cama del catálogo, se conserva.
    const keep = build('creality-falcon2-22w', probe, new Set(['max_rate']));
    expect(keep.profile.bed).toEqual([400, 415]);
    expect(keep.provenance.bed).toBe('catalog');
    expect(keep.profile.max_rate).toEqual([50000, 50000]);
  });

  it('lo que nadie sabe queda como valor de partida', () => {
    const { provenance } = build('sculpfun-s30', null, new Set());
    expect(provenance.bed).toBe('default');
    expect(provenance.accel).toBe('default');
    expect(provenance.max_rate).toBe('catalog');
  });

  it('Otro / genérico con el GRBL detectado toma los valores de la máquina', () => {
    const { profile: p, provenance } = build(null, genericGrblProbe());
    expect(p.bed).toEqual([400, 400]);
    expect(p.max_rate).toEqual([6000, 6000]);
    expect(p.baud).toBe(115200);
    expect(provenance.baud).toBe('machine');
    expect(validateDevice(p)).toEqual({});
  });

  it('Marlin y Smoothie parten de los valores de su firmware', () => {
    const marlin = build('marlin-generic', null, new Set()).profile;
    expect(marlin).toMatchObject({ kind: 'marlin', s_max: 255, baud: 250000, power_mode: 'constant' });
    expect(marlin.dialect.constant_on).toBe('M3 S0 I');
    const smoothie = build('smoothie-generic', null, new Set()).profile;
    expect(smoothie).toMatchObject({ kind: 'smoothie', s_max: 1, power_mode: 'constant' });
    expect(validateDevice(marlin)).toEqual({});
    expect(validateDevice(smoothie)).toEqual({});
  });

  it('GRBL-LPC: $30 = 1 de la máquina se respeta', () => {
    const probe: ProbeResult = { ...genericGrblProbe(), family: 'grbl_lpc', suggested_kind: 'grbl_lpc', settings: { $30: '1.000', $32: '1' } };
    const { profile: p } = build('k40-cohesion3d', probe);
    expect(p).toMatchObject({ kind: 'grbl_lpc', s_max: 1, bed: [300, 200] });
  });

  it('el id no choca con los existentes', () => {
    expect(build(null, null, new Set(), 'Otro').profile.id).toBe('otro-2');
  });

  it('todos los modelos del catálogo producen un perfil válido', () => {
    for (const e of CATALOG) {
      const { profile } = buildProfile({ entry: e, kind: e.kind, name: `${e.brand} ${e.model}`, takenIds: [], probedBaud: null, settings: null, useMachine: new Set() });
      expect(validateDevice(profile), e.id).toEqual({});
    }
  });
});

describe('guardar y dejar activa', () => {
  it('saveDevice del perfil armado lo agrega y setActiveDevice lo deja activo', async () => {
    await useStore.getState().loadDevices();
    const { profile } = buildProfile({
      entry: findEntry('creality-falcon2-pro')!,
      kind: 'grbl',
      name: 'Falcon2 Pro del taller',
      takenIds: useStore.getState().devices.map((d) => d.id),
      probedBaud: 115200,
      settings: falconProProbe().settings,
      useMachine: new Set(['bed']),
    });
    expect(await useStore.getState().saveDevice(profile)).toBe(true);
    useStore.getState().setActiveDevice(profile.id);
    const active = selectActiveDevice(useStore.getState());
    expect(active.id).toBe(profile.id);
    expect(active.name).toBe('Falcon2 Pro del taller');
    expect(active.bed).toEqual([400, 400]);
    expect(useStore.getState().devices.some((d) => d.id === profile.id)).toBe(true);
  });
});

describe('comando', () => {
  it('"Agregar máquina…" está en la categoría machine y abre el asistente', async () => {
    const def = commands.get('machine.addDevice');
    expect(def?.category).toBe('machine');
    expect(es[def!.label]).toBe('Agregar máquina…');
    expect(commands.availability('machine.addDevice', useStore.getState())).toBe(true);
    await commands.run('machine.addDevice');
    expect(useDevicesWizard.getState().open).toBe(true);
  });
});
