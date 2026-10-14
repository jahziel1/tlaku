import { useEffect } from 'react';
import { startRemote } from './remote';

export { RemotePanel } from './RemotePanel';

/**
 * Punto de montaje del control remoto (lo monta el shell una sola vez, fuera de la `key` del
 * idioma): mantiene el servidor del motor al día con las preferencias, le informa el estado del
 * trabajo y atiende los pedidos del celular.
 */
export function RemoteHost() {
  useEffect(() => startRemote(), []);
  return null;
}
