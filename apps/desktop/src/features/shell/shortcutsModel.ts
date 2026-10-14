import type { CommandCategory, CommandDef, CommandRegistry } from '@/commands';
import { formatCombo } from '@/commands';
import { t, type I18nKey, type Language } from '@/i18n';

export interface ShortcutRow {
  id: string;
  category: CommandCategory | 'canvas';
  label: string;
  /** Atajos ya formateados para esta plataforma (todos los del comando). */
  keys: string[];
  /** Texto en minúsculas y sin acentos con el que se busca. */
  haystack: string;
}

export const CATEGORY_ORDER: (CommandCategory | 'canvas')[] = ['file', 'edit', 'tools', 'arrange', 'view', 'machine', 'camera', 'panels', 'canvas'];

/** Minúsculas y sin acentos, para que "seleccion" encuentre "Selección". */
export function normalizeText(text: string): string {
  return text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
}

/** Los pasos de las flechas comparten etiqueta; se distinguen por la distancia. */
function labelOf(def: CommandDef): string {
  const base = t(def.label);
  const step = /^edit\.nudge\.\w+?(?:\.(big|fine))?$/.exec(def.id);
  if (!step) return base;
  return `${base} (${t(`history.shortcuts.step.${step[1] ?? 'normal'}` as I18nKey)})`;
}

/** Gestos del lienzo que no son comandos del registro (se muestran aparte). */
const CANVAS_GESTURES: { label: I18nKey; keys: I18nKey }[] = [
  { label: 'history.shortcuts.canvas.wheel', keys: 'history.shortcuts.canvas.wheelKeys' },
  { label: 'history.shortcuts.canvas.pinch', keys: 'history.shortcuts.canvas.pinchKeys' },
  { label: 'history.shortcuts.canvas.pan', keys: 'history.shortcuts.canvas.panKeys' },
];

/**
 * Filas del diálogo de atajos, generadas del registro: todo comando con atajos, también los
 * ocultos de los menús (flechas), en el orden de las categorías.
 */
export function buildShortcutRows(registry: CommandRegistry, mac: boolean, lang: Language = 'es'): ShortcutRow[] {
  const rows: ShortcutRow[] = [];
  for (const def of registry.list()) {
    if (!def.shortcuts?.length) continue;
    const keys = def.shortcuts.map((s) => formatCombo(s, mac, lang));
    const label = labelOf(def);
    rows.push({ id: def.id, category: def.category, label, keys, haystack: normalizeText(`${label} ${keys.join(' ')} ${def.shortcuts.join(' ')}`) });
  }
  CANVAS_GESTURES.forEach((g, i) => {
    const label = t(g.label);
    const keys = [t(g.keys)];
    rows.push({ id: `canvas.gesture.${i}`, category: 'canvas', label, keys, haystack: normalizeText(`${label} ${keys[0]}`) });
  });
  return rows.sort((a, b) => CATEGORY_ORDER.indexOf(a.category) - CATEGORY_ORDER.indexOf(b.category));
}

/** Filtra por texto: todas las palabras de la búsqueda deben aparecer en la etiqueta o en los atajos. */
export function filterShortcutRows(rows: readonly ShortcutRow[], query: string): ShortcutRow[] {
  const words = normalizeText(query).split(/\s+/).filter(Boolean);
  if (words.length === 0) return [...rows];
  return rows.filter((r) => words.every((w) => r.haystack.includes(w)));
}

/** Agrupa filas consecutivas por categoría, conservando el orden. */
export function groupShortcutRows(rows: readonly ShortcutRow[]): { category: ShortcutRow['category']; rows: ShortcutRow[] }[] {
  const groups: { category: ShortcutRow['category']; rows: ShortcutRow[] }[] = [];
  for (const r of rows) {
    const last = groups[groups.length - 1];
    if (last && last.category === r.category) last.rows.push(r);
    else groups.push({ category: r.category, rows: [r] });
  }
  return groups;
}

/**
 * Tabla Markdown de atajos para los README (formato Windows/Linux: `Ctrl`, `Mayús`/`Shift`), generada
 * desde las mismas filas del diálogo y en el idioma activo (`setLanguage`). La prueba `readme.test.ts`
 * la compara con `README.md` (inglés) y `README.es.md` (español).
 */
export function shortcutsMarkdown(rows: readonly ShortcutRow[]): string {
  const cell = (text: string) => text.replace(/\|/g, '\\|');
  const lines = [`| ${t('history.shortcuts.md.group')} | ${t('history.shortcuts.md.command')} | ${t('history.shortcuts.md.shortcut')} |`, '|---|---|---|'];
  for (const g of groupShortcutRows(rows)) {
    g.rows.forEach((r, i) => {
      const group = i === 0 ? t(`history.shortcuts.cat.${g.category}` as I18nKey) : '';
      lines.push(`| ${group} | ${cell(r.label)} | ${r.keys.map((k) => `\`${cell(k)}\``).join(` ${t('history.shortcuts.md.or')} `)} |`);
    });
  }
  return lines.join('\n');
}
