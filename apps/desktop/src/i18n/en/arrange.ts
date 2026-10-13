import type { arrange as Source } from '../es/arrange';

/** English for the `arrange` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const arrange: { [K in keyof typeof Source]?: string } = {};
