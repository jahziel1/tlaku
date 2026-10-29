import { beforeEach, describe, expect, it } from 'vitest';
import { getApi, setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import type { DesignObject } from '@/api/types';
import { applyImport, currentState } from '@/features/bundle/apply';
import { exportUserBundle, readUserBundle } from '@/features/bundle/files';
import { allSections, planImport, sectionChanges } from '@/features/bundle/model';
import { buildCurrentBundle } from '@/features/bundle/sources';
import { polylineBez } from '@/lib/bezpath';
import { makeLayer } from '@/lib/layers';
import { useStore } from '@/store';
import { useArtPanel } from './artStore';
import { saveSelectionAsComponent } from './componentActions';

const square = (id: string, layer: string, x: number): DesignObject => ({
  id,
  layer,
  transform: [1, 0, 0, 1, x, 0],
  kind: { type: 'vector', paths: [polylineBez([[0, 0], [10, 0], [10, 10], [0, 10]], true)] },
  locked: false,
  hidden: false,
  group: null,
});

const s = () => useStore.getState();

beforeEach(async () => {
  setApi(createMockApi({ latencyMs: 0 }));
  useArtPanel.setState({ components: [], user: [], saving: null, error: null });
  useStore.setState({ notices: [] });
  s().newDocument();
  await s().loadDevices();
  s().applyImport({
    objects: [square('a', '#ff0000', 0), square('b', '#ff0000', 20), square('c', '#0000ff', 40)],
    new_layers: [makeLayer('#ff0000', 'Corte', { speed: 900 }), makeLayer('#0000ff', 'Grabado')],
    warnings: [],
  });
});

describe('componentes en el respaldo y el paquete', () => {
  it('viajan con la casilla de la biblioteca de arte: ida y vuelta, sin pisar los locales', async () => {
    await saveSelectionAsComponent('hoja', ['a', 'b', 'c']);
    await saveSelectionAsComponent('panel', ['a']);
    expect(await exportUserBundle(allSections())).toBe(true);
    const file = await readUserBundle('tlaku.tlakubundle');
    expect(file.components?.items.map((c) => c.name)).toEqual(['hoja', 'panel']);
    // Sin la casilla del arte no se escriben.
    expect(await buildCurrentBundle({ materials: true })).not.toHaveProperty('components');

    // Otro equipo: ya tiene un «hoja» distinto y ningún «panel».
    await getApi().componentRemoveUser({ name: 'hoja' });
    await getApi().componentRemoveUser({ name: 'panel' });
    await saveSelectionAsComponent('hoja', ['c']);
    const plan = planImport(file, await currentState());
    expect(plan.art?.components.map((c) => c.name)).toEqual(['hoja', 'panel']);
    expect(plan.art).toMatchObject({ componentsRenamed: 1, componentsSkipped: 0, componentsInvalid: 0 });
    expect(sectionChanges(plan).art).toBe(2);
    // La vista previa no escribió nada.
    expect((await getApi().componentListUser()).map((c) => c.name)).toEqual(['hoja']);

    const result = await applyImport(plan, allSections(), 'tlaku.tlakubundle');
    expect(result.art).toMatchObject({ failed: 0, added: 2 });
    const list = await getApi().componentListUser();
    expect(list.map((c) => c.name)).toEqual(['hoja', 'hoja-2', 'panel']);
    // Lo local sigue siendo lo local.
    expect(list.find((c) => c.name === 'hoja')!.data.objects).toHaveLength(1);
  });

  it('un paquete con el mismo componente no lo duplica y descarta los inválidos', async () => {
    await saveSelectionAsComponent('hoja', ['a', 'b']);
    const file = await buildCurrentBundle(allSections());
    file.components!.items.push({ name: 'roto', data: { format: 'otro' } as never });
    const plan = planImport(file, await currentState());
    expect(plan.art).toMatchObject({ componentsSkipped: 1, componentsInvalid: 1 });
    expect(plan.art?.components).toEqual([]);
  });
});
