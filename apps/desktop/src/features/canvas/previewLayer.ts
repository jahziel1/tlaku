import { BlockCache, type BlockPaths } from '@/features/preview/blocks';
import { buildLayerPaths, drawPreviewModel, qualityLevel } from '@/features/preview/draw';
import { getModel, type PreviewData, type PreviewModel } from '@/features/preview/model';
import { drawTonal, hasTonalImage } from '@/features/tonal/layers';
import { useTonal } from '@/features/tonal/store';
import { useStore } from '@/store';
import type { Scene } from './renderer';

/**
 * Dibuja el recorrido de la vista previa sobre el lienzo. Este archivo es del área de vista
 * previa; el renderer solo lo llama al final de cada cuadro con `drawPreview(ctx, scene)`.
 * La lógica vive en `features/preview` (modelo, caché por bloques y dibujo); aquí solo se
 * conectan con `Path2D` y con el estado de la vista previa.
 */

const makePath = () => new Path2D();

interface ModelCaches {
  blocks: BlockCache<Path2D>;
  highlight: { layer: number; paths: BlockPaths<Path2D> | null } | null;
}

const caches = new WeakMap<PreviewModel, ModelCaches>();

function cachesFor(model: PreviewModel): ModelCaches {
  let c = caches.get(model);
  if (!c) {
    c = { blocks: new BlockCache(model, makePath), highlight: null };
    caches.set(model, c);
  }
  return c;
}

export function drawPreview(ctx: CanvasRenderingContext2D, scene: Scene): void {
  if (!scene.preview) return;
  // Vista «por tonos»: la imagen simulada reemplaza a los trazos (hasta que llegue, se ven los trazos).
  if (useTonal.getState().mode === 'tonal' && hasTonalImage()) {
    drawTonal(ctx, scene.view, scene.dpr);
    return;
  }
  const model = getModel(scene.preview.data);
  if (model.count === 0) return;
  const c = cachesFor(model);
  const options = useStore.getState().previewView;

  let highlight: BlockPaths<Path2D> | null = null;
  if (options.highlightLayer !== null) {
    if (c.highlight?.layer !== options.highlightLayer) {
      c.highlight = { layer: options.highlightLayer, paths: buildLayerPaths(model, options.highlightLayer, makePath) };
    }
    highlight = c.highlight.paths;
  }

  drawPreviewModel({
    ctx,
    model,
    cache: c.blocks,
    factory: makePath,
    view: scene.view,
    width: scene.width,
    height: scene.height,
    palette: scene.palette,
    scrub: scene.preview.scrub,
    options,
    highlight,
  });
}

/**
 * Construye los trazados pendientes en ratos libres, para que el primer dibujo con el recorrido
 * completo no se trabe. Se detiene sola si llega otra vista previa. Devuelve la función que cancela.
 */
export function warmPreviewCache(data: PreviewData): () => void {
  if (typeof Path2D === 'undefined') return () => {};
  const model = getModel(data);
  const { blocks } = cachesFor(model);
  const level = qualityLevel(useStore.getState().zoom);
  let timer: ReturnType<typeof setTimeout> | undefined;
  const step = () => {
    timer = undefined;
    if (!blocks.warm(6, level)) timer = setTimeout(step, 0);
  };
  timer = setTimeout(step, 0);
  return () => clearTimeout(timer);
}
