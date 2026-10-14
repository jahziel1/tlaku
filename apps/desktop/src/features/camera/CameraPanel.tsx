import { Camera, CameraOff, Snowflake } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useCommand } from '@/commands/useCommand';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { NumberField } from '@/ui/NumberField';
import { RESOLUTIONS } from './capture';
import { AlignmentSection, ViewSection, useAlignmentEditor } from './AlignmentSection';
import { AdjustSection } from './AdjustSection';
import { CaptureSection } from './CaptureSection';
import { CenterSection } from './CenterSection';
import { HeadConfirm, HeadSection } from './HeadSection';
import { NetworkUrlField, PresetsSection, SourceKindField } from './NetworkSection';
import { clearLens, loadCameraSetup, updateCameraPrefs } from './actions';
import { rateRms } from './coverage';
import { heightStatus, maxShiftMm } from './height';
import { dropHeight, openMarksAsNewDocument } from './marks';
import { openMarksDialog, useMarks, type MarkCount } from './marksState';
import { alignmentIsCurrent, lensApplies, refreshDevices } from './session';
import './camera.css';

/** Pestaña Cámara: fuente de video, fondo del lienzo, lente y alineación con la cama. */
export function CameraPanel() {
  useEffect(() => {
    void loadCameraSetup();
    void refreshDevices();
  }, []);
  const editor = useAlignmentEditor();
  return (
    <div className="panel-scroll cam-panel">
      <SourceSection />
      <PresetsSection />
      <ViewSection editor={editor} />
      <HeadConfirm />
      <BackdropSection />
      <AdjustSection />
      <CaptureSection />
      <LensSection />
      <MarksSection />
      <HeightSection />
      <AlignmentSection editor={editor} />
      <HeadSection />
      <CenterSection />
    </div>
  );
}

function SourceSection() {
  const status = useStore((s) => s.cameraStatus);
  const problem = useStore((s) => s.cameraProblem);
  const devices = useStore((s) => s.cameraDevices);
  const prefs = useStore((s) => s.cameraSetup.prefs);
  const size = useStore((s) => s.cameraStreamSize);
  const toggle = useCommand('camera.toggle');
  const resolution = prefs.width && prefs.height ? `${prefs.width}x${prefs.height}` : '';
  const known = devices.some((d) => d.id === prefs.device_id);
  const network = prefs.source === 'network';
  const detail = useStore((s) => s.cameraProblemDetail);

  return (
    <section className="section" aria-label={t('camera.source.title')}>
      <h3 className="panel-title">{t('camera.source.title')}</h3>
      <SourceKindField />
      {network && <NetworkUrlField />}
      {!network && <div className="field-grid">
        <div className="field">
          <label className="field__label" htmlFor="cam-device">{t('camera.source.device')}</label>
          <select
            id="cam-device"
            className="select"
            value={known ? (prefs.device_id ?? '') : ''}
            onChange={(e) => {
              const d = devices.find((x) => x.id === e.target.value);
              updateCameraPrefs({ device_id: d?.id ?? null, device_label: d?.label ?? null });
            }}
          >
            <option value="">{t('camera.source.default')}</option>
            {devices.map((d) => <option key={d.id} value={d.id}>{d.label}</option>)}
          </select>
        </div>
        <div className="field">
          <label className="field__label" htmlFor="cam-res">{t('camera.source.resolution')}</label>
          <select
            id="cam-res"
            className="select"
            value={resolution}
            onChange={(e) => {
              const [w, h] = e.target.value.split('x').map(Number);
              updateCameraPrefs(w && h ? { width: w, height: h } : { width: null, height: null });
            }}
          >
            <option value="">{t('camera.source.auto')}</option>
            {RESOLUTIONS.map((r) => <option key={r.width} value={`${r.width}x${r.height}`}>{r.width} × {r.height}</option>)}
          </select>
        </div>
      </div>}
      <div className="row">
        <button type="button" className={`btn grow${status === 'off' ? ' btn--accent' : ''}`} onClick={toggle.run} disabled={status === 'starting'}>
          {status === 'off' ? <Camera size={14} aria-hidden="true" /> : <CameraOff size={14} aria-hidden="true" />}
          {status === 'starting' ? t('camera.source.starting') : toggle.label}
        </button>
      </div>
      {status === 'live' && size && <p className="hint num">{t('camera.source.stream', { w: size[0], h: size[1] })}</p>}
      {problem && <p className="hint hint--danger" role="alert">{t(`camera.problem.${problem}` as I18nKey)}</p>}
      {problem === 'network' && detail && <p className="hint hint--danger">{detail}</p>}
    </section>
  );
}

