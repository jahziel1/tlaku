import { Clipboard, Download, FileSpreadsheet, X } from 'lucide-react';
import { useEffect, useId } from 'react';
import { copyText } from '@/features/help/clipboard';
import { useJobLog } from '@/features/help/jobLog';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { Modal } from '@/ui/Modal';
import { CostInputsPanel } from './CostInputsPanel';
import { CostResultPanel } from './CostResultPanel';
import { saveCostCsv } from './costFiles';
import { reportCsv, reportText } from './costReport';
import { useCostStore } from './costStore';
import { useCostCalc } from './useCostCalc';
import './costs.css';

/** Fecha de hoy en formato ISO (`2026-10-15`), hora local. */
function today(): string {
  const d = new Date();
  const two = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}`;
}

function Dialog({ onClose }: { onClose: () => void }) {
  const titleId = useId();
  const calc = useCostCalc();
  const notify = useStore((s) => s.notify);
  const stale = useStore((s) => s.previewStale);
  const status = useStore((s) => s.previewStatus);
  const compute = useStore((s) => s.computePreview);
  const refreshLog = useJobLog((s) => s.refresh);

  // Al abrir: el registro de trabajos (otra ventana pudo anotar uno) y un cálculo al día del trabajo.
  useEffect(() => {
    refreshLog();
    if (stale || status === 'idle') void compute();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- solo al abrir; después manda la sincronización normal
  }, []);

  const ctx = () => ({ ...calc.report, date: today() });
  const doCopy = async (text: string, okKey: 'costs.copied' | 'costs.copiedCsv') => {
    const ok = await copyText(text);
    notify(ok ? 'success' : 'error', ok ? t(okKey) : t('costs.copyFailed'));
  };
  const doSave = async () => {
    try {
      const where = await saveCostCsv(reportCsv(ctx()));
      if (where) notify('success', t('costs.saveDone', { path: where }));
    } catch (e) {
      notify('error', t('costs.saveFailed', { error: e instanceof Error ? e.message : String(e) }));
    }
  };

  return (
    <Modal labelId={titleId} onClose={onClose} portal closeOnBackdrop backdropClassName="mc-backdrop" cardClassName="mc-modal cost-modal" initialFocus="[data-autofocus]" passKey={(e) => (e.metaKey || e.ctrlKey) && e.key === '.'}>
      <header className="mc-modal__head">
        <div>
          <h2 id={titleId} className="mc-modal__title">{t('costs.title')}</h2>
          <p className="hint">{t('costs.subtitle')}</p>
        </div>
        <button type="button" className="icon-btn" title={t('costs.close')} aria-label={t('costs.close')} onClick={onClose}>
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      <div className="mc-modal__body cost__body">
        <CostInputsPanel calc={calc} />
        <CostResultPanel calc={calc} />
      </div>
      <footer className="mc-modal__foot cost__foot">
        <span className="hint cost__private">{t('costs.private')}</span>
        <button type="button" className="btn" onClick={() => void doCopy(reportText(ctx()), 'costs.copied')}>
          <Clipboard size={14} aria-hidden="true" />{t('costs.copyText')}
        </button>
        <button type="button" className="btn" onClick={() => void doCopy(reportCsv(ctx()), 'costs.copiedCsv')}>
          <FileSpreadsheet size={14} aria-hidden="true" />{t('costs.copyCsv')}
        </button>
        <button type="button" className="btn" onClick={() => void doSave()}>
          <Download size={14} aria-hidden="true" />{t('costs.saveCsv')}
        </button>
        <button type="button" className="btn btn--accent" data-autofocus onClick={onClose}>{t('costs.close')}</button>
      </footer>
    </Modal>
  );
}

/** Diálogo «Costos y precio». Se monta una vez en el shell y se abre con el comando `costs.open`. */
export function CostDialog() {
  const open = useCostStore((s) => s.open);
  const close = useCostStore((s) => s.closeDialog);
  return open ? <Dialog onClose={close} /> : null;
}
