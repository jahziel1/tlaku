import { getApi } from '@/api';
import type { DesignObject, Point } from '@/api/types';
import { t } from '@/i18n';
import type { AppState } from '@/store';
import { uniqueId } from '@/store/document';
import { pickDrawLayer } from './shapes';

/** Tamaño del em (mm) del texto nuevo. */
export const DEFAULT_TEXT_SIZE_MM = 10;
const FALLBACK_FONT = 'Arial';

let fonts: string[] | null = null;
let loading = false;

/** Pide la lista de fuentes una vez (al elegir la herramienta de texto) para saber si hay Arial. */
export function prefetchFonts(): void {
  if (fonts || loading) return;
  loading = true;
  getApi()
    .listFonts()
    .then((list) => {
      fonts = list.map((f) => f.family);
    })
    .catch(() => {
      fonts = [];
    })
    .finally(() => {
      loading = false;
    });
}

/** Fuente de un texto nuevo: Arial si existe; si no, la primera disponible; sin lista, Arial. */
export function defaultFontFamily(available: readonly string[] | null = fonts): string {
  if (!available || available.length === 0) return FALLBACK_FONT;
  return available.find((f) => f.toLowerCase() === FALLBACK_FONT.toLowerCase()) ?? available[0]!;
}

/** Texto nuevo con su línea base empezando en `at` (mm), sin contorno: el panel de texto lo genera. */
export function newTextObject(id: string, layer: string, at: Point, family = defaultFontFamily()): DesignObject {
  return {
    id,
    layer,
    transform: [1, 0, 0, 1, at[0], at[1]],
    kind: {
      type: 'text',
      text: t('canvas.text.default'),
      font_family: family,
      bold: false,
      italic: false,
      size_mm: DEFAULT_TEXT_SIZE_MM,
      letter_spacing_mm: 0,
      line_spacing: 1.2,
      align: 'left',
      on_path: null,
      outline: [],
    },
    locked: false,
    hidden: false,
    group: null,
  };
}

/** Crea un texto en `at` en la capa activa, lo selecciona y pide el foco del panel de texto. Devuelve su id. */
export function createTextAt(s: AppState, at: Point): string {
  const id = uniqueId('text', new Set(s.doc.objects.map((o) => o.id)));
  const layer = pickDrawLayer(s.doc.layers, s.activeLayer);
  s.addObjects([newTextObject(id, layer, at)], 'canvas.draw.text');
  s.select([id]);
  s.requestTextFocus();
  return id;
}

/** Selecciona un texto existente para editarlo y pide el foco del panel de texto. */
export function editText(s: AppState, id: string): void {
  s.select([id]);
  s.requestTextFocus();
}
