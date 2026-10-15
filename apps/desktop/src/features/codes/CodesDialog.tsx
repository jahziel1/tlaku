import { TriangleAlert, X } from 'lucide-react';
import { useId, useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { CodeSpec } from '@/api/types';
import { effectiveFont, isLineFont, pickDefaultFont, useFonts } from '@/features/text/fonts';
import { FontPicker } from '@/features/text/FontPicker';
import { bezToSvgD } from '@/features/trace/stats';
import { ColumnPicker, displayContext, issueMessage, resolveTemplate, useVariablesUi, variableCodeOf } from '@/features/variables';
import { hasVariables } from '@/features/variables/syntax';
import { t } from '@/i18n';
import { colorName } from '@/lib/palette';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { NumberField } from '@/ui/NumberField';
import { useCodesDialog } from './codesStore';
import { applyCodeToDoc, codeCenter, codeLayerFor } from './placement';
import { CODE_LIMITS, ECC_LEVELS, KINDS, kindInfo } from './kinds';
import { useCodeRun } from './useCodeRun';
import './codes.css';

/** Lo que se pide al motor: solo lo que importa para el tipo elegido (no recalcula por lo demás). */
export function specToSend(spec: CodeSpec, font: string): CodeSpec {
  const linear = kindInfo(spec.kind).linear;
  return {
    ...spec,
    human_readable: linear && spec.human_readable,
    font_family: linear && spec.human_readable ? font : '',
    ecc: spec.kind === 'qr' ? spec.ecc : 'm',
    height_mm: linear ? spec.height_mm : 15,
  };
}

/** Diálogo "Insertar código": datos y ajustes a la derecha, vista previa a la izquierda. */
export function CodesDialog() {
  const titleId = useId();
  const close = useCodesDialog((s) => s.close);
  return (
    <Modal
      labelId={titleId}
      onClose={close}
      portal
      closeOnBackdrop
      fallbackFocus="card"
      passKey={(e) => (e.metaKey || e.ctrlKey) && e.key === '.'}
      backdropClassName="cd-backdrop"
      cardClassName="cd-modal"
    >
      <CodesContent titleId={titleId} />
    </Modal>
  );
}

/** Contenido del diálogo (exportado para pruebas: el diálogo entero va en un portal). */
export function CodesContent({ titleId }: { titleId: string }) {
  const { spec, patch, close } = useCodesDialog(useShallow((s) => ({ spec: s.spec, patch: s.patch, close: s.close })));
  const layers = useStore((s) => s.doc.layers);
  const activeLayer = useStore((s) => s.activeLayer);
  const { fonts, loading } = useFonts();
  const outlineFonts = useMemo(() => fonts.filter((f) => !isLineFont(f)), [fonts]);
  const fallbackFont = useMemo(() => pickDefaultFont(outlineFonts), [outlineFonts]);
  const font = effectiveFont(outlineFonts, spec.font_family).family;

  const info = kindInfo(spec.kind);
  // Contenido con variables ({csv:Columna}): la vista previa y lo que se inserta usan la fila actual.
  const doc = useStore((s) => s.doc);
  const csvState = useVariablesUi((s) => s.csv);
  const extraState = useVariablesUi((s) => s.extra);
  // Solo si lleva una variable de verdad: unas llaves sueltas (un JSON, por ejemplo) se codifican tal cual.
  const variable = hasVariables(spec.data);
  const resolvedData = useMemo(
    () => (variable ? resolveTemplate(spec.data, displayContext(doc)) : null),
    // `csvState` y `extraState` no se leen, pero `displayContext` toma los CSV cargados del estado de la interfaz.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [variable, spec.data, doc, csvState, extraState],
  );
  const variableIssues = resolvedData?.issues ?? [];
  const toSend = useMemo(
    () => specToSend({ ...spec, data: resolvedData ? (variableIssues.length > 0 ? '' : resolvedData.text) : spec.data }, font),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [spec, font, resolvedData],
  );
  const run = useCodeRun(toSend);
  const result = run.result;

  const targetColor = codeLayerFor(layers, activeLayer);
  const targetName = layers.find((l) => l.color.toLowerCase() === targetColor.toLowerCase())?.name ?? colorName(targetColor);

  const blocked =
    spec.data.length === 0
      ? t('codes.insert.empty')
      : variableIssues.length > 0
        ? t('codes.insert.variableIssue', { problem: issueMessage(variableIssues[0]!) })
        : run.status === 'computing'
        ? t('codes.insert.busy')
        : run.status === 'error' || !result
          ? t('codes.insert.error')
          : null;

  const accept = () => {
    if (blocked || !result) return;
    const s = useStore.getState();
    const out: { id: string | null } = { id: null };
    const center = codeCenter(
      { zoom: s.zoom, panX: s.panX, panY: s.panY, width: s.viewport.width, height: s.viewport.height },
      selectActiveDevice(s).bed,
    );
    s.applyDocChange('codes.history.insert', (doc) => {
      const r = applyCodeToDoc(doc, {
        paths: result.paths,
        size: [result.width_mm, result.height_mm],
        center,
        layer: targetColor,
        code: variable ? variableCodeOf({ ...specToSend(spec, font), data: spec.data }) : undefined,
      });
      out.id = r.id;
      return r.doc;
    });
    if (out.id) {
      s.select([out.id]);
      s.notify('success', t('codes.done', { w: fmt(result.width_mm), h: fmt(result.height_mm) }));
    }
    close();
  };

  const [sizeMin, sizeMax] = CODE_LIMITS.size;
  const [heightMin, heightMax] = CODE_LIMITS.height;
  const [marginMin, marginMax] = CODE_LIMITS.margin;
  const dataId = useId();
  const kindId = useId();
  const eccId = useId();
  const readableId = useId();
  const invertId = useId();

  return (
    <>
      <header className="cd-head">
        <h2 id={titleId} className="cd-title">{t('codes.title')}</h2>
        <button type="button" className="icon-btn" title={t('codes.close')} aria-label={t('codes.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>

      <div className="cd-body">
        <div className="cd-left">
          <div className="cd-stage" role="img" aria-label={t('codes.preview.label')} data-stale={run.status === 'computing' || undefined}>
            {result && result.paths.length > 0 ? (
              <CodePreview result={result} color={targetColor} />
            ) : (
              <p className="hint">
                {spec.data.length === 0 ? t('codes.preview.empty') : run.status === 'error' ? '' : t('codes.preview.computing')}
              </p>
            )}
          </div>
          <div className="cd-stats num" aria-live="polite">
            {result && (
              <>
                <span>{t('codes.preview.size', { w: fmt(result.width_mm), h: fmt(result.height_mm) })}</span>
                <span>{t('codes.preview.modules', { n: result.modules })}</span>
              </>
            )}
          </div>
          {run.status === 'error' && (
            <p className="warn-line" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('codes.error', { msg: run.error ?? '' })}</span>
            </p>
          )}
          {result && result.warnings.length > 0 && (
            <ul className="cd-warnings" aria-label={t('codes.warnings')}>
              {result.warnings.map((w) => (
                <li key={w} className="warn-line">
                  <TriangleAlert size={14} aria-hidden="true" />
                  <span>{w}</span>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="cd-side">
          <div className="field">
            <label className="field__label" htmlFor={kindId}>{t('codes.field.kind')}</label>
            <select id={kindId} className="select" value={spec.kind} onChange={(e) => patch({ kind: e.target.value as CodeSpec['kind'] })}>
              <optgroup label={t('codes.kind.group.2d')}>
                {KINDS.filter((k) => !k.linear).map((k) => (
                  <option key={k.kind} value={k.kind}>{t(k.label)}</option>
                ))}
              </optgroup>
              <optgroup label={t('codes.kind.group.1d')}>
                {KINDS.filter((k) => k.linear).map((k) => (
                  <option key={k.kind} value={k.kind}>{t(k.label)}</option>
                ))}
              </optgroup>
            </select>
          </div>

          <div className="field">
            <label className="field__label" htmlFor={dataId}>{t('codes.field.data')}</label>
            {info.linear ? (
              <input
                id={dataId}
                className="input"
                value={spec.data}
                placeholder={t('codes.field.dataPlaceholder')}
                spellCheck={false}
                autoComplete="off"
                data-autofocus=""
                onChange={(e) => patch({ data: e.target.value })}
              />
            ) : (
              <textarea
                id={dataId}
                className="input cd-area"
                rows={3}
                value={spec.data}
                placeholder={t('codes.field.dataPlaceholder')}
                spellCheck={false}
                data-autofocus=""
                onChange={(e) => patch({ data: e.target.value })}
              />
            )}
            <p className="hint">{t(info.hint)}</p>
            <p className="hint">{t('codes.field.variableHint')}</p>
            {variable && variableIssues.length === 0 && resolvedData && (
              <p className="hint num">{t('codes.field.variablePreview', { value: resolvedData.text })}</p>
            )}
            {variableIssues.map((issue, i) => (
              <p key={i} className="warn-line">
                <TriangleAlert size={14} aria-hidden="true" />
                <span>{issueMessage(issue)}</span>
              </p>
            ))}
          </div>
          <ColumnPicker label={t('codes.field.insertColumn')} onPick={(snippet) => patch({ data: spec.data + snippet })} />

          {spec.kind === 'qr' && (
            <div className="field">
              <label className="field__label" htmlFor={eccId}>{t('codes.field.ecc')}</label>
              <select id={eccId} className="select" value={spec.ecc} onChange={(e) => patch({ ecc: e.target.value as CodeSpec['ecc'] })}>
                {ECC_LEVELS.map((l) => (
                  <option key={l.value} value={l.value}>{t(l.label)}</option>
                ))}
              </select>
            </div>
          )}

          <div className="cd-grid">
            <NumberField
              label={info.linear ? t('codes.field.sizeBars') : t('codes.field.sizeQr')}
              unit="mm"
              value={spec.size_mm}
              min={sizeMin}
              max={sizeMax}
              step={1}
              onCommit={(v) => patch({ size_mm: v })}
            />
            {info.linear && (
              <NumberField
                label={t('codes.field.height')}
                unit="mm"
                value={spec.height_mm}
                min={heightMin}
                max={heightMax}
                step={1}
                onCommit={(v) => patch({ height_mm: v })}
              />
            )}
            <NumberField
              label={t('codes.field.margin')}
              unit="mm"
              value={spec.margin_mm}
              min={marginMin}
              max={marginMax}
              step={0.5}
              onCommit={(v) => patch({ margin_mm: v })}
            />
          </div>
          <p className="hint">{t('codes.field.marginHint')}</p>

          {info.linear && (
            <>
              <label className="cd-check" htmlFor={readableId}>
                <input id={readableId} type="checkbox" checked={spec.human_readable} onChange={(e) => patch({ human_readable: e.target.checked })} />
                <span>{t('codes.field.readable')}</span>
              </label>
              {spec.human_readable && (
                <div className="field">
                  <span className="field__label">{t('codes.field.font')}</span>
                  <FontPicker
                    value={spec.font_family}
                    fallback={fallbackFont}
                    families={outlineFonts}
                    loading={loading}
                    onPick={(family) => patch({ font_family: family })}
                  />
                </div>
              )}
            </>
          )}

          <label className="cd-check" htmlFor={invertId}>
            <input id={invertId} type="checkbox" checked={spec.invert} onChange={(e) => patch({ invert: e.target.checked })} />
            <span>{t('codes.field.invert')}</span>
          </label>
          <p className="hint">{t('codes.field.invertHint')}</p>
        </div>
      </div>

      <footer className="cd-foot">
        <p className="cd-layer">
          <span className="cd-swatch" style={{ background: targetColor }} aria-hidden="true" />
          {targetName}
        </p>
        <button type="button" className="btn" onClick={close}>{t('codes.cancel')}</button>
        <button type="button" className="btn btn--accent" disabled={blocked !== null} title={blocked ?? undefined} onClick={accept}>
          {t('codes.insert')}
        </button>
      </footer>
    </>
  );
}

/** Formas del código en un SVG (mm, Y hacia arriba: se voltea con una matriz). */
function CodePreview({ result, color }: { result: NonNullable<ReturnType<typeof useCodeRun>['result']>; color: string }) {
  const d = useMemo(() => bezToSvgD(result.paths), [result.paths]);
  const { width_mm: w, height_mm: h } = result;
  const pad = Math.max(w, h) * 0.06;
  return (
    <svg className="cd-svg" viewBox={`${-pad} ${-pad} ${w + 2 * pad} ${h + 2 * pad}`} preserveAspectRatio="xMidYMid meet" aria-hidden="true">
      <g transform={`translate(0 ${h}) scale(1 -1)`}>
        <path d={d} fill={color} fillRule="evenodd" />
      </g>
    </svg>
  );
}
