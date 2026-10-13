import type { machine as Source } from '../es/machine';

/** English for the `machine` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const machine: { [K in keyof typeof Source]?: string } = {};
