/// <reference types="node" />
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { commands } from '@/commands';
import { getLanguage, setLanguage, type Language } from '@/i18n';
import { buildShortcutRows, shortcutsMarkdown } from './shortcutsModel';

/**
 * La tabla de atajos de los README sale del registro de comandos: si cambia un atajo o se agrega un
 * comando, esta prueba falla. `README.md` está en inglés y `README.es.md` en español, cada uno con
 * sus atajos en su idioma (nombres de teclas incluidos). Para regenerar los dos: `npm run readme`.
 */
const FILES: { file: string; lang: Language }[] = [
  { file: fileURLToPath(new URL('../../../README.md', import.meta.url)), lang: 'en' },
  { file: fileURLToPath(new URL('../../../README.es.md', import.meta.url)), lang: 'es' },
];
const START = '<!-- shortcuts:start (generado: npm run readme) -->';
const END = '<!-- shortcuts:end -->';

/** La tabla en el idioma pedido; deja el idioma de la interfaz como estaba. */
function tableIn(lang: Language): string {
  const before = getLanguage();
  setLanguage(lang);
  try {
    return shortcutsMarkdown(buildShortcutRows(commands, false, lang));
  } finally {
    setLanguage(before);
  }
}

describe('README: tabla de atajos (inglés y español)', () => {
  for (const { file, lang } of FILES) {
    it(`${file.split('/').pop()} coincide con el registro de comandos (${lang})`, () => {
      const generated = tableIn(lang);
      const text = readFileSync(file, 'utf8');
      const a = text.indexOf(START);
      const b = text.indexOf(END);
      expect(a, 'falta el marcador de inicio en el README').toBeGreaterThan(-1);
      expect(b, 'falta el marcador de fin en el README').toBeGreaterThan(a);
      if (process.env.UPDATE_README) {
        writeFileSync(file, `${text.slice(0, a + START.length)}\n\n${generated}\n\n${text.slice(b)}`);
        return;
      }
      const current = text.slice(a + START.length, b).trim();
      expect(current, 'README desactualizado: ejecuta `npm run readme`').toBe(generated);
    });
  }

  it('cada README tiene los atajos en su idioma', () => {
    expect(tableIn('en')).toContain('| Group | Command | Shortcut |');
    expect(tableIn('en')).toContain('`Ctrl+Shift+S`');
    expect(tableIn('es')).toContain('| Grupo | Comando | Atajo |');
    expect(tableIn('es')).toContain('`Ctrl+Mayús+S`');
  });
});
