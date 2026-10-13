import { useEffect } from 'react';
import type { JobPreview } from '@/api/types';
import { warmPreviewCache } from '@/features/canvas/previewLayer';
import { useStore } from '@/store';
import { startPreviewSync } from './sync';

/**
 * Recalcula el trabajo cuando cambian el documento, las capas o el dispositivo (ver `sync.ts`) y
 * precalienta la caché de trazados cuando llega un resultado nuevo.
 */
export function usePreviewSync(): void {
  useEffect(() => {
    const stop = startPreviewSync(useStore);
    let cancelWarm = () => {};
    const warm = (preview: JobPreview | null) => {
      cancelWarm();
      cancelWarm = preview ? warmPreviewCache(preview) : () => {};
    };
    warm(useStore.getState().preview);
    const unsubWarm = useStore.subscribe((s, prev) => {
      // El renuevo de `preview` por cambios visuales conserva `moves`: no hay nada que recalentar.
      if (s.preview?.moves !== prev.preview?.moves) warm(s.preview);
    });
    return () => {
      stop();
      unsubWarm();
      cancelWarm();
    };
  }, []);
}
