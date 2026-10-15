import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { setApi, getApi, type UserBundle } from '@/api';
import { createMockApi } from '@/api/mock';
import { sampleMaterialLibrary } from '@/api/mock/materials';
import { sampleDevices } from '@/api/mock/sample';
import { commands, MENUS } from '@/commands';
import { useMaterials } from '@/features/materials/state';
import { emptyLibrary } from '@/features/materials/library';
import { es } from '@/i18n';
import { bundle as dict } from '@/i18n/es/bundle';
import { DEFAULT_PREFS, useStore } from '@/store';
import { applyImport, currentState, importBlockedReason } from './apply';
import { applicableSections, BundleDialog, ExportBody, ImportPreview } from './BundleDialog';
import { exportUserBundle, readUserBundle } from './files';
import { allSections, buildUserBundle, planImport, sanitizeDevices, sectionChanges, type CurrentState } from './model';
import { useBundleUi } from './uiStore';

const known = new Set(commands.list().map((c) => c.id));

function current(over: Partial<CurrentState> = {}): CurrentState {
  return {
    devices: sampleDevices(),
    activeDeviceId: sampleDevices()[0]!.id,
    library: emptyLibrary(),
    prefs: { ...DEFAULT_PREFS },
    shortcuts: {},
    knownCommands: known,
    ...over,
  };
}

const source = (over: Partial<CurrentState> = {}) => {
  const c = current(over);
  return { devices: c.devices, activeDeviceId: c.activeDeviceId, library: c.library, prefs: c.prefs, shortcuts: c.shortcuts };
};

async function reset() {
  setApi(createMockApi({ latencyMs: 0 }));
  useMaterials.setState({ library: emptyLibrary(), loaded: false, loadError: null });
  useStore.setState({ jobState: 'idle', notices: [] });
  useBundleUi.setState({ mode: null });
  await useStore.getState().loadDevices();
}

beforeEach(reset);

describe('textos', () => {
  it('todas las claves empiezan con bundle. y los comandos están en el diccionario', () => {
    for (const key of Object.keys(dict)) expect(key.startsWith('bundle.'), key).toBe(true);
    for (const id of ['bundle.export', 'bundle.import']) {
      const c = commands.get(id);
      expect(c, id).toBeTruthy();
      expect(es).toHaveProperty(c!.label);
    }
  });

  it('los comandos de paquetes y de respaldo están en el menú Archivo', () => {
    const file = MENUS.find((m) => m.id === 'file')!;
    expect(file.items).toEqual(expect.arrayContaining(['bundle.export', 'bundle.import', 'backup.open', 'backup.now']));
  });
});

describe('buildUserBundle', () => {
  it('lleva el sobre, las secciones y la máquina activa', () => {
    const now = new Date('2026-01-02T03:04:05.000Z');
    const b = buildUserBundle(source({ library: sampleMaterialLibrary() }), { app: '0.1.0', now });
    expect(b).toMatchObject({ format: 'tlaku-bundle', version: 2, created: '2026-01-02T03:04:05.000Z', app: '0.1.0' });
    expect(b.devices).toHaveLength(sampleDevices().length);
    expect(b.active_device).toBe('falcon2-22w');
    expect(b.materials?.presets.length).toBeGreaterThan(0);
    expect(b.prefs).toMatchObject({ format: 'tlaku-prefs', version: 1 });
  });

  it('una sección no elegida no se escribe', () => {
    const b = buildUserBundle(source(), { sections: { devices: false, materials: true, prefs: false } });
    expect('devices' in b).toBe(false);
    expect('active_device' in b).toBe(false);
    expect(b.prefs).toBeNull();
    expect(b.materials).not.toBeNull();
  });

  it('es una copia: cambiar el paquete no toca el estado', () => {
    const src = source();
    const b = buildUserBundle(src);
    b.devices![0]!.name = 'otro';
    expect(src.devices[0]!.name).not.toBe('otro');
  });
});

