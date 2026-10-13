import { Camera, CameraOff, Snowflake } from 'lucide-react';
import { useEffect } from 'react';
import { useCommand } from '@/commands/useCommand';
import { t, type I18nKey } from '@/i18n';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { RESOLUTIONS } from './capture';
import { AlignmentSection, ViewSection, useAlignmentEditor } from './AlignmentSection';
import { clearLens, loadCameraSetup, updateCameraPrefs } from './actions';
import { rateRms } from './coverage';
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
      <ViewSection editor={editor} />
      <BackdropSection />
      <LensSection />
      <AlignmentSection editor={editor} />
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

  return (
    <section className="section" aria-label={t('camera.source.title')}>
      <h3 className="panel-title">{t('camera.source.title')}</h3>
      <div className="field-grid">
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
      </div>
      <div className="row">
        <button type="button" className={`btn grow${status === 'off' ? ' btn--accent' : ''}`} onClick={toggle.run} disabled={status === 'starting'}>
          {status === 'off' ? <Camera size={14} aria-hidden="true" /> : <CameraOff size={14} aria-hidden="true" />}
          {status === 'starting' ? t('camera.source.starting') : toggle.label}
        </button>
      </div>
      {status === 'live' && size && <p className="hint num">{t('camera.source.stream', { w: size[0], h: size[1] })}</p>}
      {problem && <p className="hint hint--danger" role="alert">{t(`camera.problem.${problem}` as I18nKey)}</p>}
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
  // Se vuelve a evaluar al cambiar alguno de estos datos.
  void calibration;
  void size;
  const stale = alignment !== null && size !== null && !alignmentIsCurrent();

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
