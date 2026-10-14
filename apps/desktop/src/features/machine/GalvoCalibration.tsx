import { Ruler, TriangleAlert } from 'lucide-react';
import { useId, useState, type ReactNode } from 'react';
import { getApi } from '@/api';
import type { DeviceProfile } from '@/api/types';
import type { FieldCorrection } from '@/bindings/FieldCorrection';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { isJobBusy } from '@/store/selectors';
import { NumInput } from './DeviceFields';
import { buildCalibDocument, calibJobOptions, calibSizes, defaultCalibMark, fitCorrection, type CalibMark, type CalibMeasures, type FitResult } from './galvoCalib';
import { GalvoCalibration9Dialog } from './GalvoCalibration9';
import { GalvoExperimentalBadge } from './GalvoNotice';
import { SOURCE_WAVELENGTH_NM } from './galvoPorts';
import { Modal } from './Modal';

export { defaultCalibMark };

type Step = 'mark' | 'measure' | 'result';

/** Por qué no se puede marcar ahora la rejilla (clave de texto), o `null` si se puede. */
export function calibBlockedReason(s: { connected: boolean; activeId: string; draftId: string; dirty: boolean; busy: boolean; isGalvo: boolean }): 'galvo.calib.need.connect' | 'galvo.calib.need.active' | 'galvo.calib.need.saved' | 'galvo.calib.need.idle' | null {
  if (!s.connected || !s.isGalvo) return 'galvo.calib.need.connect';
  if (s.activeId !== s.draftId) return 'galvo.calib.need.active';
  if (s.dirty) return 'galvo.calib.need.saved';
  if (s.busy) return 'galvo.calib.need.idle';
  return null;
}

/**
 * Botón del perfil galvo que abre el asistente de calibración del campo. Hace falta una tarjeta conectada con
 * ESTE perfil, guardado (la rejilla se marca con la corrección que usa la conexión) y sin trabajo en curso.
 */
export function GalvoCalibrationButton({ draft, dirty, onApply }: { draft: DeviceProfile; dirty: boolean; onApply: (c: FieldCorrection) => void }) {
  const connected = useStore((s) => s.connection.connected);
  const activeId = useStore((s) => s.activeDeviceId);
  const busy = useStore(isJobBusy);
  const [open, setOpen] = useState(false);
  const [open9, setOpen9] = useState(false);
  const why = calibBlockedReason({ connected, activeId: activeId ?? '', draftId: draft.id, dirty, busy, isGalvo: draft.kind === 'galvo' });
  return (
    <div className="mc-galvo-calib">
      <button type="button" className="btn" disabled={why !== null} title={why ? t(why) : t('galvo.calib.open.help')} onClick={() => setOpen(true)}>
        <Ruler size={14} aria-hidden="true" />
        {t('galvo.calib.open')}
      </button>
      <button type="button" className="btn" disabled={why !== null} title={why ? t(why) : t('galvo.calib9.open.help')} onClick={() => setOpen9(true)}>
        <Ruler size={14} aria-hidden="true" />
        {t('galvo.calib9.open')}
      </button>
      {why && <p className="hint">{t(why)}</p>}
      {open9 && (
        <GalvoCalibration9Dialog
          device={draft}
          onClose={() => setOpen9(false)}
          onApply={(c) => {
            onApply(c);
            setOpen9(false);
          }}
        />
      )}
      {open && (
        <GalvoCalibrationDialog
          device={draft}
          onClose={() => setOpen(false)}
          onApply={(c) => {
            onApply(c);
            setOpen(false);
          }}
        />
      )}
    </div>
  );
}

function Pair({ label, help, value, onChange }: { label: string; help: string; value: [number, number]; onChange: (v: [number, number]) => void }) {
  const id = useId();
  return (
    <div className="mc-field">
      <span id={id} className="field__label">{label}</span>
      <div className="field-grid" role="group" aria-labelledby={id}>
        <NumInput label={t('galvo.calib.width')} unit="mm" value={value[0]} onCommit={(v) => onChange([v, value[1]])} />
        <NumInput label={t('galvo.calib.height')} unit="mm" value={value[1]} onCommit={(v) => onChange([value[0], v])} />
      </div>
      <p className="hint mc-help">{help}</p>
    </div>
  );
}

