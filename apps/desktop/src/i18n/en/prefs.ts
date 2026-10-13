import type { prefs as Source } from '../es/prefs';

/** English for the `prefs` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const prefs: { [K in keyof typeof Source]?: string } = {};
