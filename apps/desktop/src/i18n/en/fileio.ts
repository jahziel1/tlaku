import type { fileio as Source } from '../es/fileio';

/** English for the `fileio` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const fileio: { [K in keyof typeof Source]?: string } = {};
