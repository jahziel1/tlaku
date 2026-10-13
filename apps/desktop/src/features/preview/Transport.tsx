import { useMemo } from 'react';
import { Pause, Play, SkipBack, SkipForward } from 'lucide-react';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import { useStore } from '@/store';
import { PLAY_SPEEDS, type PreviewModel } from './model';

const SLIDER_STEPS = 10000;
/** Teclas que la barra de tiempo resuelve sola; no deben llegar a los atajos globales (mover la selección). */
const OWN_KEYS = new Set(['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown']);

/** Lectura "tiempo actual de total"; es lo único que se repinta en cada cuadro de la reproducción. */
function TimeReadout({ model }: { model: PreviewModel }) {
  const scrub = useStore((s) => s.scrub);
  const text = t('preview.time', { now: fmtClock(model.total * scrub), total: fmtClock(model.total) });
  return (
    <span className="num pv__time" aria-hidden="true">
      {model.approximate && <abbr title={t('preview.time.approx')} className="pv__approx">≈</abbr>}
      {text}
    </span>
  );
}

function Timeline({ model, layerNames }: { model: PreviewModel; layerNames: string[] }) {
  const scrub = useStore((s) => s.scrub);
  const setScrub = useStore((s) => s.setScrub);
  // Marcas donde empieza cada capa (solo con los tiempos exactos del motor).
  const marks = useMemo(() => {
    const starts = model.layerStarts;
    if (!starts || model.approximate || model.total <= 0) return [];
    const out: { pct: number; name: string }[] = [];
    for (let k = 1; k + 1 < starts.length; k++) {
      const at = starts[k]! > 0 ? model.times[starts[k]! - 1]! : 0;
      out.push({ pct: (at / model.total) * 100, name: layerNames[k] ?? '' });
    }
    return out;
  }, [model, layerNames]);

  return (
    <div className="pv__timeline">
      <input
        className="range"
        type="range"
        min={0}
        max={SLIDER_STEPS}
        value={Math.round(scrub * SLIDER_STEPS)}
        aria-label={t('preview.scrub')}
        aria-valuetext={t('preview.time', { now: fmtClock(model.total * scrub), total: fmtClock(model.total) })}
        onChange={(e) => setScrub(Number(e.target.value) / SLIDER_STEPS)}
        onKeyDown={(e) => OWN_KEYS.has(e.key) && e.stopPropagation()}
      />
      {marks.length > 0 && (
        <div className="pv__marks" aria-hidden="true">
          {marks.map((m, i) => (
            <span key={i} className="pv__mark" style={{ left: `${m.pct}%` }} title={t('preview.layerStart', { name: m.name })} />
          ))}
        </div>
      )}
    </div>
  );
}

export function Transport({ model, layerNames }: { model: PreviewModel; layerNames: string[] }) {
  const play = useCommand('preview.play');
  const slower = useCommand('preview.slower');
  const faster = useCommand('preview.faster');
  const playing = useStore((s) => s.playing);
  const speed = useStore((s) => s.playSpeed);
  const setSpeed = useStore((s) => s.setPlaySpeed);
  const setScrub = useStore((s) => s.setScrub);
  const setPlaying = useStore((s) => s.setPlaying);

  return (
    <div className="pv__transport">
      <div className="pv__buttons">
        <button type="button" className="icon-btn" title={t('preview.toStart')} aria-label={t('preview.toStart')} onClick={() => { setPlaying(false); setScrub(0); }}>
          <SkipBack size={16} aria-hidden="true" />
        </button>
        <button
          type="button"
          className="icon-btn icon-btn--boxed"
          title={play.tooltip}
          aria-label={playing ? t('preview.pause') : t('preview.play')}
          aria-pressed={playing}
          onClick={play.run}
        >
          {playing ? <Pause size={16} aria-hidden="true" /> : <Play size={16} aria-hidden="true" />}
        </button>
        <button type="button" className="icon-btn" title={t('preview.toEnd')} aria-label={t('preview.toEnd')} onClick={() => { setPlaying(false); setScrub(1); }}>
          <SkipForward size={16} aria-hidden="true" />
        </button>
      </div>
      <div className="pv__speeds" role="group" aria-label={t('preview.speed')} title={`${t('preview.speed')} (${slower.shortcut} / ${faster.shortcut})`}>
        {PLAY_SPEEDS.map((n) => (
          <button
            key={n}
            type="button"
            className="pv__speed num"
            aria-pressed={speed === n}
            title={t('preview.speed.title', { n })}
            onClick={() => setSpeed(n)}
          >
            {t('preview.speed.value', { n })}
          </button>
        ))}
      </div>
      <Timeline model={model} layerNames={layerNames} />
      <TimeReadout model={model} />
    </div>
  );
}
