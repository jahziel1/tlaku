import type { devices as Source } from '../es/devices';

/** English for the `devices` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const devices: { [K in keyof typeof Source]?: string } = {};
