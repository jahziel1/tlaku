import { Eye, EyeOff, Image as ImageIcon, PaintBucket, Spline, Zap, ZapOff } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import type { LayerMode, LayerSettings } from '@/api/types';
import { t } from '@/i18n';
import { NumberField } from '@/ui/NumberField';
import { LAYER_PALETTE, paletteLabel, readableOn } from '@/lib/palette';
import { useStore } from '@/store';

const MODE_ICON = { line: Spline, fill: PaintBucket, image: ImageIcon } as const;
const MODES: LayerMode[] = ['line', 'fill', 'image'];

function swatchLabel(color: string): string {
  const entry = LAYER_PALETTE.find((p) => p.color.toLowerCase() === color.toLowerCase());
  return entry ? paletteLabel(entry) : '';
}

function LayerRow({ layer, active, count }: { layer: LayerSettings; active: boolean; count: number }) {
  const updateLayer = useStore((s) => s.updateLayer);
  const selectLayerObjects = useStore((s) => s.selectLayerObjects);
  const setActiveLayer = useStore((s) => s.setActiveLayer);
  const Icon = MODE_ICON[layer.mode];

  return (
    <li className="layer-row" data-active={active} data-hidden={!layer.visible}>
      <button
        type="button"
        className="layer-row__main"
        aria-pressed={active}
        title={t('layers.selectHint')}
        onClick={(e) => {
          setActiveLayer(layer.color);
          selectLayerObjects(layer.color, e.shiftKey);
        }}
      >
        <span className="swatch" style={{ background: layer.color, color: readableOn(layer.color) }} aria-hidden="true">
          {swatchLabel(layer.color)}
        </span>
        <span className="layer-row__text">
          <span className="layer-row__name">
            {layer.name}
            <span className="layer-row__count num" title={t('layers.objects', { n: count })}>{count}</span>
          </span>
          <span className="layer-row__meta num">
            <Icon size={11} aria-hidden="true" />
            {t(`layers.mode.${layer.mode}`)} · {layer.speed} mm/min · {layer.power} %
            {layer.passes > 1 ? ` · ×${layer.passes}` : ''}
          </span>
        </span>
      </button>
      <button
        type="button"
        className="icon-btn"
        aria-pressed={layer.visible}
        aria-label={layer.visible ? t('layers.hide') : t('layers.show')}
        title={layer.visible ? t('layers.hide') : t('layers.show')}
        onClick={() => updateLayer(layer.color, { visible: !layer.visible })}
      >
        {layer.visible ? <Eye size={15} aria-hidden="true" /> : <EyeOff size={15} aria-hidden="true" />}
      </button>
      <button
        type="button"
        className="icon-btn"
        aria-pressed={layer.output}
        aria-label={layer.output ? t('layers.outputOff') : t('layers.outputOn')}
        title={layer.output ? t('layers.outputOff') : t('layers.outputOn')}
        onClick={() => updateLayer(layer.color, { output: !layer.output })}
      >
        {layer.output ? <Zap size={15} aria-hidden="true" /> : <ZapOff size={15} aria-hidden="true" />}
      </button>
    </li>
  );
}

/** Ajustes rápidos de la capa activa. El editor completo (relleno, imagen, kerf...) llega después. */
function LayerEditor({ layer }: { layer: LayerSettings }) {
  const updateLayer = useStore((s) => s.updateLayer);
  return (
    <section className="layer-editor" aria-label={t('layers.editor', { name: layer.name })}>
      <h3 className="panel-title">{layer.name}</h3>
      <div className="seg" role="radiogroup" aria-label={t('layers.mode')}>
        {MODES.map((m) => {
          const Icon = MODE_ICON[m];
          return (
            <button
              key={m}
              type="button"
              role="radio"
              aria-checked={layer.mode === m}
              className="seg__btn"
              onClick={() => updateLayer(layer.color, { mode: m })}
            >
              <Icon size={13} aria-hidden="true" />
              {t(`layers.mode.${m}`)}
            </button>
          );
        })}
      </div>
      <div className="field-grid">
        <NumberField label={t('layers.speed')} unit="mm/min" value={layer.speed} min={1} max={30000} step={50} onCommit={(v) => updateLayer(layer.color, { speed: v })} />
        <NumberField label={t('layers.power')} unit="%" value={layer.power} min={0} max={100} step={1} onCommit={(v) => updateLayer(layer.color, { power: v })} />
        <NumberField label={t('layers.passes')} value={layer.passes} min={1} max={99} onCommit={(v) => updateLayer(layer.color, { passes: Math.round(v) })} />
        {layer.mode !== 'line' && (
          <NumberField label={t('layers.interval')} unit="mm" value={layer.interval} min={0.01} max={5} step={0.01} onCommit={(v) => updateLayer(layer.color, { interval: v })} />
        )}
      </div>
    </section>
  );
}

export function LayersPanel() {
  const layers = useStore((s) => s.doc.layers);
  const activeLayer = useStore((s) => s.activeLayer);
  const counts = useStore(
    useShallow((s) => s.doc.layers.map((l) => s.doc.objects.filter((o) => o.layer === l.color).length)),
  );
  const active = layers.find((l) => l.color === activeLayer);

  return (
    <div className="panel-scroll">
      {layers.length === 0 ? (
        <p className="panel-empty">{t('layers.empty')}</p>
      ) : (
        <ul className="layer-list">
          {layers.map((layer, i) => (
            <LayerRow key={layer.color} layer={layer} active={layer.color === activeLayer} count={counts[i] ?? 0} />
          ))}
        </ul>
      )}
      {active && <LayerEditor layer={active} />}
    </div>
  );
}
