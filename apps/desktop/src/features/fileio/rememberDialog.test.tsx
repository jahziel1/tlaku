import { isValidElement, type ReactElement } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { useStore } from '@/store';
import { defaultIo } from './actions';
import { DEFAULT_IMPORT_OPTIONS, type ImportDialogOptions } from './types';

/** En lugar de abrir el diálogo, la prueba mira con qué props se abriría y contesta como si el usuario aceptara. */
let answer: ImportDialogOptions | null = null;
let seen: Record<string, unknown> | null = null;
vi.mock('./host', () => ({
  openDialog: async <T,>(render: (done: (v: T) => void) => ReactElement): Promise<T> => {
    const el = render(() => {});
    seen = isValidElement(el) ? (el.props as Record<string, unknown>) : null;
    return answer as T;
  },
}));

const s = () => useStore.getState();

beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }));
  s().newDocument();
  s().resetPrefs();
  seen = null;
});

describe('askImport con opciones recordadas', () => {
  it('la primera vez arranca con las de fábrica y al aceptar guarda la elección', async () => {
    answer = { ...DEFAULT_IMPORT_OPTIONS, scale_percent: 300, dpi: 150, group: true };
    const out = await defaultIo.askImport(['/tmp/a.svg']);
    expect(seen!.initial).toBe(DEFAULT_IMPORT_OPTIONS);
    expect(out).toEqual(answer);
    expect(s().prefs.importOptions).toEqual({ dpi: 150, dxf_unit_mm: null, group: true, auto_group: false, target_layer: null, gcode_by_power: true });
  });

  it('la segunda vez el diálogo arranca con lo recordado, sin la escala', async () => {
    answer = { ...DEFAULT_IMPORT_OPTIONS, scale_percent: 300, dpi: 150, group: true };
    await defaultIo.askImport(['/tmp/a.svg']);
    answer = null;
    await defaultIo.askImport(['/tmp/b.svg']);
    expect(seen!.initial).toMatchObject({ dpi: 150, group: true, scale_percent: 100 });
  });

  it('cancelar no cambia lo recordado', async () => {
    s().setPref('importOptions', { dpi: 72, dxf_unit_mm: null, group: false, auto_group: false, target_layer: null, gcode_by_power: true });
    answer = null;
    await defaultIo.askImport(['/tmp/a.svg']);
    expect(s().prefs.importOptions).toMatchObject({ dpi: 72 });
  });

  it('con «recordar» apagado arranca de fábrica y no guarda nada', async () => {
    s().setPref('importRemember', false);
    s().setPref('importOptions', { dpi: 72, dxf_unit_mm: null, group: true, auto_group: false, target_layer: null, gcode_by_power: true });
    answer = { ...DEFAULT_IMPORT_OPTIONS, dpi: 300 };
    await defaultIo.askImport(['/tmp/a.svg']);
    expect(seen!.initial).toBe(DEFAULT_IMPORT_OPTIONS);
    expect(s().prefs.importOptions).toMatchObject({ dpi: 72 }); // sigue lo de antes
  });

  it('la resolución de Preferencias llega al diálogo como la predeterminada', async () => {
    s().setPref('importDpi', 200);
    answer = null;
    await defaultIo.askImport(['/tmp/a.svg']);
    expect(seen!.defaultDpi).toBe(200);
  });
});
