import { t } from '@/i18n';
import type { I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { useFilletTool } from './filletTool';
import { useWarpEdit } from './warpEditStore';

/** Clave de la ayuda de la herramienta activa, o `null` si no tiene (ver `ToolHint`). */
export function toolHintKey(tool: string, warpMode: string | null, revert: boolean): I18nKey | null {
  if (tool === 'warp') return warpMode ? (`canvas.warp.hint.${warpMode}` as I18nKey) : 'canvas.warp.hint.none';
  if (tool === 'fillet') return revert ? 'canvas.fillet.hint.revert' : 'canvas.fillet.hint.round';
  return null;
}

/** Ayuda de la herramienta `tool` (sin estado propio: la usan `ToolHint` y las pruebas). */
export function ToolHintView({ tool, mode, revert }: { tool: string; mode: string | null; revert: boolean }) {
  const key = toolHintKey(tool, mode, revert);
  if (!key) return null;
  return (
    <div className="status__item" role="note" data-testid="tool-hint">
      {t(key)}
    </div>
  );
}

/** Ayuda de la herramienta activa en la barra de estado (hoy Deformar y Redondear esquinas). */
export function ToolHint() {
  const tool = useStore((s) => s.activeTool);
  const mode = useWarpEdit((s) => s.session?.mode ?? null);
  const revert = useFilletTool((s) => s.revert);
  return <ToolHintView tool={tool} mode={mode} revert={revert} />;
}
