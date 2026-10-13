import { t } from '@/i18n';
import { NumberField } from '@/ui/NumberField';
import { useStore } from '@/store';
import { cleanupReason, runCleanup } from './cleanup';
import { Dialog, Footer } from './Dialog';
import { CLEANUP_DEFAULTS, useArrangeUi, type CleanupOp } from './uiStore';

/**
 * Unir trazos, cerrar trayectos o quitar duplicados: pide solo la tolerancia (con el valor de la
 * última vez) y explica qué hace la operación y con qué valor se resuelve la mayoría de los casos.
 */
export function CleanupDialog({ op }: { op: CleanupOp }) {
  const doc = useStore((s) => s.doc);
  const selectedIds = useStore((s) => s.selectedIds);
  const tolerance = useArrangeUi((s) => s.cleanup[op]);
  const setTolerance = useArrangeUi((s) => s.setCleanupTolerance);
  const busy = useArrangeUi((s) => s.busy);
  const close = useArrangeUi((s) => s.close);

  const reason = cleanupReason({ doc, selectedIds }, op);
  const blocked = reason !== true || busy;

  const apply = async () => {
    if (await runCleanup(useStore.getState(), op, tolerance)) close();
  };

  return (
    <Dialog
      title={t(`arrange.cleanup.title.${op}`)}
      onClose={close}
      initialFocus=".field__input"
      footer={<Footer onClose={close} onApply={() => void apply()} disabled={blocked} reason={reason === true ? null : t(reason)} applyLabel={busy ? t('arrange.dlg.working') : undefined} />}
    >
      <p className="hint">{t(`arrange.cleanup.hint.${op}`)}</p>
      <NumberField label={t('arrange.cleanup.tolerance')} value={tolerance} min={0} max={50} step={0.01} unit={t('arrange.dlg.mm')} onCommit={(v) => setTolerance(op, v)} />
      <p className="hint">{t('arrange.cleanup.toleranceHint', { default: CLEANUP_DEFAULTS[op] })}</p>
      <p className="hint">{t('arrange.cleanup.curvesHint')}</p>
    </Dialog>
  );
}
