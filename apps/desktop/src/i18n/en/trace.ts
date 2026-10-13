import type { trace as Source } from '../es/trace';

/** English for the `trace` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const trace: { [K in keyof typeof Source]?: string } = {};