describe('planImport', () => {
  it('sobre el mismo estado todo queda sin cambios', () => {
    const plan = planImport(buildUserBundle(source({ library: sampleMaterialLibrary() })), current({ library: sampleMaterialLibrary() }));
    expect(plan.devices).toMatchObject({ added: [], replaced: [], activeDevice: null });
    expect(plan.devices!.unchanged).toHaveLength(sampleDevices().length);
    expect(plan.materials).toMatchObject({ added: 0 });
    expect(sectionChanges(plan).devices).toBe(0);
    expect(sectionChanges(plan).materials).toBe(0);
  });

  it('distingue nuevos, reemplazos y sin cambios (sin importar el orden de las claves)', () => {
    const [a, b] = sampleDevices();
    const reordered = Object.fromEntries(Object.entries(a!).reverse()) as typeof a;
    const changed = { ...b!, name: 'Renombrada' };
    const fresh = { ...a!, id: 'nueva', name: 'Nueva' };
    const bundle = buildUserBundle(source({ devices: [reordered!, changed, fresh] }));
    const plan = planImport(bundle, current({ devices: [a!, b!] }));
    expect(plan.devices!.unchanged.map((d) => d.id)).toEqual([a!.id]);
    expect(plan.devices!.replaced.map((d) => d.name)).toEqual(['Renombrada']);
    expect(plan.devices!.added.map((d) => d.id)).toEqual(['nueva']);
  });

  it('completa los perfiles viejos y descarta con aviso los inválidos', () => {
    const ok = sampleDevices()[0]!;
    const bundle = {
      ...buildUserBundle(source()),
      devices: [
        { id: 'viejo', name: 'Viejo', kind: 'grbl', bed: [200, 200] }, // sin muchos campos: se completa
        { id: 'roto', name: 'Roto', bed: [-5, 0], baud: 0 },
        { name: 'Sin id' },
        { id: 'sinnombre', name: '  ' },
        'texto',
        ok,
        { ...ok, name: 'Repetido' },
      ],
    } as unknown as UserBundle;
    const plan = planImport(bundle, current({ devices: [] }));
    expect(plan.devices!.added.map((d) => d.id)).toEqual(['viejo', ok.id]);
    expect(plan.devices!.added[0]!.dialect).toBeTruthy();
    expect(plan.devices!.invalid.map((i) => [i.id, i.reason])).toEqual([
      ['roto', 'invalid'],
      ['#3', 'noId'],
      ['sinnombre', 'noName'],
      ['#5', 'notObject'],
      [ok.id, 'duplicate'],
    ]);
    expect(plan.devices!.invalid[0]!.fields!.length).toBeGreaterThan(0);
  });

  it('la máquina activa solo cuenta si existiría y es distinta de la actual', () => {
    const [a, b] = sampleDevices();
    const base = buildUserBundle(source({ devices: [a!, b!], activeDeviceId: b!.id }));
    expect(planImport(base, current({ devices: [a!, b!], activeDeviceId: a!.id })).devices!.activeDevice).toBe(b!.id);
    expect(planImport(base, current({ devices: [a!, b!], activeDeviceId: b!.id })).devices!.activeDevice).toBeNull();
    expect(planImport({ ...base, active_device: 'fantasma' }, current()).devices!.activeDevice).toBeNull();
    expect(planImport({ ...base, active_device: null }, current()).devices!.activeDevice).toBeNull();
  });

  it('materiales: mezcla sin perder lo local y avisa de los inválidos', () => {
    const lib = sampleMaterialLibrary();
    const local = { version: 1 as const, presets: [lib.presets[0]!] };
    const clash = { ...lib.presets[1]!, id: lib.presets[0]!.id }; // mismo id, contenido distinto
    const incoming = { version: 1, presets: [lib.presets[0], clash, lib.presets[2], { id: 'x' }, 'basura'] };
    const plan = planImport({ ...buildUserBundle(source()), materials: incoming as never }, current({ library: local }));
    expect(plan.materials).toMatchObject({ added: 2, renamed: 1, skipped: 1, invalid: 2 });
    expect(plan.materials!.library.presets).toHaveLength(3);
    expect(plan.materials!.library.presets[0]).toEqual(local.presets[0]);
  });

  it('un paquete parcial deja las demás secciones en null', () => {
    const prefsOnly = buildUserBundle(source(), { sections: { devices: false, materials: false, prefs: true } });
    const plan = planImport(prefsOnly, current());
    expect(plan.devices).toBeNull();
    expect(plan.materials).toBeNull();
    expect(plan.prefs).not.toBeNull();
  });

  it('preferencias: aplica las válidas e ignora las que no entiende', () => {
    const b = buildUserBundle(source());
    b.prefs = { format: 'tlaku-prefs', version: 1, prefs: { snapPx: 12, inventada: 1, units: 'parsecs' }, shortcuts: { 'edit.undo': ['Mod+U'], 'no.existe': ['Mod+9'] } };
    const { change } = planImport(b, current()).prefs!;
    expect(change.prefs.snapPx).toBe(12);
    expect(change.appliedPrefs).toBe(1);
    expect(change.ignoredPrefs).toBe(2);
    expect(change.appliedShortcuts).toBe(2);
    expect(change.unknownCommands).toBe(1);
  });

  it('rechaza lo que no es un paquete o es de otra versión', () => {
    const ok = buildUserBundle(source());
    expect(() => planImport({ ...ok, format: 'tlaku-prefs' }, current())).toThrow();
    expect(() => planImport({ ...ok, version: 3 }, current())).toThrow();
    expect(() => planImport({ ...ok, version: 1 }, current())).not.toThrow();
    expect(() => planImport(null, current())).toThrow();
    expect(() => planImport([], current())).toThrow();
  });

  it('sanitizeDevices acepta una lista vacía y rechaza lo que no es lista', () => {
    expect(sanitizeDevices([])).toEqual({ devices: [], invalid: [] });
    expect(sanitizeDevices('x')).toEqual({ devices: [], invalid: [] });
  });
});

