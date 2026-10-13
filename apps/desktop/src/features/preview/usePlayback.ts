import { useEffect } from 'react';
import { useStore } from '@/store';
import { advanceScrub, getModel } from './model';

/** Avanza `scrub` en cada cuadro mientras `playing`, a la velocidad elegida. Monta una sola vez. */
export function usePlayback(): void {
  const playing = useStore((s) => s.playing);
  useEffect(() => {
    if (!playing) return;
    let raf = 0;
    let last = performance.now();
    const tick = (now: number) => {
      // Tras una pausa larga (pestaña en segundo plano) no se salta de golpe.
      const dt = Math.min(0.1, Math.max(0, (now - last) / 1000));
      last = now;
      const s = useStore.getState();
      const model = s.preview ? getModel(s.preview) : null;
      if (!model || model.total <= 0) {
        s.setPlaying(false);
        return;
      }
      const next = advanceScrub(s.scrub, dt, s.playSpeed, model.total);
      s.setScrub(next.scrub);
      if (next.ended) s.setPlaying(false);
      else raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [playing]);
}
