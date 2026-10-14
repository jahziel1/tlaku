import { sanitizeOverrides, type ShortcutOverrides } from '@/commands/shortcuts';
import { sanitizeLayerTemplate, type LayerTemplate } from '@/lib/layers';
import { DEFAULT_DOCK_LAYOUT, sanitizeDockLayout, type DockLayout } from '@/lib/dockLayout';
import { IMPORT_DPI_DEFAULT, IMPORT_DPI_MAX, IMPORT_DPI_MIN, sanitizeRememberedImport, type RememberedImport } from '@/lib/importOptions';
import { LENGTH_UNITS, type LengthUnit } from '@/lib/units';
import { detectLanguage, setLanguage } from '@/i18n';
import { readPref, writePref } from '@/lib/platform';
import type { SliceCreator } from './types';

/**
 * Preferencias del usuario. Este es el catálogo único: cada clave tiene su valor de fábrica, su
 * tipo y sus límites, y de ahí salen la ventana de Preferencias, el saneado de lo que se lee del
 * disco o de un paquete `.tlakuprefs`, y los selectores `usePref` / `getPref` (ver `store/index.ts`).
 *
 * Dónde viven: el archivo `preferences.json` de la carpeta de configuración de la app (lo escribe
 * `features/settings/persist.ts`) y, como copia para arrancar sin parpadeos, `localStorage`.
 */

export type Language = 'es' | 'en';
export type ThemePref = 'dark' | 'light';
/** Qué tan marcada se ve la rejilla del lienzo respecto de los colores del tema. */
export type GridContrast = 'low' | 'normal' | 'high';
/** Cómo se muestra la app: `ask` = todavía no se eligió (se pregunta al arrancar), `simple` = flujo guiado, `full` = todas las herramientas. */
export type UiMode = 'ask' | 'simple' | 'full';

export interface Prefs {
  /** Idioma de la interfaz. Sin preferencia guardada se usa el del sistema (inglés si empieza por «en»; si no, español). */
  language: Language;
  /** Buscar actualizaciones unos segundos después de abrir (`features/help/updater.ts`). Apagado de fábrica. */
  updateCheckOnStart: boolean;
  /** Unidad con la que se muestran y se escriben las longitudes (el documento siempre va en mm). */
  units: LengthUnit;
  /** Paso de las flechas, en mm: normal, con Mayús y con Alt. */
  nudgeStep: number;
  nudgeStepBig: number;
  nudgeStepFine: number;
  /** Paso de la rotación con Mayús mientras se gira, en grados. */
  rotateStep: number;
  /** Distancia del ajuste magnético, en píxeles de pantalla. */
  snapPx: number;
  /** Desplazamiento de duplicar y de pegar, en mm. */
  duplicateOffset: number;
  theme: ThemePref;
  /** Separación mínima de la rejilla en mm; 0 = automática según el zoom. */
  gridMm: number;
  autosave: boolean;
  /** Cada cuántos segundos se guarda la copia de recuperación. */
  autosaveSecs: number;
  /** Cuántos archivos recientes se recuerdan. */
  recentMax: number;
  /** Modo de la interfaz: sencillo (flujo guiado de 5 pasos) o completo. Ver `features/simple`. */
  uiMode: UiMode;
  /** Modo principiante: el editor de capa oculta los ajustes avanzados (los ya configurados se siguen aplicando). */
  beginnerMode: boolean;
  /** Ajustes con que empiezan las capas nuevas; `null` = los de fábrica. Ver `lib/layers.ts` y `features/layers/defaults.ts`. */
  layerDefaults: LayerTemplate | null;
  /**
   * Casillas de la lista de comprobación del diálogo de inicio (máquina): bits aire 1, enfoque 2,
   * material 4, extractor 8, tapa 16. Ver `features/machine/checklist.ts`.
   */
  jobChecklistItems: number;
  /** El diálogo de inicio exige tener marcadas las casillas de la lista antes de enviar. */
  jobChecklistRequired: boolean;
  /**
   * Automatización por UDP: servidor local de comandos de texto (`PING`, `LOAD`, `START`, `STATUS`,
   * `STOP`; ver `docs/app-api/automatizacion.md`). Apagado por omisión; solo escucha en 127.0.0.1.
   */
  automationEnabled: boolean;
  /** Puerto UDP de la automatización (sin privilegios). */
  automationPort: number;
  /** `START` puede iniciar sin confirmar en la interfaz. Apagado por omisión: sin esto, solo abre el diálogo de inicio. */
  automationAllowStart: boolean;
  /**
   * Control remoto desde el celular por la red local (`features/remote`, `src-tauri/src/remote`).
   * Apagado de fábrica; nunca se expone a internet. No se toma de paquetes importados (ver
   * `features/settings/bundle.ts`): un archivo ajeno no debe poder abrir el láser a la red.
   */
  remoteEnabled: boolean;
  /** Puerto TCP del control remoto (sin privilegios). */
  remotePort: number;
  /** IP de la red local donde escuchar; `null` = la recomendada. */
  remoteIp: string | null;
  /** Orden, visibilidad, borde y paneles flotantes de las pestañas, y borde de las herramientas (ver `lib/dockLayout.ts`). */
  dockLayout: DockLayout;
  /** Tras dibujar una forma (rectángulo, elipse, polígono, línea...) la herramienta vuelve a Seleccionar. */
  returnToSelect: boolean;
  /** Contraste de la rejilla del lienzo respecto del tema. */
  gridContrast: GridContrast;
  /** Resolución (ppp) con que se importan los SVG sin unidades y las imágenes cuando no se elige otra. */
  importDpi: number;
  /** El diálogo «Importar con opciones» arranca con lo que se eligió la vez anterior. */
  importRemember: boolean;
  /** Última elección del diálogo de importar (`null` = todavía ninguna); ver `lib/importOptions.ts`. */
  importOptions: RememberedImport | null;
}

