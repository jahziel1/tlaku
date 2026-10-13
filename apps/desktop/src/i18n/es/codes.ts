/** Textos del área `codes` (QR y códigos de barras). Todas las claves empiezan con `codes.`. */
export const codes = {
  // ---- comandos e historial ---------------------------------------------------
  'codes.cmd.insert': 'Insertar código…',
  'codes.history.insert': 'Insertar código',
  'codes.reason.noLayer': 'No hay una capa donde insertar el código.',

  // ---- diálogo ---------------------------------------------------------------
  'codes.title': 'Insertar código',
  'codes.close': 'Cerrar',
  'codes.cancel': 'Cancelar',
  'codes.insert': 'Insertar',
  'codes.insert.busy': 'Espera a que termine de generarse el código.',
  'codes.insert.error': 'Corrige los datos para poder insertar el código.',
  'codes.insert.empty': 'Escribe el contenido del código.',
  'codes.done': 'Código insertado: {w} × {h} mm.',

  'codes.field.kind': 'Tipo de código',
  'codes.field.data': 'Contenido',
  'codes.field.dataPlaceholder': 'Texto, enlace o número',
  'codes.field.ecc': 'Corrección de errores',
  'codes.field.size': 'Tamaño',
  'codes.field.sizeQr': 'Lado del código',
  'codes.field.sizeBars': 'Ancho de las barras',
  'codes.field.height': 'Alto de las barras',
  'codes.field.margin': 'Margen',
  'codes.field.marginHint': 'Zona en blanco alrededor. Solo se dibuja al invertir, como borde del fondo.',
  'codes.field.readable': 'Texto legible debajo',
  'codes.field.invert': 'Invertir (grabar el fondo)',
  'codes.field.invertHint': 'Entrega el fondo con el código como hueco. Útil sobre materiales oscuros: no todos los lectores leen un código invertido.',
  'codes.field.font': 'Fuente del texto',

  // ---- tipos -----------------------------------------------------------------
  'codes.kind.group.2d': 'Dos dimensiones',
  'codes.kind.group.1d': 'Códigos de barras',
  'codes.kind.qr': 'QR',
  'codes.kind.data_matrix': 'Data Matrix',
  'codes.kind.code128': 'Code 128',
  'codes.kind.code39': 'Code 39',
  'codes.kind.ean13': 'EAN-13',
  'codes.kind.ean8': 'EAN-8',
  'codes.kind.upc_a': 'UPC-A',
  'codes.kind.itf': 'ITF (2 de 5 intercalado)',
  'codes.kind.codabar': 'Codabar',

  'codes.hint.qr': 'Cualquier texto o enlace.',
  'codes.hint.data_matrix': 'Cualquier texto; sirve para marcar piezas pequeñas.',
  'codes.hint.code128': 'Texto ASCII: letras, números y signos.',
  'codes.hint.code39': 'Mayúsculas, números y - . espacio $ / + %.',
  'codes.hint.ean13': '12 dígitos, o 13 con el dígito de control.',
  'codes.hint.ean8': '7 dígitos, o 8 con el dígito de control.',
  'codes.hint.upc_a': '11 dígitos, o 12 con el dígito de control.',
  'codes.hint.itf': 'Solo dígitos; con cantidad impar se añade un dígito de control.',
  'codes.hint.codabar': 'Dígitos y - $ / : . +; empieza y termina con A, B, C o D.',

  'codes.ecc.l': 'L · 7 %',
  'codes.ecc.m': 'M · 15 %',
  'codes.ecc.q': 'Q · 25 %',
  'codes.ecc.h': 'H · 30 %',

  // ---- vista previa ----------------------------------------------------------
  'codes.preview.label': 'Vista previa del código',
  'codes.preview.computing': 'Generando…',
  'codes.preview.empty': 'Escribe el contenido para ver el código.',
  'codes.preview.modules': '{n} módulos',
  'codes.preview.size': '{w} × {h} mm',
  'codes.error': 'No se pudo generar el código: {msg}',
  'codes.warnings': 'Avisos del código',
} as const;
