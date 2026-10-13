import { Camera, ScanLine } from 'lucide-react';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { updateCameraPrefs } from './actions';

/**
 * Imagen de la cama (M35): captura la foto corregida y alineada como imagen del documento, o la
 * vectoriza de una vez. También los ajustes del fondo que dependen de la imagen: escala de grises
 * y actualizar al terminar un trabajo (la opacidad está en la sección del fondo).
 */
export function CaptureSection() {
  const capture = useCommand('camera.capture');
  const vectorize = useCommand('camera.vectorize');
  const head = useStore((s) => s.cameraSetup.head?.enabled === true);
  const grayscale = useStore((s) => s.cameraSetup.prefs.grayscale === true);
  const onJob = useStore((s) => s.cameraSetup.prefs.update_on_job_finish === true);
  const frozen = useStore((s) => s.cameraFrozen);

  return (
    <section className="section" aria-label={t('camera.capture.title')}>
      <h3 className="panel-title">{t('camera.capture.title')}</h3>
      <p className="hint">{t(head ? 'camera.capture.helpHead' : 'camera.capture.help')}</p>
      <div className="row cam-wrap">
        <button type="button" className="btn btn--accent grow" aria-disabled={!capture.enabled} title={capture.enabled ? capture.tooltip : (capture.reason ?? undefined)} onClick={capture.enabled ? capture.run : undefined}>
          <Camera size={14} aria-hidden="true" />
          {t('camera.capture.button')}
        </button>
        <button type="button" className="btn grow" aria-disabled={!vectorize.enabled} title={vectorize.enabled ? vectorize.tooltip : (vectorize.reason ?? undefined)} onClick={vectorize.enabled ? vectorize.run : undefined}>
          <ScanLine size={14} aria-hidden="true" />
          {t('camera.capture.vectorize')}
        </button>
      </div>
      {!capture.enabled && capture.reason && <p className="hint">{capture.reason}</p>}
      <p className="hint">{t('camera.capture.vectorizeHint')}</p>

      <label className="mc-check">
        <input type="checkbox" checked={grayscale} onChange={(e) => updateCameraPrefs({ grayscale: e.target.checked })} />
        {t('camera.overlay.grayscale')}
      </label>
      <p className="hint">{t('camera.overlay.grayscaleHint')}</p>
      <label className="mc-check">
        <input type="checkbox" checked={onJob} onChange={(e) => updateCameraPrefs({ update_on_job_finish: e.target.checked })} />
        {t('camera.overlay.onJob')}
      </label>
      <p className="hint">{t(head ? 'camera.overlay.onJobHintHead' : 'camera.overlay.onJobHint')}</p>
      {onJob && !head && !frozen && <p className="hint">{t('camera.overlay.onJobNeedsFreeze')}</p>}
    </section>
  );
}
