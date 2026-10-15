/** Accessibility: texts only for screen readers and the keyboard. Every key starts with `a11y.`. */
export const a11y = {
  // ---- navigation ----
  'a11y.skip.canvas': 'Skip to canvas',

  // ---- floating panels ----
  'a11y.float.handle': 'Floating panel {name}: move or resize',

  // ---- canvas ----
  'a11y.canvas.desc': 'Design canvas: {name} bed, {size}. {objects}. The selection is summarized in an announcement and in the status bar; layers are in the side panel.',
  'a11y.canvas.objects.none': 'The design is empty',
  'a11y.canvas.objects.one': 'The design has 1 object',
  'a11y.canvas.objects.many': 'The design has {n} objects',
  'a11y.sel.none': 'Nothing selected.',
  'a11y.sel.one': '1 object selected: {w} by {h} {unit}, bottom-left corner at X {x}, Y {y}.',
  'a11y.sel.many': '{n} objects selected: {w} by {h} {unit}, bottom-left corner at X {x}, Y {y}.',

  // ---- job ----
  'a11y.job.running': 'Job running.',
  'a11y.job.paused': 'Job paused.',
  'a11y.job.finished': 'Job finished.',
  'a11y.job.cancelled': 'Job cancelled.',
  'a11y.job.error': 'The job stopped because of an error.',
  'a11y.job.progress': 'Job at {pct} %.',

  'a11y.manual.prev': 'Previous page',
  'a11y.manual.next': 'Next page',

  // ---- table names ----
  'a11y.table.marks': 'Camera registration marks',
  'a11y.table.headAlign': 'Points of the head alignment run',
  'a11y.table.printcut': 'Print and cut result',
  'a11y.table.variables': 'Variable data rows',
  'a11y.table.devices': 'Saved machines',
  'a11y.table.kinds': 'Machine types',
  'a11y.table.facts': 'Machine details',
  'a11y.table.log': 'Job history',
} as const;
