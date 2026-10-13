import { useEffect } from 'react';
import { startAutomation } from './automation';

export { AutomationPanel } from './AutomationPanel';

/**
 * Punto de montaje de la automatización por UDP (lo monta el shell una sola vez, fuera de la `key`
 * del idioma): mantiene el servidor del motor al día con las preferencias y atiende `LOAD` y `START`.
 */
export function AutomationHost() {
  useEffect(() => startAutomation(), []);
  return null;
}
