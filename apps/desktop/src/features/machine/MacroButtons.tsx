import { Play, TriangleAlert } from 'lucide-react';
import { t, type I18nKey } from '@/i18n';
import { isJobBusy, selectActiveDevice, useStore } from '@/store';
import { RISK_WARNING } from './CommandConfirm';
import { MAX_MACROS, planMacro } from './macros';
import { Modal } from './Modal';

/** Botones de las macros del perfil activo (hasta diez) en el panel Máquina. */
export function MacroButtons() {
  const device = useStore(selectActiveDevice);
  const connected = useStore((s) => s.connection.connected);
  const busy = useStore(isJobBusy);
  const run = useStore((s) => s.runMacro);
  const macros = device.macros.slice(0, MAX_MACROS);
  if (macros.length === 0) return null;
  const why = !connected ? t('reason.needMachine') : busy ? t('reason.jobRunning') : undefined;
  return (
    <section className="section" aria-labelledby="sec-macros">
      <h3 id="sec-macros" className="panel-title">{t('machine.macros')}</h3>
      <div className="mc-macrobtns" role="group" aria-label={t('machine.macros')}>
        {macros.map((m, i) => (
          <button key={i} type="button" className="btn" disabled={why !== undefined} title={why ?? t('machine.macros.runHint', { name: m.name })} onClick={() => void run(i)}>
            <Play size={13} aria-hidden="true" />{m.name}
          </button>
        ))}
      </div>
      <p className="hint">{t('machine.macros.panelHelp')}</p>
    </section>
  );
}

/** Muestra las líneas exactas de la macro y su riesgo antes de enviarlas. */
export function MacroConfirmDialog({ index }: { index: number }) {
  const device = useStore(selectActiveDevice);
  const close = useStore((s) => s.closeDialog);
  const send = useStore((s) => s.sendMacro);
  const macro = device.macros[index];
  const plan = macro ? planMacro(macro, device.rx_buffer) : null;
  if (!macro || !plan || !plan.ok) {
    return (
      <Modal title={t('machine.macros.confirm.title')} onClose={close} footer={<button type="button" className="btn" data-autofocus onClick={close}>{t('machine.dlg.close')}</button>}>
        <p>{t('machine.macros.invalid', { name: macro?.name ?? '' })}</p>
      </Modal>
    );
  }
  const warning: I18nKey | null = plan.lines.some((l) => l.destructive) ? 'machine.risk.destructive' : plan.risk === 'safe' ? null : RISK_WARNING[plan.risk];
  return (
    <Modal
      title={t('machine.macros.confirm.title')}
      subtitle={macro.name}
      tone="danger"
      onClose={close}
      footer={
        <>
          <button type="button" className="btn" data-autofocus onClick={close}>{t('machine.dlg.cancel')}</button>
          <button
            type="button"
            className="btn btn--danger"
            onClick={() => {
              close();
              void send(index);
            }}
          >
            {t('machine.macros.confirm.send', { n: plan.lines.length })}
          </button>
        </>
      }
    >
      <p>{t('machine.macros.confirm.exact')}</p>
      <pre className="mc-code num" aria-label={t('machine.macros.confirm.exact')}>{plan.lines.map((l) => l.text).join('\n')}</pre>
      {warning && (
        <p className="warn-line">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t(warning)}</span>
        </p>
      )}
    </Modal>
  );
}
