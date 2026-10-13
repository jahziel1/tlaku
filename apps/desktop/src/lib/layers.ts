import type { ImageSettings, LayerSettings } from '@/api/types';

export const DEFAULT_IMAGE_SETTINGS: ImageSettings = {
  mode: 'floyd',
  threshold: 128,
  brightness: 0,
  contrast: 0,
  gamma: 1,
  invert: false,
  dot_width: 0.1,
};

export function makeLayer(color: string, name: string, patch: Partial<LayerSettings> = {}): LayerSettings {
  return {
    color,
    name,
    mode: 'line',
    output: true,
    visible: true,
    speed: 1000,
    power: 50,
    min_power: 0,
    passes: 1,
    air: false,
    interval: 0.1,
    angle: 0,
    crosshatch: false,
    bidirectional: true,
    overscan: 'auto',
    kerf: 0,
    image: { ...DEFAULT_IMAGE_SETTINGS },
    order: null,
    ...patch,
  };
}
