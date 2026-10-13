import type { preview as Source } from '../es/preview';

/** English for the `preview` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const preview: { [K in keyof typeof Source]?: string } = {
  // ---- commands and shortcuts ---------------------------------------------
  'preview.cmd.play': 'Play the toolpath',
  'preview.cmd.pause': 'Pause the toolpath',
  'preview.cmd.slower': 'Slower playback',
  'preview.cmd.faster': 'Faster playback',
  'preview.reason.unavailable': 'Open the preview and wait for the job to be calculated.',

  // ---- calculation status -------------------------------------------------
  'preview.updating': 'Updating…',
  'preview.error.title': 'The preview could not be calculated',
  'preview.error.generic': 'The engine did not give a reason.',
  'preview.error.hintBed': 'Move the design inside the bed, or reduce the overscan on fill and image layers.',
  'preview.error.hintEmpty': 'Turn on output for a layer that has visible objects.',
  'preview.error.retry': 'Try again',

  // ---- playback -----------------------------------------------------------
  'preview.play': 'Play',
  'preview.pause': 'Pause',
  'preview.toStart': 'Back to start',
  'preview.toEnd': 'Jump to end',
  'preview.speed': 'Playback speed',
  'preview.speed.value': '{n}x',
  'preview.speed.title': '{n} times the real speed of the job',
  'preview.time': '{now} of {total}',
  'preview.time.approx': 'Approximate time: the engine did not report the time of each move, so it is estimated from distance and speed.',
  'preview.time.approxBadge': 'approx.',
  'preview.layerStart': 'Layer {name} starts',

  // ---- legend and filters -------------------------------------------------
  'preview.legend.title': 'Legend',
  'preview.legend.travel': 'Rapid moves',
  'preview.legend.cut': 'Cut',
  'preview.legend.engrave': 'Engrave',
  'preview.legend.overscan': 'Overscan',
  'preview.legend.hide': 'Hide {name}',
  'preview.legend.show': 'Show {name}',
  'preview.legend.power': 'The more transparent the stroke, the lower the power.',

  // ---- results ------------------------------------------------------------
  'preview.stats.title': 'Results',
  'preview.stats.total': 'Total time',
  'preview.stats.distance': '{dist} in {time}',
  'preview.stats.estimate': 'Estimate based on the acceleration model of the machine profile. Actual time may differ, especially with images.',
  'preview.stats.estimateBadge': 'Estimate',

  // ---- layers -------------------------------------------------------------
  'preview.layers.title': 'By layer',
  'preview.mode.line': 'Line',
  'preview.mode.fill': 'Fill',
  'preview.mode.image': 'Image',
  'preview.layer.share': '{pct} % of total',
  'preview.layer.saved': 'Saves {pct} % of idle travel',
  'preview.layer.expand': 'See how it was planned',
  'preview.layer.collapse': 'Hide the explanation',
  'preview.layer.highlight': 'Highlight on the canvas',
  'preview.layer.unhighlight': 'Remove the highlight',
  'preview.layer.noExplain': 'The planner left no explanations for this layer.',
  'preview.layer.clickToHighlight': 'Click to highlight the layer on the canvas.',
  'preview.layer.noFigures': 'The engine does not yet report where each shape is, so the highlight marks the whole layer.',
  'preview.order.title': 'Layer order',
  'preview.warnings.title': 'Warnings',
};
