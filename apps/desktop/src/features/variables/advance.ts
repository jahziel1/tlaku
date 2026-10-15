import type { DesignObject, Document, VariableText } from '@/api/types';
import type { StoreApi } from 'zustand';
import type { AppState } from '@/store';
import { useVariablesUi } from './context';
import { variableTextOf } from './defaults';
import { variablesOf } from './syntax';
import { hasObjectVariables, objectTemplates } from './varObjects';

/** Objetos del documento que usan variables: textos, imágenes variables y códigos variables. */
export function variableTexts(doc: Pick<Document, 'objects'>): DesignObject[] {
  return doc.objects.filter(hasObjectVariables);
}

/**
 * Cuántos pasos consume un trabajo: 1 más el mayor desplazamiento (`@N`) de las variables de
 * serie, y de las de CSV por separado. Con "una copia por fila" (N copias con `@0` a `@N-1`),
 * el trabajo gasta N números y N filas. Mínimo 1.
 */
export function jobSpans(doc: Pick<Document, 'objects'>): { serial: number; row: number } {
  let serial = 1;
  let row = 1;
  for (const o of doc.objects) {
    for (const template of objectTemplates(o)) {
      for (const v of variablesOf(template)) {
        if (v.name === 'serial') serial = Math.max(serial, 1 + v.offset);
        else if (v.name === 'csv' || v.name === 'row') row = Math.max(row, 1 + v.offset);
      }
    }
  }
  return { serial, row };
}

/**
 * Ajustes después de un trabajo: la serie sube `serial_step` por paso y la fila del CSV, una por
 * paso (solo si hay CSV). Con `rowCount` conocido, la fila se detiene en `rowCount` (pasada la
 * última): el siguiente trabajo avisa que se acabó el CSV en vez de repetir la última etiqueta.
 */
export function advanceSettings(
  vt: VariableText,
  spans: { serial: number; row: number } = { serial: 1, row: 1 },
  rowCount: number | null = null,
): VariableText {
  const row = vt.csv_path === null ? vt.csv_row : vt.csv_row + spans.row;
  return {
    ...vt,
    serial: Number(vt.serial) + Number(vt.serial_step) * spans.serial,
    csv_row: rowCount === null ? row : Math.min(row, Math.max(rowCount, vt.csv_row)),
  };
}

/** ¿La fila actual ya pasó la última del CSV? */
export function atCsvEnd(vt: VariableText, rowCount: number | null): boolean {
  return vt.csv_path !== null && rowCount !== null && vt.csv_row >= rowCount;
}

/** Filas del CSV cargado para el documento actual (`null` si no se conoce). */
export function loadedRowCount(vt: VariableText): number | null {
  const { csv } = useVariablesUi.getState();
  return vt.csv_path !== null && csv.path === vt.csv_path && csv.table ? csv.table.rows.length : null;
}

/**
 * Se llama cuando un trabajo termina bien (evento `finished`): si el documento tiene textos
 * variables y el avance automático está encendido, avanza la serie y la fila con un paso de
 * deshacer. Avisa si el CSV se acabó. Devuelve si cambió algo.
 */
export function advanceAfterJob(store: Pick<StoreApi<AppState>, 'getState'>): boolean {
  const s = store.getState();
  const vt = variableTextOf(s.doc);
  if (!vt.advance_after_job || variableTexts(s.doc).length === 0) return false;
  const rows = loadedRowCount(vt);
  const next = advanceSettings(vt, jobSpans(s.doc), rows);
  s.applyDocChange('variables.history.advance', (doc) => ({ ...doc, variable_text: next }));
  if (atCsvEnd(next, rows)) s.notifyKey('warning', 'variables.csv.endNotice');
  return true;
}
