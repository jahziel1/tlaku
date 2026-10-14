import { Repeat } from 'lucide-react';
import { useEffect } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { designBounds } from '@/features/machine/placement';
import { feedLines, MAX_FEED_MM } from '@/features/machine/DeviceFeederModel';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, selectJobSelection, useStore } from '@/store';
import { LengthField } from '@/ui/LengthField';
import { NumberField } from '@/ui/NumberField';
import { MAX_REPEATS, MIN_REPEATS, repeatBlockedReason, repeatUnavailable, useRepeat } from './repeat';
import './feeder.css';

/**
 * «Repetir con el alimentador» del diálogo de inicio: graba el trabajo N veces y entre una
 * repetición y la siguiente manda el avance del alimentador del perfil. Solo aparece con una
 * máquina GRBL; si el alimentador no está activado dice cómo hacerlo.
 */
export function RepeatSection() {
  const device = useStore(useShallow(selectActiveDevice));
  const doc = useStore((s) => s.doc);
  const selection = useStore(useShallow(selectJobSelection));
  const config = useRepeat((s) => s.config);
  const patch = useRepeat((s) => s.patchConfig);
  const unavailable = repeatUnavailable(device);
  const design = designBounds(doc, selection);
  const height = design ? design[1][1] - design[0][1] : null;

  // La casilla no sobrevive al diálogo: la próxima vez que se abra, el trabajo sale una sola vez.
  useEffect(() => () => useRepeat.getState().patchConfig({ enabled: false }), []);
  // Al activar la sección el avance parte del por defecto del perfil.
  const enable = (enabled: boolean) => patch(enabled ? { enabled, feedMm: device.feeder.feed_mm } : { enabled });

  // Sin G-code no hay nada que ofrecer, salvo explicar el motivo cuando el perfil sí tiene alimentador.
  if (unavailable === 'kind') return null;
  const blocked = config.enabled ? repeatBlockedReason(device, config) : null;
  return (
    <section className="rep" aria-labelledby="rep-title">
      <h3 id="rep-title" className="mc-sub"><Repeat size={14} aria-hidden="true" />{t('tiling.repeat.title')}</h3>
      {unavailable ? (
        <p className="hint">{t(`tiling.repeat.unavailable.${unavailable}`)}</p>
      ) : (
        <>
          <label className="mc-check">
            <input type="checkbox" checked={config.enabled} onChange={(e) => enable(e.target.checked)} />
            <span>{t('tiling.repeat.enable')}</span>
          </label>
          {config.enabled && (
            <>
              <div className="rep__grid">
                <NumberField label={t('tiling.repeat.count')} value={config.count} min={MIN_REPEATS} max={MAX_REPEATS} step={1} onCommit={(v) => patch({ count: Math.round(v) })} />
                <LengthField label={t('tiling.repeat.feed')} value={config.feedMm} min={0.1} max={MAX_FEED_MM} step={1} onCommit={(v) => patch({ feedMm: v })} />
              </div>
              {height !== null && height > 0 && (
                <button type="button" className="btn" onClick={() => patch({ feedMm: Math.min(MAX_FEED_MM, Math.round(height * 1000) / 1000) })}>
                  {t('tiling.repeat.useHeight', { mm: fmt(height, 1) })}
                </button>
              )}
              {blocked ? (
                <p className="warn-line" role="alert">{blocked}</p>
              ) : (
                <p className="hint" role="status">
                  {t('tiling.repeat.summary', { n: config.count, mm: fmt(config.feedMm, 1), gcode: feedLines(device.feeder, config.feedMm).join(' / ') })}
                </p>
              )}
            </>
          )}
        </>
      )}
    </section>
  );
}
