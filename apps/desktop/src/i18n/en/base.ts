import type { base as Source } from '../es';

/** English for the base keys of `es.ts` (`cmd.*`, `menu.*`, `reason.*`, …). */
export const base: { [K in keyof typeof Source]?: string } = {};
