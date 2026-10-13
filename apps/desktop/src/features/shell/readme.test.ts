/// <reference types="node" />
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { commands } from '@/commands';
import { buildShortcutRows, shortcutsMarkdown } from './shortcutsModel';

/**
 * La tabla de atajos del README sale del registro de comandos: si cambia un atajo o se agrega un
 * comando, esta prueba falla. Para regenerarla: `npm run readme`.
 */
const README = fileURLToPath(new URL('../../../README.md', import.meta.url));
const START = '<!-- shortcuts:start (generado: npm run readme) -->';
const END = '<!-- shortcuts:end -->';

describe('README: tabla de atajos', () => {
  it('coincide con el registro de comandos', () => {
    const generated = shortcutsMarkdown(buildShortcutRows(commands, false));
    const text = readFileSync(README, 'utf8');
    const a = text.indexOf(START);
    const b = text.indexOf(END);
    expect(a, 'falta el marcador de inicio en el README').toBeGreaterThan(-1);
    expect(b, 'falta el marcador de fin en el README').toBeGreaterThan(a);
    if (process.env.UPDATE_README) {
      writeFileSync(README, `${text.slice(0, a + START.length)}\n\n${generated}\n\n${text.slice(b)}`);
      return;
    }
    const current = text.slice(a + START.length, b).trim();
    expect(current, 'README desactualizado: ejecuta `npm run readme`').toBe(generated);
  });
});
