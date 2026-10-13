import { describe, expect, it } from 'vitest';
import { traceCommands, traceEnabled } from '@/commands/definitions/trace';
import { commands } from '@/commands';
import { es } from '@/i18n';
import { createAppStore } from '@/store';
import { sampleObjects } from '@/api/mock/sample';
import type { DesignObject } from '@/api/types';
import { useTraceDialog } from './traceStore';

const image = (id: string): DesignObject => ({
  id,
  layer: '#808080',
  transform: [1, 0, 0, 1, 0, 0],
  kind: { type: 'image', source_path: '/tmp/a.png', width_px: 10, height_px: 10, size_mm: [10, 10] },
  locked: false,
  hidden: false,
  group: null,
});

function stateWith(objects: DesignObject[], selected: string[]) {
  const store = createAppStore();
  store.getState().replaceDocument({ version: 2, objects, layers: [], device_id: null }, null);
  store.getState().select(selected);
  return store.getState();
}

describe('Vectorizar imagen…', () => {
  const vector = sampleObjects().find((o) => o.kind.type === 'vector')!;

  it('está en el menú Herramientas', () => {
    expect(traceCommands.map((c) => c.category)).toEqual(['tools']);
    expect(commands.get('tools.traceImage')).toBeDefined();
  });

  it('se habilita solo con exactamente una imagen seleccionada', () => {
    expect(traceEnabled(stateWith([image('a')], ['a']))).toBe(true);
  });

  it('sin selección o con un vector explica que falta una imagen', () => {
    expect(traceEnabled(stateWith([image('a')], []))).toBe('trace.reason.needImage');
    expect(traceEnabled(stateWith([vector], [vector.id]))).toBe('trace.reason.needImage');
  });

  it('con varias selecciones pide una sola', () => {
    expect(traceEnabled(stateWith([image('a'), image('b')], ['a', 'b']))).toBe('trace.reason.oneImage');
    expect(traceEnabled(stateWith([image('a'), vector], ['a', vector.id]))).toBe('trace.reason.oneImage');
  });

  it('todos los motivos tienen texto', () => {
    expect(es['trace.reason.needImage']).toBeTruthy();
    expect(es['trace.reason.oneImage']).toBeTruthy();
  });

  it('ejecutarlo abre el diálogo con la imagen seleccionada y no hace nada si no está habilitado', () => {
    const cmd = traceCommands[0]!;
    useTraceDialog.getState().close();
    cmd.run(stateWith([vector], [vector.id]));
    expect(useTraceDialog.getState().imageId).toBeNull();
    cmd.run(stateWith([image('a')], ['a']));
    expect(useTraceDialog.getState().imageId).toBe('a');
    useTraceDialog.getState().close();
  });
});
