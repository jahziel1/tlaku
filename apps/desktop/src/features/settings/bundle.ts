import type { PrefsBundle, PrefsFile } from '@/api';
import { sanitizeOverrides, type ShortcutOverrides } from '@/commands/shortcuts';
import { checkPref, PREF_KEYS, type Prefs } from '@/store/prefs';

/**
 * Paquetes `.tlakuprefs` (D99): un JSON con las preferencias y los atajos del usuario, para
 * llevarlos a otra computadora o compartirlos. Es el mismo sobre que `preferences.json` más la
 * marca `"format": "tlaku-prefs"`. Un paquete puede ser parcial (por ejemplo solo atajos): lo que
 * falta se deja como está al importar.
 */

export const BUNDLE_FORMAT = 'tlaku-prefs' as const;
export const BUNDLE_EXTENSION = 'tlakuprefs';
export const DEFAULT_BUNDLE_NAME = `tlaku.${BUNDLE_EXTENSION}`;
export const DEFAULT_SHORTCUTS_NAME = `atajos.${BUNDLE_EXTENSION}`;

/**
 * Preferencias que un archivo ajeno nunca enciende: abren el láser a la red (control remoto). Se
 * exportan, pero al importar se ignoran y cada persona las activa a mano en esta computadora.
 */
export const NEVER_IMPORTED: ReadonlySet<string> = new Set(['remoteEnabled']);

/** Qué parte del estado entra al exportar o se aplica al importar. */
export type BundleScope = 'all' | 'shortcuts';

export function buildBundle(prefs: Prefs, shortcuts: ShortcutOverrides, scope: BundleScope = 'all'): PrefsBundle {
  return {
    format: BUNDLE_FORMAT,
    version: 1,
    prefs: scope === 'all' ? { ...prefs } : {},
    shortcuts: structuredClone(shortcuts),
  };
}

/** Sobre de `preferences.json`. */
export function buildFile(prefs: Prefs, shortcuts: ShortcutOverrides): PrefsFile {
  return { version: 1, prefs: { ...prefs }, shortcuts: structuredClone(shortcuts) };
}

export interface BundleChange {
  prefs: Prefs;
  shortcuts: ShortcutOverrides;
  /** Preferencias del paquete que se aplicaron. */
  appliedPrefs: number;
  /** Claves o valores del paquete que no se entendieron y se descartaron. */
  ignoredPrefs: number;
  /** Comandos con atajos propios que trae el paquete. */
  appliedShortcuts: number;
  /** Atajos de comandos que esta versión no tiene (se conservan por si llegan después). */
  unknownCommands: number;
}

/**
 * Resultado de aplicar `bundle` sobre el estado actual, sin tocar el estado. Lo que falta o no es
 * válido se ignora; no se mezclan atajos: si el paquete trae atajos, reemplazan a los actuales.
 */
export function applyBundle(
  bundle: Partial<Pick<PrefsBundle, 'prefs' | 'shortcuts'>>,
  current: { prefs: Prefs; shortcuts: ShortcutOverrides },
  knownCommands: ReadonlySet<string>,
  scope: BundleScope = 'all',
): BundleChange {
  const prefs = { ...current.prefs };
  let appliedPrefs = 0;
  let ignoredPrefs = 0;
  const incoming = scope === 'all' && bundle.prefs && typeof bundle.prefs === 'object' ? (bundle.prefs as Record<string, unknown>) : {};
  const known = new Set<string>(PREF_KEYS);
  for (const [key, raw] of Object.entries(incoming)) {
    if (!known.has(key) || NEVER_IMPORTED.has(key)) {
      ignoredPrefs++;
      continue;
    }
    const checked = checkPref(key as keyof Prefs, raw);
    if (!checked) {
      ignoredPrefs++;
      continue;
    }
    (prefs as Record<string, unknown>)[key] = checked.value;
    appliedPrefs++;
  }

  const hasShortcuts = bundle.shortcuts !== undefined;
  const shortcuts = hasShortcuts ? sanitizeOverrides(bundle.shortcuts) : current.shortcuts;
  const ids = Object.keys(shortcuts);
  return {
    prefs,
    shortcuts,
    appliedPrefs,
    ignoredPrefs,
    appliedShortcuts: hasShortcuts ? ids.length : 0,
    unknownCommands: hasShortcuts ? ids.filter((id) => !knownCommands.has(id)).length : 0,
  };
}

/** Comprueba la forma básica de lo que devolvió el motor al importar. */
export function isBundle(raw: unknown): raw is PrefsBundle {
  return !!raw && typeof raw === 'object' && (raw as { format?: unknown }).format === BUNDLE_FORMAT && (raw as { version?: unknown }).version === 1;
}
