import type { layers as Source } from '../es/layers';

/** English for the `layers` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const layers: { [K in keyof typeof Source]?: string } = {};
