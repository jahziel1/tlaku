import type { history as Source } from '../es/history';

/** English for the `history` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const history: { [K in keyof typeof Source]?: string } = {};
