import { useEffect, useMemo, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { selectActiveDevice, selectJobSelection, useStore } from '@/store';
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
  const selection = useStore(useShallow(selectJobSelection));
  const useSelectionOrigin = selection !== null && options.use_selection_origin;
  const device = useStore(useShallow((s) => {
    const d = selectActiveDevice(s);
    return { bed: d.bed, origin: d.origin, swap_xy: d.swap_xy, rotary: d.rotary };
  }));
  const controllerResolves = useStore((s) => selectActiveDevice(s).kind === 'ruida');
  const head = useStore(useShallow((s) => (s.connection.connected && s.status ? s.status.work_pos : null)));
  const design = useMemo(() => designBounds(doc, selection), [doc, selection]);
  // Solo la selección, anclada a la caja de todo el diseño (a menos que se pida su propio origen).
  const originBox = useMemo(() => (selection !== null && !useSelectionOrigin ? designBounds(doc, null) : design), [doc, selection, useSelectionOrigin, design]);
  return useMemo(
    () =>
      computeJobPlacement({
        startFrom: options.start_from,
        jobOrigin: options.job_origin,
        userOrigin: options.user_origin,
        head,
        device,
        design,
        originBox,
        controllerResolves,
      }),
    [options.start_from, options.job_origin, options.user_origin, head, device, design, originBox, controllerResolves],
  );
}
