import { useEffect, useMemo, useState } from 'react';
import { t } from '@/i18n';
import { formatSize } from '@/lib/units';
import { selectActiveDevice, usePref, useStore } from '@/store';
import { describeSelection, selectionBox } from './selectionSummary';

/** `id` de la descripción del lienzo (`aria-describedby`). */
export const CANVAS_DESC_ID = 'canvas-desc';
/** Espera antes de anunciar: al arrastrar, la selección cambia decenas de veces por segundo. */
const ANNOUNCE_DELAY_MS = 700;

/**
 * Lo que un lector de pantalla necesita del lienzo, que es gráfico: una descripción fija (mesa y
 * cantidad de objetos) y un resumen vivo de la selección (cuántos, tamaño y posición). El resumen
 * se anuncia con retraso para no leer cada paso de un arrastre. Visualmente oculto.
 */
export function CanvasSummary() {
  const units = usePref('units');
  const bed = useStore((s) => selectActiveDevice(s).bed);
  const name = useStore((s) => selectActiveDevice(s).name);
  const count = useStore((s) => s.doc.objects.length);
  const objects = useStore((s) => s.doc.objects);
  const ids = useStore((s) => s.selectedIds);

  const box = useMemo(() => selectionBox(objects, ids), [objects, ids]);
  const sentence = describeSelection(box, units);
  const [announced, setAnnounced] = useState(sentence);
  useEffect(() => {
    const timer = setTimeout(() => setAnnounced(sentence), ANNOUNCE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [sentence]);

  const objectsText = count === 0 ? t('a11y.canvas.objects.none') : count === 1 ? t('a11y.canvas.objects.one') : t('a11y.canvas.objects.many', { n: count });
  return (
    <>
      <p id={CANVAS_DESC_ID} className="sr-only">
        {t('a11y.canvas.desc', { name, size: formatSize(bed[0], bed[1], units, 0), objects: objectsText })}
      </p>
      <p className="sr-only" role="status" aria-live="polite" aria-atomic="true">{announced}</p>
    </>
  );
}
