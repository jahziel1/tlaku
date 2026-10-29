import { Clock, Layers } from 'lucide-react';
import { useEffect, useRef, type KeyboardEvent } from 'react';
import { t } from '@/i18n';
import { fmtClock } from '@/lib/platform';
import { useJobPlacement } from '@/features/machine/hooks';
import { Modal } from '@/features/machine/Modal';
import { summarizeJob } from '@/features/machine/jobSummary';
import { useRuidaStartGate } from '@/features/machine/ruidaStart';
import { placementSummary } from '@/features/machine/StartFromSection';
import { activeTabLabel, selectActiveDevice, useStore } from '@/store';
import type { StartMode } from './useProductionStart';

/**
 * Paso de un clic del modo producción: «Iniciar (modo producción)». Sustituye al diálogo de inicio
 * completo SOLO cuando `useProductionStart` comprobó que nada cambió y no hay ningún aviso. «Iniciar»
 * lleva el foco, así que Enter lo confirma; «Ver la lista completa» abre el diálogo de siempre.
 * Detener y la parada de emergencia no pasan por aquí.
 */
export function ProductionStartDialog({ mode, onFull }: { mode: Exclude<StartMode, 'full'>; onFull: () => void }) {
  const close = useStore((s) => s.closeDialog);
  const confirm = useStore((s) => s.confirmStart);
  const doc = useStore((s) => s.doc);
  const docName = useStore((s) => activeTabLabel(s));
  const preview = useStore((s) => s.preview);
  const options = useStore((s) => s.jobOptions);
  const device = useStore(selectActiveDevice);
  const connected = useStore((s) => s.connection.connected);
  const placement = useJobPlacement();
  const ruidaGate = useRuidaStartGate(device, connected);
  const summary = mode === 'compact' && preview ? summarizeJob(doc, preview) : null;
  const where = placementSummary(placement, options.start_from, options.job_origin);
  const ready = mode === 'compact' && summary !== null;

  // El botón nace deshabilitado mientras se comprueba (no puede recibir el foco inicial): al quedar listo, Enter debe confirmarlo.
  const startRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (ready) startRef.current?.focus();
  }, [ready]);

  const go = () => void ruidaGate.commit().then((ok) => (ok ? confirm() : undefined));
  // Mantener Enter pulsado (el mismo gesto que abrió el diálogo con Mod+Enter) no debe iniciar el trabajo.
  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (e.repeat) e.preventDefault();
  };

  return (
    <Modal
      title={t('machine.prod.title')}
      subtitle={t('machine.prod.subtitle', { doc: docName, machine: device.name })}
      tone="warn"
      size="sm"
      onClose={close}
      footer={
        <>
          <button type="button" className="btn" onClick={close}>{t('machine.dlg.cancel')}</button>
          <button ref={startRef} type="button" className="btn btn--accent" data-autofocus disabled={!ready} onKeyDown={onKeyDown} onClick={go}>
            {t('machine.prod.confirm')}
          </button>
        </>
      }
    >
      {!ready ? (
        <p className="hint" role="status">{t('machine.prod.checking')}</p>
      ) : (
        <>
          <dl className="mc-summary">
            <div>
              <dt><Clock size={13} aria-hidden="true" />{t('machine.prod.estimate')}</dt>
              <dd className="num" data-testid="prod-time">{fmtClock(summary.totalSeconds)}</dd>
            </div>
            <div>
              <dt><Layers size={13} aria-hidden="true" />{t('machine.prod.layers', { n: summary.layers.length })}</dt>
              <dd>{summary.layers.map((l) => l.name).join(' · ')}</dd>
            </div>
          </dl>
          <p className={where.problem ? 'hint hint--danger' : 'hint'}>{where.text}</p>
          {device.kind === 'ruida' && <p className="hint">{device.ruida.run_after_upload ? t('machine.prod.ruidaRun') : t('machine.prod.ruidaStore')}</p>}
          <p className="hint">{t('machine.prod.why')}</p>
          <p className="hint">{t('machine.prod.reminder')}</p>
        </>
      )}
      <button type="button" className="btn" data-testid="prod-full" onClick={onFull}>{t('machine.prod.full')}</button>
    </Modal>
  );
}
