import type { LaserApi, UserBundle } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/**
 * Paquetes de usuario `.tlakubundle` en memoria (modo mock). Lo exportado se recuerda por ruta para
 * poder importarlo de vuelta. Valida el sobre como el motor (formato, versión, `devices` como lista)
 * y exige la extensión al escribir; una ruta que contiene `no-existe` simula un archivo ausente.
 */
export function mockBundle(lag: Lag): Pick<LaserApi, 'exportUserBundle' | 'importUserBundle'> {
  const bundles = new Map<string, UserBundle>();

  const check = (value: unknown): void => {
    const b = value as Partial<Record<string, unknown>> | null;
    if (!b || typeof b !== 'object' || Array.isArray(b)) throw new Error('El paquete no es un objeto JSON.');
    if (b.format !== 'tlaku-bundle') throw new Error('El archivo no es un paquete de Tlaku (.tlakubundle).');
    if (b.version !== 1) throw new Error(`Versión de paquete no soportada: ${String(b.version)} (esta aplicación entiende la 1).`);
    if (b.devices !== undefined && !Array.isArray(b.devices)) throw new Error('Paquete inválido: "devices" debe ser una lista.');
  };

  return {
    exportUserBundle: ({ path, bundle }) =>
      lag(() => {
        if (!path.toLowerCase().endsWith('.tlakubundle')) throw new Error(`${path}: el paquete debe guardarse con la extensión .tlakubundle.`);
        check(bundle);
        bundles.set(path, structuredClone(bundle));
      }),
    importUserBundle: ({ path }) =>
      lag(() => {
        const bundle = bundles.get(path);
        if (!bundle) throw new Error(`${path}: no existe o no es un paquete de Tlaku.`);
        check(bundle);
        return structuredClone(bundle);
      }),
  };
}
