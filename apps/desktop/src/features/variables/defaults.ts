import type { Document, VariableText } from '@/api/types';

/** Ajustes de fábrica (los mismos que `VariableText::default()` de tlaku-doc). */
export const DEFAULT_VARIABLE_TEXT: VariableText = {
  serial: 1,
  serial_step: 1,
  serial_digits: 0,
  csv_path: null,
  csv_row: 0,
  advance_after_job: true,
};

/** Ajustes del documento (o los de fábrica en un documento sin ellos). */
export function variableTextOf(doc: Pick<Document, 'variable_text'>): VariableText {
  return { ...DEFAULT_VARIABLE_TEXT, ...doc.variable_text };
}
