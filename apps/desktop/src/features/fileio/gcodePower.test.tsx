import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getApi, setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { useStore } from '@/store';
import { importWithOptions, isGcodeZeroPowerError, type FileioIo } from './actions';
import { GcodePowerDialog } from './GcodePowerDialog';
import { ImportOptionsDialog, kindsOf } from './ImportOptionsDialog';
import { DEFAULT_IMPORT_OPTIONS } from './types';

const s = () => useStore.getState();

const ZERO_POWER =
  'a.nc: el G-code no dibuja nada con el láser encendido: trae 3 avances G1/G2/G3 pero con potencia 0 (M3 S0, o sin una S mayor que 0). Fija una potencia al generarlo.';

function io(over: Partial<FileioIo> = {}): FileioIo {
  return {
    askExport: async () => null,
    askImport: async () => DEFAULT_IMPORT_OPTIONS,
    pickSavePath: async () => null,
    pickImportPaths: async () => ['/tmp/a.nc'],
    ...over,
  };
}

beforeEach(() => {
  setApi(createMockApi({ latencyMs: 0 }));
  s().newDocument();
});

describe('G-code con potencia 0 al importar', () => {
  it('reconoce el error del importador', () => {
    expect(isGcodeZeroPowerError(ZERO_POWER)).toBe(true);
    expect(isGcodeZeroPowerError('a.nc: el G-code no dibuja nada con el láser encendido (se buscan G1)')).toBe(false);
  });

  it('el diálogo de importar ofrece la potencia solo para G-code', () => {
    const gcode = renderToString(<ImportOptionsDialog fileCount={1} kinds={kindsOf(['nc'])} layers={[]} onDone={() => {}} />);
    expect(gcode).toContain('Potencia para cortes sin S');
    const svg = renderToString(<ImportOptionsDialog fileCount={1} kinds={kindsOf(['svg'])} layers={[]} onDone={() => {}} />);
    expect(svg).not.toContain('Potencia para cortes sin S');
    expect(renderToString(<GcodePowerDialog file="a.nc" onDone={() => {}} />)).toContain('El G-code no tiene cortes con potencia');
  });

  it('si el archivo importa vacío por eso, pregunta la potencia y reintenta con ella', async () => {
    const api = getApi();
    const calls: (number | null)[] = [];
    vi.spyOn(api, 'importFileWith').mockImplementation(async ({ options }) => {
      calls.push(options.gcode_default_power ?? null);
      if (options.gcode_default_power == null) throw new Error(ZERO_POWER);
      return { objects: [], new_layers: [], warnings: [] };
    });
    const ask = vi.fn(async () => 750);
    await importWithOptions(io({ askGcodePower: ask }));
    expect(ask).toHaveBeenCalledWith('a.nc');
    expect(calls).toEqual([null, 750]);
  });

  it('si se descarta la pregunta, queda el error original y no se reintenta', async () => {
    const api = getApi();
    const spy = vi.spyOn(api, 'importFileWith').mockRejectedValue(new Error(ZERO_POWER));
    await importWithOptions(io({ askGcodePower: async () => null }));
    expect(spy).toHaveBeenCalledTimes(1);
    expect(s().notices.some((n) => n.level === 'error')).toBe(true);
  });

  it('otros errores no preguntan nada', async () => {
    const api = getApi();
    vi.spyOn(api, 'importFileWith').mockRejectedValue(new Error('a.nc: archivo dañado'));
    const ask = vi.fn(async () => 500);
    await importWithOptions(io({ askGcodePower: ask }));
    expect(ask).not.toHaveBeenCalled();
  });
});
