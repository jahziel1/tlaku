import { getApi, getApiKind } from '@/api';
import { extensionOf } from '@/features/history/paths';
import { t } from '@/i18n';
import { downloadText } from '@/features/materials/files';

/** Marca de orden de bytes UTF-8: sin ella Excel lee mal los acentos de un CSV. */
const BOM = '\uFEFF';

/** Nombre propuesto para el CSV del desglose. */
export const CSV_NAME = 'costos.csv';

/**
 * Guarda el CSV donde elija la persona (extensión `.csv`). En la app lo escribe el motor con el
 * mismo comando que los respaldos de texto; en el modo simulado (navegador) lo descarga.
 * Devuelve la ruta o el nombre, o `null` si canceló.
 */
export async function saveCostCsv(csv: string, suggested = CSV_NAME): Promise<string | null> {
  if (getApiKind() !== 'tauri') return downloadText(suggested, BOM + csv);
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.save({ filters: [{ name: t('costs.csv.filter'), extensions: ['csv'] }], defaultPath: suggested, title: t('costs.csv.saveTitle') });
  if (!picked) return null;
  const ext = extensionOf(picked);
  const path = ext === 'csv' || ext === 'txt' ? picked : `${picked}.csv`;
  await getApi().writeSettingsBackup({ path, text: BOM + csv });
  return path;
}