function BackdropSection() {
  const show = useCommand('camera.backdrop');
  const freeze = useCommand('camera.freeze');
  const opacity = useStore((s) => s.cameraSetup.prefs.opacity);
  const alignment = useStore((s) => s.cameraSetup.alignment);
  const calibration = useStore((s) => s.cameraSetup.calibration);
  const size = useStore((s) => s.cameraStreamSize);
  const frozen = useStore((s) => s.cameraFrozen);
  const headMode = useStore((s) => s.cameraSetup.head?.enabled === true);
  // Se vuelve a evaluar al cambiar alguno de estos datos.
  void calibration;
  void size;
  const stale = !headMode && alignment !== null && size !== null && !alignmentIsCurrent();

  return (
    <section className="section" aria-label={t('camera.backdrop.title')}>
      <h3 className="panel-title">{t('camera.backdrop.title')}</h3>
      <label className="mc-check" title={show.tooltip}>
        <input type="checkbox" checked={show.checked} disabled={!show.enabled} onChange={show.run} />
        {t('camera.backdrop.show')}
      </label>
      {!show.enabled && show.reason && <p className="hint">{show.reason}</p>}
      <div className="cam-opacity">
        <label htmlFor="cam-opacity" className="field__label">{t('camera.backdrop.opacity')}</label>
        <input
          id="cam-opacity"
          type="range"
          className="range"
          min={0}
          max={100}
          step={5}
          value={Math.round(opacity * 100)}
          onChange={(e) => updateCameraPrefs({ opacity: Number(e.target.value) / 100 }, 300)}
        />
        <span className="num cam-opacity__value">{Math.round(opacity * 100)} %</span>
      </div>
      <button type="button" className="btn" aria-pressed={frozen} aria-disabled={!freeze.enabled} title={freeze.tooltip} onClick={freeze.enabled ? freeze.run : undefined}>
        <Snowflake size={14} aria-hidden="true" />
        {frozen ? t('camera.backdrop.unfreeze') : t('camera.backdrop.freeze')}
      </button>
      {frozen && <p className="hint">{t('camera.backdrop.frozenHint')}</p>}
      {stale && alignment && <p className="warn-line" role="alert">{t('camera.backdrop.stale', { w: alignment.image_size[0], h: alignment.image_size[1] })}</p>}
    </section>
  );
}

function LensSection() {
  const calibration = useStore((s) => s.cameraSetup.calibration);
  const size = useStore((s) => s.cameraStreamSize);
  const open = useCommand('camera.calibrate');
  void size;
  const mismatch = calibration !== null && size !== null && !lensApplies();
  const grade = calibration ? rateRms(calibration.rms_error_px) : null;

  return (
    <section className="section" aria-label={t('camera.lens.title')}>
      <h3 className="panel-title">{t('camera.lens.title')}</h3>
      {calibration && grade ? (
        <>
          <p className="hint" data-grade={grade}>
            {t('camera.lens.summary', { views: calibration.views_used, rms: fmt(calibration.rms_error_px, 2), grade: t(`camera.grade.${grade}` as I18nKey) })}
          </p>
          <p className="hint num">{calibration.width} × {calibration.height} px</p>
        </>
      ) : (
        <p className="hint">{t('camera.lens.none')}</p>
      )}
      {mismatch && calibration && <p className="warn-line" role="alert">{t('camera.lens.mismatch', { w: calibration.width, h: calibration.height })}</p>}
      <div className="row cam-wrap">
        <button type="button" className="btn grow" onClick={open.run}>{calibration ? t('camera.lens.recalibrate') : t('camera.lens.calibrate')}</button>
        {calibration && <button type="button" className="btn" onClick={clearLens}>{t('camera.lens.clear')}</button>}
      </div>
    </section>
  );
}

