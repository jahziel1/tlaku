import { t, type I18nKey } from '@/i18n';

/**
 * Máquinas virtuales: puertos `virtual:<perfil>[@<velocidad>]` que el motor abre con un emulador
 * en lugar del puerto serie. Ningún puerto real empieza así (`/dev/...`, `COM3`). La velocidad
 * (`@5`) acelera el tiempo simulado; sin ella es ×1 (tiempo real).
 */
export const VIRTUAL_PREFIX = 'virtual:';

/** Velocidades que se ofrecen (veces el tiempo real). */
export const VIRTUAL_SPEEDS = [1, 5, 20] as const;
export const DEFAULT_VIRTUAL_SPEED = 1;

const NAMES: Record<string, I18nKey> = {
  falcon2: 'machine.virtual.name.falcon2',
  grbl11: 'machine.virtual.name.grbl11',
  grblhal: 'machine.virtual.name.grblhal',
  fluidnc: 'machine.virtual.name.fluidnc',
  marlin: 'machine.virtual.name.marlin',
  smoothie: 'machine.virtual.name.smoothie',
};

export function isVirtualPort(port: string | null | undefined): boolean {
  return !!port && port.startsWith(VIRTUAL_PREFIX);
}

/** `virtual:falcon2@5` -> `virtual:falcon2` (el mismo puerto sin importar la velocidad). */
export function baseVirtualPort(port: string): string {
  const at = port.indexOf('@');
  return at < 0 ? port : port.slice(0, at);
}

/** Puerto con la velocidad pedida; ×1 es el valor por omisión y no se escribe. */
export function withVirtualSpeed(port: string, speed: number): string {
  const base = baseVirtualPort(port);
  return speed === 1 ? base : `${base}@${speed}`;
}

/** Velocidad escrita en el puerto (1 si no hay). */
export function virtualSpeed(port: string): number {
  const at = port.indexOf('@');
  const n = at < 0 ? 1 : Number(port.slice(at + 1));
  return Number.isFinite(n) && n > 0 ? n : 1;
}

/** Para comparar si dos conexiones son a la misma máquina (ignora la velocidad virtual). */
export function portKey(port: string | null | undefined): string | null {
  return port ? baseVirtualPort(port) : null;
}

/** Nombre para mostrar: "Máquina virtual · Falcon2 Pro" para las virtuales; el puerto tal cual para las demás. */
export function portLabel(port: string): string {
  if (!isVirtualPort(port)) return port;
  const id = baseVirtualPort(port).slice(VIRTUAL_PREFIX.length);
  const key = NAMES[id];
  const name = key ? t(key) : id;
  const speed = virtualSpeed(port);
  return speed === 1 ? t('machine.virtual.label', { name }) : t('machine.virtual.labelSpeed', { name, speed });
}
