import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

/** Automatización por UDP simulada: no abre ningún socket, solo valida el puerto como lo hace el motor. */
export function mockAutomation(lag: Lag): Pick<LaserApi, 'automationConfigure'> {
  return {
    automationConfigure: ({ config }) =>
      lag(() => {
        if (!config.enabled) return { running: false, port: config.port, error: null };
        if (!Number.isInteger(config.port) || config.port < 1024 || config.port > 65535) {
          return { running: false, port: config.port, error: 'El puerto debe estar entre 1024 y 65535 (los menores necesitan privilegios).' };
        }
        return { running: true, port: config.port, error: null };
      }),
  };
}
