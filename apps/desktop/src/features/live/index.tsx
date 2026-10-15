import { useEffect } from 'react';
import { useStore } from '@/store';
import { startLiveBridge } from './detach';
import { startLive } from './service';

export { LiveWindow } from './LiveWindow';
export { LiveToggle } from './LiveToggle';

/**
 * Punto de montaje del servicio de la vista en vivo (lo monta el shell una vez, fuera de la `key`
 * del idioma): sigue la posición de la máquina y, si la ventana aparte está abierta, le manda la
 * ruta y el estado. La ventana flotante (`LiveWindow`) va dentro del shell, con el resto de los
 * diálogos de la máquina.
 */
export function LiveHost() {
  useEffect(() => {
    const stopService = startLive(useStore);
    const stopBridge = startLiveBridge();
    return () => {
      stopBridge();
      stopService();
    };
  }, []);
  return null;
}
