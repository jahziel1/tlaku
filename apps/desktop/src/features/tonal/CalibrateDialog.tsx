import { X } from 'lucide-react';
import { useId, useMemo, useState } from 'react';
import { persistMaterials } from '@/features/materials/actions';
import { useMaterials } from '@/features/materials/state';
import { axisValues, defaultForm, valueText, type TestForm } from '@/features/materials/testPlan';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { cellKey, gridCells, marksFromSamples, samplesOf, type GridCell } from './calibration';
import { curveFor, fitCurve, hexOf, presetCurve, rgbOfHex, toneDarkness, toneMix, withCalibration, type ToneCurve, type ToneSample } from './curve';
import { useTonal } from './store';

/** Pasos de la escala de tono que se ofrecen (porcentaje de oscuridad). */
const TONE_STEPS = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100] as const;

const nameOf = (material: string) => t(`preview.tonal.material.${material}` as I18nKey);

/** Misma cuadrícula que el formulario de la prueba de materiales, o la de fábrica si nunca se abrió. */
function useGridForm(): TestForm {
  const stored = useMaterials.getState().testForm;
  const layer = useStore.getState().doc.layers.find((l) => l.color.toLowerCase() === useStore.getState().activeLayer?.toLowerCase()) ?? null;
  return useMemo(() => stored ?? defaultForm(layer), [stored, layer]);
}

/** Dosis para mostrar: tres cifras significativas. */
const doseText = (d: number) => (d >= 10 ? fmt(d, 1) : d >= 1 ? fmt(d, 2) : fmt(d, 3));

function CellButton({ cell, curve, observed, selected, onSelect }: { cell: GridCell; curve: ToneCurve; observed: number | undefined; selected: boolean; onSelect: () => void }) {
  const dark = observed ?? undefined;
  const info = t('preview.tonal.cal.cellInfo', { speed: fmt(cell.speed, 0), power: fmt(cell.power, 0), passes: cell.passes, dose: doseText(cell.dose) });
  const label = t('preview.tonal.cal.cellLabel', { row: cell.row + 1, col: cell.col + 1, info });
  // El color de la celda es el que predice la curva; con una observación, un aro marca que el usuario ya opinó.
  const predicted = hexOf(toneMix(curve, toneDarkness(curve, cell.dose)));
  return (
    <button
      type="button"
      className="tn-cell"
      aria-pressed={selected}
      aria-label={dark === undefined ? label : `${label}; ${t('preview.tonal.cal.observed', { n: Math.round(dark * 100) })}`}
      title={label}
      data-observed={dark !== undefined}
      style={{ background: predicted }}
      onClick={onSelect}
    >
      {dark !== undefined && <span className="tn-cell__obs" style={{ background: hexOf(toneMix(curve, dark)) }} aria-hidden="true" />}
    </button>
  );
}

/**
 * Calibración del tono con la prueba de materiales (mejora 8): se muestra la cuadrícula del formulario
 * de la prueba con el tono que predice la curva y el usuario indica a qué punto de la escala se parece
 * cada celda grabada de verdad. `fitCurve` ajusta la curva del material y se guarda en la biblioteca
 * local de materiales (`tones`).
 */
