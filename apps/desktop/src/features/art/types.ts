import type { DesignObject, LayerSettings, Point } from '@/api/types';

/** Un SVG de la biblioteca del usuario (`<configuración>/art/<name>.svg`). */
export interface UserArt {
  name: string;
  svg: string;
}

/** Lo que dejó agregar archivos: los que entraron y el motivo de los que no. */
export interface AddArtResult {
  added: UserArt[];
  errors: string[];
}

/**
 * Contenido de un componente guardado: los objetos con sus capas y ajustes, en el formato propio de
 * Tlaku (los mismos `DesignObject` y `LayerSettings` del documento y del portapapeles interno).
 */
export interface ComponentData {
  format: 'tlaku-component';
  version: 1;
  /** Fecha de creación, ISO 8601. */
  created: string;
  objects: DesignObject[];
  layers: LayerSettings[];
  /** Caja original de los objetos en el documento, en mm: `[[x0, y0], [x1, y1]]`. */
  bounds: [Point, Point];
  /** SVG en miniatura (vacío si no hay nada dibujable, por ejemplo solo imágenes). */
  thumb: string;
}

/** Un componente de la biblioteca del usuario (`<configuración>/components/<name>.tlakucomp`). */
export interface UserComponent {
  name: string;
  data: ComponentData;
}

/** Lo que dejó guardar componentes: los que entraron (con su nombre final) y el motivo de los que no. */
export interface SaveComponentsResult {
  added: UserComponent[];
  errors: string[];
}
