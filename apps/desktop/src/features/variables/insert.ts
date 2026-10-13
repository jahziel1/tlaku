import type { I18nKey } from '@/i18n';
import type { Lang } from './format';

/** Ejemplos de variables para la ayuda del panel, con los nombres del idioma. */
export function variableExamples(lang: Lang): string {
  const n = NAMES[lang];
  return `{${n.serial}}, {csv:${lang === 'es' ? 'Columna' : 'Column'}}, {${n.date}}, {${n.time}}`;
}

/** Nombres con que se escriben las variables en cada idioma de la interfaz. */
const NAMES: Record<Lang, { serial: string; row: string; date: string; time: string; copy: string }> = {
  es: { serial: 'serie', row: 'fila', date: 'fecha', time: 'hora', copy: 'copia' },
  en: { serial: 'serial', row: 'row', date: 'date', time: 'time', copy: 'copy' },
};

export interface InsertOption {
  id: string;
  label: I18nKey;
  /** Columna del CSV (solo en las opciones de dato). */
  column?: string;
  snippet: string;
  group: 'serial' | 'csv' | 'time' | 'other';
}

/**
 * Opciones del menú "Insertar variable". Las columnas del CSV cargado salen una por una; sin CSV
 * hay una opción genérica que el usuario completa. Una columna cuyo nombre rompería la sintaxis
 * (lleva `}` o `@`) se inserta por su número.
 */
export function insertOptions(lang: Lang, headers: readonly string[]): InsertOption[] {
  const n = NAMES[lang];
  const opts: InsertOption[] = [
    { id: 'serial', label: 'variables.insert.serial', snippet: `{${n.serial}}`, group: 'serial' },
    { id: 'serial6', label: 'variables.insert.serial6', snippet: `{${n.serial}:6}`, group: 'serial' },
  ];
  if (headers.length === 0) {
    opts.push({ id: 'csv', label: 'variables.insert.csvGeneric', snippet: `{csv:${lang === 'es' ? 'Columna' : 'Column'}}`, group: 'csv' });
  } else {
    headers.forEach((h, i) => {
      const safe = h.trim() !== '' && !/[}@]/.test(h) && !/^\d+$/.test(h.trim());
      opts.push({
        id: `csv:${i}`,
        label: 'variables.insert.csvColumn',
        column: h.trim() === '' ? String(i + 1) : h,
        snippet: `{csv:${safe ? h.trim() : i + 1}}`,
        group: 'csv',
      });
    });
  }
  opts.push(
    { id: 'row', label: 'variables.insert.row', snippet: `{${n.row}}`, group: 'csv' },
    { id: 'date', label: 'variables.insert.date', snippet: `{${n.date}}`, group: 'time' },
    {
      id: 'dateLong',
      label: 'variables.insert.dateLong',
      snippet: lang === 'es' ? `{${n.date}:d 'de' MMMM 'de' yyyy}` : `{${n.date}:MMMM d, yyyy}`,
      group: 'time',
    },
    { id: 'time', label: 'variables.insert.time', snippet: `{${n.time}}`, group: 'time' },
    { id: 'timeSec', label: 'variables.insert.timeSec', snippet: `{${n.time}:HH:mm:ss}`, group: 'time' },
    { id: 'copy', label: 'variables.insert.copy', snippet: `{${n.copy}}`, group: 'other' },
    { id: 'braceOpen', label: 'variables.insert.braceOpen', snippet: '{{', group: 'other' },
    { id: 'braceClose', label: 'variables.insert.braceClose', snippet: '}}', group: 'other' },
  );
  return opts;
}

/** Texto con `snippet` en lugar de la selección `[start, end)` y la posición del cursor después. */
export function insertSnippet(text: string, start: number, end: number, snippet: string): { text: string; caret: number } {
  const a = Math.max(0, Math.min(start, end, text.length));
  const b = Math.min(text.length, Math.max(start, end));
  return { text: text.slice(0, a) + snippet + text.slice(b), caret: a + snippet.length };
}
