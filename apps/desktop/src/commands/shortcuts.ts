/**
 * Atajos de teclado: representación textual, lectura de eventos y formato para mostrar.
 *
 * Formato de un atajo: modificadores en orden fijo `Mod`, `Alt`, `Shift` y la tecla, separados
 * por `+`. `Mod` es Cmd en macOS y Ctrl en el resto. Ejemplos: `Mod+Shift+Z`, `Delete`, `V`.
 * Teclas: letras en mayúscula (`Z`), dígitos (`0`), signos (`=`, `-`, `.`, `,`) y nombres
 * (`Escape`, `Delete`, `Backspace`, `Enter`, `Space`, `Tab`, `ArrowLeft`, `F1`...).
 */

export interface KeyEventLike {
  key: string;
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

const MODIFIERS = ['Mod', 'Alt', 'Shift'] as const;

const CODE_TO_KEY: Record<string, string> = {
  Equal: '=',
  Minus: '-',
  Period: '.',
  Comma: ',',
  Slash: '/',
  Backquote: '`',
  BracketLeft: '[',
  BracketRight: ']',
  Semicolon: ';',
  Quote: "'",
  Backslash: '\\',
};

const NAMED_KEYS = new Set([
  'Escape', 'Delete', 'Backspace', 'Enter', 'Tab', 'Space',
  'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End',
  'F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9', 'F10', 'F11', 'F12',
]);

export interface ParsedCombo {
  mod: boolean;
  alt: boolean;
  shift: boolean;
  key: string;
}

export function parseCombo(combo: string): ParsedCombo {
  // El signo "+" como tecla no se admite (usar `Shift+=`): así el separador no es ambiguo.
  const parts = combo.split('+');
  const key = parts[parts.length - 1] ?? '';
  const mods = new Set(parts.slice(0, -1));
  for (const m of mods) {
    if (!(MODIFIERS as readonly string[]).includes(m)) throw new Error(`Modificador desconocido "${m}" en "${combo}"`);
  }
  if (key === '' || (!/^[A-Z0-9]$/.test(key) && !(key in invert(CODE_TO_KEY)) && !NAMED_KEYS.has(key))) {
    throw new Error(`Tecla desconocida "${key}" en "${combo}"`);
  }
  return { mod: mods.has('Mod'), alt: mods.has('Alt'), shift: mods.has('Shift'), key };
}

function invert(o: Record<string, string>): Record<string, string> {
  return Object.fromEntries(Object.entries(o).map(([k, v]) => [v, k]));
}

/** Forma canónica (orden de modificadores fijo) para comparar atajos. */
export function normalizeCombo(combo: string): string {
  const p = parseCombo(combo);
  return [p.mod && 'Mod', p.alt && 'Alt', p.shift && 'Shift', p.key].filter(Boolean).join('+');
}

/**
 * Atajo que representa un evento del teclado, o null si no es uno válido
 * (solo un modificador, Ctrl en macOS sin Cmd, Meta en Windows...).
 */
export function eventToCombo(e: KeyEventLike, mac: boolean): string | null {
  if (mac ? e.ctrlKey : e.metaKey) return null;
  const mod = mac ? e.metaKey : e.ctrlKey;

  let key: string | null = null;
  if (/^Key[A-Z]$/.test(e.code)) {
    // La letra que escribe el usuario manda (QWERTZ, AZERTY...); con Alt en Mac `key` es un símbolo.
    key = /^[a-zA-Z]$/.test(e.key) ? e.key.toUpperCase() : e.code.slice(3);
  } else if (/^Digit[0-9]$/.test(e.code)) key = e.code.slice(5);
  else if (e.code in CODE_TO_KEY) key = CODE_TO_KEY[e.code]!;
  else if (e.key === ' ') key = 'Space';
  else if (NAMED_KEYS.has(e.key)) key = e.key;
  if (key === null) return null;

  return [mod && 'Mod', e.altKey && 'Alt', e.shiftKey && 'Shift', key].filter(Boolean).join('+');
}

/** Texto para mostrar al usuario: `⌘⇧Z` en macOS, `Ctrl+Mayús+Z` en el resto. */
export function formatCombo(combo: string, mac: boolean): string {
  const p = parseCombo(combo);
  const key = KEY_LABELS[p.key] ?? p.key;
  if (mac) return `${p.alt ? '⌥' : ''}${p.shift ? '⇧' : ''}${p.mod ? '⌘' : ''}${key}`;
  return [p.mod && 'Ctrl', p.alt && 'Alt', p.shift && 'Mayús', key].filter(Boolean).join('+');
}

const KEY_LABELS: Record<string, string> = {
  Escape: 'Esc',
  Delete: 'Supr',
  Backspace: '⌫',
  Enter: 'Intro',
  Space: 'Espacio',
  ArrowLeft: '←',
  ArrowRight: '→',
  ArrowUp: '↑',
  ArrowDown: '↓',
  PageUp: 'RePág',
  PageDown: 'AvPág',
};

/** ¿El foco está en un campo donde se escribe texto? Ahí los atajos de una tecla no deben actuar. */
export function isEditableTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el || typeof el.tagName !== 'string') return false;
  const tag = el.tagName;
  if (tag === 'TEXTAREA' || tag === 'SELECT') return true;
  if (tag === 'INPUT') {
    const type = (el as HTMLInputElement).type;
    return !['button', 'checkbox', 'radio', 'range', 'submit', 'reset', 'color'].includes(type);
  }
  return el.isContentEditable || el.getAttribute?.('role') === 'textbox';
}

/** Atajos que los campos de texto resuelven solos; no se interceptan ahí. */
export const NATIVE_TEXT_COMBOS: ReadonlySet<string> = new Set([
  'Mod+A', 'Mod+C', 'Mod+X', 'Mod+V', 'Mod+Z', 'Mod+Shift+Z', 'Mod+Y',
]);

// ---- atajos del usuario ---------------------------------------------------------------------

/**
 * Atajos que el usuario cambió: id de comando -> lista de atajos que lo reemplaza por completo
 * (lista vacía = el comando queda sin atajo). Un comando que no aparece conserva el de fábrica.
 * Se guardan en forma canónica con `Mod`, así el mismo archivo sirve en macOS y en el resto.
 */
export type ShortcutOverrides = Record<string, string[]>;

/** Atajo en forma canónica, o `null` si no es válido. */
export function tryNormalizeCombo(combo: unknown): string | null {
  if (typeof combo !== 'string') return null;
  try {
    return normalizeCombo(combo);
  } catch {
    return null;
  }
}

/**
 * Limpia datos leídos del disco o de un paquete: descarta ids vacíos, atajos inválidos y repetidos.
 * No sabe qué comandos existen (un paquete puede venir de otra versión): los desconocidos se
 * conservan y el registro simplemente los ignora.
 */
export function sanitizeOverrides(raw: unknown): ShortcutOverrides {
  const out: ShortcutOverrides = {};
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return out;
  for (const [id, list] of Object.entries(raw as Record<string, unknown>)) {
    if (id === '' || !Array.isArray(list)) continue;
    const combos = [...new Set(list.map(tryNormalizeCombo).filter((c): c is string => c !== null))];
    out[id] = combos;
  }
  return out;
}

/** ¿Las dos listas de atajos son la misma (sin importar el orden)? */
export function sameCombos(a: readonly string[], b: readonly string[]): boolean {
  const x = new Set(a.map(normalizeCombo));
  const y = new Set(b.map(normalizeCombo));
  return x.size === y.size && [...x].every((c) => y.has(c));
}
