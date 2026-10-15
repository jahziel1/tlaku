import { useEffect, useId, useState } from 'react';
import { Frame } from '@/features/bundle/Frame';
import { useBundleUi } from '@/features/bundle/uiStore';
import { t, locale } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { loadBackupConfig, pickBackupFolder, refreshBackupList, runBackup, saveBackupSettings } from './actions';
import { useBackup } from './store';
import './backup.css';

/** Opciones de «cada cuántos días» y «cuántos conservar». */
export const EVERY_DAYS_OPTIONS = [0, 1, 3, 7, 14, 30] as const;
export const KEEP_OPTIONS = [3, 5, 10, 20, 50] as const;

const fmtDate = (ms: number) => new Date(ms).toLocaleString(locale());
const fmtSize = (bytes: number) => (bytes >= 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`);

/** Diálogo «Respaldo de ajustes»: carpeta, cuándo respaldar, cuántos conservar y restaurar uno de la lista. */
export function BackupDialog({ onClose }: { onClose: () => void }) {
  const config = useBackup((s) => s.config);
  const list = useBackup((s) => s.list);
  const busy = useBackup((s) => s.busy);
  const newer = useBackup((s) => s.newer);
  const openImportFrom = useBundleUi((s) => s.openImportFrom);
  const [error, setError] = useState<string | null>(null);
  const ids = { close: useId(), days: useId(), keep: useId(), hist: useId() };

  useEffect(() => {
    void (async () => {
      try {
        const c = await loadBackupConfig();
        if (c.folder) await refreshBackupList();
      } catch (e) {
        setError(errorMessage(e));
      }
    })();
  }, []);

  const save = async (patch: Parameters<typeof saveBackupSettings>[0]) => {
    try {
      setError(null);
      await saveBackupSettings(patch);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const chooseFolder = async () => {
    const folder = await pickBackupFolder();
    if (folder) await save({ folder });
  };

  const backupNow = async () => {
    setError(null);
    await runBackup('manual');
  };

  const restore = (entry: { path: string; created_ms: number }) => {
    openImportFrom({ path: entry.path, createdMs: entry.created_ms });
  };

  const folder = config?.folder ?? null;
  const unavailable = !!folder && !!list && !list.available;
  const last = config && config.last_backup_ms > 0 ? t('bundle.backup.last', { date: fmtDate(config.last_backup_ms) }) : t('bundle.backup.never');

  const foot = (
    <>
      <button type="button" className="btn" onClick={onClose}>{t('bundle.close')}</button>
      <button type="button" className="btn btn--accent" data-autofocus aria-disabled={!folder || busy} onClick={() => folder && !busy && void backupNow()}>
        {busy ? t('bundle.backup.running') : t('bundle.backup.now')}
      </button>
    </>
  );

  return (
    <Frame title={t('bundle.backup.title')} onClose={onClose} foot={foot}>
      <div className="bnd-body bkp-body">
        <p className="hint">{t('bundle.backup.hint')}</p>

        {newer && (
          <div className="bnd-section bkp-newer" role="status">
            <p className="bnd-detail bnd-detail--warn">{t('bundle.backup.newerBanner', { date: fmtDate(newer.created_ms) })}</p>
            <button type="button" className="btn" onClick={() => restore(newer)}>{t('bundle.backup.review')}</button>
          </div>
        )}

        <div className="bnd-section" data-section="folder">
          <span className="bnd-check">{t('bundle.backup.folder')}</span>
          <p className="bnd-detail bkp-path" data-testid="backup-folder">{folder ?? t('bundle.backup.noFolder')}</p>
          {unavailable && <p className="bnd-detail bnd-detail--warn" role="alert">{t('bundle.backup.unavailable')}</p>}
          <div className="bkp-row">
            <button type="button" className="btn" onClick={() => void chooseFolder()}>{folder ? t('bundle.backup.changeFolder') : t('bundle.backup.chooseFolder')}</button>
            {folder && <button type="button" className="btn" onClick={() => void save({ folder: null })}>{t('bundle.backup.clearFolder')}</button>}
          </div>
          <p className="bnd-detail">{t('bundle.backup.folderHint')}</p>
        </div>

        <div className="bnd-section" data-section="schedule" data-off={!folder || undefined}>
          <span className="bnd-check">{t('bundle.backup.auto')}</span>
          <label className="bnd-check bkp-opt" htmlFor={ids.close}>
            <input id={ids.close} type="checkbox" checked={config?.on_close ?? false} disabled={!folder} onChange={(e) => void save({ on_close: e.target.checked })} />
            <span>{t('bundle.backup.onClose')}</span>
          </label>
          <div className="bkp-row">
            <label htmlFor={ids.days}>{t('bundle.backup.everyDays')}</label>
            <select id={ids.days} className="select" value={config?.every_days ?? 0} disabled={!folder} onChange={(e) => void save({ every_days: Number(e.target.value) })}>
              {EVERY_DAYS_OPTIONS.map((n) => <option key={n} value={n}>{n === 0 ? t('bundle.backup.everyNever') : n === 1 ? t('bundle.backup.everyDay') : t('bundle.backup.everyN', { n })}</option>)}
            </select>
          </div>
          <div className="bkp-row">
            <label htmlFor={ids.keep}>{t('bundle.backup.keep')}</label>
            <select id={ids.keep} className="select" value={config?.keep ?? 10} disabled={!folder} onChange={(e) => void save({ keep: Number(e.target.value) })}>
              {[...new Set([...KEEP_OPTIONS, config?.keep ?? 10])].sort((a, b) => a - b).map((n) => <option key={n} value={n}>{t('bundle.backup.keepN', { n })}</option>)}
            </select>
          </div>
          <label className="bnd-check bkp-opt" htmlFor={ids.hist}>
            <input id={ids.hist} type="checkbox" checked={config?.include_history ?? false} disabled={!folder} onChange={(e) => void save({ include_history: e.target.checked })} />
            <span>{t('bundle.backup.includeHistory')}</span>
          </label>
          <p className="bnd-detail">{t('bundle.backup.what')}</p>
          <p className="bnd-detail">{t('bundle.backup.noSecrets')}</p>
          <p className="bnd-detail" data-testid="backup-last">{last}</p>
        </div>

        {folder && (
          <div className="bnd-section" data-section="list">
            <span className="bnd-check">{t('bundle.backup.inFolder')}</span>
            {list && list.available && list.entries.length === 0 && <p className="bnd-detail">{t('bundle.backup.empty')}</p>}
            {list && list.entries.length > 0 && (
              <ul className="bkp-list">
                {list.entries.map((e) => (
                  <li key={e.name} className="bkp-item">
                    <span className="bkp-item__name">{fmtDate(e.created_ms)}<span className="bkp-item__size"> · {fmtSize(e.size_bytes)}</span></span>
                    <button type="button" className="btn" aria-label={t('bundle.backup.restoreAria', { date: fmtDate(e.created_ms) })} onClick={() => restore(e)}>{t('bundle.backup.restore')}</button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        {error && <p className="bnd-detail bnd-detail--warn" role="alert">{error}</p>}
      </div>
    </Frame>
  );
}
