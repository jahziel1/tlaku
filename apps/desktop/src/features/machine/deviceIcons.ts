import { Box, Cpu, Flame, Gauge, Layers, Printer, Scissors, Sun, Wrench, Zap, type LucideIcon } from 'lucide-react';
import type { DeviceProfile } from '@/api/types';
import type { I18nKey } from '@/i18n';

/** Juego de íconos que se puede elegir para un perfil (de `lucide-react`, ya incluido en la app). */
export const DEVICE_ICONS = [
  { id: 'cpu', Icon: Cpu, label: 'devices.icon.cpu' },
  { id: 'zap', Icon: Zap, label: 'devices.icon.zap' },
  { id: 'flame', Icon: Flame, label: 'devices.icon.flame' },
  { id: 'sun', Icon: Sun, label: 'devices.icon.sun' },
  { id: 'scissors', Icon: Scissors, label: 'devices.icon.scissors' },
  { id: 'printer', Icon: Printer, label: 'devices.icon.printer' },
  { id: 'wrench', Icon: Wrench, label: 'devices.icon.wrench' },
  { id: 'box', Icon: Box, label: 'devices.icon.box' },
  { id: 'layers', Icon: Layers, label: 'devices.icon.layers' },
  { id: 'gauge', Icon: Gauge, label: 'devices.icon.gauge' },
] as const satisfies readonly { id: string; Icon: LucideIcon; label: I18nKey }[];

export type DeviceIconId = (typeof DEVICE_ICONS)[number]['id'];

/** Ícono por omisión según el tipo de máquina: el que se ve mientras el perfil no elige uno. */
export function defaultIconId(kind: DeviceProfile['kind']): DeviceIconId {
  if (kind === 'galvo') return 'sun';
  if (kind === 'ruida' || kind === 'lhy_micro') return 'flame';
  return 'cpu';
}

/** Ícono del perfil: el elegido (si existe en el juego) o el del tipo. */
export function deviceIcon(device: Pick<DeviceProfile, 'icon' | 'kind'>): LucideIcon {
  const chosen = DEVICE_ICONS.find((i) => i.id === device.icon);
  const id = chosen?.id ?? defaultIconId(device.kind);
  return DEVICE_ICONS.find((i) => i.id === id)!.Icon;
}
