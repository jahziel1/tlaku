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
