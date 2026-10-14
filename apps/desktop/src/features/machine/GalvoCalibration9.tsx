import { TriangleAlert } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { getApi } from '@/api';
import type { DeviceProfile } from '@/api/types';
import type { FieldCorrection } from '@/bindings/FieldCorrection';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { NumInput } from './DeviceFields';
import { calibJobOptions, defaultCalibMark, type CalibMark } from './galvoCalib';
import { buildGrid9Document, fitNinePoints, grid9Points, type Fit9Result, type Pt } from './galvoCalib9';
import { GalvoExperimentalBadge } from './GalvoNotice';
import { SOURCE_WAVELENGTH_NM } from './galvoPorts';
import { Modal } from './Modal';

type Step = 'mark' | 'measure' | 'result';

/** Antes y después de los siete parámetros que ajusta el asistente, más el error por cruz. */
export function Calib9ResultView({ fit, current }: { fit: Fit9Result; current: FieldCorrection }) {
  const f = (v: number, d = 5) => fmt(v, d);
  const n = fit.correction;
  const rows: [string, number, number][] = [
    [t('galvo.calib.scaleX'), current.scale[0], n.scale[0]],
    [t('galvo.calib.scaleY'), current.scale[1], n.scale[1]],
    [t('galvo.calib9.rotation'), current.rotation_deg, n.rotation_deg],
    [t('galvo.calib.bulgeX'), current.bulge[0], n.bulge[0]],
    [t('galvo.calib.bulgeY'), current.bulge[1], n.bulge[1]],
    [t('galvo.calib9.trapX'), current.trapezoid[0], n.trapezoid[0]],
    [t('galvo.calib9.trapY'), current.trapezoid[1], n.trapezoid[1]],
  ];
  return (
    <>
      <table className="mc-table" aria-label={t('galvo.calib.result')}>
        <thead>
          <tr>
            <th scope="col">{t('galvo.calib.param')}</th>
            <th scope="col">{t('devices.kind.dlg.current')}</th>
            <th scope="col">{t('devices.kind.dlg.new')}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map(([label, before, after]) => (
            <tr key={label}>
              <th scope="row">{label}</th>
              <td className="num">{f(before)}</td>
              <td className="num mc-new">{f(after)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="hint num">{t('galvo.calib9.errors', { before: f(fit.maxBefore, 3), after: f(fit.maxAfter, 3) })}</p>
      <table className="mc-table" aria-label={t('galvo.calib9.points')}>
        <caption className="hint">{t('galvo.calib9.points')}</caption>
        <tbody>
          {[0, 1, 2].map((row) => (
            <tr key={row}>
              {[0, 1, 2].map((col) => {
                const e = fit.errorsBefore[row * 3 + col]!;
                return (
                  <td key={col} className="num">
                    {f(e[0], 2)}; {f(e[1], 2)}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
      {fit.clamped && (
        <p className="warn-line" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('galvo.calib.clamped')}</span>
        </p>
      )}
    </>
  );
}

/**
 * Asistente de nueve puntos (M43): 1) marca una cuadrícula de 3 × 3 cruces; 2) mides dónde cayó cada una; 3) ves la
 * escala, el giro, el barril y el trapecio que corrigen el error y los aplicas al perfil. Modelo propio, sin verificar
 * con una lente real (ver `galvoCalib9.ts`).
 */
export function GalvoCalibration9Dialog({ device, onClose, onApply }: { device: DeviceProfile; onClose: () => void; onApply: (c: FieldCorrection) => void }) {
  const g = device.galvo;
  const nominal = grid9Points(g.field_mm);
  const side = g.field_mm * 0.35;
  const [step, setStep] = useState<Step>('mark');
  const [mark, setMark] = useState<CalibMark>(() => defaultCalibMark(device));
  const [measured, setMeasured] = useState<Pt[]>(() => nominal.map((p): Pt => [Math.round(p[0] * 1000) / 1000, Math.round(p[1] * 1000) / 1000]));
  const [rotation, setRotation] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const jobState = useStore((s) => s.jobState);
  const jobOptions = useStore((s) => s.jobOptions);
  const nm = SOURCE_WAVELENGTH_NM[g.source];
  const fit = fitNinePoints(g.field_mm, g.correction, measured, rotation);

  const markGrid = async () => {
    setError(null);
    try {
      await getApi().startJob({ doc: buildGrid9Document(device.bed, g.field_mm, mark), device, options: calibJobOptions(jobOptions) });
      setSent(true);
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const marking = jobState === 'running';
  const done = sent && jobState === 'finished';
  const setPoint = (i: number, axis: 0 | 1, v: number) => setMeasured(measured.map((p, k): Pt => (k === i ? (axis === 0 ? [v, p[1]] : [p[0], v]) : p)));

  let body: ReactNode;
  let footer: ReactNode;
  if (step === 'mark') {
    body = (
      <>
        <p>{t('galvo.calib9.mark.text', { side: fmt(side * 2, 1), half: fmt(side, 1), field: fmt(g.field_mm, 0) })}</p>
        <p className="warn-line">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('galvo.calib.mark.warn', { nm, watts: fmt(g.watts, 0) })}</span>
        </p>
        <div className="field-grid">
          <div className="mc-field">
            <span className="field__label">{t('galvo.calib.power')}</span>
            <NumInput label={t('galvo.calib.power')} unit="%" value={mark.power} onCommit={(v) => setMark({ ...mark, power: Math.min(Math.max(v, 0.1), g.max_power_pct) })} />
          </div>
          <div className="mc-field">
            <span className="field__label">{t('galvo.calib.speed')}</span>
            <NumInput label={t('galvo.calib.speed')} unit="mm/s" value={mark.speedMmS} onCommit={(v) => setMark({ ...mark, speedMmS: Math.max(v, 1) })} />
          </div>
          <div className="mc-field">
            <span className="field__label">{t('galvo.calib.freq')}</span>
            <NumInput label={t('galvo.calib.freq')} unit="kHz" value={mark.frequencyKhz} onCommit={(v) => setMark({ ...mark, frequencyKhz: Math.min(Math.max(v, g.freq_min_khz), g.freq_max_khz) })} />
          </div>
        </div>
        <p className="hint">{t('galvo.calib.mark.current')}</p>
        {error && <p className="hint hint--danger" role="alert">{error}</p>}
        {sent && <p className="hint" role="status">{done ? t('galvo.calib.mark.done') : marking ? t('galvo.calib.mark.running') : t('galvo.calib.mark.sent')}</p>}
      </>
    );
    footer = (
      <>
        <button type="button" className="btn" onClick={onClose}>{t('machine.dlg.cancel')}</button>
        <span className="topbar__spacer" />
        <button type="button" className="btn btn--danger" disabled={marking} onClick={() => void markGrid()}>{t('galvo.calib.mark.go')}</button>
        <button type="button" className="btn btn--accent" disabled={marking} title={sent ? undefined : t('galvo.calib.mark.skip')} onClick={() => setStep('measure')}>{t(sent ? 'galvo.calib.next' : 'galvo.calib.haveGrid')}</button>
      </>
    );
  } else if (step === 'measure') {
    body = (
      <>
        <p>{t('galvo.calib9.measure.text')}</p>
        <div className="mc-galvo-grid" role="group" aria-label={t('galvo.calib9.title')}>
          {measured.map((p, i) =>
            i === 4 ? (
              <p key={i} className="hint">{t('galvo.calib9.center')}</p>
            ) : (
              <fieldset key={i} className="mc-group">
                <legend>{t('galvo.calib9.point', { n: i + 1 })}</legend>
                <NumInput label="X" unit="mm" value={p[0]} onCommit={(v) => setPoint(i, 0, v)} />
                <NumInput label="Y" unit="mm" value={p[1]} onCommit={(v) => setPoint(i, 1, v)} />
              </fieldset>
            ),
          )}
        </div>
        <label className="mc-check">
          <input type="checkbox" checked={rotation} onChange={(e) => setRotation(e.target.checked)} />
          <span>{t('galvo.calib9.measure.rotation')}</span>
        </label>
        <p className="hint">{t('galvo.calib9.measure.rotation.help')}</p>
        {typeof fit === 'string' && <p className="hint hint--danger" role="alert">{t(`galvo.calib9.err.${fit}`)}</p>}
      </>
    );
    footer = (
      <>
        <button type="button" className="btn" onClick={() => setStep('mark')}>{t('catalog.back')}</button>
        <span className="topbar__spacer" />
        <button type="button" className="btn btn--accent" disabled={typeof fit === 'string'} onClick={() => setStep('result')}>{t('galvo.calib.compute')}</button>
      </>
    );
  } else {
    body =
      typeof fit === 'string' ? null : (
        <>
          <p>{t('galvo.calib9.result.text')}</p>
          <Calib9ResultView fit={fit} current={g.correction} />
          {g.correction.cor_file && <p className="hint">{t('galvo.calib9.cor')}</p>}
          <p className="hint">{t('galvo.calib.result.reconnect')}</p>
          <p className="hint">{t('galvo.calib.result.iterate')}</p>
        </>
      );
    footer = (
      <>
        <button type="button" className="btn" onClick={() => setStep('measure')}>{t('catalog.back')}</button>
        <span className="topbar__spacer" />
        <button type="button" className="btn btn--accent" disabled={typeof fit === 'string'} onClick={() => typeof fit !== 'string' && onApply(fit.correction)}>
          {t('galvo.calib.apply')}
        </button>
      </>
    );
  }
  return (
    <Modal title={t('galvo.calib9.title')} subtitle={t(`galvo.calib.step.${step}`)} size="md" tone="warn" onClose={onClose} footer={footer}>
      <p className="hint"><GalvoExperimentalBadge /> {t('galvo.calib.unverified')}</p>
      {body}
    </Modal>
  );
}
