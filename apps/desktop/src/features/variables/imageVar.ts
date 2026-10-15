import type { DesignObject, ImageFit } from '@/api/types';
import { roundedRectBez } from '@/lib/bezpath';
import type { VarContext } from './resolve';
import { variablesOf } from './syntax';
import type { VariableImageObject } from './varObjects';

/** ¿Es una ruta absoluta (`/x`, `C:\x`, `\\servidor\x`)? */
export function isAbsolutePath(path: string): boolean {
  return /^([a-zA-Z]:[\\/]|[\\/])/.test(path);
}

/** Carpeta de un archivo (sin la barra final); vacío si la ruta no tiene carpeta. */
export function dirName(path: string): string {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return i < 0 ? '' : i === 0 ? path.slice(0, 1) : path.slice(0, i);
}

/** Une una carpeta y una ruta relativa, resolviendo `.` y `..` y respetando el separador de la carpeta. */
export function joinPath(dir: string, rel: string): string {
  const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
  const root = /^[a-zA-Z]:/.exec(dir)?.[0] ?? (dir.startsWith('\\\\') ? '\\\\' : dir.startsWith('/') ? '/' : '');
  const parts: string[] = dir.slice(root.length).split(/[\\/]/).filter((p) => p !== '');
  for (const seg of rel.split(/[\\/]/)) {
    if (seg === '' || seg === '.') continue;
    if (seg === '..') parts.pop();
    else parts.push(seg);
  }
  const lead = root === '/' || root === '\\\\' ? root : root !== '' ? root + sep : '';
  return lead + parts.join(sep);
}

/** Carpeta del CSV de donde sale la primera columna de la plantilla (las rutas relativas de imagen parten de ella). */
export function csvBaseDir(template: string, ctx: VarContext): string | null {
  const v = variablesOf(template).find((x) => x.name === 'csv' || x.name === 'row');
  if (!v) return null;
  const src = v.source === '' ? ctx.sources?.[0] : ctx.sources?.find((s) => s.name === v.source.toLowerCase());
  return src?.path ? dirName(src.path) : null;
}

/**
 * Ruta del archivo de una celda del CSV: sin comillas ni espacios en los extremos; si es
 * relativa, parte de la carpeta del CSV de donde salió (`baseDir`). Sin carpeta conocida se deja
 * relativa (y el sondeo dirá que no existe).
 */
export function resolveImagePath(cell: string, baseDir: string | null): string {
  const raw = cell.trim().replace(/^"(.*)"$/, '$1').trim();
  if (raw === '' || isAbsolutePath(raw) || !baseDir) return raw;
  return joinPath(baseDir, raw);
}

/**
 * La imagen variable con el archivo `path` (de `px` píxeles) puesto en la caja de la imagen
 * marcada. `contain` la deja entera y centrada en la caja; `cover` llena la caja y recorta lo que
 * sobra con una máscara. Conserva capa, orden y ajustes del objeto, así que se graba con el modo de
 * imagen de su capa. La caja se mide con el objeto ya escalado (el `transform` puede escalar distinto
 * en X e Y).
 */
export function fitImageObject(o: VariableImageObject, path: string, px: { width: number; height: number }, fit: ImageFit = o.kind.variable.fit): DesignObject {
  const [a, b, c, d, e, f] = o.transform;
  const sx = Math.hypot(a, b);
  const sy = Math.hypot(c, d);
  const [bw, bh] = o.kind.size_mm;
  const boxW = bw * sx;
  const boxH = bh * sy;
  if (!(sx > 0 && sy > 0 && boxW > 0 && boxH > 0 && px.width > 0 && px.height > 0)) {
    return { ...o, kind: { ...o.kind, source_path: path, width_px: px.width, height_px: px.height, mask: [] } };
  }
  const k = fit === 'cover' ? Math.max(boxW / px.width, boxH / px.height) : Math.min(boxW / px.width, boxH / px.height);
  // Tamaño nuevo y desplazamiento de su esquina, en unidades locales del objeto.
  const lw = (px.width * k) / sx;
  const lh = (px.height * k) / sy;
  const dx = (bw - lw) / 2;
  const dy = (bh - lh) / 2;
  // Solo se recorta si la imagen nueva sobrepasa la caja.
  const overflow = lw > bw + 1e-9 || lh > bh + 1e-9;
  const mask = fit === 'cover' && overflow ? [roundedRectBez(-dx, -dy, -dx + bw, -dy + bh)] : [];
  return {
    ...o,
    transform: [a, b, c, d, e + a * dx + c * dy, f + b * dx + d * dy],
    kind: { ...o.kind, source_path: path, width_px: px.width, height_px: px.height, size_mm: [lw, lh], mask },
  };
}