export type PrefKey = keyof Prefs;

type NumberSpec = { kind: 'number'; min: number; max: number; step: number; unit?: 'mm' | 'px' | 'deg' | 's' | 'count' | 'dpi'; integer?: boolean };
type ChoiceSpec<T extends string> = { kind: 'choice'; options: readonly T[] };
type BoolSpec = { kind: 'bool' };
/** Valor estructurado (un objeto): `check` lo valida y devuelve el valor saneado, o `null` si no sirve. */
type JsonSpec<V> = { kind: 'json'; check: (raw: unknown) => { value: V } | null };

type SpecOf<V> = [V] extends [boolean] ? BoolSpec : [V] extends [number] ? NumberSpec : [V] extends [string] ? ChoiceSpec<V & string> : JsonSpec<V>;
export type PrefSpecs = { [K in PrefKey]: SpecOf<Prefs[K]> & { default: Prefs[K] } };

/** Valores de fábrica y límites. Las opciones de un `choice` son textos de i18n en la ventana. */
export const PREF_SPECS: PrefSpecs = {
  language: { kind: 'choice', options: ['es', 'en'], default: 'es' },
  updateCheckOnStart: { kind: 'bool', default: false },
  units: { kind: 'choice', options: LENGTH_UNITS, default: 'mm' },
  nudgeStep: { kind: 'number', min: 0.001, max: 100, step: 0.5, unit: 'mm', default: 1 },
  nudgeStepBig: { kind: 'number', min: 0.001, max: 500, step: 5, unit: 'mm', default: 10 },
  nudgeStepFine: { kind: 'number', min: 0.001, max: 10, step: 0.05, unit: 'mm', default: 0.1 },
  rotateStep: { kind: 'number', min: 1, max: 180, step: 5, unit: 'deg', default: 15 },
  snapPx: { kind: 'number', min: 1, max: 40, step: 1, unit: 'px', integer: true, default: 8 },
  duplicateOffset: { kind: 'number', min: 0, max: 200, step: 1, unit: 'mm', default: 5 },
  theme: { kind: 'choice', options: ['dark', 'light'], default: 'dark' },
  gridMm: { kind: 'number', min: 0, max: 100, step: 1, unit: 'mm', default: 0 },
  autosave: { kind: 'bool', default: true },
  autosaveSecs: { kind: 'number', min: 10, max: 3600, step: 10, unit: 's', integer: true, default: 60 },
  recentMax: { kind: 'number', min: 0, max: 30, step: 1, unit: 'count', integer: true, default: 10 },
  uiMode: { kind: 'choice', options: ['ask', 'simple', 'full'], default: 'ask' },
  beginnerMode: { kind: 'bool', default: false },
  jobChecklistItems: { kind: 'number', min: 0, max: 31, step: 1, integer: true, default: 31 },
  jobChecklistRequired: { kind: 'bool', default: false },
  automationEnabled: { kind: 'bool', default: false },
  automationPort: { kind: 'number', min: 1024, max: 65535, step: 1, integer: true, default: 47801 },
  automationAllowStart: { kind: 'bool', default: false },
  remoteEnabled: { kind: 'bool', default: false },
  remotePort: { kind: 'number', min: 1024, max: 65535, step: 1, integer: true, default: 47802 },
  remoteIp: {
    kind: 'json',
    default: null,
    check: (raw) => (raw === null ? { value: null } : typeof raw === 'string' && /^\d{1,3}(\.\d{1,3}){3}$/.test(raw) ? { value: raw } : null),
  },
  dockLayout: { kind: 'json', default: DEFAULT_DOCK_LAYOUT, check: (raw) => ({ value: sanitizeDockLayout(raw) }) },
  returnToSelect: { kind: 'bool', default: true },
  gridContrast: { kind: 'choice', options: ['low', 'normal', 'high'], default: 'normal' },
  importDpi: { kind: 'number', min: IMPORT_DPI_MIN, max: IMPORT_DPI_MAX, step: 1, unit: 'dpi', integer: true, default: IMPORT_DPI_DEFAULT },
  importRemember: { kind: 'bool', default: true },
  importOptions: {
    kind: 'json',
    default: null,
    check: (raw) => {
      if (raw === null) return { value: null };
      const value = sanitizeRememberedImport(raw);
      return value ? { value } : null;
    },
  },
  layerDefaults: {
    kind: 'json',
    default: null,
    check: (raw) => {
      if (raw === null) return { value: null };
      const value = sanitizeLayerTemplate(raw);
      return value ? { value } : null;
    },
  },
};

