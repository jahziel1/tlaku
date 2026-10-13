import { TOOL_IDS, type ToolId } from '@/store/selection';
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

export const toolCommands: CommandDef[] = TOOL_IDS.map((tool) => ({
  id: `tool.${tool}`,
  label: `cmd.tool.${tool}` as I18nKey,
  category: 'tools',
  shortcuts: KEYS[tool],
  checked: (s) => s.activeTool === tool,
  run: (s) => {
    // Escape en Seleccionar quita la selección: es la salida universal.
    if (tool === 'select' && s.activeTool === 'select') s.clearSelection();
    s.setTool(tool);
  },
}));
