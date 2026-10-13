/** Texts of the `variables` area (variable text). Every key starts with `variables.`. */
export const variables = {
  // ---- history and commands --------------------------------------------------
  'variables.history.edit': 'Edit variable text',
  'variables.history.advance': 'Advance variable text serial and row',
  'variables.history.copies': 'One copy per row',
  'variables.cmd.open': 'Variable text…',

  // ---- dialog ------------------------------------------------------------------
  'variables.title': 'Variable text',
  'variables.close': 'Close',
  'variables.done': 'Done',

  'variables.serial.title': 'Serial number',
  'variables.serial.next': 'Next number',
  'variables.serial.step': 'Step',
  'variables.serial.digits': 'Digits',
  'variables.serial.preview': 'It will be engraved as: {value}. With 0 digits no zeros are added.',
  'variables.serial.prev': 'Previous',
  'variables.serial.nextBtn': 'Next',

  'variables.csv.title': 'Data from a CSV',
  'variables.csv.none': 'No CSV file',
  'variables.csv.choose': 'Choose CSV…',
  'variables.csv.reload': 'Reload',
  'variables.csv.clear': 'Remove',
  'variables.csv.pickTitle': 'Choose the CSV file',
  'variables.csv.pickFailed': 'Could not choose the file: {message}',
  'variables.csv.loading': 'Reading the CSV…',
  'variables.csv.error': 'Could not read the CSV. {message}',
  'variables.csv.summary': '{cols} columns, {rows} rows, separator "{sep}"',
  'variables.csv.sepTab': 'tab',
  'variables.csv.end': 'The CSV is finished: there are no rows left. Pick another row or go back to the start to continue.',
  'variables.csv.endNotice': 'The last CSV row was engraved. For the next job go back to the start or change the file.',

  'variables.row.prev': 'Previous row',
  'variables.row.next': 'Next row',
  'variables.row.first': 'To the start',
  'variables.row.goto': 'Go to row (of {total})',

  'variables.advance.title': 'Advance',
  'variables.advance.auto': 'Advance when a job finishes',
  'variables.advance.hint': 'When a job finishes successfully, the serial goes up one step and the CSV moves to the next row. A cancelled or failed job does not advance. Save the project to keep the number.',
  'variables.canvas.template': 'Show the template on the canvas',
  'variables.canvas.templateShort': 'Show template',
  'variables.canvas.templateHint': 'Shows the text with its braces instead of the current values. The values always go out when sending.',

  'variables.table.empty': 'Choose a CSV to see its rows. The highlighted row is the one engraved in the next job.',
  'variables.table.label': 'CSV rows',
  'variables.table.window': 'Showing rows {from} to {to} of {total}.',

  'variables.texts.title': 'Texts with variables in the design',
  'variables.texts.none': 'No text uses variables yet. Type, for example, Part {example} in a text.',

  'variables.copies.title': 'One copy per row',
  'variables.copies.hint': 'Copies the selection in a grid; each copy takes the next CSV row (and the next serial number), so many labels are engraved in a single job.',
  'variables.copies.count': 'Copies',
  'variables.copies.cols': 'Per row',
  'variables.copies.gap': 'Gap',
  'variables.copies.make': 'Make copies',
  'variables.copies.makeTitle': 'Creates the copies in one undo step',
  'variables.copies.noSelection': 'Select the text or shapes of one label first.',
  'variables.copies.few': 'At least 2 copies are needed.',
  'variables.copies.done': '{n} copies created.',

  // ---- text panel ------------------------------------------------------------------
  'variables.panel.open': 'Variable text…',
  'variables.panel.syntaxHint': 'Variables: {examples}. To write a brace, double it: {{ or }}.',

  'variables.insert.label': 'Insert variable',
  'variables.insert.placeholder': 'Choose a variable…',
  'variables.insert.group.serial': 'Serial',
  'variables.insert.group.csv': 'CSV',
  'variables.insert.group.time': 'Date and time',
  'variables.insert.group.other': 'Other',
  'variables.insert.serial': 'Serial number',
  'variables.insert.serial6': 'Serial number, 6 digits',
  'variables.insert.csvGeneric': 'CSV value (type the column)',
  'variables.insert.csvColumn': 'Value: {column}',
  'variables.insert.row': 'CSV row number',
  'variables.insert.date': 'Date',
  'variables.insert.dateLong': 'Long date',
  'variables.insert.time': 'Time',
  'variables.insert.timeSec': 'Time with seconds',
  'variables.insert.copy': 'Copy number',
  'variables.insert.braceOpen': 'Literal opening brace',
  'variables.insert.braceClose': 'Literal closing brace',

  // ---- resolution problems -----------------------------------------------------------
  'variables.issue.unknown': '"{raw}" is not a known variable (use serial, csv, row, date, time or copy; for a literal brace type {{ or }}).',
  'variables.issue.unclosed': 'The closing brace is missing in "{raw}".',
  'variables.issue.empty': 'There are empty braces: "{raw}".',
  'variables.issue.noCsv': '"{raw}" needs a CSV and the design has none that can be read.',
  'variables.issue.noColumn': 'The CSV has no column "{detail}" (in "{raw}").',
  'variables.issue.noColumnArg': '"{raw}" needs the column name, for example {csv:Name}.',
  'variables.issue.rowEnd': 'Row {detail} does not exist in the CSV (in "{raw}"). The data ran out.',
  'variables.issue.badFormat': 'The serial format "{detail}" is not understood (use a number of digits, hex or HEX).',

  // ---- when sending ------------------------------------------------------------------
  'variables.job.csvFailed': 'Could not read the variable text CSV. {message}',
  'variables.job.blocked': 'The job did not go out because some variable text could not be resolved. {problems}',
  'variables.job.outlineFailed': 'Could not generate the text "{text}". {message}',
} as const;
