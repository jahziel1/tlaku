import { TOOL_IDS, type ToolId } from '@/store/selection';
import { cancelGesture, closePolyline, polylineInProgress } from '@/features/canvas/interaction';
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
 *  - L `tool.line`: clic a clic (polilínea). Doble clic, Intro o clic derecho terminan; `C` la
 *    cierra; Retroceso quita el último punto; Mayús fija ángulos de 45°. Arrastrar de punto a
 *    punto dibuja una sola línea.
 *  - R `tool.rect` / E `tool.ellipse`: arrastrar; Mayús = cuadrado / círculo. El radio de esquinas
 *    del rectángulo está en la barra numérica.
 */
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
      if (tool === 'select' && s.activeTool === 'select') s.clearSelection();
      s.setTool(tool);
    },
  })),
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
