import { useEffect } from 'react';
import { useStore } from '@/store';
import { startLive } from './service';

export { LiveOverlay } from './LiveOverlay';
export { LiveToggle } from './LiveToggle';

/**
 * Punto de montaje del servicio de la vista en vivo (lo monta el shell una vez, fuera de la `key`
 * del idioma): sigue la posición de la máquina. El lienzo (`LiveOverlay`) va dentro del shell, con
 * el resto de los diálogos de la máquina, porque se monta en el área del lienzo.
 */
export function LiveHost() {
  useEffect(() => startLive(useStore), []);
  return null;
}
