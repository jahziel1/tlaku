import type { text as Source } from '../es/text';

/** English for the `text` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const text: { [K in keyof typeof Source]?: string } = {};
