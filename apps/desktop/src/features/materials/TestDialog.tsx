import { TriangleAlert, X } from 'lucide-react';
import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { getApi } from '@/api';
import type { TestAxis, TestParam } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt, fmtClock } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { NumberField } from '@/ui/NumberField';
import { activeLayerOf, openTestAsNewDocument } from './actions';
import { useMaterials } from './state';
import {
  MAX_COUNT,
  TEST_ORIGIN,
  axisValues,
  buildParams,
  defaultForm,
  fixedFrom,
  fixedParams,
  gridLayout,
  paramsFor,
  setAxisParam,
  setMode,
  validateForm,
  valueText,
  type TestForm,
} from './testPlan';

const PARAM_UNIT: Record<TestParam, string> = { speed: 'mm/min', power: '%', passes: '', interval: 'mm' };
const PARAM_BOUNDS: Record<TestParam, { min: number; max: number; step: number }> = {
  speed: { min: 1, max: 100000, step: 100 },
  power: { min: 0, max: 100, step: 5 },
  passes: { min: 1, max: 1000, step: 1 },
  interval: { min: 0.01, max: 10, step: 0.01 },
};

// ---------------------------------------------------------------------------
// Vista previa esquemática
// ---------------------------------------------------------------------------

/** Qué tanta energía recibe una celda según la posición `pos` (0..1) a lo largo de un eje: más potencia o pasadas, o menos velocidad o intervalo. */
function energy(param: TestParam, pos: number): number {
  return param === 'speed' || param === 'interval' ? 1 - pos : pos;
}

/** Índices de valores que se rotulan en la vista previa (todos si son pocos). */
export function shownIndexes(count: number): number[] {
  if (count <= 6) return Array.from({ length: count }, (_, i) => i);
  const last = count - 1;
  return [...new Set([0, Math.round(last / 3), Math.round((2 * last) / 3), last])];
}

const MARGIN_LEFT = 16;
const MARGIN_TOP = 9;

export function GridPreview({ form }: { form: TestForm }) {
  const rows = axisValues(form.rows);
  const cols = axisValues(form.cols);
  const [cw, ch] = form.cell;
  const gap = form.gap;
  const w = MARGIN_LEFT + cols.length * cw + Math.max(0, cols.length - 1) * gap;
  const h = MARGIN_TOP + rows.length * ch + Math.max(0, rows.length - 1) * gap;
  const pos = (i: number, n: number) => (n <= 1 ? 0 : i / (n - 1));
  const font = Math.max(1.6, Math.min(3, Math.min(cw, ch) * 0.45));
  if (!(w > MARGIN_LEFT && h > MARGIN_TOP) || !Number.isFinite(w + h)) return <div className="mat-pv" aria-hidden="true" />;
  return (
    <svg className="mat-pv" viewBox={`0 0 ${w} ${h}`} role="img" aria-label={t('materials.test.previewLabel', { rows: rows.length, cols: cols.length })} preserveAspectRatio="xMidYMid meet">
      {rows.flatMap((_, i) =>
        cols.map((__, j) => {
          const e = (energy(form.rows.param, pos(i, rows.length)) + energy(form.cols.param, pos(j, cols.length))) / 2;
          return (
            <rect key={`${i}-${j}`} className="mat-pv__cell" x={MARGIN_LEFT + j * (cw + gap)} y={MARGIN_TOP + i * (ch + gap)} width={cw} height={ch} style={{ fillOpacity: 0.1 + 0.75 * e }} />
          );
        }),
      )}
      <g className="mat-pv__val" fontSize={font} data-printed={form.labels}>
        {shownIndexes(cols.length).map((j) => (
          <text key={`c${j}`} x={MARGIN_LEFT + j * (cw + gap) + cw / 2} y={MARGIN_TOP - font * 0.5} textAnchor="middle">
            {valueText(form.cols.param, cols[j]!)}
          </text>
        ))}
        {shownIndexes(rows.length).map((i) => (
          <text key={`r${i}`} x={MARGIN_LEFT - font * 0.6} y={MARGIN_TOP + i * (ch + gap) + ch / 2 + font * 0.35} textAnchor="end">
            {valueText(form.rows.param, rows[i]!)}
          </text>
        ))}
      </g>
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Estimación de tiempo
// ---------------------------------------------------------------------------

type Estimate = { status: 'idle' } | { status: 'busy' } | { status: 'ok'; seconds: number; warnings: string[] } | { status: 'error'; message: string };

/** Genera la cuadrícula y estima el trabajo con `computeJob`; espera a que el usuario deje de teclear. */
function useEstimate(params: ReturnType<typeof buildParams> | null): Estimate {
  const device = useStore(selectActiveDevice);
  const options = useStore((s) => s.jobOptions);
  const [estimate, setEstimate] = useState<Estimate>({ status: 'idle' });
  useEffect(() => {
    if (!params) {
      setEstimate({ status: 'idle' });
      return;
    }
    let alive = true;
    setEstimate({ status: 'busy' });
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const doc = await getApi().materialTest({ params });
          const job = await getApi().computeJob({ doc, device, options: { ...options, selection: null } });
          if (alive) setEstimate({ status: 'ok', seconds: job.total_s, warnings: job.warnings });
        } catch (e) {
          if (alive) setEstimate({ status: 'error', message: errorMessage(e) });
        }
      })();
    }, 450);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [params, device, options]);
  return estimate;
}

