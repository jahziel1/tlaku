import type { Point } from '@/api/types';
import { applyAffine, objectBounds, unionBounds, type Bounds } from '@/lib/geometry';
import { virtualCopies } from '@/lib/virtualArray';
import type { DesignObject } from '@/api/types';
import { setDockTabHidden } from '@/lib/dockLayout';
import { readPref, writePref } from '@/lib/platform';
import { fitBounds, zoomAround } from '@/lib/viewport';
import { selectActiveDevice, selectSelectedObjects } from './selectors';
import type { SliceCreator } from './types';

export type Theme = 'dark' | 'light';
export type DockTab = 'layers' | 'machine' | 'console' | 'history' | 'camera' | 'materials' | 'notes';

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
  /** Muestra las guías del documento (Ver); ocultas no se dibujan ni atraen. */
  showGuides: boolean;
  /** Muestra la flecha de inicio y sentido de todos los contornos con inicio fijado (Ver); con el objeto seleccionado siempre se ve. */
  showStartMarks: boolean;
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
  toggleGuides: () => void;
  toggleStartMarks: () => void;
  requestTextFocus: () => void;
}

/** Caja del objeto y de sus copias virtuales (arreglo virtual): lo que ocupa en el trabajo. */
export function objectBoundsWithCopies(o: DesignObject): Bounds | null {
  const own = objectBounds(o);
  if (!own || !o.array) return own;
  const [[x0, y0], [x1, y1]] = own;
  const boxes = virtualCopies(o).map((m): Bounds => {
    const pts = [applyAffine(m, [x0, y0]), applyAffine(m, [x1, y0]), applyAffine(m, [x1, y1]), applyAffine(m, [x0, y1])];
    const xs = pts.map((p) => p[0]);
    const ys = pts.map((p) => p[1]);
    return [[Math.min(...xs), Math.min(...ys)], [Math.max(...xs), Math.max(...ys)]];
  });
  return unionBounds([own, ...boxes]);
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
  showGuides: initialFlag('showGuides', true),
  showStartMarks: initialFlag('showStartMarks', false),
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
    const b = unionBounds(objs.map(objectBoundsWithCopies));
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
  setDockTab: (dockTab) => {
    // Abrir una pestaña oculta la vuelve a mostrar (por ejemplo, con su atajo de teclado).
    const layout = get().prefs.dockLayout;
    if (layout.hidden.includes(dockTab)) get().setPref('dockLayout', setDockTabHidden(layout, dockTab, false));
    set({ dockTab });
  },
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
  toggleGuides: () => {
    const showGuides = !get().showGuides;
    writePref('showGuides', showGuides ? '1' : '0');
    set({ showGuides });
  },
  toggleStartMarks: () => {
    const showStartMarks = !get().showStartMarks;
    writePref('showStartMarks', showStartMarks ? '1' : '0');
    set({ showStartMarks });
  },
  toggleSnapObjects: () => {
    const snapObjects = !get().snapObjects;
    writePref('snapObjects', snapObjects ? '1' : '0');
    set({ snapObjects });
  },
});
