/** Utilidades de rutas y formatos de archivo (sin dependencias de interfaz). */

/** Formatos que sabe importar el motor. */
export const IMPORT_EXTENSIONS = ['svg', 'dxf', 'png', 'jpg', 'jpeg', 'bmp'] as const;

/** Nombre corto para mostrar (último tramo de la ruta). */
export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

export function extensionOf(path: string): string {
  const name = baseName(path);
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
}

export function isImportable(path: string): boolean {
  return (IMPORT_EXTENSIONS as readonly string[]).includes(extensionOf(path));
}

/** Agrega la extensión si quien eligió el nombre no escribió ninguna. */
export function withExtension(path: string, ext: string): string {
  return extensionOf(path) === '' ? `${path}.${ext}` : path;
}
