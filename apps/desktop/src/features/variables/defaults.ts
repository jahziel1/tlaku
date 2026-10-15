import type { DataSource, Document, VariableText } from '@/api/types';

/** Ajustes de fábrica (los mismos que `VariableText::default()` de tlaku-doc). */
export const DEFAULT_VARIABLE_TEXT: VariableText = {
  serial: 1,
  serial_step: 1,
  serial_digits: 0,
  csv_path: null,
  csv_row: 0,
  advance_after_job: true,
  sources: [],
};

/** Nombre de fuente válido: letras, números, `_` y `-` (igual que `clean_source_name` de tlaku-doc). */
export function cleanSourceName(raw: string): string {
  return raw.trim().replace(/[^\p{L}\p{N}_-]/gu, '');
}

/** Nombre de archivo sin carpeta ni extensión. */
function fileStem(path: string): string {
  const base = path.split(/[\\/]/).pop() ?? path;
  const dot = base.lastIndexOf('.');
  return dot > 0 ? base.slice(0, dot) : base;
}

/**
 * Deja las fuentes de datos en forma canónica; es la misma regla que `VariableText::normalize`
 * de tlaku-doc:
 * - sin `sources` y con `csv_path` (documento de antes de los varios CSV), la fuente principal sale del archivo;
 * - `csv_path` repite la ruta de la principal;
 * - los nombres quedan válidos, sin repetir (sin distinguir mayúsculas) y sin vacíos.
 */
export function normalizeVariableText(vt: VariableText): VariableText {
  let sources: DataSource[] = vt.sources ?? [];
  if (sources.length === 0 && vt.csv_path && vt.csv_path.trim() !== '') {
    sources = [{ name: fileStem(vt.csv_path), path: vt.csv_path, on_short: 'repeat' }];
  }
  const seen: string[] = [];
  let changed = sources !== vt.sources;
  const fixed = sources.map((s, i) => {
    let name = cleanSourceName(s.name);
    if (name === '') name = i === 0 ? 'datos' : `datos${i + 1}`;
    const base = name;
    for (let n = 2; seen.includes(name.toLowerCase()); n++) name = `${base}${n}`;
    seen.push(name.toLowerCase());
    if (name === s.name && s.on_short !== undefined) return s;
    changed = true;
    return { ...s, name, on_short: s.on_short ?? 'repeat' };
  });
  const main = fixed[0]?.path;
  const csvPath = main !== undefined && main.trim() !== '' ? main : null;
  if (csvPath === vt.csv_path && !changed) return vt;
  return { ...vt, sources: fixed, csv_path: csvPath };
}

/** Ajustes del documento (o los de fábrica en un documento sin ellos), con las fuentes normalizadas. */
export function variableTextOf(doc: Pick<Document, 'variable_text'>): VariableText {
  return normalizeVariableText({ ...DEFAULT_VARIABLE_TEXT, ...doc.variable_text });
}
