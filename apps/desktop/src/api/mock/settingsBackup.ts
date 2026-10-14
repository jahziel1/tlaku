import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/** Mismo límite que el motor (256 KiB). */
const MAX_BYTES = 256 * 1024;

/**
 * Respaldos de ajustes GRBL en memoria (modo mock). Lo guardado se recuerda por ruta para poder
 * leerlo de vuelta; una ruta sin la extensión `.tlakuset` o `.txt` se rechaza, como en el motor.
 */
export function mockSettingsBackup(lag: Lag): Pick<LaserApi, 'writeSettingsBackup' | 'readSettingsBackup' | 'grblSettingsAutobackup'> {
  const files = new Map<string, string>();
  let seq = 0;
  return {
    grblSettingsAutobackup: ({ text }) =>
      lag(() => {
        seq += 1;
        const path = `/mock/grbl-backups/grbl-20261014-1200${String(seq).padStart(2, '0')}.tlakuset`;
        files.set(path, text);
        return path;
      }),
    writeSettingsBackup: ({ path, text }) =>
      lag(() => {
        if (!/\.(tlakuset|tlakuruida|txt)$/i.test(path)) throw new Error(`${path}: el respaldo debe guardarse con la extensión .tlakuset, .tlakuruida o .txt.`);
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
