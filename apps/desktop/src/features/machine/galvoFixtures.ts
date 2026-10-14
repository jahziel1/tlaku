import type { ReactElement } from 'react';
import { renderToString } from 'react-dom/server';
import type { DeviceProfile } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import { useStore } from '@/store';

/** Un perfil galvo de partida para las pruebas: fibra de 20 W, campo de 110 mm, origen abajo a la izquierda. */
export const galvoDevice = (over: Partial<DeviceProfile> = {}): DeviceProfile => ({
  ...structuredClone(FALLBACK_DEVICE),
  id: 'galvo-1',
  name: 'Mi galvo',
  kind: 'galvo',
  bed: [110, 110],
  origin: 'bottom_left',
  ...over,
});

/**
 * `renderToString` lee el estado INICIAL de la tienda (zustand usa `getInitialState` en el servidor), no el
 * actual: se copia el estado de ahora a ese objeto mientras se pinta y se restaura después.
 */
export function ssr(node: ReactElement): string {
  const store = useStore as unknown as { getInitialState: () => Record<string, unknown>; getState: () => Record<string, unknown> };
  const saved = { ...store.getInitialState() };
  Object.assign(store.getInitialState(), store.getState());
  try {
    return renderToString(node);
  } finally {
    Object.assign(store.getInitialState(), saved);
  }
}
