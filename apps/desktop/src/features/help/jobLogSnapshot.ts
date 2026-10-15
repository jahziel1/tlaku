import type { DesignObject, Document, JobPreview, LayerSettings, MaterialPreset } from '@/api/types';
import { jobMetrics } from '@/features/costs/costMetrics';
import { layerMatchesPreset } from '@/features/materials/library';
import { materialName, titleName } from '@/features/materials/labels';
import { useMaterials } from '@/features/materials/state';
import { cutRuns, simplify } from '@/features/remote/snapshot';
import { selectActiveDevice } from '@/store';
import { tabDoc } from '@/store/tabs';
import type { AppState } from '@/store/types';
import { LAYER_LOG_FIELDS, THUMB_MAX_CHARS, type JobLayerLog, type JobSnapshot, type LayerLogSettings } from './jobLog';

/**
 * Lo que el historial anota de un trabajo al empezar, aparte del tiempo: de qué pestaña y archivo salió, las
 * capas con sus ajustes, el material de la biblioteca (si los ajustes de una capa coinciden con los de un
 * material), el área, la longitud de corte y una miniatura del recorrido. Todo sale de la pestaña desde la que
 * se lanzó el trabajo (`jobOrigin`), no de la que esté activa ahora.
 */

/** Más movimientos que esto: no se arma ni la miniatura ni las medidas (trabajos enormes de imagen). */
const MAX_MOVES = 2_000_000;
/** Puntos de la miniatura: poco, para que la entrada pese unos 3 KB. */
const THUMB_POINTS = 450;

const presetName = (p: MaterialPreset): string => `${materialName(p)} · ${titleName(p)}`;

/** Ajustes de la capa que el historial recuerda. */
export function layerLogSettings(layer: LayerSettings): LayerLogSettings {
  const out: LayerLogSettings = {};
  const src = layer as unknown as Record<string, unknown>;
  for (const k of LAYER_LOG_FIELDS) {
    const v = src[k];
    if (typeof v === 'boolean' || (typeof v === 'number' && Number.isFinite(v))) out[k] = v;
  }
  return out;
}

/** Material cuyos ajustes coinciden con los de la capa (primero la biblioteca del usuario, luego la de fábrica). */
export function materialOfLayer(layer: LayerSettings, presets: readonly MaterialPreset[]): string | null {
  const hit = presets.find((p) => layerMatchesPreset(layer, p));
  return hit ? presetName(hit) : null;
}

/** Capas que salieron en el trabajo: las de la vista previa si la hay; si no, las que tienen salida y objetos. */
function jobLayers(doc: Pick<Document, 'layers' | 'objects'>, preview: JobPreview | null): LayerSettings[] {
  const byColor = new Map(doc.layers.map((l) => [l.color.toLowerCase(), l]));
  if (preview && preview.layers.length > 0) {
    const seen = new Set<string>();
    const out: LayerSettings[] = [];
    for (const pl of preview.layers) {
      const key = pl.color.toLowerCase();
      const layer = byColor.get(key);
      if (!layer || seen.has(key)) continue;
      seen.add(key);
      out.push(layer);
    }
    return out;
  }
  const used = new Set(doc.objects.filter((o: DesignObject) => !o.hidden).map((o) => o.layer.toLowerCase()));
  return doc.layers.filter((l) => l.output && used.has(l.color.toLowerCase()));
}

/** Resumen de capas con sus ajustes y, si coinciden con la biblioteca, su material. */
export function summarizeLayers(doc: Pick<Document, 'layers' | 'objects'>, preview: JobPreview | null, presets: readonly MaterialPreset[]): JobLayerLog[] {
  return jobLayers(doc, preview).map((l) => {
    const material = materialOfLayer(l, presets);
    return { color: l.color, name: l.name, mode: l.mode, ...(material ? { material } : {}), settings: layerLogSettings(l) };
  });
}

const r1 = (v: number) => String(Math.round(v * 10) / 10);

/**
 * Miniatura SVG del recorrido con láser (sin saltos), a la escala de la cama y con Y hacia arriba. Corta y
 * pequeña a propósito (`THUMB_MAX_CHARS`); `null` si no hay nada que dibujar.
 */
export function buildThumbnail(preview: JobPreview | null, bed: readonly [number, number]): string | null {
  if (!preview || preview.moves.length === 0 || preview.moves.length / 4 > MAX_MOVES) return null;
  const [w, h] = bed;
  if (!(w > 0) || !(h > 0)) return null;
  let runs = cutRuns(preview.moves);
  if (runs.length === 0) return null;
  const eps = Math.hypot(w, h) / 220;
  runs = runs.map((r) => simplify(r, eps));
  const points = runs.reduce((a, r) => a + r.length / 2, 0);
  if (points > THUMB_POINTS) {
    const stride = Math.ceil(points / THUMB_POINTS);
    runs = runs.filter((_, i) => i % stride === 0);
  }
  const d = runs.map((r) => 'M' + r1(r[0]!) + ' ' + r1(r[1]!) + r.slice(2).reduce((acc, v, k) => acc + (k % 2 === 0 ? 'L' : ' ') + r1(v), '')).join('');
  const sw = r1(Math.max(w, h) / 120);
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${r1(w)} ${r1(h)}" width="96" height="${Math.max(24, Math.round((96 * h) / w))}">` +
    `<rect width="${r1(w)}" height="${r1(h)}" fill="#1b1f27"/>` +
    `<path transform="matrix(1 0 0 -1 0 ${r1(h)})" d="${d}" fill="none" stroke="#f2b84b" stroke-width="${sw}" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
  return svg.length <= THUMB_MAX_CHARS ? svg : null;
}

/** Pestaña, archivo, capas, material, medidas y miniatura del trabajo que acaba de empezar. */
export function buildJobSnapshot(s: AppState): JobSnapshot {
  const origin = s.jobOrigin;
  const doc = (origin ? tabDoc(s, origin.tabId) : null) ?? s.doc;
  const preview = origin ? origin.preview : s.previewStatus === 'ready' && !s.previewStale ? s.preview : null;
  const presets = (() => {
    const ui = useMaterials.getState();
    return [...ui.library.presets, ...ui.factory.presets];
  })();
  const layers = summarizeLayers(doc, preview, presets);
  const materials = [...new Set(layers.map((l) => l.material).filter((m): m is string => !!m))];
  let areaMm2: number | null = null;
  let lengthMm: number | null = null;
  if (preview && preview.moves.length / 4 <= MAX_MOVES) {
    try {
      const m = jobMetrics(doc, preview, null);
      areaMm2 = m.hasDesign ? Math.round(m.realMm2) : null;
      lengthMm = m.hasPreview ? Math.round(m.cutMm) : null;
    } catch {
      /* la medida es un extra: un fallo no debe impedir anotar el trabajo */
    }
  }
  const device = selectActiveDevice(s);
  return {
    documentPath: origin ? origin.path : s.filePath,
    tab: origin ? origin.name : null,
    layers,
    material: materials.length > 0 ? materials.join(', ') : null,
    areaMm2,
    lengthMm,
    thumb: buildThumbnail(preview, [device.bed[0], device.bed[1]]),
  };
}
