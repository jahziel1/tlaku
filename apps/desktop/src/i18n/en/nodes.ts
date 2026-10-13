import type { nodes as Source } from '../es/nodes';

/** English for the `nodes` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const nodes: { [K in keyof typeof Source]?: string } = {};
