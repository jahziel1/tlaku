import { describe, expect, it } from 'vitest';
import { DEFAULT_REMEMBERED_IMPORT, effectiveImportDpi, rememberImport, sanitizeRememberedImport } from './importOptions';

describe('opciones de importación recordadas', () => {
  it('lo que no es un objeto no se recuerda', () => {
    expect(sanitizeRememberedImport(null)).toBeNull();
    expect(sanitizeRememberedImport('x')).toBeNull();
  });

  it('completa lo que falta y descarta lo inválido', () => {
    expect(sanitizeRememberedImport({})).toEqual(DEFAULT_REMEMBERED_IMPORT);
    const out = sanitizeRememberedImport({ dpi: 99999, dxf_unit_mm: -3, target_layer: 'rojo', group: 'sí', gcode_by_power: false, scale_percent: 400 });
    expect(out).toEqual({ ...DEFAULT_REMEMBERED_IMPORT, dpi: 2400, gcode_by_power: false });
    expect(out).not.toHaveProperty('scale_percent');
  });

  it('conserva una capa válida y una unidad de DXF', () => {
    expect(sanitizeRememberedImport({ target_layer: '#FF0000', dxf_unit_mm: 25.4, dpi: 150 })).toMatchObject({ target_layer: '#FF0000', dxf_unit_mm: 25.4, dpi: 150 });
  });

  it('agrupar y agrupar por contención no van juntos', () => {
    expect(sanitizeRememberedImport({ group: true, auto_group: true })).toMatchObject({ group: true, auto_group: false });
    expect(rememberImport({ ...DEFAULT_REMEMBERED_IMPORT, auto_group: true })).toMatchObject({ auto_group: true });
  });

  it('la resolución elegida manda; «predeterminada» usa la de Preferencias', () => {
    expect(effectiveImportDpi(300, 150)).toBe(300);
    expect(effectiveImportDpi(null, 150)).toBe(150);
    // Con la de fábrica el motor ya usa 96 por sí solo.
    expect(effectiveImportDpi(null, 96)).toBeNull();
  });
});
