import type { Point } from '@/api/types';
import { objectBounds, unionBounds, type Bounds } from '@/lib/geometry';
import { readPref, writePref } from '@/lib/platform';
import { fitBounds, zoomAround } from '@/lib/viewport';
import { selectActiveDevice, selectSelectedObjects } from './selectors';
import type { SliceCreator } from './types';

export type Theme = 'dark' | 'light';
export type DockTab = 'layers' | 'machine' | 'console' | 'history' | 'camera';

export interface ViewSlice {
  zoom: number;
  panX: number;
  panY: number;
  viewport: { width: number; height: number };
  /** Mientras no se haya movido la vista a mano, la cama se reencuadra al cambiar el tamaño de la ventana. */
  autoFit: boolean;
  /** Posición del cursor en mm, o null si está fuera del lienzo. */
  cursor: Point | null;
  wireframe: boolean;
  theme: Theme;
  dockTab: DockTab;
  dockWidth: number;
  /** Ajuste magnético: interruptor general y por tipo (cuadrícula / objetos y cama). */
  snapEnabled: boolean;
  snapGrid: boolean;
  snapObjects: boolean;
  /**
   * Contador que sube cuando el lienzo pide el foco del panel de texto (al crear un texto con la
   * herramienta de texto o al editarlo con doble clic). El panel de texto lo observa y enfoca su campo.
   */
  textFocusToken: number;

  setViewport: (width: number, height: number) => void;
  setCursor: (p: Point | null) => void;
  /** `factor` > 1 acerca; el punto de pantalla (sx, sy) no se mueve. */
  zoomAt: (factor: number, sx?: number, sy?: number) => void;
  panBy: (dx: number, dy: number) => void;
  fitBed: () => void;
  /** Encuadra la selección o, sin selección, todo el diseño. */
  fitContent: () => void;
  fitBounds: (b: Bounds, margin?: number) => void;
  toggleWireframe: () => void;
  setTheme: (t: Theme) => void;
  toggleTheme: () => void;
  setDockTab: (t: DockTab) => void;
  setDockWidth: (w: number) => void;
  toggleSnap: () => void;
  toggleSnapGrid: () => void;
  toggleSnapObjects: () => void;
  requestTextFocus: () => void;
}

export const DOCK_MIN = 280;
export const DOCK_MAX = 520;

function initialTheme(): Theme {
  const stored = readPref('theme');
  return stored === 'light' || stored === 'dark' ? stored : 'dark';
}

function initialDock(): number {
  const n = Number(readPref('dockWidth'));
  return Number.isFinite(n) && n >= DOCK_MIN && n <= DOCK_MAX ? n : 332;
}

/** Preferencia booleana guardada ('1' / '0'); `fallback` si no hay nada. */
function initialFlag(key: string, fallback: boolean): boolean {
  const v = readPref(key);
  return v === '1' ? true : v === '0' ? false : fallback;
}

export function applyThemeToDocument(theme: Theme): void {
  if (typeof document !== 'undefined') document.documentElement.dataset.theme = theme;
}

export const createViewSlice: SliceCreator<ViewSlice> = (set, get) => ({
  zoom: 1.5,
  panX: 40,
  panY: 600,
  viewport: { width: 0, height: 0 },
  autoFit: true,
  cursor: null,
  wireframe: false,
  theme: initialTheme(),
  dockTab: 'layers',
  dockWidth: initialDock(),
  snapEnabled: initialFlag('snap', true),
  snapGrid: initialFlag('snapGrid', true),
  snapObjects: initialFlag('snapObjects', true),
  textFocusToken: 0,

  setViewport: (width, height) => {
    set({ viewport: { width, height } });
    if (get().autoFit && width > 0) get().fitBed();
  },
  setCursor: (cursor) => set({ cursor }),

  zoomAt: (factor, sx, sy) => {
    const { zoom, panX, panY, viewport } = get();
    const next = zoomAround({ zoom, panX, panY }, factor, sx ?? viewport.width / 2, sy ?? viewport.height / 2);
    set({ ...next, autoFit: false });
  },
  panBy: (dx, dy) => set((s) => ({ panX: s.panX + dx, panY: s.panY + dy, autoFit: false })),

  fitBounds: (b, margin) => {
    const { width, height } = get().viewport;
    if (width <= 0 || height <= 0) return;
    set({ ...fitBounds(b, width, height, margin), autoFit: false });
  },
  fitBed: () => {
    const [w, h] = selectActiveDevice(get()).bed;
    get().fitBounds([[0, 0], [w, h]], 36);
    set({ autoFit: true });
  },
  fitContent: () => {
    const s = get();
    const objs = s.selectedIds.length ? selectSelectedObjects(s) : s.doc.objects.filter((o) => !o.hidden);
    const b = unionBounds(objs.map(objectBounds));
    if (!b) get().fitBed();
    else get().fitBounds(b, 64);
  },

  toggleWireframe: () => set((s) => ({ wireframe: !s.wireframe })),
  setTheme: (theme) => {
    writePref('theme', theme);
    applyThemeToDocument(theme);
    set({ theme });
  },
  toggleTheme: () => get().setTheme(get().theme === 'dark' ? 'light' : 'dark'),
  setDockTab: (dockTab) => set({ dockTab }),
  setDockWidth: (w) => {
    const dockWidth = Math.round(Math.min(DOCK_MAX, Math.max(DOCK_MIN, w)));
    writePref('dockWidth', String(dockWidth));
    set({ dockWidth });
  },
  toggleSnap: () => {
    const snapEnabled = !get().snapEnabled;
    writePref('snap', snapEnabled ? '1' : '0');
    set({ snapEnabled });
  },
  toggleSnapGrid: () => {
    const snapGrid = !get().snapGrid;
    writePref('snapGrid', snapGrid ? '1' : '0');
    set({ snapGrid });
  },
  requestTextFocus: () => set((s) => ({ textFocusToken: s.textFocusToken + 1 })),
  toggleSnapObjects: () => {
    const snapObjects = !get().snapObjects;
    writePref('snapObjects', snapObjects ? '1' : '0');
    set({ snapObjects });
  },
});
