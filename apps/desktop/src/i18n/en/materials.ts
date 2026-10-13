import type { materials as Source } from '../es/materials';

/** English for the `materials` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const materials: { [K in keyof typeof Source]?: string } = {};
