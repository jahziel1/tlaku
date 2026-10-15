import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getApi, setApi, type LaserApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { exampleCatalogIndex } from '@/api/mock/communityCatalog';
import type { CatalogEntry, CatalogState } from '@/api/types';
import { useHelpUi } from '@/features/help/helpStore';
import { materialShareUrl } from '@/features/help/githubForms';
import { REPO_URL } from '@/features/help/links';
import { openEnv } from '@/features/help/openExternal';
import { usePrompts } from '@/features/history/prompt';
import { setLanguage } from '@/i18n';
import { makeLayer } from '@/lib/layers';
import { selectActiveDevice, useStore } from '@/store';
import { applyEntryToLayer, closeCommunity, copyEntryToLibrary, entryNotes, openCommunity, refreshCommunity, shareFromActiveLayer } from './communityActions';
import { EMPTY_FILTER, myMachine } from './communityModel';
import { useCommunity } from './communityStore';
import { loadMaterials, persistMaterials } from './actions';
import { emptyLibrary } from './library';
import { useMaterials } from './state';

const RED = '#ff3b4a';
const ENTRIES = exampleCatalogIndex().entries;
const falcon = ENTRIES.find((e) => e.id.endsWith('.101'))!;

/** La API de prueba, anotando cada llamada: sirve para comprobar qué toca (y qué NO toca) cada acción. */
function recording(inner: LaserApi): { api: LaserApi; calls: string[] } {
  const calls: string[] = [];
  const api = new Proxy(inner, {
    get(target, key) {
      const value = Reflect.get(target, key) as unknown;
      if (typeof value !== 'function') return value;
      return (...args: unknown[]) => {
        calls.push(String(key));
        return (value as (...a: unknown[]) => unknown).apply(target, args);
      };
    },
  });
  return { api, calls };
}

function reset(overrides: Partial<LaserApi> = {}) {
  const { api, calls } = recording({ ...createMockApi({ latencyMs: 0 }), ...overrides });
  setApi(api);
  setLanguage('es');
  useCommunity.setState({ open: false, catalog: null, status: 'idle', filter: EMPTY_FILTER, selectedId: null });
  useMaterials.setState({ library: emptyLibrary(), factory: emptyLibrary(), machineClass: null, loaded: false, loadError: null, search: '', onlyDevice: false, editor: null });
  usePrompts.setState({ current: null, queue: [] });
  useHelpUi.setState({ dialog: null, sharePreset: null });
  const s = useStore.getState();
  s.replaceDocument({ version: 2, objects: [], layers: [makeLayer(RED, 'Corte', { speed: 280, power: 95 })], device_id: null }, null);
  s.setActiveLayer(RED);
  return { api, calls };
}

const mine = (e: CatalogEntry) => useMaterials.getState().library.presets.filter((p) => p.source === e.source);
const layer = () => useStore.getState().doc.layers.find((l) => l.color === RED)!;
const lastNotice = () => useStore.getState().notices.at(-1);
/** Contesta la pregunta pendiente (la acción la hace con `askChoice`). */
async function answer(id: string) {
  await vi.waitFor(() => expect(usePrompts.getState().current).not.toBeNull());
  const prompt = usePrompts.getState().current!;
  usePrompts.getState().answer(id);
  return prompt;
}

let calls: string[];
beforeEach(() => {
  ({ calls } = reset());
});