describe('mock de los paquetes', () => {
  it('exige la extensión y valida el sobre como el motor', async () => {
    const api = getApi();
    const b = buildUserBundle(source());
    await expect(api.exportUserBundle({ path: 'x.json', bundle: b })).rejects.toThrow('.tlakubundle');
    await expect(api.exportUserBundle({ path: 'x.tlakubundle', bundle: { ...b, version: 3 } as never })).rejects.toThrow('no soportada');
    await expect(api.exportUserBundle({ path: 'x.tlakubundle', bundle: { ...b, devices: {} } as never })).rejects.toThrow('devices');
    await expect(api.importUserBundle({ path: 'falta.tlakubundle' })).rejects.toThrow();
    await api.exportUserBundle({ path: 'x.tlakubundle', bundle: b });
    expect(await api.importUserBundle({ path: 'x.tlakubundle' })).toEqual(b);
  });
});

describe('ida y vuelta contra el mock', () => {
  it('exporta todo, cambia el estado y al importar vuelve a lo exportado', async () => {
    const s = useStore.getState();
    // Estado A: una máquina con macros, materiales de ejemplo y una preferencia propia.
    const base = sampleDevices()[0]!;
    await s.saveDevice({ ...base, macros: [{ name: 'Home', gcode: '$H', confirm: false }], positions: [{ name: 'Centro', x: 100, y: 100 }] });
    s.setActiveDevice(base.id);
    useMaterials.setState({ library: sampleMaterialLibrary(), loaded: true });
    s.replacePrefs({ prefs: { snapPx: 15 }, shortcuts: { 'edit.undo': ['Mod+U'] } }, false);
    expect(await exportUserBundle(allSections())).toBe(true);

    // Estado B: otro.
    await s.saveDevice({ ...base, name: 'Cambiada', macros: [], positions: [] });
    useMaterials.setState({ library: emptyLibrary() });
    s.replacePrefs({ prefs: { snapPx: 4 }, shortcuts: {} }, false);

    const imported = await readUserBundle('tlaku.tlakubundle');
    const plan = planImport(imported, await currentState());
    expect(plan.devices!.replaced.map((d) => d.id)).toEqual([base.id]);
    expect(plan.devices!.replaced[0]!.macros).toHaveLength(1);
    expect(plan.materials!.added).toBe(sampleMaterialLibrary().presets.length);
    // Vista previa: nada se escribió.
    expect(useStore.getState().devices.find((d) => d.id === base.id)!.name).toBe('Cambiada');
    expect(useStore.getState().prefs.snapPx).toBe(4);

    const result = await applyImport(plan, allSections(), 'tlaku.tlakubundle');
    expect(result).toMatchObject({ blocked: false, devices: { saved: 1, failed: 0 }, materials: { added: sampleMaterialLibrary().presets.length } });
    const restored = useStore.getState().devices.find((d) => d.id === base.id)!;
    expect(restored.name).toBe(base.name);
    expect(restored.positions).toEqual([{ name: 'Centro', x: 100, y: 100 }]);
    expect(useMaterials.getState().library.presets).toHaveLength(sampleMaterialLibrary().presets.length);
    expect((await getApi().loadMaterialLibrary()).presets).toHaveLength(sampleMaterialLibrary().presets.length);
    expect(useStore.getState().prefs.snapPx).toBe(15);
    expect(useStore.getState().shortcutOverrides).toEqual({ 'edit.undo': ['Mod+U'] });
    expect(useStore.getState().notices.at(-1)?.level).toBe('success');
  });

  it('solo aplica las secciones marcadas', async () => {
    const s = useStore.getState();
    const b = buildUserBundle(source({ prefs: { ...DEFAULT_PREFS, snapPx: 21 }, library: sampleMaterialLibrary() }));
    useMaterials.setState({ library: emptyLibrary(), loaded: true });
    const plan = planImport(b, await currentState());
    const before = useStore.getState().prefs.snapPx;
    const result = await applyImport(plan, { ...allSections(false), materials: true });
    expect(result.devices).toBeNull();
    expect(result.prefs).toBeNull();
    expect(result.materials!.added).toBeGreaterThan(0);
    expect(useStore.getState().prefs.snapPx).toBe(before);
    expect(useStore.getState().devices).toBe(s.devices);
    expect((await getApi().loadMaterialLibrary()).presets.length).toBeGreaterThan(0);
  });

  it('el perfil importado nuevo queda guardado y puede quedar como máquina activa', async () => {
    const nueva = { ...sampleDevices()[0]!, id: 'traida', name: 'Traída' };
    const b = buildUserBundle(source({ devices: [nueva], activeDeviceId: 'traida' }));
    const plan = planImport(b, await currentState());
    await applyImport(plan, allSections());
    expect(useStore.getState().devices.some((d) => d.id === 'traida')).toBe(true);
    expect(useStore.getState().activeDeviceId).toBe('traida');
    expect(sectionChanges(plan).devices).toBe(2);
  });

  it('con un trabajo en curso no se aplica nada', async () => {
    const plan = planImport(buildUserBundle(source({ prefs: { ...DEFAULT_PREFS, snapPx: 30 } })), await currentState());
    useStore.setState({ jobState: 'running' });
    expect(importBlockedReason(useStore.getState())).toBe('bundle.blocked.job');
    const result = await applyImport(plan, allSections());
    expect(result.blocked).toBe(true);
    expect(useStore.getState().prefs.snapPx).not.toBe(30);
    expect(useStore.getState().notices.at(-1)?.level).toBe('warning');
  });

  it('no pisa la biblioteca si no se pudo leer', async () => {
    const plan = planImport(buildUserBundle(source({ library: sampleMaterialLibrary() })), current());
    const base = createMockApi({ latencyMs: 0 });
    setApi({ ...base, loadMaterialLibrary: () => Promise.reject(new Error('rota')) });
    useMaterials.setState({ library: emptyLibrary(), loaded: false, loadError: null });
    const result = await applyImport(plan, { ...allSections(false), materials: true });
    expect(result.materials).toBeNull();
    expect(useMaterials.getState().library.presets).toHaveLength(0);
    expect(useStore.getState().notices.some((n) => n.level === 'error')).toBe(true);
  });
});

