import { convertFileSrc } from '@tauri-apps/api/core';
import type { LaserApi } from './api';
import { createMockApi } from './mock';
import { createTauriApi } from './tauri';

export type { LaserApi, Unlisten } from './api';
export type { PrefsBundle, PrefsFile, RecoveryInfo, TemplateInfo, UserBundle } from './api';
export * from './types';

export type ApiKind = 'mock' | 'tauri';

/** VITE_API=mock|tauri. Por defecto, mock. */
export function resolveApiKind(raw: string | undefined = import.meta.env.VITE_API): ApiKind {
  return raw === 'tauri' ? 'tauri' : 'mock';
}

let current: LaserApi | null = null;
let currentKind: ApiKind | null = null;

/** Implementación activa (singleton). Se crea la primera vez que se pide. */
export function getApi(): LaserApi {
  if (!current) {
    currentKind = resolveApiKind();
    current = currentKind === 'tauri' ? createTauriApi() : createMockApi();
  }
  return current;
}

export function getApiKind(): ApiKind {
  getApi();
  return currentKind ?? 'mock';
}

/** Para pruebas: sustituye la implementación activa. */
export function setApi(api: LaserApi, kind: ApiKind = 'mock'): void {
  current = api;
  currentKind = kind;
}

/**
 * Imagen gris de relleno para el modo mock (navegador): no hay disco local que servir.
 * Es un SVG en data: URI, permitido por la CSP (`img-src data:`).
 */
export const IMAGE_PLACEHOLDER =
  'data:image/svg+xml;utf8,' +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" preserveAspectRatio="none">' +
      '<rect width="64" height="64" fill="#7d8590"/>' +
      '<path d="M0 64 L22 30 L36 48 L46 36 L64 64 Z" fill="#a9b1ba"/>' +
      '<circle cx="46" cy="18" r="6" fill="#a9b1ba"/></svg>',
  );

/**
 * URL con la que la interfaz muestra un archivo de imagen del disco (`ImageObject.source_path`)
 * en un `<img>` o en un canvas.
 *
 * - Tauri: `convertFileSrc(path)` (protocolo `asset://`). Funciona si la ruta está dentro del
 *   alcance de `app.security.assetProtocol` en `tauri.conf.json` o si el motor la permitió al
 *   importarla o abrir el proyecto (`import_file` / `load_project`).
 * - Mock: un SVG de relleno, siempre el mismo.
 *
 * Para dibujar en un canvas sin "ensuciarlo", crea la imagen con `crossOrigin = 'anonymous'`.
 */
export function imageUrl(path: string): string {
  return getApiKind() === 'tauri' ? convertFileSrc(path) : IMAGE_PLACEHOLDER;
}
