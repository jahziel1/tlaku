import { ArrowLeft } from 'lucide-react';
import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import type { LayerSettings } from '@/api/types';
import { t } from '@/i18n';
import { normalizeLayer } from '@/lib/layers';
import { colorName, isGuideColor, readableOn } from '@/lib/palette';
import { usePref, useStore } from '@/store';
import { selectActiveDevice } from '@/store/selectors';
import { currentDefaultsLevel, hasColorDefaults, removeColorDefaults, resetLayerDefaults, saveColorDefaults, saveLayerAsDefaults } from './defaults';
import { LayerFields, ModePicker } from './LayerFields';
import { NumField, Group, Toggle } from './ParamFields';
import { focusLayerRow, useLayerUi } from './layerUi';
import { orderConflict } from './order';
import { SubLayers } from './SubLayers';
import { useLayerSetters } from './targets';

/**
 * Editor de ajustes de una capa. Muestra solo los campos que el modo usa (los demás se
 * conservan en el documento). Cada campo tiene su paso de deshacer; los valores inválidos se
 * marcan y no se aplican. Esc cierra el editor.
 */
export function LayerEditor({ layer: rawLayer }: { layer: LayerSettings }) {
  // Una capa guardada antes de la Fase 4 puede venir sin los campos nuevos.
  const layer = normalizeLayer(rawLayer);
  const color = layer.color;
  const guide = isGuideColor(color);
  const headRef = useRef<HTMLHeadingElement>(null);
  const showHelp = useLayerUi((s) => s.showHelp);
  const layerCount = useStore((s) => s.doc.layers.length);
  const allLayers = useStore((s) => s.doc.layers);
  const { set } = useLayerSetters({ color, sub: null });

  useEffect(() => headRef.current?.focus(), []);

  const close = () => {
    useLayerUi.getState().closeEditor();
    focusLayerRow(color);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  };

  const name = layer.name || colorName(color);
  const pinned = layer.order !== null;

  return (
    <section className="lyr-editor panel-scroll" aria-label={t('layers.editor', { name })} onKeyDown={onKeyDown}>
      <header className="lyr-editor__head">
        <button type="button" className="icon-btn icon-btn--boxed" onClick={close} aria-label={t('layers.editorBack')} title={t('layers.editorBack')}>
          <ArrowLeft size={15} aria-hidden="true" />
        </button>
        <span className="lyr-swatch" data-guide={guide} style={{ '--sw': color, '--ink': readableOn(color) } as React.CSSProperties} aria-hidden="true" />
        <h3 ref={headRef} tabIndex={-1} className="lyr-editor__title">
          {name}
        </h3>
        <button
          type="button"
          className="btn"
          aria-pressed={showHelp}
          title={t('layers.helpToggleHint')}
          onClick={() => useLayerUi.getState().setShowHelp(!showHelp)}
        >
          {t('layers.helpToggle')}
        </button>
      </header>

      <Group title={t('layers.group.general')}>
        <NameField value={layer.name} fallback={colorName(color)} onApply={(v) => set({ name: v })} />
        {guide ? (
          <p className="lyr-info">{t('layers.guideInfo')}</p>
        ) : (
          <ModePicker mode={layer.mode} onChange={(mode) => set({ mode })} />
        )}
        <Toggle label={t('layers.visible')} checked={layer.visible} onChange={(v) => set({ visible: v })} help={t('layers.help.visible')} />
        <Toggle
          label={t('layers.output')}
          checked={layer.output && !guide}
          disabled={guide}
          reason={t('layers.outputGuide')}
          onChange={(v) => set({ output: v })}
          help={t('layers.help.output')}
        />
      </Group>

      {!guide && (
        <>
          <LayerFields layer={layer} target={{ color, sub: null }} />

          <SubLayers layer={layer} />

          <Group title={t('layers.group.order')}>
            <Toggle
              label={t('layers.orderPin')}
              checked={pinned}
              onChange={(v) => set({ order: v ? firstFreePosition(allLayers, color) : null })}
              help={t('layers.help.order')}
            />
            {pinned && (
              <NumField
                field="order"
                label={t('layers.orderPosition')}
                value={layer.order ?? 1}
                ctx={{ layerCount }}
                onApply={(v) => set({ order: v })}
                extra={(v) => {
                  const other = orderConflict(allLayers, color, v);
                  return other ? { key: 'layers.err.orderTaken', params: { name: other.name || colorName(other.color) } } : null;
                }}
              />
            )}
          </Group>

          <DefaultsGroup color={color} />
        </>
      )}
    </section>
  );
}