export const PREF_KEYS = Object.keys(PREF_SPECS) as PrefKey[];

export const DEFAULT_PREFS: Prefs = Object.fromEntries(PREF_KEYS.map((k) => [k, PREF_SPECS[k].default])) as unknown as Prefs;

/** Resultado de leer un valor suelto: el valor ya saneado, o `null` si no es del tipo o rango válido. */
export function checkPref<K extends PrefKey>(key: K, raw: unknown): { value: Prefs[K] } | null {
  const spec = PREF_SPECS[key] as NumberSpec | ChoiceSpec<string> | BoolSpec | JsonSpec<unknown>;
  switch (spec.kind) {
    case 'json':
      return spec.check(raw) as { value: Prefs[K] } | null;
    case 'bool':
      return typeof raw === 'boolean' ? { value: raw as Prefs[K] } : null;
    case 'choice':
      return typeof raw === 'string' && spec.options.includes(raw) ? { value: raw as Prefs[K] } : null;
    case 'number': {
      if (typeof raw !== 'number' || !Number.isFinite(raw)) return null;
      const clamped = Math.min(spec.max, Math.max(spec.min, raw));
      return { value: (spec.integer ? Math.round(clamped) : Math.round(clamped * 1000) / 1000) as Prefs[K] };
    }
  }
}

/** Valor válido para `key`: el que se pasa (acotado al rango) o, si no sirve, el de fábrica. */
export function sanitizePref<K extends PrefKey>(key: K, raw: unknown): Prefs[K] {
  return checkPref(key, raw)?.value ?? PREF_SPECS[key].default;
}

/** Preferencias completas a partir de datos de cualquier origen: lo inválido o ausente toma el valor de fábrica. */
export function sanitizePrefs(raw: unknown): Prefs {
  const src = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : {};
  const out: Record<string, unknown> = {};
  for (const key of PREF_KEYS) out[key] = sanitizePref(key, src[key]);
  // Sin idioma guardado (instalación nueva o archivo sin ese campo) manda el del sistema.
  if (checkPref('language', src.language) === null) out.language = detectLanguage();
  return out as unknown as Prefs;
}

