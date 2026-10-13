import { Check, SquareDashed } from 'lucide-react';
import { useEffect, useRef, type KeyboardEvent } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { t } from '@/i18n';
import { isMac } from '@/lib/platform';
import { formatCombo } from '@/commands/shortcuts';
import { GUIDE_ENTRY, LAYER_PALETTE, colorName, isGuideColor, paletteLabel, readableOn, type PaletteEntry } from '@/lib/palette';
import { useStore } from '@/store';
import { pickHint, pickPaletteEntry } from './actions';

/** La capa de guías nunca sale a la máquina: si llega con salida (archivo o importación), se apaga. */
function useGuideGuard(): void {
  const output = useStore((s) => s.doc.layers.find((l) => isGuideColor(l.color))?.output ?? false);
  useEffect(() => {
    if (output) useStore.getState().updateLayer(GUIDE_ENTRY.color, { output: false });
  }, [output]);
}

/**
 * Franja de colores de capa. Las muestras se identifican por nombre (en el tooltip y para el
 * lector de pantalla), no por número. Con objetos seleccionados, elegir una los mueve a esa
 * capa; sin selección, la vuelve la capa activa. La última muestra, punteada, son las guías.
 */
export function ColorPalette() {
  const selectedCount = useStore((s) => s.selectedIds.length);
  const activeLayer = useStore((s) => s.activeLayer);
  const used = useStore(useShallow((s) => [...new Set(s.doc.objects.map((o) => o.layer.toLowerCase()))]));
  const groupRef = useRef<HTMLDivElement>(null);
  const mac = isMac();
  useGuideGuard();

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const keys = ['ArrowLeft', 'ArrowRight', 'Home', 'End'];
    if (!keys.includes(e.key)) return;
    const items = [...(groupRef.current?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0) return;
    e.preventDefault();
    const next = e.key === 'Home' ? 0 : e.key === 'End' ? items.length - 1 : (i + (e.key === 'ArrowRight' ? 1 : -1) + items.length) % items.length;
    items[next]?.focus();
  };

  const active = activeLayer?.toLowerCase() ?? null;
  // Con roving tabindex solo una muestra entra en el orden de tabulación: la activa o la primera.
  const stop = LAYER_PALETTE.some((p) => p.color === active) ? active : LAYER_PALETTE[0]!.color;

  return (
    <div className="cpal">
      <div ref={groupRef} className="cpal__row" role="group" aria-label={t('layers.palette.label')} onKeyDown={onKeyDown}>
        {LAYER_PALETTE.map((entry) => (
          <Swatch
            key={entry.id}
            entry={entry}
            active={active === entry.color}
            inUse={used.includes(entry.color)}
            selectedCount={selectedCount}
            tabStop={stop === entry.color}
            shortcut={entry.shortcut ? formatCombo(entry.shortcut, mac) : ''}
          />
        ))}
      </div>
      <p className="cpal__readout" aria-live="polite">
        {selectedCount > 0
          ? t('layers.palette.readoutSelection', { n: selectedCount })
          : activeLayer
            ? t('layers.palette.readoutActive', { name: colorName(activeLayer) })
            : t('layers.palette.readoutNone')}
      </p>
    </div>
  );
}

function Swatch(props: {
  entry: PaletteEntry;
  active: boolean;
  inUse: boolean;
  selectedCount: number;
  tabStop: boolean;
  shortcut: string;
}) {
  const { entry, active, inUse, selectedCount, tabStop, shortcut } = props;
  const name = paletteLabel(entry);
  const ink = readableOn(entry.color);
  const hint = pickHint(entry, selectedCount);
  const tip = [hint, entry.guide ? t('layers.palette.guideHint') : '', shortcut ? `(${shortcut})` : ''].filter(Boolean).join(' ');
  return (
    <button
      type="button"
      className="cpal__swatch"
      data-active={active}
      data-used={inUse}
      data-guide={entry.guide ?? false}
      style={{ '--sw': entry.color, '--ink': ink } as React.CSSProperties}
      aria-pressed={active}
      aria-label={entry.guide ? t('layers.palette.guideLabel') : name}
      title={`${name}. ${tip}`}
      tabIndex={tabStop ? 0 : -1}
      onClick={() => pickPaletteEntry(useStore.getState(), entry)}
    >
      {entry.guide && <SquareDashed size={14} aria-hidden="true" />}
      {active && !entry.guide && <Check size={14} strokeWidth={3} aria-hidden="true" />}
      {inUse && <span className="cpal__dot" aria-hidden="true" />}
    </button>
  );
}
