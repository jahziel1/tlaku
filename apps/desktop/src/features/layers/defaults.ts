import { t } from '@/i18n';
import { layerTemplate, setLayerDefaults } from '@/lib/layers';
import type { AppState } from '@/store';
import { useStore } from '@/store';

/**
 * Valores por defecto de las capas nuevas (C35). Viven en las preferencias (`layerDefaults`) y
 * `lib/layers.ts` los usa al crear una capa; este módulo mantiene ambos al día.
 */
export function syncLayerDefaults(): void {
  setLayerDefaults(useStore.getState().prefs.layerDefaults);
}

// Al cargar el módulo y cada vez que cambian las preferencias (se leen del disco después de arrancar).
syncLayerDefaults();
useStore.subscribe((s, prev) => {
  if (s.prefs.layerDefaults !== prev.prefs.layerDefaults) setLayerDefaults(s.prefs.layerDefaults);
});

/** Guarda los ajustes de esta capa como los valores con que empiezan las capas nuevas. */
export function saveLayerAsDefaults(s: AppState, color: string): void {
  const layer = s.doc.layers.find((l) => l.color === color);
  if (!layer) return;
  const template = layerTemplate(layer);
  s.setPref('layerDefaults', template);
  // Que `makeLayer` los use en este mismo instante, sin esperar al aviso del store.
  setLayerDefaults(template);
  s.notify('success', t('layers.defaults.saved'));
}

/** Vuelve a los valores de fábrica para las capas nuevas. */
export function resetLayerDefaults(s: AppState): void {
  s.setPref('layerDefaults', null);
  setLayerDefaults(null);
  s.notify('success', t('layers.defaults.restored'));
}
