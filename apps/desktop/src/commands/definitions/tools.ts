import { TOOL_IDS, type ToolId } from '@/store/selection';
import { cancelGesture, closePolyline, polylineInProgress } from '@/features/canvas/interaction';
import { NODE_OPS, nodeOpAvailability, runNodeOp, type NodeOp } from '@/features/canvas/nodeActions';
import type { I18nKey } from '@/i18n';
import type { CommandDef } from '../types';

/**
 * Una tecla por herramienta. Funcionan en toda la ventana salvo al escribir en un campo
 * (no dependen de qué panel tiene el foco). Escape vuelve a Seleccionar.
 */
const KEYS: Record<ToolId, string[]> = {
  select: ['V', 'Escape'],
  nodes: ['N'],
  line: ['L'],
  rect: ['R'],
  ellipse: ['E'],
  polygon: ['G'],
  star: ['Shift+G'],
  text: ['T'],
  pan: ['H'],
  measure: ['M'],
};

/**
 * Cómo se usa cada herramienta (lo que hace con ratón y modificadores). Va aquí, junto al atajo,
 * para que quien lea el comando sepa qué hace:
 *
 *  - V / Esc `tool.select`: clic selecciona; Mayús+clic suma o quita; arrastrar sobre vacío dibuja
 *    una caja (izquierda a derecha = solo lo que queda dentro, continua; derecha a izquierda = lo
 *    que toque, discontinua); arrastrar un objeto lo mueve (Mayús = solo en horizontal o vertical,
 *    Alt = sin ajuste magnético); un segundo clic sobre la selección alterna escalar / girar.
 *    Las manijas escalan (esquinas proporcional, Mayús libre; Alt desde el centro) y la perilla
 *    superior gira (Mayús = pasos de 15°; Alt = alrededor del punto de referencia).
 *    Esc cancela el arrastre en curso; sin arrastre quita la selección.
 *  - L `tool.line`: clic a clic (segmentos rectos). Arrastrar al poner un nodo saca sus manijas y
 *    el tramo siguiente sale curvo (Bézier); Mayús fija ángulos de 45° (también en las manijas).
 *    Un clic sobre el primer nodo cierra la forma. Doble clic, Intro o clic derecho la dejan
 *    abierta; `C` la cierra; Retroceso quita el último nodo. El cursor se pega a nodos, puntos
 *    medios, centros e intersecciones (Alt lo suspende).
 *  - R `tool.rect` / E `tool.ellipse`: arrastrar; Mayús = cuadrado / círculo. El radio de esquinas
 *    del rectángulo está en la barra numérica.
 *  - G `tool.polygon` / Mayús+G `tool.star`: arrastrar desde el centro; el radio y el giro los marca
 *    el puntero (Mayús = hacia arriba). Lados, puntas, radio interno y redondeo están en la barra
 *    numérica y, ya dibujada la forma, en su ventana de propiedades.
 *  - M `tool.measure`: arrastrar entre dos puntos muestra distancia, ángulo y Δx/Δy (Mayús =
 *    0/45/90°); un clic sobre una forma muestra sus nodos, perímetro y área.
 *  - N `tool.nodes`: muestra los nodos y manejadores del objeto vectorial seleccionado. Clic elige
 *    un nodo (Mayús+clic suma o quita; clic en un segmento elige sus dos nodos; arrastrar sobre
 *    vacío dibuja una caja de nodos). Arrastrar mueve los nodos elegidos (Mayús = horizontal o
 *    vertical, Alt = sin ajuste magnético) y arrastrar un manejador lo mueve (Alt = rompe la
 *    simetría, Mayús = ángulos de 45°). Doble clic en un segmento agrega un nodo sin cambiar la
 *    forma. Las flechas mueven los nodos elegidos (Mayús 10 mm, Alt 0,1 mm) y Supr o Retroceso los
 *    borran; la barra de nodos fija X e Y exactas. Con la herramienta de nodos activa, Supr y las
 *    flechas actúan sobre los nodos elegidos en lugar de sobre el objeto; el resto de teclas de
 *    nodos solo existen aquí y cada una hace una sola cosa. Esc sale a Seleccionar. Deshacer y
 *    rehacer funcionan siempre, con nodos elegidos o sin ellos.
 *  - T `tool.text`: clic en vacío crea un texto ahí; clic en un texto lo selecciona. El doble clic
 *    sobre un texto con Seleccionar también pasa a editarlo.
 */

/**
 * Teclas de las operaciones de nodos (solo valen con la herramienta de nodos y nodos elegidos; si
 * no, el comando está deshabilitado y la tecla no hace nada). Supr, Retroceso y las flechas las
 * resuelven `edit.delete` y `edit.nudge*` (definitions/edit.ts), que ramifican a los nodos con esta herramienta.
 */
const NODE_KEYS: Record<NodeOp, string[]> = {
  insert: ['I'],
  delete: [],
  toLine: ['Shift+L'],
  toCurve: ['Shift+U'],
  corner: ['Shift+E'],
  smooth: ['Shift+S'],
  symmetric: ['Shift+M'],
  break: ['B'],
  join: ['J'],
  close: ['O'],
};

const nodeCommands: CommandDef[] = NODE_OPS.map<CommandDef>((op) => ({
  id: `nodes.${op}`,
  label: `nodes.cmd.${op}` as I18nKey,
  category: 'tools',
  shortcuts: NODE_KEYS[op],
  enabled: (s) => nodeOpAvailability(s, op),
  run: (s) => void runNodeOp(s, op),
}));
export const toolCommands: CommandDef[] = [
  ...TOOL_IDS.map<CommandDef>((tool) => ({
    id: `tool.${tool}`,
    label: `cmd.tool.${tool}` as I18nKey,
    category: 'tools',
    shortcuts: KEYS[tool],
    checked: (s) => s.activeTool === tool,
    run: (s) => {
      // Escape primero cancela lo que se esté arrastrando o dibujando; solo si no hay nada en
      // curso quita la selección (y vuelve a Seleccionar) como antes.
      if (tool === 'select' && cancelGesture()) return;
      // Esc también apaga el modo "Posicionar el láser" (sin tocar la selección).
      if (tool === 'select' && s.positioningLaser) return s.setPositioningLaser(false);
      if (tool === 'select' && s.activeTool === 'select') s.clearSelection();
      s.setTool(tool);
    },
  })),
  ...nodeCommands,
  {
    // Ventana de propiedades de forma (parámetros de rectángulos, elipses, polígonos y estrellas).
    id: 'panel.shapeProps',
    label: 'cmd.panel.shapeProps',
    category: 'panels',
    checked: (s) => s.shapePropsOpen,
    run: (s) => s.setShapePropsOpen(!s.shapePropsOpen),
  },
  {
    // Ver: números pequeños sobre el lienzo con la prioridad de corte de cada objeto que la tiene.
    id: 'view.priorityBadges',
    label: 'canvas.cmd.priorityBadges',
    category: 'view',
    checked: (s) => s.priorityBadges,
    run: (s) => s.setPriorityBadges(!s.priorityBadges),
  },
  {
    // C: cierra la polilínea en curso uniendo el último punto con el primero. Intro la deja abierta.
    id: 'draw.closePath',
    label: 'canvas.cmd.closePath',
    category: 'tools',
    shortcuts: ['C'],
    enabled: () => (polylineInProgress() ? true : 'canvas.reason.noPolyline'),
    run: () => closePolyline(),
  },
];
