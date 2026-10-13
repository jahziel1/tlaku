import { useShallow } from 'zustand/react/shallow';
import { t } from '@/i18n';
import { LAYER_PALETTE, paletteLabel, readableOn } from '@/lib/palette';
import { useStore } from '@/store';

/**
 * Franja de colores de capa. Con objetos seleccionados, un clic los mueve a esa capa;
 * sin selección, fija la capa de lo que se dibuje después.
 */
export function ColorPalette() {
  const selectedCount = useStore((s) => s.selectedIds.length);
  const activeLayer = useStore((s) => s.activeLayer);
  const used = useStore(useShallow((s) => [...new Set(s.doc.objects.map((o) => o.layer.toLowerCase()))].sort()));
  const setObjectsLayer = useStore((s) => s.setObjectsLayer);
  const setActiveLayer = useStore((s) => s.setActiveLayer);

  return (
    <div className="palette" role="group" aria-label={t('palette.label')}>
      {LAYER_PALETTE.map((entry) => {
        const isActive = activeLayer?.toLowerCase() === entry.color.toLowerCase();
        const inUse = used.includes(entry.color.toLowerCase());
        const label = paletteLabel(entry);
        return (
          <button
            key={entry.color}
            type="button"
            className="palette__swatch"
            data-active={isActive}
            data-used={inUse}
            data-tool={entry.tool ?? false}
            style={{ background: entry.color, color: readableOn(entry.color) }}
            aria-pressed={isActive}
            aria-label={t(entry.tool ? 'palette.toolLayer' : 'palette.layer', { n: label })}
            title={selectedCount > 0 ? t('palette.moveHint', { n: label }) : t('palette.pickHint', { n: label })}
            onClick={() => {
              if (selectedCount > 0) setObjectsLayer(useStore.getState().selectedIds, entry.color);
              setActiveLayer(entry.color);
            }}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
}
