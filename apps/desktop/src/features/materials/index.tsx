import { t } from '@/i18n';

/** Panel de la biblioteca de materiales (pestaña del dock). TODO(fase4). */
export function MaterialsPanel() {
  return <div className="materials-panel">{t('materials.panel.empty')}</div>;
}

/**
 * Punto de montaje del área `materials` (lo monta el shell una sola vez): carga la biblioteca y
 * aloja el diálogo de prueba de materiales. TODO(fase4).
 */
export function MaterialsDialogHost() {
  return null;
}