describe('descarga: solo cuando se abre el navegador', () => {
  it('cargar la biblioteca de materiales (lo que pasa al arrancar) no toca el catálogo', async () => {
    await loadMaterials(true);
    expect(calls).not.toContain('communityCatalogRefresh');
    expect(calls).not.toContain('communityCatalogCached');
    expect(useCommunity.getState().open).toBe(false);
    expect(useCommunity.getState().catalog).toBeNull();
  });

  it('al abrirlo lee primero la copia guardada (sin red) y después pide el catálogo', async () => {
    await openCommunity();
    const order = calls.filter((c) => c.startsWith('communityCatalog'));
    expect(order).toEqual(['communityCatalogCached', 'communityCatalogRefresh']);
    const s = useCommunity.getState();
    expect(s.open).toBe(true);
    expect(s.status).toBe('done');
    expect(s.catalog?.source).toBe('network');
    expect(s.catalog?.index?.entries).toHaveLength(ENTRIES.length);
  });

  it('cerrarlo no vuelve a descargar nada; reabrirlo sí, porque la persona lo abrió', async () => {
    await openCommunity();
    closeCommunity();
    expect(useCommunity.getState().open).toBe(false);
    expect(calls.filter((c) => c === 'communityCatalogRefresh')).toHaveLength(1);
    await openCommunity();
    expect(calls.filter((c) => c === 'communityCatalogRefresh')).toHaveLength(2);
  });

  it('dos «Actualizar» seguidos hacen una sola descarga', async () => {
    useCommunity.getState().setOpen(true);
    await Promise.all([refreshCommunity(), refreshCommunity()]);
    expect(calls.filter((c) => c === 'communityCatalogRefresh')).toHaveLength(1);
  });

  it('sin conexión muestra la última copia, con su fecha y el motivo', async () => {
    const copy: CatalogState = { index: exampleCatalogIndex(), source: 'cache', fetched_at: Date.UTC(2026, 9, 12, 15, 30), error: 'No se pudo conectar con el sitio de Tlaku: sin red.' };
    ({ calls } = reset({ communityCatalogCached: () => Promise.resolve({ ...copy, error: null }), communityCatalogRefresh: () => Promise.resolve(copy) }));
    await openCommunity();
    const { catalog, status } = useCommunity.getState();
    expect(status).toBe('done');
    expect(catalog?.source).toBe('cache');
    expect(catalog?.fetched_at).toBe(copy.fetched_at);
    expect(catalog?.error).toContain('sin red');
    expect(catalog?.index?.entries).toHaveLength(ENTRIES.length);
  });

  it('si la propia llamada falla conserva lo que ya había y deja el error', async () => {
    ({ calls } = reset({ communityCatalogRefresh: () => Promise.reject(new Error('boom')) }));
    await openCommunity();
    const { catalog, status } = useCommunity.getState();
    expect(status).toBe('done');
    expect(catalog?.index).toBeNull();
    expect(catalog?.error).toContain('boom');
  });

  it('sin copia y sin red hay estado «ninguno» con motivo, no una pantalla rota', async () => {
    ({ calls } = reset({ communityCatalogRefresh: () => Promise.resolve({ index: null, source: 'none', fetched_at: null, error: 'sin red' }) }));
    await openCommunity();
    expect(useCommunity.getState().catalog).toMatchObject({ index: null, source: 'none', error: 'sin red' });
  });
});

describe('«Usar»: copiar a mi biblioteca', () => {
  it('guarda una copia con su origen y no toca la capa ni la máquina', async () => {
    await copyEntryToLibrary(falcon);
    const [p] = mine(falcon);
    expect(p).toMatchObject({ thickness: 3, role: 'cut', machine_class: 'diode_20', source: falcon.source, confidence: 'forum', device_id: null });
    expect(p!.settings).toMatchObject({ speed: 500, power: 100, passes: 2, air: true, mode: 'line' });
    expect(p!.notes).toContain('@ejemplo-uno');
    expect(p!.notes).toContain('Creality Falcon2 22 W');
    expect(p!.notes).toContain('Verificado por 2 personas');
    // Se guardó en disco con la API normal de la biblioteca.
    await persistMaterials();
    expect(calls).toContain('saveMaterialLibrary');
    expect((await getApi().loadMaterialLibrary()).presets.filter((x) => x.source === falcon.source)).toHaveLength(1);
    expect(lastNotice()?.level).toBe('success');
    // La capa quedó igual y no se llamó a nada de la máquina.
    expect(layer()).toMatchObject({ speed: 280, power: 95 });
    expect(calls.filter((c) => !/^(communityCatalog|loadMaterialLibrary|loadFactoryMaterials|saveMaterialLibrary)/.test(c))).toEqual([]);
  });

  it('el mismo aporte con los mismos valores no se duplica', async () => {
    expect(await copyEntryToLibrary(falcon)).toBe('added');
    expect(await copyEntryToLibrary(falcon)).toBe('exists');
    expect(mine(falcon)).toHaveLength(1);
    expect(lastNotice()?.level).toBe('info');
  });

  it('si la biblioteca no se pudo leer no guarda nada (no pisa el archivo)', async () => {
    useMaterials.setState({ loaded: true, loadError: 'archivo dañado' });
    expect(await copyEntryToLibrary(falcon)).toBe('blocked');
    expect(mine(falcon)).toHaveLength(0);
    expect(lastNotice()?.level).toBe('error');
  });

  it('las notas dicen de dónde viene sin inventar datos', () => {
    const anon: CatalogEntry = { ...falcon, contributor: undefined, verified_by: 0, confirmations: undefined, notes: undefined };
    expect(entryNotes(anon)).toBe('Comunidad · Creality Falcon2 22 W · 22 W');
  });
});

