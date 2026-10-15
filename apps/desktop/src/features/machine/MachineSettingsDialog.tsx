import { Download, RefreshCw, Search, TriangleAlert, Upload } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { getApi } from '@/api';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';
import { selectFirmwareFamily } from '@/store/machine';
import { firmwareFamily } from './grblCodes';
import { isCriticalSetting } from './grblSettingsInfo';
import { EditConfirmBody, RestorePreviewBody } from './MachineSettingsConfirm';
import { openBackupFile, readMachineSettings, saveBackupFile } from './MachineSettingsIO';
import {
  buildBackupText,
  diffBackup,
  needsSoftLimits,
  parseBackupText,
  settingLine,
  validateSettingValue,
  type BackupDiff,
  type ParsedSettings,
} from './MachineSettingsLogic';
import { GrblAxisCalibration } from './GrblAxisCalibration';
import { outcomeNotice, type SettingsNotice } from './MachineSettingsReport';
import { MachineSettingsTable, type EditingState } from './MachineSettingsTable';
import { settingsBlockedReason, verifyWritten, writeSettingLines } from './MachineSettingsWrite';
import { Modal } from './Modal';
import './MachineSettings.css';

type View =
  | { kind: 'list' }
  | { kind: 'confirm'; id: number; previous: string; line: string }
  | { kind: 'restore'; fileName: string; backupFirmware: string | null; familyMismatch: boolean; diff: BackupDiff; selected: ReadonlySet<number> }
  | { kind: 'busy'; done: number; total: number };

/**
 * Ventana «Ajustes de la máquina»: lee `$$`, muestra cada ajuste con su nombre y unidad, permite
 * editarlo (con confirmación de la línea exacta) y hacer o restaurar un respaldo. Toda escritura
 * va de una en una y se detiene en la primera que no conteste `ok`.
 */
