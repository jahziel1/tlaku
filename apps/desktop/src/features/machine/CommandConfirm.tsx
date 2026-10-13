import { TriangleAlert } from 'lucide-react';
import { useCallback, useState } from 'react';
import { t, type I18nKey } from '@/i18n';
import { selectActiveDevice, useStore } from '@/store';
import { commandErrorText } from '@/store/machine';
import { isDestructive, needsConfirmation, parseCommand, type PreparedCommand, type Risk } from './commandSafety';
import { Modal } from './Modal';

/** Advertencia de cada nivel de riesgo: las mismas del CLI (`Risk::warning`). */
export const RISK_WARNING: Record<Exclude<Risk, 'safe'>, I18nKey> = {
  unlock: 'machine.risk.unlock',
  home: 'machine.risk.home',
  moves: 'machine.risk.moves',
  laser: 'machine.risk.laser',
  setting: 'machine.risk.setting',
  other: 'machine.risk.other',
};

/**
 * Advertencia que acompaña a la confirmación. `$RST=` y `$SLP` no se pueden deshacer: su advertencia
 * propia reemplaza la genérica de `other`.
 */
export function commandWarning(cmd: PreparedCommand): I18nKey | null {
  if (cmd.kind === 'line' && isDestructive(cmd.text)) return 'machine.risk.destructive';
  return cmd.risk === 'safe' ? null : RISK_WARNING[cmd.risk];
}

/** Qué se va a enviar y por qué pide confirmación. */
export function CommandConfirmDialog({ cmd, onCancel, onConfirm }: { cmd: PreparedCommand; onCancel: () => void; onConfirm: () => void }) {
  const warning = commandWarning(cmd);
  return (
    <Modal
      title={t('machine.confirm.title')}
      tone="danger"
      onClose={onCancel}
      footer={
        <>
          <button type="button" className="btn" data-autofocus onClick={onCancel}>{t('machine.dlg.cancel')}</button>
          <button type="button" className="btn btn--danger" onClick={onConfirm}>{t('machine.confirm.send')}</button>
        </>
      }
    >
      <p>{t('machine.confirm.exact')}</p>
      <pre className="mc-code num" aria-label={t('machine.confirm.exact')}>{cmd.shown}</pre>
      {warning && (
        <p className="warn-line">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t(warning)}</span>
        </p>
      )}
    </Modal>
  );
}

/**
 * Enviar un comando respetando las reglas del CLI: lo de solo lectura sale directo; lo demás abre
 * un diálogo con lo que se enviará exactamente. Devuelve `ask` y el diálogo para pintarlo.
 */
export function useCommandConfirm() {
  const rxBuffer = useStore((s) => selectActiveDevice(s).rx_buffer);
  const sendLine = useStore((s) => s.sendLine);
  const notify = useStore((s) => s.notify);
  const [pending, setPending] = useState<PreparedCommand | null>(null);

  /** Devuelve true si la línea es válida (se envió o quedó esperando confirmación). */
  const ask = useCallback(
    (raw: string): boolean => {
      const parsed = parseCommand(raw, rxBuffer);
      if (!parsed.ok) {
        notify('error', commandErrorText(parsed.error));
        return false;
      }
      if (needsConfirmation(parsed.cmd.risk)) setPending(parsed.cmd);
      else void sendLine(parsed.cmd.kind === 'line' ? parsed.cmd.text : parsed.cmd.name);
      return true;
    },
    [rxBuffer, sendLine, notify],
  );

  const dialog = pending ? (
    <CommandConfirmDialog
      cmd={pending}
      onCancel={() => setPending(null)}
      onConfirm={() => {
        const text = pending.kind === 'line' ? pending.text : pending.name;
        setPending(null);
        void sendLine(text, { confirmed: true });
      }}
    />
  ) : null;

  return { ask, dialog };
}
