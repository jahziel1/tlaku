import type { codes as Source } from '../es/codes';

/** English for the `codes` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const codes: { [K in keyof typeof Source]?: string } = {};
