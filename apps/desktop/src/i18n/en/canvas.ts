import type { canvas as Source } from '../es/canvas';

/** English for the `canvas` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const canvas: { [K in keyof typeof Source]?: string } = {
  // ---- commands and reasons -------------------------------------------------
  'canvas.cmd.closePath': 'Close polyline',
  'canvas.cmd.snap': 'Snapping',
  'canvas.cmd.snapGrid': 'Snap to grid',
  'canvas.cmd.snapObjects': 'Snap to objects and bed',
  'canvas.reason.noPolyline': 'There is no polyline in progress. Draw one with the line tool.',
  'canvas.reason.snapOff': 'Snapping is off.',

  // ---- snapping --------------------------------------------------------
  'canvas.snap.group': 'Snapping',

  // ---- numeric edit bar -----------------------------------------
  'canvas.bar.label': 'Numeric editing of the selection',
  'canvas.bar.anchor': 'Reference point (origin of X and Y, scale anchor, and center of rotation)',
  'canvas.bar.anchor.0': 'Top left',
  'canvas.bar.anchor.1': 'Top center',
  'canvas.bar.anchor.2': 'Top right',
  'canvas.bar.anchor.3': 'Middle left',
  'canvas.bar.anchor.4': 'Center',
  'canvas.bar.anchor.5': 'Middle right',
  'canvas.bar.anchor.6': 'Bottom left',
  'canvas.bar.anchor.7': 'Bottom center',
  'canvas.bar.anchor.8': 'Bottom right',
  'canvas.bar.w': 'Width',
  'canvas.bar.h': 'Height',
  'canvas.bar.rot': 'Rotation',
  'canvas.bar.radius': 'Corner radius',
  'canvas.bar.hint': 'Accepts expressions: 100/3, +5 (add), *2 (multiply), 50%, 5in. Enter or Tab applies.',
  'canvas.bar.rotHint': 'Degrees, counterclockwise. Rotates around the reference point.',
  'canvas.bar.radiusHint': 'Corner radius of the rectangles you draw, in mm.',
  'canvas.bar.invalid': 'Invalid expression.',
  'canvas.bar.lock': 'Aspect ratio locked: changing the width or the height scales both.',
  'canvas.bar.unlock': 'Free aspect ratio.',
  'canvas.bar.lockLabel': 'Keep aspect ratio',
  'canvas.bar.rotateImage': 'Images cannot be rotated.',

  // ---- live measurements ----------------------------------------------------
  'canvas.tip.move': 'Δx {dx} · Δy {dy} mm',
  'canvas.tip.scale': '{w} × {h} mm · {pct}%',
  'canvas.tip.rotate': '{deg}°',
  'canvas.tip.size': '{w} × {h} mm',
  'canvas.tip.circle': 'Ø {d} mm',
  'canvas.tip.line': '{len} mm · {deg}°',

  // ---- history (undo step labels) -------------------------
  'canvas.draw.rect': 'Draw rectangle',
  'canvas.draw.ellipse': 'Draw ellipse',
  'canvas.draw.line': 'Draw line',
  'canvas.draw.polyline': 'Draw polyline',
  'canvas.draw.text': 'Create text',

  // ---- snapping: point types ----------------------------------------------
  'canvas.snap.node': 'Node',
  'canvas.snap.mid': 'Midpoint',
  'canvas.snap.center': 'Center',
  'canvas.snap.inter': 'Intersection',

  // ---- curves, polygons, and stars (bar and live measurements) ------------------------
  'canvas.draw.curve': 'Draw curve',
  'canvas.draw.polygon': 'Draw polygon',
  'canvas.draw.star': 'Draw star',
  'canvas.tip.radius': 'Radius {r} mm · Ø {d} mm',
  'canvas.tip.handle': 'Handle {len} mm',
  'canvas.bar.step': 'Rotation step',
  'canvas.bar.stepHint': 'Degrees for quick rotation with Shift+[ and Shift+]. [ and ] rotate 90°; Alt+[ and Alt+] rotate 5°.',
  'canvas.bar.sides': 'Sides',
  'canvas.bar.sidesHint': 'Number of sides of the polygon (3 to 100).',
  'canvas.bar.points': 'Points',
  'canvas.bar.pointsHint': 'Number of points of the star (3 to 100).',
  'canvas.bar.inner': 'Inner radius',
  'canvas.bar.innerHint': 'Inner radius of the star as a percentage of the outer radius (5 to 95%).',
  'canvas.bar.rounding': 'Rounding',
  'canvas.bar.roundingHint': 'Radius used to round the corners, in mm. 0 = sharp corners.',

  // ---- transform switches -------------------------------------------------
  'canvas.xform.group': 'Transform switches',
  'canvas.xform.off.move': 'Move is turned off. Turn it on in Arrange > Allow Move.',
  'canvas.xform.off.scale': 'Scale is turned off. Turn it on in Arrange > Allow Scale.',
  'canvas.xform.off.rotate': 'Rotate is turned off. Turn it on in Arrange > Allow Rotate.',

  // ---- measure ------------------------------------------------------------------------------------------
  'canvas.measure.tip': '{dist} · {deg}° · Δx {dx} · Δy {dy}',
  'canvas.measure.title': 'Measure',
  'canvas.measure.hint': 'Drag between two points to measure, or click a shape to see its details. Shift snaps to 0, 45, or 90°.',
  'canvas.measure.shape': 'Shape',
  'canvas.measure.distance': 'Distance',
  'canvas.measure.angle': 'Angle',
  'canvas.measure.nodes': 'Nodes',
  'canvas.measure.segments': 'Segments',
  'canvas.measure.segmentsValue': '{lines} straight, {curves} curved',
  'canvas.measure.size': 'Size',
  'canvas.measure.perimeter': 'Perimeter',
  'canvas.measure.area': 'Area',
  'canvas.measure.state': 'Outline',
  'canvas.measure.closed': 'Closed',
  'canvas.measure.open': 'Open',
  'canvas.measure.mixed': 'Mixed',
  'canvas.measure.diameter': 'Diameter',
  'canvas.measure.center': 'Center',

  // ---- mirror across a line and rotate/scale by two points ----------------------------------
  'canvas.pick.mirror.help': 'Mirror: click two points (or drag) to draw the mirror line. Esc cancels.',
  'canvas.pick.mirror.a': 'First point of the mirror line',
  'canvas.pick.mirror.b': 'Second point of the mirror line',
  'canvas.pick.mirror.line': 'Line of {len} mm',
  'canvas.pick.twoPoint.help': 'Click the pivot, then drag from a reference point to where it should end up. Shift also scales. Esc cancels.',
  'canvas.pick.twoPoint.pivot': 'Click the pivot',
  'canvas.pick.twoPoint.ref': 'Drag from a reference point',
  'canvas.pick.twoPoint.both': '{deg}° · {pct}%',

  // ---- special selections -----------------------------------------------------------------------
  'canvas.select.none.open': 'There are no open shapes.',
  'canvas.select.none.openFill': 'There are no open shapes on fill layers.',
  'canvas.select.none.layer': 'The layer has no selectable objects.',
  'canvas.select.none.contained': 'There are no shapes inside the selected one.',
  'canvas.select.none.smaller': 'There are no shapes smaller than the selection.',
  'canvas.select.reason.noLayer': 'Choose a layer in the layers panel or select an object.',

  // ---- context menu ----------------------------------------------------------------------------------
  'canvas.ctx.label': 'Canvas context menu',
  'canvas.ctx.editText': 'Edit text',
  'canvas.ctx.textToPaths': 'Convert to paths',
  'canvas.ctx.order': 'Draw order',
  'canvas.ctx.transform': 'Flip and rotate',
  'canvas.ctx.align': 'Align',
  'canvas.ctx.select': 'Select',

  // ---- clipboard between applications ------------------------------------------------------------
  'canvas.history.pasteSvg': 'Paste SVG',
  'canvas.history.shapeProps': 'Edit shape properties',
  'canvas.clip.copied': 'Copied to the system clipboard as SVG.',
  'canvas.clip.copyFailed': 'The system did not allow writing to the clipboard. The selection was copied only within Tlaku.',
  'canvas.clip.skipped': 'The SVG was pasted; {n} elements that are not shapes (text, images, references) were skipped.',

  // ---- shape properties -------------------------------------------------------------------------------
  'canvas.shape.title': 'Shape properties',
  'canvas.shape.close': 'Close shape properties',
  'canvas.shape.rect': 'Rectangle',
  'canvas.shape.ellipse': 'Ellipse',
  'canvas.shape.polygon': 'Polygon',
  'canvas.shape.star': 'Star',
  'canvas.shape.radius': 'Radius',
  'canvas.shape.outer': 'Outer radius',
  'canvas.shape.radiusHint': 'Radius of the four corners, in mm.',
  'canvas.shape.radiusPolyHint': 'Radius of the circle that passes through the vertices, in mm.',
  'canvas.shape.locked': 'Lock',
  'canvas.shape.mixed': 'Some of the selected objects are not editable shapes and will not change.',
  'canvas.shape.note': 'Editing nodes, joining, or trimming converts the shape to a path. The parameters are not saved in the file.',

  // ---- text tool ----------------------------------------------------
  'canvas.text.default': 'Text',
};