/**
 * Guardar los ajustes de esta capa como valores por defecto de las capas nuevas (C35): para todas, para
 * este color en cualquier máquina o para este color en la máquina activa; o volver a los de fábrica.
 */
function DefaultsGroup({ color }: { color: string }) {
  const custom = usePref('layerDefaults') !== null;
  const showHelp = useLayerUi((s) => s.showHelp);
  const note = useId();
  const prefs = useStore((s) => s.prefs);
  const activeDeviceId = useStore((s) => s.activeDeviceId);
  const devices = useStore((s) => s.devices);
  const device = selectActiveDevice({ devices, activeDeviceId }).name;
  const state = { prefs, activeDeviceId };
  const level = currentDefaultsLevel(state, color);
  const hasMachine = hasColorDefaults(state, color, 'machine');
  const hasColor = hasColorDefaults(state, color, 'color');
  const noDevice = state.activeDeviceId === null;
  return (
    <Group title={t('layers.group.defaults')}>
      <p id={note} className="lyr-live">
        {t(`layers.defaults.level.${level}`, { device: device })}
      </p>
      <div className="lyr-actions">
        <button type="button" className="btn" aria-describedby={note} onClick={() => saveLayerAsDefaults(useStore.getState(), color)}>
          {t('layers.defaults.save')}
        </button>
        <button type="button" className="btn" disabled={!custom} title={custom ? undefined : t('layers.reason.noCustomDefaults')} onClick={() => resetLayerDefaults(useStore.getState())}>
          {t('layers.defaults.reset')}
        </button>
      </div>
      <div className="lyr-actions">
        <button type="button" className="btn" disabled={noDevice} title={noDevice ? t('layers.reason.noDevice') : undefined} onClick={() => saveColorDefaults(useStore.getState(), color, 'machine')}>
          {t('layers.defaults.saveMachine', { device: device })}
        </button>
        <button type="button" className="btn" onClick={() => saveColorDefaults(useStore.getState(), color, 'color')}>
          {t('layers.defaults.saveColor')}
        </button>
      </div>
      {(hasMachine || hasColor) && (
        <div className="lyr-actions">
          {hasMachine && (
            <button type="button" className="btn" onClick={() => removeColorDefaults(useStore.getState(), color, 'machine')}>
              {t('layers.defaults.removeMachine', { device: device })}
            </button>
          )}
          {hasColor && (
            <button type="button" className="btn" onClick={() => removeColorDefaults(useStore.getState(), color, 'color')}>
              {t('layers.defaults.removeColor')}
            </button>
          )}
        </div>
      )}
      {!custom && level === 'factory' && <p className="lyr-live">{t('layers.reason.noCustomDefaults')}</p>}
      {showHelp && <p className="lyr-help">{t('layers.defaults.help')}</p>}
    </Group>
  );
}

/** Nombre de la capa: no puede quedar vacío. */
function NameField({ value, fallback, onApply }: { value: string; fallback: string; onApply: (v: string) => void }) {
  const id = useId();
  const [text, setText] = useState(value || fallback);
  useEffect(() => setText(value || fallback), [value, fallback]);
  const empty = text.trim() === '';
  return (
    <div className="lyr-field" data-invalid={empty}>
      <div className="lyr-field__top">
        <label htmlFor={id} className="lyr-field__label">
          {t('layers.name')}
        </label>
        <input
          id={id}
          className="input lyr-field__input lyr-name-input"
          value={text}
          maxLength={60}
          aria-invalid={empty}
          onChange={(e) => setText(e.target.value)}
          onBlur={() => !empty && text.trim() !== value && onApply(text.trim())}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !empty) onApply(text.trim());
            else if (e.key === 'Escape' && text !== (value || fallback)) {
              e.stopPropagation();
              setText(value || fallback);
            }
          }}
        />
      </div>
      {empty && (
        <p className="lyr-err" role="alert">
          {t('layers.err.nameEmpty')}
        </p>
      )}
    </div>
  );
}

/** Primera posición de ejecución que ninguna otra capa tiene fijada. */
function firstFreePosition(layers: LayerSettings[], color: string): number {
  const taken = new Set(layers.filter((l) => l.color !== color && l.order !== null).map((l) => l.order));
  for (let n = 1; n <= layers.length; n++) if (!taken.has(n)) return n;
  return layers.length;
}
