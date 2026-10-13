import type { nodes as Source } from '../es/nodes';

/** English for the `nodes` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const nodes: { [K in keyof typeof Source]?: string } = {
  // ---- commands --------------------------------------------------------------
  'nodes.cmd.insert': 'Insert node at midpoint',
  'nodes.cmd.delete': 'Delete nodes',
  'nodes.cmd.toLine': 'Segment to line',
  'nodes.cmd.toCurve': 'Segment to curve',
  'nodes.cmd.corner': 'Corner node',
  'nodes.cmd.smooth': 'Smooth node',
  'nodes.cmd.symmetric': 'Symmetric node',
  'nodes.cmd.break': 'Break path at node',
  'nodes.cmd.join': 'Join endpoints',
  'nodes.cmd.close': 'Close path',

  // ---- why it is unavailable ----------------------------------------------------
  'nodes.reason.notTool': 'Choose the Edit Nodes tool (N).',
  'nodes.reason.noNodes': 'Select one or more nodes of the path.',
  'nodes.reason.needSegment': 'Select a segment (click it) or two adjacent nodes.',
  'nodes.reason.cantBreak': 'Select a node that is not the endpoint of an open path.',
  'nodes.reason.needEnds': 'Select exactly two endpoints of open paths belonging to the same object.',
  'nodes.reason.noOpen': 'The selected nodes do not belong to any open path.',

  // ---- undo steps -----------------------------------------------------------
  'nodes.history.move': 'Move nodes',
  'nodes.history.nudge': 'Nudge nodes with the keyboard',
  'nodes.history.coord': 'Set a node\'s position',
  'nodes.history.handle': 'Move handle',
  'nodes.history.insert': 'Insert node',
  'nodes.history.delete': 'Delete nodes',
  'nodes.history.toLine': 'Segment to line',
  'nodes.history.toCurve': 'Segment to curve',
  'nodes.history.type': 'Change node type',
  'nodes.history.break': 'Break path',
  'nodes.history.join': 'Join endpoints',
  'nodes.history.close': 'Close path',

  // ---- node bar -----------------------------------------------------------------
  'nodes.bar.label': 'Node editing',
  'nodes.bar.x': 'X',
  'nodes.bar.y': 'Y',
  'nodes.bar.hint': 'Exact position in the document, in mm. With several nodes it is the center of their bounding box and moves them all. Accepts expressions.',
  'nodes.bar.one': '1 node selected',
  'nodes.bar.many': '{n} nodes selected',
  'nodes.bar.none': 'No nodes selected. Click, Shift+click, or drag a box; double-click a segment to add a node.',
  'nodes.bar.pick': 'Click a path to edit its nodes.',
  'nodes.bar.notVector': 'Text and images have no editable nodes: convert them to paths.',
  'nodes.bar.type': 'Node type',
  'nodes.bar.segment': 'Segment',
  'nodes.bar.path': 'Path',

  // ---- live measurements -------------------------------------------------------------------
  'nodes.tip.point': 'X {x} · Y {y} mm',
};
