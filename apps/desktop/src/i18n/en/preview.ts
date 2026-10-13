import type { preview as Source } from '../es/preview';

/** English for the `preview` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const preview: { [K in keyof typeof Source]?: string } = {};