export function prefsEqual(a: Prefs, b: Prefs): boolean {
  // Los valores estructurados (`layerDefaults`) se comparan por contenido.
  return PREF_KEYS.every((k) => a[k] === b[k] || (typeof a[k] === 'object' && JSON.stringify(a[k]) === JSON.stringify(b[k])));
}

// ---- copia local para arrancar sin parpadeo --------------------------------------------------------

const CACHE_KEY = 'prefs';
/** Clave completa en `localStorage` (con el prefijo de `readPref`); la usa el aviso `storage` de otras ventanas. */
export const CACHE_STORAGE_KEY = `tlaku.${CACHE_KEY}`;

/** Lo que se guarda en disco y en la copia local. */
export interface PrefsData {
  prefs: Prefs;
  shortcuts: ShortcutOverrides;
}

export function readCache(): PrefsData {
  try {
    const raw: unknown = JSON.parse(readPref(CACHE_KEY) ?? 'null');
    const obj = (raw && typeof raw === 'object' ? raw : {}) as { prefs?: unknown; shortcuts?: unknown };
    const prefs = sanitizePrefs(obj.prefs);
    // El tema lo puede cambiar el comando "Cambiar tema" sin pasar por aquí; manda lo último guardado.
    const theme = readPref('theme');
    if (theme === 'light' || theme === 'dark') prefs.theme = theme;
    return { prefs, shortcuts: sanitizeOverrides(obj.shortcuts) };
  } catch {
    return { prefs: { ...DEFAULT_PREFS, language: detectLanguage() }, shortcuts: {} };
  }
}

export function writeCache(data: PrefsData): void {
  writePref(CACHE_KEY, JSON.stringify(data));
}

// ---- slice ----------------------------------------------------------------------------------------

export interface PrefsSlice {
  prefs: Prefs;
  /** Atajos del usuario encima de los de fábrica (ver `commands/shortcuts.ts`). */
  shortcutOverrides: ShortcutOverrides;
  /** `true` cuando ya se leyó `preferences.json`; antes no se escribe nada para no pisarlo. */
  prefsLoaded: boolean;

  setPref: <K extends PrefKey>(key: K, value: Prefs[K]) => void;
  /** Sustituye todo (al leer el disco o importar un paquete). Sanea lo que reciba. */
  replacePrefs: (data: { prefs?: unknown; shortcuts?: unknown }, loaded?: boolean) => void;
  resetPrefs: () => void;
  setShortcutOverrides: (overrides: ShortcutOverrides) => void;
}

export const createPrefsSlice: SliceCreator<PrefsSlice> = (set, get) => {
  const initial = readCache();
  // El idioma se aplica antes del primer pintado, con lo que haya en la copia local.
  setLanguage(initial.prefs.language);
  return {
    prefs: initial.prefs,
    shortcutOverrides: initial.shortcuts,
    prefsLoaded: false,

    setPref: (key, value) => {
      const next = sanitizePref(key, value);
      if (get().prefs[key] === next) return;
      // El idioma se fija antes de avisar a la interfaz, para que el primer repintado ya salga traducido.
      if (key === 'language') setLanguage(next as Language);
      set((s) => ({ prefs: { ...s.prefs, [key]: next } }));
    },
    replacePrefs: (data, loaded = true) => {
      const prefs = sanitizePrefs(data.prefs);
      setLanguage(prefs.language);
      set({
        prefs,
        shortcutOverrides: sanitizeOverrides(data.shortcuts),
        prefsLoaded: loaded || get().prefsLoaded,
      });
    },
    resetPrefs: () => {
      const prefs = { ...DEFAULT_PREFS, language: detectLanguage() };
      setLanguage(prefs.language);
      set({ prefs });
    },
    setShortcutOverrides: (shortcutOverrides) => set({ shortcutOverrides: sanitizeOverrides(shortcutOverrides) }),
  };
};
