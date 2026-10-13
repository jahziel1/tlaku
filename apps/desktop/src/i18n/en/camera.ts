import type { camera as Source } from '../es/camera';

/** English for the `camera` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const camera: { [K in keyof typeof Source]?: string } = {};
