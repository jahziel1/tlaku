import { getApi, getApiKind } from '@/api';
import { bytesToBase64, textToBase64 } from './board';

type Format = 'pdf' | 'svg';

/** Descarga en el navegador (modo mock): no hay diálogo nativo. */
function download(name: string, mime: string, data: BlobPart): string {
  const url = URL.createObjectURL(new Blob([data], { type: mime }));
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  return name;
}

/**
 * Pide dónde guardar el tablero y lo escribe. Devuelve la ruta (o `null` si el usuario cancela).
 */
export async function saveBoard(format: Format, content: string | Uint8Array): Promise<string | null> {
  const name = `tablero-calibracion.${format}`;
  if (getApiKind() !== 'tauri') {
    return download(name, format === 'pdf' ? 'application/pdf' : 'image/svg+xml', content as BlobPart);
  }
  const dialog = await import('@tauri-apps/plugin-dialog');
  const path = await dialog.save({ defaultPath: name, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
  if (!path) return null;
  const content_base64 = typeof content === 'string' ? textToBase64(content) : bytesToBase64(content);
  await getApi().saveBoardFile({ path, content_base64 });
  return path;
}
