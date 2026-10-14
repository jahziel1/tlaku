import { designReason } from '@/features/arrange/designOps';
import { setFilletRevert, useFilletTool } from '@/features/canvas/filletTool';
import { WARP_MODES, type WarpMode } from '@/features/canvas/warpEdit';
import { applyWarp, canApplyWarp, resetWarp, setWarpMode, useWarpEdit } from '@/features/canvas/warpEditStore';
import type { I18nKey } from '@/i18n';
import type { CommandDef } from '../types';

/**
 * Comandos de las herramientas del lienzo Deformar (Mayús+W) y Redondear esquinas con clic (Mayús+F).
 * El atajo de cada herramienta está en `tools.ts`; aquí van sus acciones: aplicar y restablecer la
 * malla, cambiar de modo y alternar redondear / revertir. Intro aplica la deformación, pero no es un
 * atajo del registro (atraparía Intro en cualquier botón): lo atiende `useCanvasTools`.
 */

const MODE_LABEL: Record<WarpMode, I18nKey> = {
  bilinear: 'canvas.cmd.warpBilinear',
  perspective: 'canvas.cmd.warpPerspective',
  mesh: 'canvas.cmd.warpMesh',
};

const warpActive = (s: { activeTool: string }): true | I18nKey => (s.activeTool === 'warp' ? true : 'canvas.reason.noWarp');

export const canvasToolCommands: CommandDef[] = [
  {
    id: 'warp.apply',
    label: 'canvas.cmd.warpApply',
    category: 'tools',
    enabled: (s) => {
      const active = warpActive(s);
      if (active !== true) return active;
      const reason = designReason(s);
      if (reason !== true) return reason;
      return canApplyWarp() ? true : 'canvas.reason.noWarpChange';
    },
    run: () => void applyWarp(),
  },
  {
    id: 'warp.reset',
    label: 'canvas.cmd.warpReset',
    category: 'tools',
    enabled: warpActive,
    run: () => resetWarp(),
  },
  ...WARP_MODES.map<CommandDef>((mode) => ({
    id: `warp.mode.${mode}`,
    label: MODE_LABEL[mode],
    category: 'tools',
    enabled: warpActive,
    checked: () => useWarpEdit.getState().session?.mode === mode,
    run: () => setWarpMode(mode),
  })),
  {
    id: 'fillet.revert',
    label: 'canvas.cmd.filletRevert',
    category: 'tools',
    enabled: (s) => (s.activeTool === 'fillet' ? true : 'canvas.reason.noFillet'),
    checked: () => useFilletTool.getState().revert,
    run: () => setFilletRevert(!useFilletTool.getState().revert),
  },
];
