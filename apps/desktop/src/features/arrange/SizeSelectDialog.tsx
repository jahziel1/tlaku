import { useMemo } from 'react';
import { t } from '@/i18n';
import { NumberField } from '@/ui/NumberField';
import { useStore } from '@/store';
import { Dialog, Footer } from './Dialog';
import { selectableObjects, selectCircles } from './special';
import { useArrangeUi } from './uiStore';

/**
 * Selecciona los círculos cuyo diámetro está en un rango (o es exacto, con el mismo valor en los
 * dos campos): útil para encontrar de una vez los agujeros de tornillo de un diseño.
 */
export function SizeSelectDialog() {
  const doc = useStore((s) => s.doc);
  const range = useArrangeUi((s) => s.circles);
  const patch = useArrangeUi((s) => s.patchCircles);
  const close = useArrangeUi((s) => s.close);

  const candidates = useMemo(() => selectableObjects(doc.objects, doc.layers), [doc]);
  const found = useMemo(() => selectCircles(candidates, range.min, range.max), [candidates, range.min, range.max]);

  const apply = () => {
    useStore.getState().select(found);
    close();
  };

  return (
    <Dialog
      title={t('arrange.circles.title')}
      onClose={close}
      initialFocus=".field__input"
      footer={<Footer onClose={close} onApply={apply} disabled={found.length === 0} reason={found.length === 0 ? t('arrange.circles.none') : null} applyLabel={t('arrange.circles.apply')} />}
    >
      <p className="hint">{t('arrange.circles.hint')}</p>
      <div className="ar-cols">
        <NumberField label={t('arrange.circles.min')} value={range.min} min={0} max={10000} step={0.1} unit={t('arrange.dlg.mm')} onCommit={(min) => patch({ min })} />
        <NumberField label={t('arrange.circles.max')} value={range.max} min={0} max={10000} step={0.1} unit={t('arrange.dlg.mm')} onCommit={(max) => patch({ max })} />
      </div>
      <p className="hint" role="status">{t('arrange.circles.count', { n: found.length })}</p>
    </Dialog>
  );
}
