import type { trace as Source } from '../es/trace';

/** English for the `trace` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const trace: { [K in keyof typeof Source]?: string } = {
  'trace.cmd.image': 'Trace image…',
  'trace.reason.needImage': 'Select an image to trace it',
  'trace.reason.oneImage': 'Select a single image to trace it',
  'trace.history': 'Trace image',

  'trace.title': 'Trace image',
  'trace.subtitle': '{mm} mm · {px} px',
  'trace.close': 'Close',
  'trace.cancel': 'Cancel',
  'trace.accept': 'Trace',

  'trace.preview.label': 'Preview of the trace over the image',
  'trace.preview.showImage': 'Show the image',
  'trace.preview.zoomIn': 'Zoom in',
  'trace.preview.zoomOut': 'Zoom out',
  'trace.preview.fit': 'Fit to view',
  'trace.preview.hint': 'Scroll to zoom, drag to pan',
  'trace.preview.computing': 'Computing…',

  'trace.stats.shapes': '{n} shapes',
  'trace.stats.nodes': '{n} nodes',
  'trace.stats.none': 'No shapes',
  'trace.rough':
    'The result has many small shapes or too many nodes, and the edges may look rough. Raise "Remove specks" or the smoothing, or try a little blur.',
  'trace.warnings': 'Warnings',
  'trace.error': 'The image could not be traced: {msg}',

  'trace.set.threshold': 'Threshold',
  'trace.set.threshold.help': 'Anything darker than this brightness is traced.',
  'trace.set.smoothing': 'Smoothing',
  'trace.set.smoothing.help': 'Higher: fewer nodes and longer curves.',
  'trace.set.speckle': 'Remove specks',
  'trace.set.speckle.value': '{px} px ≈ {mm} mm',
  'trace.set.speckle.off': 'Unfiltered',
  'trace.set.speckle.help': 'Discards specks and holes smaller than this side length.',
  'trace.set.corner': 'Corners',
  'trace.set.corner.help': 'Turns sharper than this angle are kept as sharp corners.',
  'trace.set.corner.polygonOnly': 'Only used in Curves mode.',
  'trace.set.blur': 'Blur',
  'trace.set.blur.help': 'Smooths noisy edges before tracing.',
  'trace.set.invert': 'Invert (trace the light areas)',
  'trace.set.mode': 'Mode',
  'trace.mode.spline': 'Curves',
  'trace.mode.polygon': 'Polygon',
  'trace.set.reset': 'Reset settings',

  'trace.out.layer': 'It will be added to layer {name}',
  'trace.out.deleteImage': 'Delete the original image',
  'trace.out.deleteLocked': 'The image is locked: it cannot be deleted.',
  'trace.accept.busy': 'Wait for the preview to finish.',
  'trace.accept.error': 'Fix the settings: the preview failed.',
  'trace.accept.empty': 'These settings produce no shapes to add.',
  'trace.done': 'Traced: {shapes} shapes and {nodes} nodes.',
};
