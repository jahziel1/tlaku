import type { codes as Source } from '../es/codes';

/** English for the `codes` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const codes: { [K in keyof typeof Source]?: string } = {
  // ---- commands and history ---------------------------------------------------
  'codes.cmd.insert': 'Insert code…',
  'codes.history.insert': 'Insert code',
  'codes.reason.noLayer': 'There is no layer to insert the code into.',

  // ---- dialog ---------------------------------------------------------------
  'codes.title': 'Insert code',
  'codes.close': 'Close',
  'codes.cancel': 'Cancel',
  'codes.insert': 'Insert',
  'codes.insert.busy': 'Wait for the code to finish generating.',
  'codes.insert.error': 'Fix the input to be able to insert the code.',
  'codes.insert.variableIssue': 'The code cannot be shown with the current row data. {problem}',
  'codes.insert.empty': 'Enter the content of the code.',
  'codes.done': 'Code inserted: {w} × {h} mm.',

  'codes.field.kind': 'Code type',
  'codes.field.data': 'Content',
  'codes.field.variableHint': 'It can contain variables, for example https://shop.example/{csv:Id}: the code is generated again when sending, with the data of each row. Loose braces (JSON, for example) are encoded as they are.',
  'codes.field.variablePreview': 'With the current row: {value}',
  'codes.field.insertColumn': 'Insert CSV column',
  'codes.field.dataPlaceholder': 'Text, link, or number',
  'codes.field.ecc': 'Error correction',
  'codes.field.size': 'Size',
  'codes.field.sizeQr': 'Code side length',
  'codes.field.sizeBars': 'Bar width',
  'codes.field.height': 'Bar height',
  'codes.field.margin': 'Margin',
  'codes.field.marginHint': 'Blank zone around the code. It is only drawn when inverted, as the border of the background.',
  'codes.field.readable': 'Human-readable text below',
  'codes.field.invert': 'Invert (engrave the background)',
  'codes.field.invertHint': 'Outputs the background with the code as a cutout. Useful on dark materials; not every reader can scan an inverted code.',
  'codes.field.font': 'Text font',

  // ---- types -----------------------------------------------------------------
  'codes.kind.group.2d': 'Two-dimensional',
  'codes.kind.group.1d': 'Barcodes',
  'codes.kind.qr': 'QR',
  'codes.kind.data_matrix': 'Data Matrix',
  'codes.kind.code128': 'Code 128',
  'codes.kind.code39': 'Code 39',
  'codes.kind.ean13': 'EAN-13',
  'codes.kind.ean8': 'EAN-8',
  'codes.kind.upc_a': 'UPC-A',
  'codes.kind.itf': 'ITF (interleaved 2 of 5)',
  'codes.kind.codabar': 'Codabar',

  'codes.hint.qr': 'Any text or link.',
  'codes.hint.data_matrix': 'Any text; suited to marking small parts.',
  'codes.hint.code128': 'ASCII text: letters, numbers, and symbols.',
  'codes.hint.code39': 'Uppercase letters, numbers, and - . space $ / + %.',
  'codes.hint.ean13': '12 digits, or 13 including the check digit.',
  'codes.hint.ean8': '7 digits, or 8 including the check digit.',
  'codes.hint.upc_a': '11 digits, or 12 including the check digit.',
  'codes.hint.itf': 'Digits only; with an odd count, a check digit is added.',
  'codes.hint.codabar': 'Digits and - $ / : . +; starts and ends with A, B, C, or D.',

  'codes.ecc.l': 'L · 7%',
  'codes.ecc.m': 'M · 15%',
  'codes.ecc.q': 'Q · 25%',
  'codes.ecc.h': 'H · 30%',

  // ---- preview ----------------------------------------------------------
  'codes.preview.label': 'Code preview',
  'codes.preview.computing': 'Generating…',
  'codes.preview.empty': 'Enter the content to see the code.',
  'codes.preview.modules': '{n} modules',
  'codes.preview.size': '{w} × {h} mm',
  'codes.error': 'The code could not be generated: {msg}',
  'codes.warnings': 'Code warnings',
};
