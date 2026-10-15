import { Palette, TriangleAlert } from 'lucide-react';
import { useId, useMemo } from 'react';
import { useMaterials } from '@/features/materials/state';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { selectActiveDevice } from '@/store/selectors';
import { laserWattsOf } from '@/lib/device';
import { NumberField } from '@/ui/NumberField';
import { CalibrateDialog } from './CalibrateDialog';
import { curveFor, hexOf, SPOT_RANGE, TONE_MATERIALS } from './curve';
import { useTonal } from './store';
import { useTonalSync } from './service';
import './tonal.css';

/** Nombre traducido de un material de la vista por tonos. */
export const toneMaterialName = (id: string): string => t(`preview.tonal.material.${id}` as I18nKey);

/**
 * Fila de la barra de la vista previa para elegir el aspecto («Normal» / «Por tonos»), el material de la
 * simulación, el ancho del punto del láser y calibrar la curva. Mantiene las imágenes de la simulación
 * mientras la barra está en pantalla.
 */
export function TonalControls() {
  useTonalSync();
  const id = useId();
  const mode = useTonal((s) => s.mode);
  const material = useTonal((s) => s.material);
  const spotMm = useTonal((s) => s.spotMm);
  const status = useTonal((s) => s.status);
  const error = useTonal((s) => s.error);
  const calibrateOpen = useTonal((s) => s.calibrateOpen);
  const { setMode, setMaterial, setSpot, setCalibrateOpen } = useTonal.getState();
  const tones = useMaterials((s) => s.library.tones);
  const hasPreview = useStore((s) => s.preview !== null);
  const empty = useStore((s) => s.preview !== null && s.preview.moves.length === 0);
  const stale = useStore((s) => s.previewStale);
  const watts = useStore((s) => laserWattsOf(selectActiveDevice(s)));
  const curve = useMemo(() => curveFor(material, tones, watts), [material, tones, watts]);
  const calibrated = tones?.some((c) => c.material === material) ?? false;
  const tonal = mode === 'tonal';

  return (
    <div className="tn">
      <div className="tn__row">
        <div className="tn__seg" role="group" aria-label={t('preview.tonal.group')} title={t('preview.tonal.modeHint')}>
          <button type="button" className="tn__opt" aria-pressed={!tonal} onClick={() => setMode('normal')}>
            {t('preview.tonal.normal')}
          </button>
          <button type="button" className="tn__opt" aria-pressed={tonal} onClick={() => setMode('tonal')}>
            {t('preview.tonal.tonal')}
          </button>
        </div>
        {tonal && (
          <>
            <div className="tn__field">
              <label htmlFor={`${id}-mat`}>{t('preview.tonal.material')}</label>
              <select id={`${id}-mat`} className="select" value={material} onChange={(e) => setMaterial(e.target.value)}>
                {TONE_MATERIALS.map((m) => (
                  <option key={m} value={m}>{toneMaterialName(m)}</option>
                ))}
              </select>
            </div>
            <span title={t('preview.tonal.spotHint')}>
              <NumberField label={t('preview.tonal.spot')} value={spotMm} min={SPOT_RANGE.min} max={SPOT_RANGE.max} step={SPOT_RANGE.step} unit="mm" onCommit={setSpot} />
            </span>
            <div
              className="tn__scale"
              role="img"
              aria-label={`${t('preview.tonal.legend')}: ${t('preview.tonal.legend.none')} - ${t('preview.tonal.legend.max')}`}
              style={{ ['--tn-from' as string]: hexOf(curve.base), ['--tn-to' as string]: hexOf(curve.burn) }}
            >
              <span className="tn__scale-label">{t('preview.tonal.legend.none')}</span>
              <span className="tn__scale-bar" aria-hidden="true" />
              <span className="tn__scale-label">{t('preview.tonal.legend.max')}</span>
            </div>
            <button type="button" className="btn tn__cal" title={t('preview.tonal.calibrateHint')} onClick={() => setCalibrateOpen(true)}>
              <Palette size={14} aria-hidden="true" />
              {t('preview.tonal.calibrate')}
              {calibrated && <span className="tn__badge">{t('preview.tonal.calibrated')}</span>}
            </button>
          </>
        )}
      </div>
      {tonal && (
        <>
          <p className="hint tn__approx" role="note">{t('preview.tonal.approx')}</p>
          <div aria-live="polite">
            {status === 'error' && error && (
              <p className="warn-line" role="alert">
                <TriangleAlert size={14} aria-hidden="true" />
                <span>{t('preview.tonal.error', { error })}</span>
              </p>
            )}
            {status === 'busy' && <p className="hint" role="status">{t('preview.tonal.computing')}</p>}
            {empty && <p className="hint" role="status">{t('preview.tonal.empty')}</p>}
            {!empty && status !== 'busy' && status !== 'error' && (!hasPreview || stale) && <p className="hint" role="status">{t('preview.tonal.waiting')}</p>}
          </div>
        </>
      )}
      {calibrateOpen && <CalibrateDialog />}
    </div>
  );
}
