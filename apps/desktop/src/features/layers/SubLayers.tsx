import { ArrowDown, ArrowUp, ChevronDown, ChevronRight, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';
import type { LayerMode, LayerSettings } from '@/api/types';
import { t } from '@/i18n';
import { MAX_SUB_LAYERS } from '@/lib/layers';
import { useStore } from '@/store';
import { addSubLayer, moveSubLayer, removeSubLayer } from './actions';
import { LayerFields, ModePicker } from './LayerFields';
import { Group, SelectField } from './ParamFields';
import { useLayerSetters } from './targets';

const MODES: LayerMode[] = ['line', 'fill', 'image'];

/** Resumen corto de una operación adicional para su encabezado. */
function subSummary(l: LayerSettings): string {
  const parts = [`${l.speed} mm/min`, `${l.power} %`];
  if (l.mode !== 'line') parts.push(`${l.interval} mm`);
  if (l.passes > 1) parts.push(`×${l.passes}`);
  return parts.join(' · ');
}

/**
 * Lista de operaciones adicionales de una capa (sub-capas): se ejecutan sobre los mismos objetos,
 * en orden, después de la principal. Cada una se edita con los mismos campos que la capa.
 */
export function SubLayers({ layer }: { layer: LayerSettings }) {
  const subs = layer.sub_layers;
  const [mode, setMode] = useState<LayerMode>('line');
  const [open, setOpen] = useState<number | null>(null);
  const full = subs.length >= MAX_SUB_LAYERS;
  const st = () => useStore.getState();

  return (
    <Group title={t('layers.adv.sub')}>
      <p className="lyr-help">{t('layers.sub.help')}</p>
      {subs.length === 0 ? (
        <p className="lyr-live">{t('layers.sub.empty')}</p>
      ) : (
        <ol className="lyr-sub-list" aria-label={t('layers.adv.sub')}>
          {subs.map((sub, i) => (
            <SubItem
              key={i}
              layer={layer}
              sub={sub}
              index={i}
              count={subs.length}
              open={open === i}
              onToggle={() => setOpen(open === i ? null : i)}
              onMove={(delta) => {
                moveSubLayer(st(), layer.color, i, i + delta);
                if (open === i) setOpen(i + delta);
                else if (open === i + delta) setOpen(i);
              }}
              onDelete={() => {
                removeSubLayer(st(), layer.color, i);
                setOpen(null);
              }}
            />
          ))}
        </ol>
      )}
      <div className="lyr-sub-add">
        <SelectField<LayerMode>
          label={t('layers.sub.addMode')}
          value={mode}
          options={MODES.map((m) => ({ value: m, label: t(`layers.mode.${m}`) }))}
          onChange={setMode}
          disabled={full}
        />
        <button
          type="button"
          className="btn"
          disabled={full}
          onClick={() => {
            addSubLayer(st(), layer.color, mode);
            setOpen(subs.length);
          }}
        >
          <Plus size={13} aria-hidden="true" /> {t('layers.sub.add')}
        </button>
        {full && <p className="lyr-live">{t('layers.sub.max', { max: MAX_SUB_LAYERS })}</p>}
      </div>
    </Group>
  );
}

interface ItemProps {
  layer: LayerSettings;
  sub: LayerSettings;
  index: number;
  count: number;
  open: boolean;
  onToggle: () => void;
  onMove: (delta: -1 | 1) => void;
  onDelete: () => void;
}

function SubItem({ layer, sub, index, count, open, onToggle, onMove, onDelete }: ItemProps) {
  const n = index + 1;
  const target = { color: layer.color, sub: index };
  const { set } = useLayerSetters(target);
  const Chevron = open ? ChevronDown : ChevronRight;
  return (
    <li className="lyr-sub" data-open={open}>
      <div className="lyr-sub__head">
        <button type="button" className="lyr-sub__toggle" aria-expanded={open} aria-label={t('layers.sub.edit', { n })} onClick={onToggle}>
          <Chevron size={14} aria-hidden="true" />
          <span className="lyr-sub__title">{t('layers.sub.item', { n, mode: t(`layers.mode.${sub.mode}`) })}</span>
          <span className="lyr-sub__meta num">{subSummary(sub)}</span>
        </button>
        <button type="button" className="icon-btn" disabled={index === 0} aria-label={t('layers.sub.up', { n })} title={t('layers.sub.up', { n })} onClick={() => onMove(-1)}>
          <ArrowUp size={14} aria-hidden="true" />
        </button>
        <button
          type="button"
          className="icon-btn"
          disabled={index === count - 1}
          aria-label={t('layers.sub.down', { n })}
          title={t('layers.sub.down', { n })}
          onClick={() => onMove(1)}
        >
          <ArrowDown size={14} aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn" aria-label={t('layers.sub.delete', { n })} title={t('layers.sub.delete', { n })} onClick={onDelete}>
          <Trash2 size={14} aria-hidden="true" />
        </button>
      </div>
      {open && (
        <div className="lyr-sub__body">
          <ModePicker mode={sub.mode} onChange={(m) => set({ mode: m })} />
          <LayerFields layer={sub} target={target} beginner={false} />
        </div>
      )}
    </li>
  );
}