// ---------------------------------------------------------------------------
// Controles
// ---------------------------------------------------------------------------

interface AxisProps {
  which: 'rows' | 'cols';
  form: TestForm;
  onParam: (p: TestParam) => void;
  onAxis: (patch: Partial<TestAxis>) => void;
}

function AxisFields({ which, form, onParam, onAxis }: AxisProps) {
  const id = useId();
  const axis = form[which];
  const b = PARAM_BOUNDS[axis.param];
  const unit = PARAM_UNIT[axis.param];
  return (
    <fieldset className="mat-axis">
      <legend className="panel-title">{t(which === 'rows' ? 'materials.test.rows' : 'materials.test.cols')}</legend>
      <div className="field">
        <label className="field__label" htmlFor={id}>{t('materials.test.param')}</label>
        <select id={id} className="select" value={axis.param} onChange={(e) => onParam(e.target.value as TestParam)}>
          {paramsFor(form.mode).map((p) => (
            <option key={p} value={p}>{t(`materials.param.${p}` as I18nKey)}</option>
          ))}
        </select>
      </div>
      <div className="mat-axis__nums">
        <NumberField label={t('materials.test.count')} value={axis.count} min={1} max={MAX_COUNT} step={1} onCommit={(n) => onAxis({ count: Math.round(n) })} />
        <NumberField label={t('materials.test.from')} value={axis.from} min={b.min} max={b.max} step={b.step} unit={unit} onCommit={(n) => onAxis({ from: n })} />
        <NumberField label={t('materials.test.to')} value={axis.to} min={b.min} max={b.max} step={b.step} unit={unit} onCommit={(n) => onAxis({ to: n })} />
      </div>
    </fieldset>
  );
}

