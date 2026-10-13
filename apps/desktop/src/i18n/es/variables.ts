/** Textos del área `variables` (texto variable). Todas las claves empiezan con `variables.`. */
export const variables = {
  // ---- historial y comandos -------------------------------------------------
  'variables.history.edit': 'Editar texto variable',
  'variables.history.advance': 'Avanzar serie y fila del texto variable',
  'variables.history.copies': 'Una copia por fila',
  'variables.cmd.open': 'Texto variable…',

  // ---- diálogo ---------------------------------------------------------------
  'variables.title': 'Texto variable',
  'variables.close': 'Cerrar',
  'variables.done': 'Listo',

  'variables.serial.title': 'Número de serie',
  'variables.serial.next': 'Próximo número',
  'variables.serial.step': 'Paso',
  'variables.serial.digits': 'Dígitos',
  'variables.serial.preview': 'Se grabará como: {value}. Con 0 dígitos no se rellenan ceros.',
  'variables.serial.prev': 'Anterior',
  'variables.serial.nextBtn': 'Siguiente',

  'variables.csv.title': 'Datos de un CSV',
  'variables.csv.none': 'Sin archivo CSV',
  'variables.csv.choose': 'Elegir CSV…',
  'variables.csv.reload': 'Volver a leer',
  'variables.csv.clear': 'Quitar',
  'variables.csv.pickTitle': 'Elige el archivo CSV',
  'variables.csv.pickFailed': 'No se pudo elegir el archivo: {message}',
  'variables.csv.loading': 'Leyendo el CSV…',
  'variables.csv.error': 'No se pudo leer el CSV. {message}',
  'variables.csv.summary': '{cols} columnas, {rows} filas, separador «{sep}»',
  'variables.csv.sepTab': 'tabulador',
  'variables.csv.end': 'Se acabó el CSV: ya no quedan filas. Elige otra fila o vuelve al inicio para seguir.',
  'variables.csv.endNotice': 'Se grabó la última fila del CSV. Para el próximo trabajo vuelve al inicio o cambia de archivo.',

  'variables.row.prev': 'Fila anterior',
  'variables.row.next': 'Fila siguiente',
  'variables.row.first': 'Al inicio',
  'variables.row.goto': 'Ir a la fila (de {total})',

  'variables.advance.title': 'Avance',
  'variables.advance.auto': 'Avanzar solo al terminar un trabajo',
  'variables.advance.hint': 'Al terminar bien un trabajo, la serie sube un paso y el CSV pasa a la fila siguiente. Un trabajo cancelado o con error no avanza. Guarda el proyecto para conservar el número.',
  'variables.canvas.template': 'Ver la plantilla en el lienzo',
  'variables.canvas.templateShort': 'Ver plantilla',
  'variables.canvas.templateHint': 'Muestra el texto con sus llaves en lugar de los valores actuales. Al enviar siempre salen los valores.',

  'variables.table.empty': 'Elige un CSV para ver sus filas. La fila resaltada es la que se graba en el próximo trabajo.',
  'variables.table.label': 'Filas del CSV',
  'variables.table.window': 'Mostrando las filas {from} a {to} de {total}.',

  'variables.texts.title': 'Textos con variables del diseño',
  'variables.texts.none': 'Ningún texto usa variables todavía. Escribe, por ejemplo, Pieza {example} en un texto.',

  'variables.copies.title': 'Una copia por fila',
  'variables.copies.hint': 'Copia la selección en rejilla; cada copia toma la fila siguiente del CSV (y el número de serie siguiente), así se graban muchas etiquetas en un solo trabajo.',
  'variables.copies.count': 'Copias',
  'variables.copies.cols': 'Por fila',
  'variables.copies.gap': 'Espacio',
  'variables.copies.make': 'Crear copias',
  'variables.copies.makeTitle': 'Crea las copias con un paso de deshacer',
  'variables.copies.noSelection': 'Selecciona primero el texto o las formas de una etiqueta.',
  'variables.copies.few': 'Se necesitan al menos 2 copias.',
  'variables.copies.done': 'Se crearon {n} copias.',

  // ---- panel de texto ----------------------------------------------------------
  'variables.panel.open': 'Texto variable…',
  'variables.panel.syntaxHint': 'Variables: {examples}. Para escribir una llave, duplícala: {{ o }}.',

  'variables.insert.label': 'Insertar variable',
  'variables.insert.placeholder': 'Elige una variable…',
  'variables.insert.group.serial': 'Serie',
  'variables.insert.group.csv': 'CSV',
  'variables.insert.group.time': 'Fecha y hora',
  'variables.insert.group.other': 'Otras',
  'variables.insert.serial': 'Número de serie',
  'variables.insert.serial6': 'Número de serie, 6 dígitos',
  'variables.insert.csvGeneric': 'Dato del CSV (escribe la columna)',
  'variables.insert.csvColumn': 'Dato: {column}',
  'variables.insert.row': 'Número de fila del CSV',
  'variables.insert.date': 'Fecha',
  'variables.insert.dateLong': 'Fecha larga',
  'variables.insert.time': 'Hora',
  'variables.insert.timeSec': 'Hora con segundos',
  'variables.insert.copy': 'Número de copia',
  'variables.insert.braceOpen': 'Llave de apertura literal',
  'variables.insert.braceClose': 'Llave de cierre literal',

  // ---- problemas al resolver ----------------------------------------------------
  'variables.issue.unknown': '«{raw}» no es una variable conocida (usa serie, csv, fila, fecha, hora o copia; para una llave literal escribe {{ o }}).',
  'variables.issue.unclosed': 'Falta la llave de cierre en «{raw}».',
  'variables.issue.empty': 'Hay unas llaves vacías: «{raw}».',
  'variables.issue.noCsv': '«{raw}» necesita un CSV y el diseño no tiene uno que se pueda leer.',
  'variables.issue.noColumn': 'El CSV no tiene la columna «{detail}» (en «{raw}»).',
  'variables.issue.noColumnArg': '«{raw}» necesita el nombre de la columna, por ejemplo {csv:Nombre}.',
  'variables.issue.rowEnd': 'La fila {detail} no existe en el CSV (en «{raw}»). Se acabaron los datos.',
  'variables.issue.badFormat': 'El formato «{detail}» de la serie no se entiende (usa un número de dígitos, hex o HEX).',

  // ---- al enviar -----------------------------------------------------------------
  'variables.job.csvFailed': 'No se pudo leer el CSV del texto variable. {message}',
  'variables.job.blocked': 'El trabajo no salió porque hay texto variable sin resolver. {problems}',
  'variables.job.outlineFailed': 'No se pudo generar el texto «{text}». {message}',
} as const;