export function MachineSettingsDialog() {
  const close = useStore((s) => s.closeDialog);
  const firmware = useStore((s) => s.firmware);
  const family = useStore(selectFirmwareFamily);
  const blocked = useStore(settingsBlockedReason);

  const [data, setData] = useState<ParsedSettings | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [view, setView] = useState<View>({ kind: 'list' });
  const [editing, setEditing] = useState<EditingState | null>(null);
  const [filter, setFilter] = useState('');
  const [notice, setNotice] = useState<SettingsNotice | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const busy = view.kind === 'busy';

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      setData(await readMachineSettings());
    } catch (e) {
      setLoadError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // El foco inicial lo pone `Modal` al abrir; al cambiar de vista o empezar a editar se mueve a mano.
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    const card = rootRef.current?.closest('.mc-modal');
    const target = editing ? card?.querySelector<HTMLElement>('.mset-edit input') : card?.querySelector<HTMLElement>('[data-autofocus]');
    target?.focus();
  }, [view.kind, editing?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  /** Escribe las líneas una a una, relee `$$`, verifica y deja el resultado a la vista. */
  const runWrite = async (lines: string[]) => {
    setEditing(null);
    setNotice(null);
    // Respaldo automático ANTES de escribir: si no se puede guardar, no se escribe nada.
    let autoPath: string | null = null;
    if (data) {
      try {
        autoPath = await getApi().grblSettingsAutobackup({ text: buildBackupText(data.settings, firmware, new Date()) });
      } catch (e) {
        setNotice({ tone: 'error', text: t('mset.auto.failed', { error: errorMessage(e) }) });
        setView({ kind: 'list' });
        return;
      }
    }
    setView({ kind: 'busy', done: 0, total: lines.length });
    const outcome = await writeSettingLines(lines, { onProgress: (done, total) => setView({ kind: 'busy', done, total }) });
    let mismatches = null;
    try {
      const reread = await readMachineSettings();
      setData(reread);
      mismatches = verifyWritten(outcome.written, reread.settings);
      // El resto de la aplicación (S máx, área...) también lee estos valores.
      await useStore.getState().readSettings({ silent: true });
    } catch (e) {
      setLoadError(errorMessage(e));
    }
    const done = outcomeNotice(outcome, family, mismatches);
    setNotice(autoPath ? { ...done, details: [...(done.details ?? []), t('mset.auto.saved', { path: autoPath })] } : done);
    setView({ kind: 'list' });
  };

  const startEdit = (id: number) => {
    const current = data?.settings.find((s) => s.id === id);
    if (current) setEditing({ id, text: current.raw });
    setNotice(null);
  };

  /** Lleva a la confirmación normal de `$20=1` (con el respaldo automático al escribir): nunca se escribe solo. */
  const enableSoftLimits = () => {
    const current = data?.settings.find((s) => s.id === 20);
    if (!current) return;
    setEditing(null);
    setNotice(null);
    setView({ kind: 'confirm', id: 20, previous: current.raw, line: settingLine(20, '1') });
  };

  const review = () => {
    if (!editing || !data) return;
    const verdict = validateSettingValue(editing.id, editing.text, family);
    const current = data.settings.find((s) => s.id === editing.id);
    if (!verdict.ok || !current) return;
    setView({ kind: 'confirm', id: editing.id, previous: current.raw, line: settingLine(editing.id, verdict.text) });
  };

  const exportBackup = async () => {
    if (!data) return;
    try {
      const path = await saveBackupFile(buildBackupText(data.settings, firmware, new Date()));
      if (path) setNotice({ tone: 'ok', text: t('mset.backup.saved', { path, n: data.settings.length }) });
    } catch (e) {
      setNotice({ tone: 'error', text: t('mset.backup.saveFailed', { error: errorMessage(e) }) });
    }
  };

  const restore = async () => {
    if (!data) return;
    setNotice(null);
    try {
      const file = await openBackupFile();
      if (!file) return;
      const backup = parseBackupText(file.text);
      if (backup.entries.length === 0) {
        setNotice({ tone: 'error', text: t('mset.backup.empty', { file: file.name }) });
        return;
      }
      const diff = diffBackup(data.settings, backup.entries, family);
      const backupFamily = backup.firmware ? firmwareFamily(backup.firmware) : 'unknown';
      setView({
        kind: 'restore',
        fileName: file.name,
        backupFirmware: backup.firmware,
        familyMismatch: backupFamily !== 'unknown' && family !== 'unknown' && backupFamily !== family,
        diff,
        selected: new Set(diff.changes.map((c) => c.id)),
      });
    } catch (e) {
      setNotice({ tone: 'error', text: t('mset.backup.readFailed', { error: errorMessage(e) }) });
    }
  };

  const toggle = (id: number) => {
    if (view.kind !== 'restore') return;
    const next = new Set(view.selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setView({ ...view, selected: next });
  };

  const onClose = () => {
    if (!busy) close();
  };

  let body: React.ReactNode;
  let footer: React.ReactNode;
  let title = t('mset.title');
  let tone: 'danger' | undefined;

  if (view.kind === 'confirm') {
    title = t('mset.confirm.title');
    tone = 'danger';
    body = <EditConfirmBody id={view.id} previous={view.previous} line={view.line} />;
    footer = (
      <>
        <button type="button" className="btn" data-autofocus onClick={() => setView({ kind: 'list' })}>{t('machine.dlg.cancel')}</button>
        <button
          type="button"
          className="btn btn--danger"
          disabled={blocked !== null}
          title={blocked ? t(blocked) : undefined}
          onClick={() => void runWrite([view.line])}
        >
          {t('mset.confirm.send', { line: view.line })}
        </button>
      </>
    );
  } else if (view.kind === 'restore') {
    title = t('mset.restore.title');
    tone = 'danger';
    const chosen = view.diff.changes.filter((c) => view.selected.has(c.id));
    body = (
      <RestorePreviewBody
        fileName={view.fileName}
        backupFirmware={view.backupFirmware}
        familyMismatch={view.familyMismatch}
        diff={view.diff}
        selected={view.selected}
        onToggle={toggle}
        onToggleAll={(on) => setView({ ...view, selected: new Set(on ? view.diff.changes.map((c) => c.id) : []) })}
      />
    );
    footer = (
      <>
        <button type="button" className="btn" data-autofocus onClick={() => setView({ kind: 'list' })}>{t('machine.dlg.cancel')}</button>
        <button
          type="button"
          className="btn btn--danger"
          disabled={chosen.length === 0 || blocked !== null}
          title={blocked ? t(blocked) : undefined}
          onClick={() => void runWrite(chosen.map((c) => c.line))}
        >
          {t('mset.restore.send', { n: chosen.length })}
        </button>
      </>
    );
  } else if (view.kind === 'busy') {
    body = (
      <p role="status" aria-live="polite">
        {t('mset.busy', { done: view.done, total: view.total })}
      </p>
    );
    footer = <button type="button" className="btn" disabled>{t('machine.dlg.close')}</button>;
  } else {
    const criticalCount = data?.settings.filter((s) => isCriticalSetting(s.id)).length ?? 0;
    body = (
      <>
        <p className="hint">{t('mset.intro')}</p>
        {family === 'fluidnc' && (
          <p className="warn-line">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('mset.note.fluidnc')}</span>
          </p>
        )}
        {data && needsSoftLimits(data.settings) && (
          <div className="warn-line mset-softlimits" role="status">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('mset.softLimits.text')}</span>
            <button type="button" className="btn" disabled={blocked !== null} title={blocked ? t(blocked) : undefined} onClick={enableSoftLimits}>
              {t('mset.softLimits.button')}
            </button>
          </div>
        )}
        {data && data.namedLines > 0 && <p className="hint">{t('mset.note.named', { n: data.namedLines })}</p>}
        {blocked && (
          <p className="warn-line" role="status">
            <TriangleAlert size={14} aria-hidden="true" />
            <span>{t('mset.blocked', { reason: t(blocked) })}</span>
          </p>
        )}
        <div className="mset-toolbar">
          <label className="mset-search">
            <Search size={14} aria-hidden="true" />
            <input
              className="input"
              type="search"
              value={filter}
              data-autofocus
              placeholder={t('mset.search')}
              aria-label={t('mset.search')}
              onChange={(e) => setFilter(e.target.value)}
              onKeyDown={(e) => {
                // Escape borra la búsqueda antes de cerrar la ventana.
                if (e.key === 'Escape' && filter !== '') {
                  e.preventDefault();
                  e.stopPropagation();
                  setFilter('');
                }
              }}
            />
          </label>
          <button type="button" className="btn" disabled={loading || blocked !== null} title={blocked ? t(blocked) : t('mset.reload.help')} onClick={() => void load()}>
            <RefreshCw size={13} aria-hidden="true" />
            {t('mset.reload')}
          </button>
          <button type="button" className="btn" disabled={!data} onClick={() => void exportBackup()}>
            <Download size={13} aria-hidden="true" />
            {t('mset.backup.export')}
          </button>
          <button type="button" className="btn" disabled={!data || blocked !== null} title={blocked ? t(blocked) : undefined} onClick={() => void restore()}>
            <Upload size={13} aria-hidden="true" />
            {t('mset.backup.restore')}
          </button>
        </div>
        {notice && (
          <div className="mset-notice" data-tone={notice.tone} role={notice.tone === 'error' ? 'alert' : 'status'}>
            <p>{notice.text}</p>
            {notice.details && (
              <ul className="mc-list num">
                {notice.details.map((d, i) => <li key={`${i}-${d}`}>{d}</li>)}
              </ul>
            )}
          </div>
        )}
        {loading && !data && <p role="status">{t('mset.loading')}</p>}
        {loadError && (
          <p className="hint hint--danger" role="alert">{t('mset.loadFailed', { error: loadError })}</p>
        )}
        {data && (
          <>
            <p className="hint">{t('mset.count', { n: data.settings.length, critical: criticalCount })}</p>
            <GrblAxisCalibration settings={data.settings} disabled={blocked !== null} onUse={(id, text) => { setEditing({ id, text }); setNotice(null); }} />
            <MachineSettingsTable
              settings={data.settings}
              family={family}
              filter={filter}
              blocked={blocked}
              editing={editing}
              onEditStart={startEdit}
              onEditChange={(text) => setEditing((e) => (e ? { ...e, text } : e))}
              onEditCancel={() => setEditing(null)}
              onReview={review}
            />
          </>
        )}
      </>
    );
    footer = <button type="button" className="btn" onClick={onClose}>{t('machine.dlg.close')}</button>;
  }

  return (
    <Modal title={title} subtitle={firmware ?? undefined} size="lg" tone={tone} onClose={onClose} footer={footer}>
      <div className="mset" ref={rootRef}>{body}</div>
    </Modal>
  );
}
