import { Box, Cog, FlaskConical, Spline, X } from 'lucide-react';
import { useEffect, useId, useMemo } from 'react';
import { selectActiveDevice, useStore } from '@/store';
import { t, tn, type I18nKey } from '@/i18n';
import { groupMembers } from '@/lib/groups';
import { Modal } from '@/ui/Modal';
import { commitParts, defaultCutLayer, loadGenerators } from './actions';
import { BED_MARGIN_MM } from './partsModel';
import { ParamForm } from './ParamForm';
import { PartsPreview } from './PartsPreview';
import { NEW_ENGRAVE, NO_ENGRAVE, usePartsUi, valuesFor } from './partsStore';
import { usePartsPreview } from './usePartsPreview';
import './parts.css';

const ICONS: Record<string, typeof Box> = { box: Box, hinge: Spline, fit_test: FlaskConical, gears: Cog };

function Dialog() {
  const titleId = useId();
  const cutId = useId();
  const engraveId = useId();
  const ui = usePartsUi();
  const doc = useStore((s) => s.doc);
  const activeLayer = useStore((s) => s.activeLayer);
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const info = ui.generators.find((g) => g.id === ui.generator);
  const values = valuesFor(ui, ui.generator);
  const regen = ui.regenGroup !== null;
  const lineLayers = doc.layers.filter((l) => l.mode === 'line');
  const cutLayer = ui.cutLayer ?? defaultCutLayer(doc, activeLayer);
  const maxWidth = Math.max(10, bed[0] - 2 * BED_MARGIN_MM);

  useEffect(() => {
    void loadGenerators();
  }, []);
  const { result, loading, failure } = usePartsPreview(ui.generator, values, maxWidth, !!info);

  const cutColor = lineLayers.find((l) => l.color.toLowerCase() === cutLayer.toLowerCase())?.color ?? cutLayer;
  const engraveColor = lineLayers.find((l) => l.color.toLowerCase() === ui.engraveLayer.toLowerCase())?.color ?? 'var(--accent)';
  const cutSettings = doc.layers.find((l) => l.color.toLowerCase() === cutLayer.toLowerCase());
  const wantsKerfHint = ui.generator !== 'hinge';
  const hasLabels = !!result?.parts.some((p) => p.engraves.length > 0);
  const expected = result ? result.parts.reduce((n, p) => n + (p.cuts.length > 0 ? 1 : 0) + (ui.engraveLayer !== NO_ENGRAVE && p.engraves.length > 0 ? 1 : 0), 0) : 0;
  const existing = useMemo(() => (ui.regenGroup ? groupMembers(doc.objects, ui.regenGroup).length : 0), [doc.objects, ui.regenGroup]);
  const tooBig = !!result && !result.error && (result.width > bed[0] || result.height > bed[1]);

  const close = () => ui.closeDialog();
  const ok = !!result && !result.error && result.parts.length > 0 && !ui.busy;

  return (
    <Modal labelId={titleId} onClose={close} portal backdropClassName="mc-backdrop" cardClassName="mc-modal parts-modal" initialFocus="[data-autofocus]" passKey={(e) => (e.metaKey || e.ctrlKey) && e.key === '.'}>
      <header className="mc-modal__head">
        <div>
          <h2 id={titleId} className="mc-modal__title">{t(regen ? 'parts.titleRegen' : 'parts.title')}</h2>
          <p className="hint">{t('parts.subtitle')}</p>
        </div>
        <button type="button" className="icon-btn" title={t('parts.close')} aria-label={t('parts.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      <div className="mc-modal__body parts__body">
        <nav className="parts__list" aria-label={t('parts.list.label')}>
          {ui.generators.map((g) => {
            const Icon = ICONS[g.id] ?? Box;
            const active = g.id === ui.generator;
            return (
              <button key={g.id} type="button" className="parts__gen" aria-pressed={active} disabled={regen && !active} onClick={() => ui.setGenerator(g.id)}>
                <Icon size={16} aria-hidden="true" />
                <span>
                  <span className="parts__gen-name">{t(`parts.generator.${g.id}` as I18nKey)}</span>
                  <span className="hint parts__gen-desc">{t(`parts.generator.${g.id}.desc` as I18nKey)}</span>
                </span>
              </button>
            );
          })}
        </nav>
        <section className="parts__col" aria-label={t('parts.form.label')}>
          {info && <ParamForm info={info} values={values} error={result?.error ?? null} onChange={(k, v) => ui.setValue(ui.generator, k, v)} />}
        </section>
        <section className="parts__col parts__result" aria-label={t('parts.preview.label')}>
          <div className="parts__stage" aria-busy={loading}>
            {result && !result.error && result.parts.length > 0 ? (
              <PartsPreview result={result} cutColor={cutColor} engraveColor={engraveColor} />
            ) : (
              <p className="hint parts__empty" role="status">{loading ? t('parts.preview.loading') : (failure ?? t('parts.preview.empty'))}</p>
            )}
          </div>
          {result && !result.error && result.parts.length > 0 && (
            <p className="hint num" role="status">
              {tn('parts.preview.size', result.parts.length, { w: Math.round(result.width * 10) / 10, h: Math.round(result.height * 10) / 10 })}
            </p>
          )}
          {tooBig && result && (
            <p className="hint hint--danger" role="alert">
              {t('parts.warn.bed', { w: Math.round(result.width), h: Math.round(result.height), bw: Math.round(bed[0]), bh: Math.round(bed[1]) })}
            </p>
          )}
          {regen && result && !result.error && existing !== expected && <p className="hint hint--danger" role="alert">{t('parts.warn.edited')}</p>}
          {result?.warnings.map((w, i) => (
            <p key={i} className="hint parts__warn">{w}</p>
          ))}
          <fieldset className="parts__group">
            <legend className="mc-sub">{t('parts.layers.title')}</legend>
            <div className="field">
              <label htmlFor={cutId} className="field__label">{t('parts.layers.cut')}</label>
              <select id={cutId} className="select" value={cutLayer} onChange={(e) => ui.setCutLayer(e.target.value)}>
                {!lineLayers.some((l) => l.color.toLowerCase() === cutLayer.toLowerCase()) && <option value={cutLayer}>{cutLayer}</option>}
                {lineLayers.map((l) => (
                  <option key={l.color} value={l.color}>{l.name}</option>
                ))}
              </select>
            </div>
            {hasLabels && (
              <div className="field">
                <label htmlFor={engraveId} className="field__label">{t('parts.layers.engrave')}</label>
                <select id={engraveId} className="select" value={ui.engraveLayer} onChange={(e) => ui.setEngraveLayer(e.target.value)}>
                  <option value={NO_ENGRAVE}>{t('parts.layers.engraveNone')}</option>
                  <option value={NEW_ENGRAVE}>{t('parts.layers.newEngrave')}</option>
                  {lineLayers.filter((l) => l.color.toLowerCase() !== cutLayer.toLowerCase()).map((l) => (
                    <option key={l.color} value={l.color}>{l.name}</option>
                  ))}
                </select>
              </div>
            )}
            {wantsKerfHint && (
              <p className="hint">
                {cutSettings && cutSettings.kerf !== 0
                  ? t('parts.kerf.on', { layer: cutSettings.name, kerf: cutSettings.kerf })
                  : t('parts.kerf.off')}
              </p>
            )}
          </fieldset>
        </section>
      </div>
      <footer className="mc-modal__foot">
        <button type="button" className="btn" onClick={() => ui.resetValues(ui.generator)}>{t('parts.reset')}</button>
        <span className="parts__spacer" />
        <button type="button" className="btn" onClick={close}>{t('parts.cancel')}</button>
        <button
          type="button"
          className="btn btn--accent"
          data-autofocus
          disabled={!ok}
          onClick={() => void commitParts().then((done) => done && usePartsUi.getState().closeDialog())}
        >
          {ui.busy ? t('parts.working') : t(regen ? 'parts.regenerate' : 'parts.insert')}
        </button>
      </footer>
    </Modal>
  );
}

/** Diálogo «Generadores de piezas» (menú Herramientas). Se monta una vez en `AppShell`. */
export function PartsDialog() {
  const open = usePartsUi((s) => s.open);
  return open ? <Dialog /> : null;
}
