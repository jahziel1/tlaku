import type { catalog as Source } from '../es/catalog';

/** English for the `catalog` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const catalog: { [K in keyof typeof Source]?: string } = {};
