import { Check, RotateCcw, X } from 'lucide-react';
import { TriangleAlert } from 'lucide-react';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { isIdentity, isValid, WARP_MODES } from './warpEdit';
import { applyWarp, resetWarp, setWarpMode, useWarpEdit } from './warpEditStore';
import './tool-bars.css';

/**
 * Opciones de la herramienta Deformar: tipo de deformación (bilineal, perspectiva, malla 4×4),
 * Aplicar, Restablecer y Cancelar. Solo aparece con la herramienta activa; los puntos se arrastran
 * en el lienzo.
 */
export function WarpBar() {
  const active = useStore((s) => s.activeTool === 'warp');
  const session = useWarpEdit((s) => s.session);
  const reason = useWarpEdit((s) => s.reason);
  const busy = useWarpEdit((s) => s.busy);
  const cancel = useCommand('tool.select');
  if (!active) return null;
  const ready = !!session && isValid(session) && !isIdentity(session) && !busy;
  return (
    <div className="tool-bar" role="group" aria-label={t('canvas.warp.bar')} data-testid="warp-bar">
      {session ? (
        <>
          <div className="tool-bar__chips" role="group" aria-label={t('canvas.warp.modes')}>
            {WARP_MODES.map((m) => (
              <button key={m} type="button" className="chip" aria-pressed={session.mode === m} disabled={busy} onClick={() => setWarpMode(m)}>
                {t(`canvas.warp.mode.${m}`)}
              </button>
            ))}
          </div>
          {!isValid(session) && (
            <span className="tool-bar__warn" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              {t('canvas.warp.invalid')}
            </span>
          )}
          <button type="button" className="btn" disabled={busy || isIdentity(session)} onClick={resetWarp}>
            <RotateCcw size={14} aria-hidden="true" />
            {t('canvas.warp.reset')}
          </button>
          <button type="button" className="btn btn--accent" disabled={!ready} onClick={() => void applyWarp()}>
            <Check size={14} aria-hidden="true" />
            {busy ? t('canvas.warp.applying') : t('canvas.warp.apply')}
          </button>
        </>
      ) : (
        <span className="tool-bar__note" role="status">{t(reason ?? 'canvas.warp.tipSelect')}</span>
      )}
      <button type="button" className="btn" onClick={cancel.run} disabled={busy}>
        <X size={14} aria-hidden="true" />
        {t('canvas.warp.cancel')}
      </button>
    </div>
  );
}