describe('«Aplicar a la capa»: con confirmación', () => {
  const me = () => myMachine(selectActiveDevice(useStore.getState()), 'diode_20');

  it('pregunta antes y, si se cancela, no cambia nada', async () => {
    const run = applyEntryToLayer(falcon, me());
    const prompt = await answer('cancel');
    expect(await run).toBe(false);
    expect(prompt.message).toContain('500 mm/min · 100 % · ×2');
    expect(prompt.message).toContain('Corte');
    expect(layer()).toMatchObject({ speed: 280, power: 95, passes: 1 });
  });

  it('al confirmar copia los valores a la capa con un paso de deshacer y no manda nada a la máquina', async () => {
    const run = applyEntryToLayer(falcon, me());
    await answer('apply');
    expect(await run).toBe(true);
    expect(layer()).toMatchObject({ speed: 500, power: 100, passes: 2, air: true });
    expect(calls.filter((c) => !/^(communityCatalog|loadMaterialLibrary|loadFactoryMaterials|saveMaterialLibrary)/.test(c))).toEqual([]);
    useStore.getState().undo();
    expect(layer()).toMatchObject({ speed: 280, power: 95 });
  });

  it('de otra potencia o de otro láser, la pregunta lo advierte con claridad', async () => {
    const other = ENTRIES.find((e) => e.id.endsWith('.103'))!; // diodo de 10 W
    const run = applyEntryToLayer(other, me());
    const prompt = await answer('cancel');
    await run;
    expect(prompt.message).toContain('solo orientativo');
    expect(prompt.message).toContain('10 W');

    const co2 = ENTRIES.find((e) => e.id.endsWith('.104'))!;
    const run2 = applyEntryToLayer(co2, me());
    const prompt2 = await answer('cancel');
    await run2;
    expect(prompt2.message).toContain('probablemente no sirva tal cual');
  });

  it('sin capa seleccionada no pregunta nada y lo dice', async () => {
    useStore.getState().setActiveLayer(null);
    expect(await applyEntryToLayer(falcon, me())).toBe(false);
    expect(usePrompts.getState().current).toBeNull();
    expect(lastNotice()?.level).toBe('warning');
  });
});

describe('compartir: arma el formulario y no envía nada', () => {
  it('abre el diálogo con los valores de la capa y la dirección del formulario del repositorio', () => {
    const open = vi.spyOn(openEnv, 'open').mockResolvedValue(undefined);
    useCommunity.getState().setOpen(true);
    shareFromActiveLayer();
    expect(useCommunity.getState().open).toBe(false);
    const help = useHelpUi.getState();
    expect(help.dialog).toBe('shareMaterial');
    expect(help.sharePreset).toMatchObject({ settings: { speed: 280, power: 95 } });
    const url = new URL(materialShareUrl(help.sharePreset, selectActiveDevice(useStore.getState()), null, '0.1.0'));
    expect(`${url.origin}${url.pathname}`).toBe(`${REPO_URL}/issues/new`);
    expect(url.searchParams.get('template')).toBe('material_settings.yml');
    expect(url.searchParams.get('speed')).toBe('280');
    expect(url.searchParams.get('power')).toBe('95');
    expect(url.searchParams.get('tlaku_version')).toBe('0.1.0');
    // Nada se abre ni se envía solo.
    expect(open).not.toHaveBeenCalled();
    open.mockRestore();
  });

  it('sin capa abre el diálogo vacío (solo la máquina)', () => {
    useStore.getState().setActiveLayer(null);
    shareFromActiveLayer();
    expect(useHelpUi.getState()).toMatchObject({ dialog: 'shareMaterial', sharePreset: null });
  });
});
