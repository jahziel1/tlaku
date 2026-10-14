import { describe, expect, it } from 'vitest';
import { ENGINE_ERROR, errorCode, errorMessage } from './errors';

describe('errores del motor con código', () => {
  const coded = '[gcode_zero_power] a.nc: no dibuja nada';

  it('lee el código y quita el prefijo del texto que ve el usuario', () => {
    expect(errorCode(coded)).toBe(ENGINE_ERROR.gcodeZeroPower);
    expect(errorCode(new Error(coded))).toBe('gcode_zero_power');
    expect(errorMessage(coded)).toBe('a.nc: no dibuja nada');
    expect(errorMessage(new Error(coded))).toBe('a.nc: no dibuja nada');
  });

  it('un error sin código queda igual', () => {
    expect(errorCode('a.nc: algo falló')).toBeNull();
    expect(errorMessage('a.nc: algo falló')).toBe('a.nc: algo falló');
    expect(errorMessage({ x: 1 })).toBe('{"x":1}');
  });

  it('solo reconoce un código al inicio y en minúsculas', () => {
    expect(errorCode('a.nc: [gcode_zero_power] x')).toBeNull();
    expect(errorCode('[Mayúsculas] x')).toBeNull();
    expect(errorCode('[] x')).toBeNull();
  });
});