/** Resultado del ajuste: antes y después, para ver qué se aplicaría. */
export function CalibResultView({ fit, current }: { fit: FitResult; current: Pick<FieldCorrection, 'scale' | 'bulge'> }) {
  const f = (v: number, d = 4) => fmt(v, d);
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
          <tr>
            <th scope="row">{t('galvo.calib.scaleX')}</th>
            <td className="num">{f(current.scale[0], 5)}</td>
            <td className="num mc-new">{f(fit.scale[0], 5)}</td>
          </tr>
          <tr>
            <th scope="row">{t('galvo.calib.scaleY')}</th>
            <td className="num">{f(current.scale[1], 5)}</td>
            <td className="num mc-new">{f(fit.scale[1], 5)}</td>
          </tr>
          <tr>
            <th scope="row">{t('galvo.calib.bulgeX')}</th>
            <td className="num">{f(current.bulge[0], 5)}</td>
            <td className="num mc-new">{f(fit.bulge[0], 5)}</td>
          </tr>
          <tr>
            <th scope="row">{t('galvo.calib.bulgeY')}</th>
            <td className="num">{f(current.bulge[1], 5)}</td>
            <td className="num mc-new">{f(fit.bulge[1], 5)}</td>
          </tr>
        </tbody>
      </table>
      <p className="hint num">
        {t('galvo.calib.errors', {
          ox: f(fit.errorOuter[0], 2),
          oy: f(fit.errorOuter[1], 2),
          ix: f(fit.errorInner[0], 2),
          iy: f(fit.errorInner[1], 2),
        })}
      </p>
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
 * Asistente de calibración del campo (M43): 1) marca una rejilla de prueba; 2) la mides con una regla; 3) ves la escala
 * y el barril que corrigen el error y los aplicas al perfil. EXPERIMENTAL: el modelo es propio y no está verificado
 * con una lente real.
 */
export function GalvoCalibrationDialog({ device, onClose, onApply }: { device: DeviceProfile; onClose: () => void; onApply: (c: FieldCorrection) => void }) {
  const g = device.galvo;
  const { outer, inner } = calibSizes(g.field_mm);
  const [step, setStep] = useState<Step>('mark');
  const [mark, setMark] = useState<CalibMark>(() => defaultCalibMark(device));
  const [measures, setMeasures] = useState<CalibMeasures>(() => ({ outer: [outer, outer], inner: [inner, inner] }));
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const jobState = useStore((s) => s.jobState);
  const jobOptions = useStore((s) => s.jobOptions);
  const nm = SOURCE_WAVELENGTH_NM[g.source];
  const fit = fitCorrection(g.field_mm, g.correction, measures);

  const markGrid = async () => {
    setError(null);
    try {
      await getApi().startJob({ doc: buildCalibDocument(device.bed, g.field_mm, mark), device, options: calibJobOptions(jobOptions) });
      setSent(true);
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const marking = jobState === 'running';
  const done = sent && jobState === 'finished';

  let body: ReactNode;
  let footer: ReactNode;
  if (step === 'mark') {
    body = (
      <>
        <p>{t('galvo.calib.mark.text', { outer: fmt(outer, 1), inner: fmt(inner, 1), field: fmt(g.field_mm, 0) })}</p>
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
        <p>{t('galvo.calib.measure.text')}</p>
        <Pair label={t('galvo.calib.outer', { mm: fmt(outer, 1) })} help={t('galvo.calib.measure.help')} value={measures.outer} onChange={(v) => setMeasures({ ...measures, outer: v })} />
        <Pair label={t('galvo.calib.inner', { mm: fmt(inner, 1) })} help={t('galvo.calib.measure.help')} value={measures.inner} onChange={(v) => setMeasures({ ...measures, inner: v })} />
        <p className="hint">{t('galvo.calib.measure.xmark')}</p>
        {typeof fit === 'string' && (
          <p className="hint hint--danger" role="alert">{t(fit === 'invalid' ? 'galvo.calib.err.invalid' : 'galvo.calib.err.implausible')}</p>
        )}
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
          <p>{t('galvo.calib.result.text')}</p>
          <CalibResultView fit={fit} current={g.correction} />
          <p className="hint">{t('galvo.calib.result.reconnect')}</p>
          <p className="hint">{t('galvo.calib.result.iterate')}</p>
        </>
      );
    footer = (
      <>
        <button type="button" className="btn" onClick={() => setStep('measure')}>{t('catalog.back')}</button>
        <span className="topbar__spacer" />
        <button
          type="button"
          className="btn btn--accent"
          disabled={typeof fit === 'string'}
          onClick={() => typeof fit !== 'string' && onApply({ ...g.correction, scale: fit.scale, bulge: fit.bulge })}
        >
          {t('galvo.calib.apply')}
        </button>
      </>
    );
  }
  return (
    <Modal title={t('galvo.calib.title')} subtitle={t(`galvo.calib.step.${step}`)} size="md" tone="warn" onClose={onClose} footer={footer}>
      <p className="hint"><GalvoExperimentalBadge /> {t('galvo.calib.unverified')}</p>
      {body}
    </Modal>
  );
}
