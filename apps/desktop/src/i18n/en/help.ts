import type { help as Source } from '../es/help';

/** English for the `help` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const help: { [K in keyof typeof Source]?: string } = {};
