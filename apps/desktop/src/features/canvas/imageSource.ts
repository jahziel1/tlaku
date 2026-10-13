import { imageUrl } from '@/api';

/** URL para dibujar una imagen del documento (`imageUrl` viene de `src/api`); `null` si falla. */
export function resolveImageUrl(sourcePath: string): string | null {
  try {
    return imageUrl(sourcePath);
  } catch {
    return null;
  }
}

interface Entry {
  img: HTMLImageElement | null;
  state: 'loading' | 'ready' | 'failed';
}

/** Imágenes ya pedidas, por ruta de origen. Una falla no se reintenta en cada cuadro. */
export class ImageCache {
  private entries = new Map<string, Entry>();
  constructor(private readonly onChange: () => void) {}

  /** Imagen lista para dibujar, o `null` (cargando, fallida o sin `imageUrl`: se dibuja un marcador). */
  get(sourcePath: string): HTMLImageElement | null {
    let e = this.entries.get(sourcePath);
    if (!e) {
      const url = resolveImageUrl(sourcePath);
      if (!url || typeof Image === 'undefined') {
        e = { img: null, state: 'failed' };
      } else {
        const img = new Image();
        e = { img, state: 'loading' };
        const entry = e;
        img.onload = () => {
          entry.state = 'ready';
          this.onChange();
        };
        img.onerror = () => {
          entry.state = 'failed';
        };
        img.src = url;
      }
      this.entries.set(sourcePath, e);
    }
    return e.state === 'ready' ? e.img : null;
  }

  clear(): void {
    this.entries.clear();
  }
}
