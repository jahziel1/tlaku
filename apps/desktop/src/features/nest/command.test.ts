import { describe, expect, it } from 'vitest';
import { commands } from '@/commands';
import { nestCommands, nestEnabled } from '@/commands/definitions/nest';
import { MENUS } from '@/commands/menus';
import { en, es } from '@/i18n';
import { createAppStore } from '@/store';
import { useNestUi } from './nestStore';
import { docOf, rectObject } from './testUtils';
import type { DesignObject } from '@/api/types';

function stateWith(objects: DesignObject[], selected: string[]) {
  const store = createAppStore();
  store.getState().replaceDocument(docOf(objects), null);
  store.getState().select(selected);
  return store.getState();
}

describe('Acomodar piezas (nesting)…', () => {
  const a = rectObject('a', 10, 10);
  const b = rectObject('b', 10, 10, [20, 0]);

  it('está en el menú Organizar, después del arreglo circular', () => {
    expect(nestCommands.map((c) => [c.id, c.category])).toEqual([['arrange.nest', 'arrange']]);
    expect(commands.get('arrange.nest')).toBeDefined();
    const items = MENUS.find((m) => m.id === 'arrange')!.items;
    expect(items[items.indexOf('arrange.circular') + 1]).toBe('arrange.nest');
  });

  it('se habilita con al menos una unidad móvil seleccionada', () => {
    expect(nestEnabled(stateWith([a], ['a']))).toBe(true);
    expect(nestEnabled(stateWith([a, b], ['a', 'b']))).toBe(true);
  });

  it('sin selección o con todo bloqueado/oculto explica por qué', () => {
    expect(nestEnabled(stateWith([a], []))).toBe('nest.reason.noSelection');
    expect(nestEnabled(stateWith([{ ...a, locked: true }], ['a']))).toBe('nest.reason.nothingMovable');
    expect(nestEnabled(stateWith([{ ...a, hidden: true }], ['a']))).toBe('nest.reason.nothingMovable');
  });

  it('los motivos y la etiqueta tienen texto en ambos idiomas', () => {
    for (const k of ['nest.cmd.open', 'nest.reason.noSelection', 'nest.reason.nothingMovable', 'nest.history'] as const) {
      expect(es[k]).toBeTruthy();
      expect(en[k]).toBeTruthy();
    }
    expect(es['nest.cmd.open']).toBe('Acomodar piezas (nesting)…');
    expect(en['nest.cmd.open']).toBe('Nest parts…');
  });

  it('ejecutarlo abre el diálogo y no hace nada si no está habilitado', () => {
    const cmd = nestCommands[0]!;
    useNestUi.getState().close();
    cmd.run(stateWith([a], []));
    expect(useNestUi.getState().open).toBe(false);
    cmd.run(stateWith([a], ['a']));
    expect(useNestUi.getState().open).toBe(true);
    useNestUi.getState().close();
  });
});
