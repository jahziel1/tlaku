import type { text as Source } from '../es/text';

/** English for the `text` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const text: { [K in keyof typeof Source]?: string } = {
  // ---- history -----------------------------------------------------------
  'text.history.edit': 'Edit text',
  'text.history.convert': 'Convert text to paths',

  // ---- panel ----------------------------------------------------------------
  'text.panel.label': 'Text properties',
  'text.panel.title': 'Text',
  'text.field.text': 'Content',
  'text.field.textPlaceholder': 'Type here. Enter adds a line.',
  'text.field.font': 'Font',
  'text.field.size': 'Size',
  'text.field.letterSpacing': 'Letter spacing',
  'text.field.lineSpacing': 'Line spacing',
  'text.field.sizeHint': 'Size of the em, not of the capital letter.',
  'text.field.lineSpacingHint': 'In multiples of the size.',

  // ---- style and alignment --------------------------------------------------
  'text.style.bold': 'Bold',
  'text.style.italic': 'Italic',
  'text.align.label': 'Alignment',
  'text.align.left': 'Align left',
  'text.align.center': 'Center',
  'text.align.right': 'Align right',

  // ---- font picker ----------------------------------------------------
  'text.font.search': 'Search fonts…',
  'text.font.loading': 'Loading fonts…',
  'text.font.empty': 'No fonts available.',
  'text.font.noMatch': 'No font matches the search.',
  'text.font.more': '{n} more: type to narrow the search.',
  'text.font.default': '{family} (default)',
  'text.font.styles': 'Styles: {styles}',

  // ---- single-line fonts and SHX -------------------------------------------
  'text.font.sectionSystem': 'System fonts',
  'text.font.sectionLine': 'Single-line fonts',
  'text.font.lineTitle': 'Single-stroke font: it is engraved as a line, not as an outline',
  'text.line.hint': 'Single-line font: the text comes out as open paths. Use a layer in Line mode to engrave them. It has no bold or italic.',
  'text.line.credit': 'Hershey fonts: created by Dr. A. V. Hershey (U. S. National Bureau of Standards); data format by James Hurt (Cognition, Inc.).',
  'text.shx.import': 'Import SHX font…',
  'text.shx.importTitle': 'Add AutoCAD SHX fonts you already have. They are saved for future sessions.',
  'text.shx.pickTitle': 'Choose SHX fonts',
  'text.shx.imported': 'SHX fonts imported: {names}',
  'text.shx.failed': 'The SHX font could not be imported: {message}',
  'text.cmd.importShx': 'Import SHX font…',

  // ---- bending and welding -----------------------------------------------------
  'text.field.bend': 'Bend',
  'text.field.bendHint': 'Degrees the text spans along an arc. Positive arches it upward, negative downward, and 0 leaves it straight.',
  'text.field.bendIgnored': 'Bend is ignored while the text follows a path.',
  'text.field.welded': 'Weld letters',
  'text.field.weldedTitle': 'Merges letters that touch or overlap into a single outline (useful with script fonts)',
  'text.field.weldedLine': 'Does not apply to single-line fonts: they have no outlines to merge.',

  // ---- on path --------------------------------------------------------
  'text.onPath.enable': 'On path',
  'text.onPath.target': 'Guide object',
  'text.onPath.offset': 'Offset',
  'text.onPath.flip': 'Flip',
  'text.onPath.none': 'Draw a line or a curve so you can type along it.',
  'text.onPath.missing': 'The guide object no longer exists or has no path. Choose another one or turn off "On path".',

  // ---- conversion ------------------------------------------------------------
  'text.convert': 'Convert to paths',
  'text.convert.title': 'Replaces the text with editable paths. Afterward the font and content can no longer be changed.',
  'text.convert.reasonBusy': 'Wait for the outline to finish generating.',
  'text.convert.reasonEmpty': 'The text has no outline yet: type something using an installed font.',

  // ---- outline status ----------------------------------------------------
  'text.status.busy': 'Generating outline…',
  'text.status.error': 'The outline could not be generated: {message}',
  'text.warn.title': 'Outline warnings',
  'text.warn.fontFallback': "The font '{family}' is not installed; '{used}' is used instead.",
};