describe('diálogo', () => {
  it('la exportación pinta una casilla por sección con su detalle', () => {
    const html = renderToString(<ExportBody choice={allSections()} onChange={() => {}} counts={{ devices: 3, materials: 7 }} />);
    expect((html.match(/type="checkbox"/g) ?? []).length).toBe(9);
    expect(html).toContain('Máquinas');
    expect(html).toContain('Materiales');
    expect(html).toContain('Preferencias y atajos');
    expect(html).toContain('3 perfiles de máquina');
    expect(html).not.toMatch(/bundle\.[a-z]/);
  });

  it('el diálogo de importar es un diálogo accesible con el botón de elegir archivo', () => {
    const html = renderToString(<BundleDialog mode="import" onClose={() => {}} />);
    expect(html).toContain('role="dialog"');
    expect(html).toContain('Importar paquete de configuración');
    expect(html).toContain('Elegir archivo…');
    expect(html).not.toMatch(/bundle\.[a-z]/);
  });

  it('la vista previa resume cada sección, avisa de los descartes y respeta las casillas', () => {
    const [a, b] = sampleDevices();
    const bundle = {
      ...buildUserBundle(source({ devices: [{ ...a!, name: 'Cambiada' }, { ...b!, id: 'nueva', name: 'Nueva' }], library: sampleMaterialLibrary() })),
    } as UserBundle;
    bundle.devices = [...bundle.devices!, { id: 'roto', name: 'Roto', bed: [0, 0] } as never];
    const plan = planImport(bundle, current({ devices: [a!] }));
    const html = renderToString(<ImportPreview plan={plan} choice={{ ...allSections(false), devices: true, prefs: true }} onChange={() => {}} />);
    expect(html).toContain('1 nuevas');
    expect(html).toContain('Chocan con una máquina local del mismo id: Cambiada');
    expect(html).toContain('1 perfiles descartados');
    expect(html).toContain('«roto»');
    expect(html).not.toMatch(/bundle\.[a-z]/);
    expect(html.match(/checked=""/g)?.length).toBe(2); // máquinas y preferencias; materiales desmarcado
  });

  it('una sección ausente aparece desactivada y no se aplica', () => {
    const prefsOnly = buildUserBundle(source(), { sections: { devices: false, materials: false, prefs: true } });
    const plan = planImport(prefsOnly, current());
    const html = renderToString(<ImportPreview plan={plan} choice={allSections()} onChange={() => {}} />);
    expect((html.match(/disabled=""/g) ?? []).length).toBe(7);
    expect(html).toContain('no incluye esta sección');
    expect(applicableSections(plan, allSections())).toEqual({ ...allSections(false), prefs: true });
  });

  it('los comandos abren el diálogo en su modo', async () => {
    await commands.run('bundle.export');
    expect(useBundleUi.getState().mode).toBe('export');
    await commands.run('bundle.import');
    expect(useBundleUi.getState().mode).toBe('import');
  });
});
