import { Grid2x2 } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { designBounds } from '@/features/machine/placement';
import { t } from '@/i18n';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, selectJobSelection, useStore } from '@/store';
import { exceedsBed } from './plan';
import { openTiling } from './store';

/**
 * Aviso «el diseño no cabe en la cama» con el botón «Dividir en tramos…». No se pinta si cabe.
 * Va en la sección de trabajo de la pestaña Máquina y en el diálogo de inicio.
 */
export function TilingOffer() {
  const doc = useStore((s) => s.doc);
  const selection = useStore(useShallow(selectJobSelection));
  const bed = useStore(useShallow((s) => selectActiveDevice(s).bed));
  const design = designBounds(doc, selection);
  if (!design || !exceedsBed(design, bed)) return null;
  return (
    <div className="til-offer" role="status">
      <p>{t('tiling.offer.text', { w: fmt(design[1][0] - design[0][0], 0), h: fmt(design[1][1] - design[0][1], 0), bw: fmt(bed[0], 0), bh: fmt(bed[1], 0) })}</p>
      <button type="button" className="btn" onClick={() => { useStore.getState().closeDialog(); openTiling(); }}>
        <Grid2x2 size={14} aria-hidden="true" />{t('tiling.cmd.open')}
      </button>
    </div>
  );
}
