import type { CommandCategory, CommandDef, CommandRegistry } from '@/commands';
import { formatCombo, normalizeCombo, sameCombos, type ShortcutOverrides } from '@/commands/shortcuts';
import { t, type I18nKey } from '@/i18n';

/** Fila del editor de atajos: un comando con sus atajos de hoy y los de fábrica. */
export interface EditorRow {
  id: string;
  category: CommandCategory;
  label: string;
  /** Atajos efectivos, en forma canónica. */
  combos: string[];
  factory: string[];
  /** El usuario los cambió respecto a los de fábrica. */
  customized: boolean;
  /** Minúsculas y sin acentos, para buscar. */
  haystack: string;
}

export const EDITOR_CATEGORY_ORDER: CommandCategory[] = ['file', 'edit', 'tools', 'arrange', 'view', 'machine', 'camera', 'panels'];

/** Minúsculas y sin acentos, para que «seleccion» encuentre «Selección». */
export function normalizeText(text: string): string {
  return text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
}

/** Los pasos de las flechas comparten etiqueta; se distinguen por su tamaño (normal, grande, fino). */
export function commandLabel(def: CommandDef): string {
  const base = t(def.label) || def.id; // sin traducción se ve el id en vez de romper el editor
  const step = /^edit\.nudge\.\w+?(?:\.(big|fine))?$/.exec(def.id);
  return step ? `${base} (${t(`prefs.shortcuts.step.${step[1] ?? 'normal'}` as I18nKey)})` : base;
}

/** Todos los comandos (también los que ningún menú muestra, como las flechas), por categoría y nombre. */
export function buildEditorRows(registry: CommandRegistry, mac: boolean): EditorRow[] {
  const rows = registry.list().map<EditorRow>((def) => {
    const label = commandLabel(def);
    const combos = registry.shortcutsOf(def.id);
    return {
      id: def.id,
      category: def.category,
      label,
      combos,
      factory: registry.factoryShortcuts(def.id),
      customized: registry.isCustomized(def.id),
      haystack: normalizeText(`${label} ${def.id} ${combos.map((c) => `${formatCombo(c, mac)} ${c}`).join(' ')}`),
    };
  });
  const order = (c: CommandCategory) => EDITOR_CATEGORY_ORDER.indexOf(c);
  return rows.sort((a, b) => order(a.category) - order(b.category) || a.label.localeCompare(b.label, 'es'));
}

/** Todas las palabras de la búsqueda deben aparecer en el nombre, el id o los atajos. */
export function filterEditorRows(rows: readonly EditorRow[], query: string): EditorRow[] {
  const words = normalizeText(query).split(/\s+/).filter(Boolean);
  return words.length === 0 ? [...rows] : rows.filter((r) => words.every((w) => r.haystack.includes(w)));
}

export function groupEditorRows(rows: readonly EditorRow[]): { category: CommandCategory; rows: EditorRow[] }[] {
  const groups: { category: CommandCategory; rows: EditorRow[] }[] = [];
  for (const r of rows) {
    const last = groups[groups.length - 1];
    if (last && last.category === r.category) last.rows.push(r);
    else groups.push({ category: r.category, rows: [r] });
  }
  return groups;
}

/** Comandos que ya usan alguno de estos atajos (distintos de `id`). */
export function findConflicts(registry: CommandRegistry, id: string, combos: readonly string[]): { combo: string; owner: CommandDef }[] {
  const out: { combo: string; owner: CommandDef }[] = [];
  for (const combo of combos) {
    const owner = registry.conflictOf(combo, id);
    if (owner) out.push({ combo: normalizeCombo(combo), owner });
  }
  return out;
}

/** Guarda `list` como los atajos de `id`; si coincide con los de fábrica, quita el cambio. */
function setOwn(out: ShortcutOverrides, registry: CommandRegistry, id: string, list: string[]): void {
  if (sameCombos(list, registry.factoryShortcuts(id))) delete out[id];
  else out[id] = list;
}

/**
 * Cambios del usuario resultantes de dejar `combos` como los atajos de `id`.
 * Con `steal`, cada atajo que ya tenía otro comando se le quita a ese comando; sin él, un conflicto
 * es un error (la interfaz lo detecta antes con `findConflicts` y pregunta).
 */
export function assignShortcuts(registry: CommandRegistry, overrides: ShortcutOverrides, id: string, combos: readonly string[], steal = false): ShortcutOverrides {
  const list = [...new Set(combos.map(normalizeCombo))];
  const conflicts = findConflicts(registry, id, list);
  if (conflicts.length > 0 && !steal) throw new Error(`Conflicto de atajos: ${conflicts.map((c) => `${c.combo} -> ${c.owner.id}`).join(', ')}`);
  const out: ShortcutOverrides = { ...overrides };
  // Lista resultante de cada comando al que se le quita algo (acumula si pierde varios atajos).
  const remaining = new Map<string, string[]>();
  for (const { combo, owner } of conflicts) {
    const current = remaining.get(owner.id) ?? registry.shortcutsOf(owner.id);
    remaining.set(owner.id, current.filter((c) => c !== combo));
  }
  for (const [owner, left] of remaining) setOwn(out, registry, owner, left);
  setOwn(out, registry, id, list);
  return out;
}

/** Quita el atajo de `id` (queda sin atajo). */
export function clearShortcuts(registry: CommandRegistry, overrides: ShortcutOverrides, id: string): ShortcutOverrides {
  return assignShortcuts(registry, overrides, id, []);
}

/**
 * Devuelve `id` a sus atajos de fábrica. Si otro comando se había quedado con alguno, se le quita
 * (los de fábrica del propio comando mandan al restablecer).
 */
export function restoreShortcuts(registry: CommandRegistry, overrides: ShortcutOverrides, id: string): ShortcutOverrides {
  const without = { ...overrides };
  delete without[id];
  const out = { ...without };
  const factory = registry.factoryShortcuts(id);
  for (const def of registry.list()) {
    if (def.id === id || !(def.id in out)) continue;
    const own = out[def.id]!;
    if (own.some((c) => factory.includes(c))) setOwn(out, registry, def.id, own.filter((c) => !factory.includes(c)));
  }
  return out;
}
