import { PanelSection } from '@/ui/PanelSection';
import { Crosshair, LocateFixed } from 'lucide-react';
import { useStore } from '@/store';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { moveHeadToCenter, setOriginAtCenter, useCenter } from './centerFinder';
import { useViewTool } from './headState';
import { currentMapping, MAPPING_PROBLEM_KEY } from './mapping';
import { positionLaserBlockedReason } from '@/store/machine';

/** Buscador de centro (M48): centro de un objeto redondo con la cámara. */
export function CenterSection() {
  const tool = useViewTool((s) => s.tool);
  const setTool = useViewTool((s) => s.setTool);
  const { mode, picks, result, busy, error, setMode, reset } = useCenter();
  const mapProblem = useStore((s) => {
    const m = currentMapping(s);
    return m.ok ? null : m.reason;
  });
  const blocked = useStore((s) => positionLaserBlockedReason(s));
  const active = tool === 'center';
  const mapReason = mapProblem ? t(MAPPING_PROBLEM_KEY[mapProblem]) : null;

  const start = () => {
    reset();
    setTool('center');
  };

  return (
    <PanelSection id="camera.center" title={t('camera.center.title')}>
      <p className="hint">{t('camera.center.help')}</p>
      <div className="field">
        <label className="field__label" htmlFor="cam-center-mode">{t('camera.center.mode')}</label>
        <select id="cam-center-mode" className="select" value={mode} onChange={(e) => setMode(e.target.value as 'three' | 'auto')}>
          <option value="three">{t('camera.center.mode.three')}</option>
          <option value="auto">{t('camera.center.mode.auto')}</option>
        </select>
      </div>
      <button type="button" className={active ? 'btn' : 'btn btn--accent'} aria-pressed={active} aria-disabled={!!mapReason} title={mapReason ?? undefined} onClick={mapReason ? undefined : active ? () => setTool('align') : start}>
        <Crosshair size={14} aria-hidden="true" />
        {active ? t('camera.center.stop') : t('camera.center.start')}
      </button>
      {mapReason && <p className="hint">{mapReason}</p>}
      {active && <p className="hint num" role="status">{busy ? t('camera.center.detecting') : mode === 'three' ? t('camera.center.picks', { n: picks.length }) : t('camera.center.clickInside')}</p>}
      {error && <p className="warn-line" role="alert">{error}</p>}
      {result && (
        <div role="status" aria-live="polite">
          <p className="hint cam-ok num">
            {t('camera.center.result', { x: fmt(result.circle.center[0], 2), y: fmt(result.circle.center[1], 2), r: fmt(result.circle.radius, 2), d: fmt(result.circle.radius * 2, 2) })}
          </p>
          {result.source === 'auto' && <p className="hint num">{t('camera.center.fitRms', { rms: fmt(result.circle.rms, 2) })}</p>}
          <div className="row cam-wrap">
            <button type="button" className="btn btn--accent" aria-disabled={!!blocked} title={blocked ? t(blocked) : t('camera.center.moveHint')} onClick={blocked ? undefined : () => void moveHeadToCenter()}>
              <LocateFixed size={14} aria-hidden="true" />
              {t('camera.center.move')}
            </button>
            <button type="button" className="btn" onClick={() => setOriginAtCenter()}>{t('camera.center.setOrigin')}</button>
          </div>
          {blocked && <p className="hint">{t(blocked)}</p>}
        </div>
      )}
      <p className="hint hint--warn">{t('camera.center.unvalidated')}</p>
    </PanelSection>
  );
}
