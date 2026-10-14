import { TilingPanel } from './TilingPanel';
import { useTilingUi } from './store';

export { TilingOffer } from './TilingOffer';
export { closeTiling, openTiling, useTilingUi } from './store';

/** Punto de montaje del asistente de tiling (lo monta el shell una sola vez): pinta el panel si está abierto. */
export function TilingHost() {
  const open = useTilingUi((s) => s.open);
  return open ? <TilingPanel /> : null;
}
