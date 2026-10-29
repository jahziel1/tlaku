import { FlipHorizontal2, FlipVertical2, Play, RotateCcw, RotateCw, Star } from 'lucide-react';
import { useEffect, useMemo, type ReactNode } from 'react';
import type { MaterialPreset } from '@/api/types';
import { useCommand } from '@/commands/useCommand';
import { loadMaterials } from '@/features/materials/actions';
import { applyFavoriteToSelection, favoriteApplyReason, favoritePresets, presetLabel } from '@/features/materials/favorites';
import { titleName } from '@/features/materials/labels';
import { useMaterials } from '@/features/materials/state';
import { t } from '@/i18n';
import { usePref, useStore } from '@/store';
import './quickbar.css';

/** Cuántos favoritos caben en la barra; el resto se ve en la pestaña Materiales. */
export const QUICK_FAVORITES = 4;

function IconAction({ id, icon }: { id: string; icon: ReactNode }) {
  const cmd = useCommand(id);
  return (
    <button type="button" className="icon-btn" title={cmd.tooltip} aria-label={cmd.label} aria-disabled={!cmd.enabled} onClick={cmd.enabled ? cmd.run : undefined}>
      {icon}
    </button>
  );
}

function FavoriteChip({ preset }: { preset: MaterialPreset }) {
  // Selector primitivo: la clave del motivo (un objeto nuevo en cada lectura haría repintar sin fin).
  const key = useStore((s) => favoriteApplyReason(s, preset)?.key ?? null);
  const full = presetLabel(preset);
  const reason = key ? t(key, { mode: t(`materials.mode.${preset.settings.mode}`) }) : null;
  const title = t('machine.quick.favTitle', { name: full });
  return (
    <button type="button" className="chip qbar__fav" title={reason ? `${title}. ${reason}` : title} aria-label={title} aria-disabled={reason !== null} onClick={reason ? undefined : () => applyFavoriteToSelection(preset)}>
      <Star size={11} aria-hidden="true" fill="currentColor" />
      <span className="qbar__fav-name">{titleName(preset)}</span>
    </button>
  );
}

/**
 * Barra rápida de la selección (mejora del flujo de producción). Flota abajo y al centro del lienzo, no
 * sobre la pieza: así no tapa lo que se está girando ni obliga a perseguir la selección con el puntero.
 * Solo reúne comandos y acciones que ya existen: girar 90°, voltear, mover a un material favorito
 * (`applyFavoriteToSelection`) e «Iniciar la selección» (`machine.startSelection`, que abre el diálogo de
 * inicio normal con «Solo la selección» marcada). Se oculta en Preferencias › General › Producción.
 */
export function QuickBar() {
  const enabled = usePref('quickBar');
  const count = useStore((s) => s.selectedIds.length);
  const tool = useStore((s) => s.activeTool);
  const ids = usePref('favoriteMaterials');
  const library = useMaterials((s) => s.library);
  const factory = useMaterials((s) => s.factory);
  const setDockTab = useStore((s) => s.setDockTab);
  const start = useCommand('machine.startSelection');
  const shown = enabled && count > 0 && tool === 'select';
  useEffect(() => {
    if (shown) void loadMaterials();
  }, [shown]);
  const favorites = useMemo(() => favoritePresets(library.presets, factory.presets, ids), [library.presets, factory.presets, ids]);
  if (!shown) return null;
  const visible = favorites.slice(0, QUICK_FAVORITES);
  return (
    <div className="qbar" role="toolbar" aria-label={t('machine.quick.group')} data-testid="quick-bar">
      <IconAction id="arrange.rotate.ccw90" icon={<RotateCcw size={15} aria-hidden="true" />} />
      <IconAction id="arrange.rotate.cw90" icon={<RotateCw size={15} aria-hidden="true" />} />
      <IconAction id="arrange.flip.h" icon={<FlipHorizontal2 size={15} aria-hidden="true" />} />
      <IconAction id="arrange.flip.v" icon={<FlipVertical2 size={15} aria-hidden="true" />} />
      <span className="qbar__sep" aria-hidden="true" />
      <div className="qbar__favs" role="group" aria-label={t('machine.quick.layers')}>
        {visible.map((p) => (
          <FavoriteChip key={p.id} preset={p} />
        ))}
        <button
          type="button"
          className="icon-btn"
          title={visible.length === 0 ? `${t('machine.quick.noFavs')}. ${t('machine.quick.noFavs.help')}` : t('machine.quick.openMaterials')}
          aria-label={t('machine.quick.openMaterials')}
          onClick={() => setDockTab('materials')}
        >
          <Star size={14} aria-hidden="true" />
        </button>
      </div>
      <span className="qbar__sep" aria-hidden="true" />
      <button type="button" className="btn btn--accent qbar__start" title={start.enabled ? `${start.tooltip}. ${t('machine.startSelection.help')}` : start.tooltip} aria-disabled={!start.enabled} onClick={start.enabled ? start.run : undefined}>
        <Play size={13} aria-hidden="true" />
        {t('machine.quick.startSelection')}
      </button>
    </div>
  );
}
