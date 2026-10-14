/**
 * Direcciones públicas del proyecto. Las de donación quedan vacías (`''`) mientras no existan: la app
 * no inventa direcciones y el diálogo de donaciones dice que aún no hay cuentas.
 *
 * PENDIENTE (opcional): `sponsors`, página de GitHub Sponsors (https://github.com/sponsors/<usuario>).
 *
 * Las direcciones que la app puede abrir en el navegador (`openExternal.ts`) deben coincidir con el
 * permiso `opener:allow-open-url` de `src-tauri/capabilities/default.json`.
 */
export interface ProjectLinks {
  repo: string;
  issues: string;
  sponsors: string;
  kofi: string;
}

/** Repositorio público del proyecto y su sitio (GitHub Pages). */
export const REPO_URL = 'https://github.com/jahziel1/tlaku';
export const SITE_URL = 'https://jahziel1.github.io/tlaku';
/** Página de donaciones en Ko-fi (la única dirección fuera del repositorio que la app puede abrir). */
export const KOFI_URL = 'https://ko-fi.com/danielmartinez55371';

export const PROJECT_LINKS: ProjectLinks = {
  repo: REPO_URL,
  issues: `${REPO_URL}/issues`,
  sponsors: '',
  kofi: KOFI_URL,
};

/** Los enlaces de donación que sí están configurados, en orden de aparición. */
export function donationLinks(links: ProjectLinks = PROJECT_LINKS): { id: 'sponsors' | 'kofi'; url: string }[] {
  return (['sponsors', 'kofi'] as const).flatMap((id) => (links[id].trim() ? [{ id, url: links[id].trim() }] : []));
}
