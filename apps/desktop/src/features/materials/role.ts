import type { MaterialPreset, MaterialRole } from '@/api/types';

/** Rol de un ajuste; si falta (bibliotecas viejas) se deduce del modo de la capa: línea = corte, lo demás = grabado. */
export function effectiveRole(p: Pick<MaterialPreset, 'role' | 'settings'>): MaterialRole {
  return p.role ?? (p.settings.mode === 'line' ? 'cut' : 'engrave');
}
