import type { StoreApi, UseBoundStore } from 'zustand';
import type { AppState } from '@/store';
import { useStore } from '@/store';

/** Para las pruebas: deja una sola pestaña, vacía y nueva (sin preguntar nada). */
export function resetTabs(store: UseBoundStore<StoreApi<AppState>> = useStore): void {
  const s = () => store.getState();
  for (const e of [...s().tabs]) if (s().tabs.length > 1) s().removeTab(e.id);
  s().removeTab(s().activeTabId);
}
