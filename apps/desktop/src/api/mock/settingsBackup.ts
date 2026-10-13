import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/** Mismo límite que el motor (256 KiB). */
const MAX_BYTES = 256 * 1024;

/**
 * Respaldos de ajustes GRBL en memoria (modo mock). Lo guardado se recuerda por ruta para poder
 * leerlo de vuelta; una ruta sin la extensión `.tlakuset` o `.txt` se rechaza, como en el motor.
 */
export function mockSettingsBackup(lag: Lag): Pick<LaserApi, 'writeSettingsBackup' | 'readSettingsBackup'> {
  const files = new Map<string, string>();
  return {
    writeSettingsBackup: ({ path, text }) =>
      lag(() => {
        if (!/\.(tlakuset|txt)$/i.test(path)) throw new Error(`${path}: el respaldo debe guardarse con la extensión .tlakuset o .txt.`);
        if (text.length > MAX_BYTES) throw new Error('El respaldo es demasiado grande.');
        files.set(path, text);
      }),
    readSettingsBackup: ({ path }) =>
      lag(() => {
        const text = files.get(path);
        if (text === undefined) throw new Error(`${path}: no existe o no es un respaldo de ajustes.`);
        return text;
      }),
  };
}