/** Diálogo "Prueba de materiales…": cuadrícula de celdas con un parámetro por eje. */
export function TestDialog() {
  const titleId = useId();
  const close = useMaterials((s) => s.closeTest);
  const active = useStore((s) => activeLayerOf(s));
  const device = useStore(selectActiveDevice);
  const [form, setForm] = useState<TestForm>(() => useMaterials.getState().testForm ?? defaultForm(active));
  const [creating, setCreating] = useState(false);
  const formRef = useRef(form);
  formRef.current = form;
  // Se recuerda lo último que se dejó para reabrir la prueba donde se quedó.
  useEffect(() => () => useMaterials.getState().setTestForm(formRef.current), []);

  // Si la capa activa desaparece (deshacer), deja de ser la fuente de los ajustes base.
  const usable = form.fromActiveLayer && active !== null;
  const patch = (p: Partial<TestForm>) => setForm((f) => ({ ...f, ...p }));
  const patchAxis = (which: 'rows' | 'cols', p: Partial<TestAxis>) => setForm((f) => ({ ...f, [which]: { ...f[which], ...p } }));

  const problem = validateForm(form);
  const params = useMemo(() => (problem ? null : buildParams(form, usable ? active : null)), [form, active, usable, problem]);
  const estimate = useEstimate(params);
  const layout = useMemo(() => gridLayout(form), [form]);
  const fits = TEST_ORIGIN[0] + layout.width <= device.bed[0] && TEST_ORIGIN[1] + layout.height <= device.bed[1];
  const cells = form.rows.count * form.cols.count;

  const blocked = problem ? t(problem.key, problem.params) : creating ? t('materials.test.creating') : null;
  const accept = async () => {
    if (blocked) return;
    setCreating(true);
    const ok = await openTestAsNewDocument({ ...form, fromActiveLayer: usable });
    setCreating(false);
    if (ok) close();
  };

  return (
    <Modal labelId={titleId} onClose={close} portal closeOnBackdrop fallbackFocus="card" passKey={(e) => (e.metaKey || e.ctrlKey) && e.key === '.'} backdropClassName="mat-backdrop" cardClassName="mat-modal mat-modal--wide">
      <header className="mat-modal__head">
        <div>
          <h2 id={titleId} className="mat-modal__title">{t('materials.test.title')}</h2>
          <p className="hint">{t('materials.test.subtitle')}</p>
        </div>
        <button type="button" className="icon-btn" title={t('materials.close')} aria-label={t('materials.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>

      <div className="mat-test">
        <div className="mat-test__left">
          <div className="mat-test__stage">
            <GridPreview form={form} />
          </div>
          <div className="mat-test__stats" aria-live="polite">
            <span className="num">{t('materials.test.stats', { cells, w: fmt(layout.width, 0), h: fmt(layout.height, 0) })}</span>
            <span className="num">
              {estimate.status === 'ok' && t('materials.test.estimate', { time: fmtClock(estimate.seconds) })}
              {estimate.status === 'busy' && t('materials.test.estimating')}
              {estimate.status === 'idle' && '—'}
            </span>
          </div>
          {estimate.status === 'error' && (
            <p className="warn-line" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('materials.test.estimateFailed', { error: estimate.message })}</span>
            </p>
          )}
          {estimate.status === 'ok' && estimate.warnings.length > 0 && (
            <ul className="mat-warnings" aria-label={t('materials.test.warnings')}>
              {estimate.warnings.map((w) => <li key={w}>{w}</li>)}
            </ul>
          )}
          {!fits && !problem && (
            <p className="warn-line" role="status">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('materials.test.tooBig', { w: fmt(layout.width, 0), h: fmt(layout.height, 0), bw: fmt(device.bed[0], 0), bh: fmt(device.bed[1], 0) })}</span>
            </p>
          )}
          <p className="hint">{t('materials.test.opensNew')}</p>
        </div>

        <div className="mat-test__controls">
          <div className="field">
            <label className="field__label" htmlFor="mat-test-mode">{t('materials.test.mode')}</label>
            <select id="mat-test-mode" className="select" value={form.mode} onChange={(e) => setForm((f) => setMode(f, e.target.value as TestForm['mode']))}>
              <option value="line">{t('materials.mode.line')}</option>
              <option value="fill">{t('materials.mode.fill')}</option>
            </select>
          </div>

          <AxisFields which="rows" form={form} onParam={(p) => setForm((f) => setAxisParam(f, 'rows', p))} onAxis={(p) => patchAxis('rows', p)} />
          <AxisFields which="cols" form={form} onParam={(p) => setForm((f) => setAxisParam(f, 'cols', p))} onAxis={(p) => patchAxis('cols', p)} />

          <fieldset className="mat-axis">
            <legend className="panel-title">{t('materials.test.cells')}</legend>
            <div className="mat-axis__nums">
              <NumberField label={t('materials.test.cellW')} value={form.cell[0]} min={1} max={200} step={1} unit="mm" onCommit={(n) => patch({ cell: [n, form.cell[1]] })} />
              <NumberField label={t('materials.test.cellH')} value={form.cell[1]} min={1} max={200} step={1} unit="mm" onCommit={(n) => patch({ cell: [form.cell[0], n] })} />
              <NumberField label={t('materials.test.gap')} value={form.gap} min={0} max={100} step={1} unit="mm" onCommit={(n) => patch({ gap: n })} />
            </div>
            <label className="mat-check">
              <input type="checkbox" checked={form.labels} onChange={(e) => patch({ labels: e.target.checked })} />
              {t('materials.test.labels')}
            </label>
            {form.labels && (
              <div className="mat-axis__nums">
                <NumberField label={t('materials.test.labelSpeed')} value={form.labelSpeed} min={1} max={100000} step={100} unit="mm/min" onCommit={(n) => patch({ labelSpeed: n })} />
                <NumberField label={t('materials.test.labelPower')} value={form.labelPower} min={0} max={100} step={1} unit="%" onCommit={(n) => patch({ labelPower: n })} />
              </div>
            )}
          </fieldset>

          <fieldset className="mat-axis">
            <legend className="panel-title">{t('materials.test.fixed')}</legend>
            <label className="mat-check" title={active ? undefined : t('materials.reason.noLayer')}>
              <input
                type="checkbox"
                checked={usable}
                disabled={active === null}
                onChange={(e) => patch({ fromActiveLayer: e.target.checked, fixed: fixedFrom(e.target.checked ? active : null) })}
              />
              {active ? t('materials.test.fromLayer', { name: active.name || active.color }) : t('materials.test.fromLayerNone')}
            </label>
            <div className="mat-axis__nums">
              {fixedParams(form).map((p) => {
                const b = PARAM_BOUNDS[p];
                return (
                  <NumberField
                    key={p}
                    label={t(`materials.param.${p}` as I18nKey)}
                    value={form.fixed[p]}
                    min={b.min}
                    max={b.max}
                    step={b.step}
                    unit={PARAM_UNIT[p]}
                    onCommit={(n) => patch({ fixed: { ...form.fixed, [p]: p === 'passes' ? Math.round(n) : n } })}
                  />
                );
              })}
            </div>
          </fieldset>
        </div>
      </div>

      <footer className="mat-modal__foot">
        {problem && <p className="hint hint--danger mat-modal__problem" role="status">{t(problem.key, problem.params)}</p>}
        <button type="button" className="btn" onClick={close}>{t('materials.cancel')}</button>
        <button type="button" className="btn btn--accent" aria-disabled={blocked !== null} title={blocked ?? undefined} onClick={() => void accept()}>
          {t('materials.test.accept')}
        </button>
      </footer>
    </Modal>
  );
}
