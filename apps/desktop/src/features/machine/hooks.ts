import { useEffect, useMemo, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { selectActiveDevice, useStore } from '@/store';
import { computeJobPlacement, designBounds, type Placement } from './placement';

/** Reloj que avanza cada segundo mientras `active` (para el tiempo transcurrido y restante). */
export function useNow(active: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [active]);
  return now;
}

/** Dónde quedará el trabajo con las opciones actuales: se recalcula solo si cambia algo que importa. */
export function useJobPlacement(): Placement {
  const doc = useStore((s) => s.doc);
  const options = useStore((s) => s.jobOptions);
  const device = useStore(useShallow((s) => {
    const d = selectActiveDevice(s);
    return { bed: d.bed, origin: d.origin };
  }));
  const head = useStore(useShallow((s) => (s.connection.connected && s.status ? s.status.work_pos : null)));
  const design = useMemo(() => designBounds(doc, options.selection), [doc, options.selection]);
  return useMemo(
    () =>
      computeJobPlacement({
        startFrom: options.start_from,
        jobOrigin: options.job_origin,
        userOrigin: options.user_origin,
        head,
        device,
        design,
      }),
    [options.start_from, options.job_origin, options.user_origin, head, device, design],
  );
}
