import type { canvas as Source } from '../es/canvas';

/** English for the `canvas` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const canvas: { [K in keyof typeof Source]?: string } = {};