/** Alinear con cruces grabadas: generar el documento, capturar, detectar y medir la precisión. */
function MarksSection() {
  const count = useMarks((s) => s.count);
  const setCount = useMarks((s) => s.setCount);
  const alignment = useStore((s) => s.cameraSetup.alignment);
  const [opening, setOpening] = useState(false);
  const samples = alignment?.method === 'marks' ? (alignment.samples ?? []) : [];

  const generate = async () => {
    setOpening(true);
    try {
      await openMarksAsNewDocument(count);
    } finally {
      setOpening(false);
    }
  };

  return (
    <section className="section" aria-label={t('camera.marks.title')}>
      <h3 className="panel-title">{t('camera.marks.title')}</h3>
      <p className="hint">{t('camera.marks.help')}</p>
      <div className="field">
        <label className="field__label" htmlFor="cam-marks-count">{t('camera.marks.count')}</label>
        <select id="cam-marks-count" className="select" value={count} onChange={(e) => setCount(Number(e.target.value) as MarkCount)}>
          <option value={9}>{t('camera.marks.count9')}</option>
          <option value={4}>{t('camera.marks.count4')}</option>
        </select>
      </div>
      <button type="button" className="btn" disabled={opening} onClick={() => void generate()}>{t('camera.marks.generate')}</button>
      <p className="hint">{t('camera.marks.generateHint')}</p>
      <button type="button" className="btn btn--accent" onClick={openMarksDialog}>{t('camera.marks.open')}</button>
      {alignment && samples.length > 0 && (
        <>
          <p className="hint cam-ok" role="status">
            {t('camera.marks.heights', { list: samples.map((x) => fmt(x.height_mm, 1)).join(', ') })}
          </p>
          <ul className="cam-heights" aria-label={t('camera.marks.heights', { list: '' })}>
            {samples.map((x) => (
              <li key={x.height_mm}>
                <button type="button" className="chip" onClick={() => dropHeight(alignment, x.height_mm)} title={t('camera.marks.removeHeight', { h: fmt(x.height_mm, 1) })}>
                  {fmt(x.height_mm, 1)} mm ×
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  );
}

/** Grosor del material: el fondo de la cámara se corrige a esa altura. */
function HeightSection() {
  const alignment = useStore((s) => s.cameraSetup.alignment);
  const thickness = useStore((s) => s.cameraSetup.prefs.material_height_mm ?? 0);
  const status = heightStatus(alignment, thickness);
  const ref = alignment?.reference_height_mm ?? 0;
  const shift = alignment && thickness !== ref ? maxShiftMm(alignment, thickness) : 0;

  return (
    <section className="section" aria-label={t('camera.height.title')}>
      <h3 className="panel-title">{t('camera.height.title')}</h3>
      <NumberField
        label={t('camera.height.thickness')}
        unit="mm"
        value={thickness}
        min={0}
        max={300}
        step={0.5}
        disabled={!alignment}
        onCommit={(v) => updateCameraPrefs({ material_height_mm: v })}
      />
      <p className="hint">{t('camera.height.help')}</p>
      {!alignment && <p className="hint">{t('camera.height.noAlignment')}</p>}
      {alignment && status.kind === 'none' && <p className="hint">{t('camera.height.none', { ref: fmt(ref, 1) })}</p>}
      {alignment && status.kind === 'modelled' && <p className="hint">{t('camera.height.modelled')}</p>}
      {alignment && status.kind === 'measured' && (
        <p className="hint cam-ok">{t('camera.height.measured', { list: (alignment.samples ?? []).map((x) => fmt(x.height_mm, 1)).join(', ') })}</p>
      )}
      {status.kind === 'measured' && status.outside > 0 && (
        <p className="warn-line" role="alert">
          {t('camera.height.outside', { mm: fmt(status.outside, 1), min: fmt(status.min, 1), max: fmt(status.max, 1) })}
        </p>
      )}
      {alignment && alignment.height && shift > 0.05 && (
        <p className="hint num" role="status">{t('camera.height.shift', { mm: fmt(shift, 1), ref: fmt(ref, 1) })}</p>
      )}
    </section>
  );
}
