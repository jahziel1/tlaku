import type { BezPath } from '@/lib/bezpath';

/**
 * Firma con la que se generó cada contorno. Un texto está al día si la firma de su contorno
 * coincide con la de sus parámetros actuales. Un contorno que no conocemos (archivo abierto,
 * pegado de otro proyecto) se da por bueno la primera vez que se ve: así abrir un proyecto no
 * regenera todos los textos, y deshacer/rehacer devuelven contornos que ya están al día.
 */
const outlineSigs = new WeakMap<BezPath[], string>();

/** ¿Este contorno se generó con la firma `sig`? (sin dar por buenos los que no se conocen). */
export function outlineMatches(outline: BezPath[], sig: string): boolean {
  return outlineSigs.get(outline) === sig;
}

/** Registra el contorno recién generado para la firma `sig` (lo usa también la resolución al enviar). */
export function rememberOutline(outline: BezPath[], sig: string): void {
  outlineSigs.set(outline, sig);
}

/** ¿El contorno del texto corresponde a `sig`? Registra los contornos desconocidos y no vacíos. */
export function isOutlineCurrent(outline: BezPath[], sig: string, doneSig?: string, trustUnknown = true): boolean {
  const known = outlineSigs.get(outline);
  if (known !== undefined) return known === sig;
  // Un texto variable guardado con otros valores no se da por bueno: se regenera con los de ahora.
  if (outline.length > 0 && trustUnknown) {
    outlineSigs.set(outline, sig);
    return true;
  }
  // Un texto vacío (o solo espacios) genera un contorno vacío legítimo.
  return doneSig === sig;
}

