import { getApi, getApiKind } from '@/api';
import { csvField } from '@/features/costs/costReport';
import { extensionOf } from '@/features/history/paths';
import { downloadText } from '@/features/materials/files';
import { t, type I18nKey } from '@/i18n';
import type { JobLogEntry } from './jobLog';
import { materialsOf } from './jobLogFilter';

/** Marca de orden de bytes UTF-8: sin ella Excel lee mal los acentos de un CSV. */
const BOM = '﻿';

/** Nombre propuesto para el CSV del historial. */
export const JOB_CSV_NAME = 'historial-de-trabajos.csv';

const COLUMNS: I18nKey[] = [
  'help.jobLog.csv.date',
  'help.jobLog.csv.machine',
  'help.jobLog.csv.document',
  'help.jobLog.csv.path',
  'help.jobLog.csv.tab',
  'help.jobLog.csv.material',
  'help.jobLog.csv.result',
  'help.jobLog.csv.message',
  'help.jobLog.csv.estimatedS',
  'help.jobLog.csv.actualS',
  'help.jobLog.csv.areaMm2',
  'help.jobLog.csv.lengthMm',
  'help.jobLog.csv.layers',
  'help.jobLog.csv.notes',
];

/** Fecha y hora local `AAAA-MM-DD HH:MM:SS` (sin zona: es la hora de la computadora). */
export function isoLocal(ms: number): string {
  const d = new Date(ms);
  const p = (n: number, w = 2) => String(n).padStart(w, '0');
  return `${p(d.getFullYear(), 4)}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

/** Las capas en una celda: «#ff0000 line 1200 mm/min 60 % ×2 | ...». */
export function layersCell(e: JobLogEntry): string {
  return (e.layers ?? [])
    .map((l) => {
      const s = l.settings;
      const parts = [`${l.name || l.color} (${l.mode})`];
      if (typeof s.speed === 'number') parts.push(`${Math.round(s.speed)} mm/min`);
      if (typeof s.power === 'number') parts.push(`${Math.round(s.power * 10) / 10} %`);
      if (typeof s.passes === 'number' && s.passes > 1) parts.push(`×${s.passes}`);
      if (typeof s.interval === 'number' && l.mode !== 'line') parts.push(`${s.interval} mm`);
      return parts.join(' ');
    })
    .join(' | ');
}

/** CSV del historial (coma, comillas y fin de línea CRLF), una fila por trabajo, de la más nueva a la más vieja. */
export function jobsCsv(entries: readonly JobLogEntry[]): string {
  const rows: string[] = [COLUMNS.map((k) => csvField(t(k))).join(',')];
  for (const e of entries) {
    const result = t(`help.jobLog.result.${e.result}` as I18nKey);
    rows.push(
      [
        isoLocal(e.at),
        e.machine,
        e.document ?? '',
        e.documentPath ?? '',
        e.tab ?? '',
        materialsOf(e).join(', '),
        result,
        e.message,
        e.estimatedS === null ? '' : String(Math.round(e.estimatedS)),
        String(Math.round(e.actualS)),
        e.areaMm2 == null ? '' : String(Math.round(e.areaMm2)),
        e.lengthMm == null ? '' : String(Math.round(e.lengthMm)),
        layersCell(e),
        e.notes ?? '',
      ]
        .map(csvField)
        .join(','),
    );
  }
  return rows.join('\r\n') + '\r\n';
}

/**
 * Guarda el CSV donde elija la persona. En la app lo escribe el motor (`job_history_export`); en el modo
 * simulado (navegador) lo descarga. Devuelve la ruta o el nombre, o `null` si canceló.
 */
export async function saveJobsCsv(csv: string, suggested = JOB_CSV_NAME): Promise<string | null> {
  if (getApiKind() !== 'tauri') return downloadText(suggested, BOM + csv);
  const dialog = await import('@tauri-apps/plugin-dialog');
  const picked = await dialog.save({ filters: [{ name: t('help.jobLog.csv.filter'), extensions: ['csv'] }], defaultPath: suggested, title: t('help.jobLog.csv.saveTitle') });
  if (!picked) return null;
  const path = extensionOf(picked) === 'csv' ? picked : `${picked}.csv`;
  await getApi().jobHistoryExport({ path, text: BOM + csv });
  return path;
}
