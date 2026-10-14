/**
 * Direcciones públicas del proyecto. Vacías (`''`) mientras no existan: la app no inventa
 * direcciones; el diálogo de donaciones dice que aún no hay cuentas y el de reporte, que el enlace
 * está pendiente.
 *
 * PENDIENTE (lo llena quien publique el proyecto; ver «Pasos para publicar» en docs/publicar.md):
 *  - `repo` / `issues`: URL del repositorio y de su página de incidencias.
 *  - `sponsors`: página de GitHub Sponsors (https://github.com/sponsors/<usuario>).
 *  - `kofi`: página de Ko-fi (https://ko-fi.com/<usuario>).
 */
export interface ProjectLinks {
  repo: string;
  issues: string;
  sponsors: string;
  kofi: string;
}

export const PROJECT_LINKS: ProjectLinks = {
  repo: '',
  issues: '',
  sponsors: '',
  kofi: '',
};

/** Los enlaces de donación que sí están configurados, en orden de aparición. */
export function donationLinks(links: ProjectLinks = PROJECT_LINKS): { id: 'sponsors' | 'kofi'; url: string }[] {
  return (['sponsors', 'kofi'] as const).flatMap((id) => (links[id].trim() ? [{ id, url: links[id].trim() }] : []));
}
