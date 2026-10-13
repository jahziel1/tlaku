import { TriangleAlert } from 'lucide-react';
import { t } from '@/i18n';
import { isCriticalSetting, settingNameKey } from './grblSettingsInfo';
import { validationParams, type BackupDiff } from './MachineSettingsLogic';
import { SettingTags } from './MachineSettingsTable';
import './MachineSettings.css';

/** Confirmación de UNA escritura: la línea exacta, el valor anterior y la advertencia. */
export function EditConfirmBody({ id, previous, line }: { id: number; previous: string; line: string }) {
  return (
    <div className="mset-confirm">
      <p>{t('mset.confirm.intro', { n: id, name: t(settingNameKey(id)) })}</p>
      <p>{t('machine.confirm.exact')}</p>
      <pre className="mc-code num" aria-label={t('machine.confirm.exact')}>{line}</pre>
      <p className="mset-prev">
        {t('mset.confirm.previous')} <strong className="num">{previous}</strong>
      </p>
      <p className="warn-line">
        <TriangleAlert size={14} aria-hidden="true" />
        <span>{t('mset.confirm.warn')}</span>
      </p>
      {isCriticalSetting(id) && (
        <p className="warn-line" data-critical>
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('mset.confirm.warnCritical')}</span>
        </p>
      )}
    </div>
  );
}

interface RestoreProps {
  fileName: string;
  /** Firmware anotado en el respaldo (o `null`). */
  backupFirmware: string | null;
  /** El respaldo es de otra familia de firmware que la máquina conectada. */
  familyMismatch: boolean;
  diff: BackupDiff;
  selected: ReadonlySet<number>;
  onToggle: (id: number) => void;
  onToggleAll: (on: boolean) => void;
}

/** Vista previa de lo que cambiaría al restaurar un respaldo: solo las diferencias, con casillas. */
export function RestorePreviewBody({ fileName, backupFirmware, familyMismatch, diff, selected, onToggle, onToggleAll }: RestoreProps) {
  const all = diff.changes.length > 0 && diff.changes.every((c) => selected.has(c.id));
  return (
    <div className="mset-restore">
      <p>{t('mset.restore.intro', { file: fileName })}</p>
      {backupFirmware && <p className="hint">{t('mset.restore.firmware', { firmware: backupFirmware })}</p>}
      {familyMismatch && (
        <p className="warn-line">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('mset.restore.familyMismatch')}</span>
        </p>
      )}
      <p className="hint" role="status">
        {t('mset.restore.summary', { changes: diff.changes.length, same: diff.same })}
      </p>
      {diff.missing.length > 0 && (
        <p className="hint">{t('mset.restore.missing', { list: diff.missing.map((n) => `$${n}`).join(', ') })}</p>
      )}
      {diff.invalid.length > 0 && (
        <div className="hint hint--danger">
          <p>{t('mset.restore.invalid')}</p>
          <ul className="mc-list">
            {diff.invalid.map((i) => (
              <li key={i.id} className="num">{`$${i.id}=${i.raw}`} <span className="mset-why">{t(i.error.key, validationParams(i.error))}</span></li>
            ))}
          </ul>
        </div>
      )}
      {diff.changes.length === 0 ? (
        <p role="status">{t('mset.restore.none')}</p>
      ) : (
        <>
          <p className="warn-line">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('mset.restore.warn')}</span>
          </p>
          <table className="mc-table mset-table" aria-label={t('mset.restore.table')}>
            <thead>
              <tr>
                <th scope="col">
                  <input type="checkbox" checked={all} aria-label={t('mset.restore.selectAll')} onChange={(e) => onToggleAll(e.target.checked)} />
                </th>
                <th scope="col">{t('mset.col.id')}</th>
                <th scope="col">{t('mset.col.name')}</th>
                <th scope="col">{t('mset.restore.col.change')}</th>
              </tr>
            </thead>
            <tbody>
              {diff.changes.map((c) => (
                <tr key={c.id} data-critical={isCriticalSetting(c.id) ? '' : undefined}>
                  <td>
                    <input type="checkbox" checked={selected.has(c.id)} aria-label={t('mset.restore.pick', { n: c.id })} onChange={() => onToggle(c.id)} />
                  </td>
                  <th scope="row" className="num mset-id">{`$${c.id}`}</th>
                  <td>
                    <div className="mset-name">
                      <span>{t(settingNameKey(c.id))}</span>
                      <SettingTags id={c.id} />
                    </div>
                  </td>
                  <td className="num">
                    {c.current} <span aria-hidden="true">→</span>
                    <span className="sr-only">{t('mset.restore.to')}</span> <strong>{c.next}</strong>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}