export function CalibrateDialog() {
  const titleId = useId();
  const id = useId();
  const material = useTonal((s) => s.material);
  const spotMm = useTonal((s) => s.spotMm);
  const library = useMaterials((s) => s.library);
  const loadError = useMaterials((s) => s.loadError);
  const form = useGridForm();
  const cells = useMemo(() => gridCells(form, spotMm), [form, spotMm]);
  const saved = library.tones?.find((c) => c.material === material) ?? null;

  const [colors, setColors] = useState(() => {
    const c = curveFor(material, library.tones);
    return { base: c.base, burn: c.burn };
  });
  const [marks, setMarks] = useState<Record<string, number>>(() => marksFromSamples(cells, saved?.samples ?? []));
  // Observaciones guardadas que no corresponden a ninguna celda de esta cuadrícula: se conservan.
  const [kept] = useState<ToneSample[]>(() => {
    const matched = marksFromSamples(cells, saved?.samples ?? []);
    const used = new Set(samplesOf(cells, matched).map((s) => s.dose));
    return (saved?.samples ?? []).filter((s) => !used.has(s.dose));
  });
  const [selected, setSelected] = useState<string | null>(null);

  const start = useMemo<ToneCurve>(() => ({ ...presetCurve(material), base: colors.base, burn: colors.burn }), [material, colors]);
  const samples = useMemo(() => [...kept, ...samplesOf(cells, marks)], [kept, cells, marks]);
  const curve = useMemo(() => fitCurve(start, samples), [start, samples]);
  const sel = selected ? cells.flat().find((c) => cellKey(c.row, c.col) === selected) ?? null : null;

  const close = () => useTonal.getState().setCalibrateOpen(false);
  const notify = useStore.getState().notify;

  const save = () => {
    if (loadError) {
      notify('error', t('preview.tonal.cal.loadError'));
      return;
    }
    const cal = { material, curve, samples };
    useMaterials.getState().setLibrary({ ...library, tones: withCalibration(library.tones, material, cal) });
    void persistMaterials();
    notify('info', t('preview.tonal.cal.saved', { material: nameOf(material) }));
    close();
  };
  const reset = () => {
    if (loadError) {
      notify('error', t('preview.tonal.cal.loadError'));
      return;
    }
    useMaterials.getState().setLibrary({ ...library, tones: withCalibration(library.tones, material, null) });
    void persistMaterials();
    notify('info', t('preview.tonal.cal.removed', { material: nameOf(material) }));
    close();
  };

  const setMark = (key: string, value: number | null) =>
    setMarks((m) => {
      const next = { ...m };
      if (value === null) delete next[key];
      else next[key] = value;
      return next;
    });
  const setColor = (which: 'base' | 'burn', hex: string) => {
    const rgb = rgbOfHex(hex);
    if (rgb) setColors((c) => ({ ...c, [which]: rgb }));
  };

  const rows = axisValues(form.rows);
  const cols = axisValues(form.cols);
  const axisName = (a: TestForm['rows']) => `${t(`materials.param.${a.param}` as I18nKey)} (${valueText(a.param, a.from)}–${valueText(a.param, a.to)})`;

  return (
    <Modal labelId={titleId} onClose={close} portal closeOnBackdrop fallbackFocus="card" backdropClassName="mat-backdrop" cardClassName="mat-modal mat-modal--wide">
      <header className="mat-modal__head">
        <div>
          <h2 id={titleId} className="mat-modal__title">{t('preview.tonal.cal.title', { material: nameOf(material) })}</h2>
          <p className="hint">{t('preview.tonal.cal.subtitle')}</p>
        </div>
        <button type="button" className="icon-btn" title={t('preview.tonal.cal.close')} aria-label={t('preview.tonal.cal.close')} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>

      <div className="tn-cal">
        <div className="tn-cal__grid">
          <p className="hint">{t('preview.tonal.cal.gridSource', { rows: axisName(form.rows), cols: axisName(form.cols) })}</p>
          <div className="tn-grid" role="group" aria-label={t('preview.tonal.cal.gridLabel')} style={{ ['--tn-cols' as string]: cols.length }}>
            {cells.map((row, i) => (
              <div className="tn-grid__row" key={i} role="presentation">
                {row.map((c) => (
                  <CellButton
                    key={cellKey(c.row, c.col)}
                    cell={c}
                    curve={curve}
                    observed={marks[cellKey(c.row, c.col)]}
                    selected={selected === cellKey(c.row, c.col)}
                    onSelect={() => setSelected(cellKey(c.row, c.col))}
                  />
                ))}
              </div>
            ))}
          </div>
          <p className="hint num">{rows.length} × {cols.length}</p>
        </div>

        <div className="tn-cal__side">
          <section aria-labelledby={`${id}-pick`}>
            <h3 id={`${id}-pick`} className="panel-title">{t('preview.tonal.cal.pick')}</h3>
            {sel ? (
              <>
                <p className="num">{t('preview.tonal.cal.cellInfo', { speed: fmt(sel.speed, 0), power: fmt(sel.power, 0), passes: sel.passes, dose: doseText(sel.dose) })}</p>
                <div className="tn-steps" role="group" aria-label={t('preview.tonal.cal.pick')}>
                  {TONE_STEPS.map((n) => {
                    const active = marks[cellKey(sel.row, sel.col)] !== undefined && Math.round(marks[cellKey(sel.row, sel.col)]! * 100) === n;
                    return (
                      <button
                        key={n}
                        type="button"
                        className="tn-step"
                        aria-pressed={active}
                        aria-label={t('preview.tonal.cal.tone', { n })}
                        title={t('preview.tonal.cal.tone', { n })}
                        style={{ background: hexOf(toneMix(curve, n / 100)) }}
                        onClick={() => setMark(cellKey(sel.row, sel.col), n / 100)}
                      >
                        <span className="tn-step__n">{n}</span>
                      </button>
                    );
                  })}
                </div>
                <p className="hint">{t('preview.tonal.cal.pickHint')}</p>
                <button type="button" className="btn" disabled={marks[cellKey(sel.row, sel.col)] === undefined} onClick={() => setMark(cellKey(sel.row, sel.col), null)}>
                  {t('preview.tonal.cal.clear')}
                </button>
              </>
            ) : (
              <p className="hint">{t('preview.tonal.cal.noCell')}</p>
            )}
          </section>

          <section aria-labelledby={`${id}-colors`}>
            <h3 id={`${id}-colors`} className="panel-title">{t('preview.tonal.cal.colors')}</h3>
            <div className="tn-colors">
              <label className="tn-color">
                <input type="color" value={hexOf(colors.base)} onChange={(e) => setColor('base', e.target.value)} />
                {t('preview.tonal.cal.baseColor')}
              </label>
              <label className="tn-color">
                <input type="color" value={hexOf(colors.burn)} onChange={(e) => setColor('burn', e.target.value)} />
                {t('preview.tonal.cal.burnColor')}
              </label>
            </div>
          </section>

          <p className="hint num" aria-live="polite">
            {samples.length > 0 ? t('preview.tonal.cal.fit', { n: samples.length, dose: fmt(curve.dose_char, 3), gamma: fmt(curve.gamma, 2) }) : t('preview.tonal.cal.noMarks')}
          </p>
        </div>
      </div>

      <footer className="mat-modal__foot">
        <button type="button" className="btn" disabled={saved === null} onClick={reset}>{t('preview.tonal.cal.reset')}</button>
        <button type="button" className="btn" onClick={close}>{t('preview.tonal.cal.cancel')}</button>
        <button type="button" className="btn btn--accent" onClick={save}>{t('preview.tonal.cal.save')}</button>
      </footer>
    </Modal>
  );
}
