import { Download, RefreshCw, Upload } from 'lucide-react';
import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { getApi } from '@/api';
import type { RuidaApplyReport, RuidaBackup, RuidaCalibration, RuidaControllerSettings, RuidaSettingChange, RuidaSettingsPlan } from '@/api/types';
import { baseName } from '@/features/history/paths';
import { t, type I18nKey } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import { getPref, usePref } from '@/store';
import { Modal } from './Modal';
import { kindsOf, RuidaDiffTable, RuidaVerifyTable, settingName, WarnLine } from './RuidaDiff';
import { openRuidaBackup, saveRuidaBackup } from './ruidaBackupIO';
import { CALIBRATABLE, editText, formatValue, groupSettings, pendingChanges, rangeText, unitKey, type EditError } from './ruidaSettings';
import './MachineSettings.css';
import './RuidaSettings.css';

type View =
  | { kind: 'edit' }
  | { kind: 'review'; plan: RuidaSettingsPlan; changes: RuidaSettingChange[] }
  | { kind: 'restore'; plan: RuidaSettingsPlan; json: string; label: string }
  | { kind: 'report'; report: RuidaApplyReport; restored: boolean };

const unitText = (kind: Parameters<typeof unitKey>[0]) => t(unitKey(kind));

function editError(e: EditError): string {
  switch (e.key) {
    case 'ruida.set.bad.number':
      return t(e.key);
    case 'ruida.set.bad.integer':
      return t(e.key, { unit: e.unit });
    case 'ruida.set.bad.range':
      return t(e.key, { min: e.min, max: e.max, unit: e.unit });
  }
}

/**
 * Ajustes de la controladora Ruida (M25 y M28). **EXPERIMENTAL: sin probar con una controladora
 * real.** Leer, editar, revisar las diferencias, confirmar con la casilla explícita, escribir (el
 * motor guarda antes un respaldo con fecha y relee después) y restaurar un respaldo con las mismas
 * protecciones. También calibra los ejes y exporta/importa un archivo `.tlakuruida`.
 */
export function RuidaSettingsDialog({ onClose }: { onClose: () => void }) {
  const [data, setData] = useState<RuidaControllerSettings | null>(null);
  const [edits, setEdits] = useState<Record<string, string>>({});
  const [view, setView] = useState<View>({ kind: 'edit' });
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [backups, setBackups] = useState<RuidaBackup[]>([]);
  const [confirmed, setConfirmed] = useState(false);
  const writesOn = usePref('ruidaExperimentalWrites');

  const kinds = useMemo(() => kindsOf(data?.settings ?? []), [data]);
  const pending = useMemo(() => (data ? pendingChanges(data.settings, edits, unitText) : { changes: [], errors: {} }), [data, edits]);

  const loadBackups = useCallback(async () => {
    try {
      setBackups(await getApi().ruidaBackups());
    } catch (e) {
      setNotice(t('ruida.set.backup.listFailed', { error: errorMessage(e) }));
    }
  }, []);

  const load = useCallback(async () => {
    setBusy('read');
    setError(null);
    try {
      setData(await getApi().ruidaSettingsRead());
      setEdits({});
      await loadBackups();
    } catch (e) {
      setError(t('ruida.set.loadFailed', { error: errorMessage(e) }));
    } finally {
      setBusy(null);
    }
  }, [loadBackups]);

  useEffect(() => {
    void load();
  }, [load]);

  const goTo = (v: View) => {
    setView(v);
    setConfirmed(false);
    setError(null);
  };

  const review = async () => {
    setBusy('preview');
    setError(null);
    try {
      const plan = await getApi().ruidaSettingsPreview({ changes: pending.changes });
      goTo({ kind: 'review', plan, changes: pending.changes });
    } catch (e) {
      setError(t('ruida.set.review.failed', { error: errorMessage(e) }));
    } finally {
      setBusy(null);
    }
  };

  const write = async () => {
    if (view.kind !== 'review' || !confirmed) return;
    setBusy('write');
    setError(null);
    try {
      const report = await getApi().ruidaSettingsApply({ changes: view.changes, confirmed, experimental_writes: getPref('ruidaExperimentalWrites') });
      goTo({ kind: 'report', report, restored: false });
      await load();
    } catch (e) {
      setError(t('ruida.set.review.failed', { error: errorMessage(e) }));
    } finally {
      setBusy(null);
    }
  };

  /** Prepara la restauración de un respaldo (texto JSON): diferencias con la máquina y a confirmar. */
  const prepareRestore = async (json: string, label: string) => {
    setBusy('preview');
    setError(null);
    try {
      const plan = await getApi().ruidaSettingsPreviewFile({ json });
      goTo({ kind: 'restore', plan, json, label });
    } catch (e) {
      setError(t('ruida.set.restore.failed', { error: errorMessage(e) }));
    } finally {
      setBusy(null);
    }
  };

  const restore = async () => {
    if (view.kind !== 'restore' || !confirmed) return;
    setBusy('write');
    setError(null);
    try {
      const report = await getApi().ruidaSettingsRestore({ json: view.json, confirmed, experimental_writes: getPref('ruidaExperimentalWrites') });
      goTo({ kind: 'report', report, restored: true });
      await load();
    } catch (e) {
      setError(t('ruida.set.restore.failed', { error: errorMessage(e) }));
    } finally {
      setBusy(null);
    }
  };

  const restoreBackupByName = async (name: string) => {
    setBusy('preview');
    try {
      const json = await getApi().ruidaBackupText({ name });
      await prepareRestore(json, name);
    } catch (e) {
      setBusy(null);
      setError(t('ruida.set.backup.readFailed', { error: errorMessage(e) }));
    }
  };

  const exportBackup = async () => {
    setNotice(null);
    try {
      const path = await saveRuidaBackup(await getApi().ruidaSettingsExport());
      if (path) setNotice(t('ruida.set.backup.saved', { path }));
    } catch (e) {
      setNotice(t('ruida.set.backup.saveFailed', { error: errorMessage(e) }));
    }
  };

  const importBackup = async () => {
    setNotice(null);
    try {
      const file = await openRuidaBackup();
      if (file) await prepareRestore(file.text, file.name);
    } catch (e) {
      setNotice(t('ruida.set.backup.readFailed', { error: errorMessage(e) }));
    }
  };

  let body: ReactNode;
  let footer: ReactNode;
  let title = t('ruida.set.title');

  if (view.kind === 'review' || view.kind === 'restore') {
    const isRestore = view.kind === 'restore';
    title = t(isRestore ? 'ruida.set.restore.title' : 'ruida.set.review.title');
    const { plan } = view;
    // Un respaldo de otra tarjeta se puede mirar pero no restaurar; sin diferencias no hay nada que hacer.
    const blocked = plan.changes.length === 0 || (isRestore && plan.warnings.length > 0);
    body = (
      <div className="rset">
        <WarnLine>{t('ruida.set.experimental')}</WarnLine>
        <p>{isRestore ? t('ruida.set.restore.intro', { name: view.label }) : t('ruida.set.review.intro')}</p>
        {plan.warnings.length > 0 && (
          <div role="alert">
            <p><strong>{t('ruida.set.warnings')}</strong></p>
            <ul className="mc-list">{plan.warnings.map((w) => <li key={w}>{w}</li>)}</ul>
          </div>
        )}
        {plan.changes.length === 0 ? <p role="status">{t('ruida.set.review.none')}</p> : <RuidaDiffTable changes={plan.changes} kinds={kinds} />}
        {plan.unchanged > 0 && <p className="hint">{t('ruida.set.review.unchanged', { n: plan.unchanged })}</p>}
        <label className="rset-confirm">
          <input type="checkbox" checked={confirmed} disabled={blocked || !writesOn} onChange={(e) => setConfirmed(e.target.checked)} />
          {t('ruida.set.review.confirm')}
        </label>
        {error && <p className="hint hint--danger" role="alert">{error}</p>}
      </div>
    );
    footer = (
      <>
        <button type="button" className="btn" data-autofocus onClick={() => goTo({ kind: 'edit' })}>{t('ruida.set.review.back')}</button>
        <button type="button" className="btn btn--accent" disabled={!writesOn || !confirmed || blocked || busy !== null} title={writesOn ? undefined : t('ruida.exp.off')} onClick={() => void (isRestore ? restore() : write())}>
          {busy === 'write' ? t('ruida.set.review.writing') : t(isRestore ? 'ruida.set.restore.write' : 'ruida.set.review.write')}
        </button>
      </>
    );
  } else if (view.kind === 'report') {
    const { report, restored } = view;
    const back = report.backup ? baseName(report.backup) : null;
    body = (
      <div className="rset">
        <WarnLine>{t('ruida.set.experimental')}</WarnLine>
        {report.changes.length === 0 ? (
          <p role="status">{t('ruida.set.report.noChanges')}</p>
        ) : report.ok ? (
          <p role="status">{t(restored ? 'ruida.set.restore.ok' : 'ruida.set.report.ok', { n: report.changes.length })}</p>
        ) : (
          <WarnLine role="alert">{t('ruida.set.report.mismatch')}</WarnLine>
        )}
        {report.write_error && <WarnLine role="alert">{t('ruida.set.report.writeError', { error: report.write_error })}</WarnLine>}
        {report.backup && <p className="hint">{t('ruida.set.report.backup', { path: report.backup })}</p>}
        {report.verified.length > 0 && <RuidaVerifyTable rows={report.verified} kinds={kinds} />}
        {error && <p className="hint hint--danger" role="alert">{error}</p>}
      </div>
    );
    footer = (
      <>
        {report.restore_advised && back && (
          <button type="button" className="btn btn--accent" disabled={busy !== null || !writesOn} title={writesOn ? undefined : t('ruida.exp.off')} onClick={() => void restoreBackupByName(back)}>
            {t('ruida.set.report.restore')}
          </button>
        )}
        <button type="button" className="btn" data-autofocus onClick={() => goTo({ kind: 'edit' })}>{t('ruida.set.report.close')}</button>
      </>
    );
  } else {
    body = (
      <div className="rset">
        <WarnLine>
          <strong>{t('ruida.set.experimental')}.</strong> {t('ruida.set.intro')}
        </WarnLine>
        <p className="hint">{t('ruida.set.rangeNote')}</p>
        {!writesOn && <WarnLine>{t('ruida.exp.off')}</WarnLine>}
        <div className="mset-toolbar">
          <button type="button" className="btn" disabled={busy !== null} onClick={() => void load()}>
            <RefreshCw size={13} aria-hidden="true" />
            {busy === 'read' ? t('ruida.set.reading') : t('ruida.set.read')}
          </button>
          <button type="button" className="btn" disabled={!data || busy !== null} title={t('ruida.set.backup.exportHelp')} onClick={() => void exportBackup()}>
            <Download size={13} aria-hidden="true" />
            {t('ruida.set.backup.export')}
          </button>
          <button type="button" className="btn" disabled={!data || busy !== null} title={t('ruida.set.backup.importHelp')} onClick={() => void importBackup()}>
            <Upload size={13} aria-hidden="true" />
            {t('ruida.set.backup.import')}
          </button>
        </div>
        {notice && <p className="hint" role="status">{notice}</p>}
        {error && <p className="hint hint--danger" role="alert">{error}</p>}
        {data && (
          <>
            <SettingsTable writesOn={writesOn} data={data} edits={edits} errors={pending.errors} onEdit={(key, text) => setEdits((e) => ({ ...e, [key]: text }))} />
            <p className="hint">{t('ruida.set.rotary.note')}</p>
            <Calibration
              writesOn={writesOn}
              data={data}
              onUse={(c) => {
                const kind = kinds.get(c.key) ?? 'step';
                setEdits((e) => ({ ...e, [c.key]: editText(kind, c.proposed) }));
                setNotice(t('ruida.cal.used'));
              }}
            />
            <BackupList writesOn={writesOn} backups={backups} busy={busy !== null} onUse={(b) => void restoreBackupByName(b.name)} />
          </>
        )}
      </div>
    );
    footer = (
      <>
        <span className="hint" role="status">{pending.changes.length > 0 ? t('ruida.set.pending', { n: pending.changes.length }) : ''}</span>
        <button type="button" className="btn" disabled={Object.keys(edits).length === 0} onClick={() => setEdits({})}>{t('ruida.set.discard')}</button>
        <button type="button" className="btn btn--accent" disabled={!writesOn || pending.changes.length === 0 || Object.keys(pending.errors).length > 0 || busy !== null} title={writesOn ? undefined : t('ruida.exp.off')} onClick={() => void review()}>
          {t('ruida.set.review')}
        </button>
        <button type="button" className="btn" data-autofocus onClick={onClose}>{t('machine.dlg.close')}</button>
      </>
    );
  }

  return (
    <Modal title={title} subtitle={data ? `${data.model ?? t('ruida.info.modelUnknown')} · ${data.card_id}` : undefined} size="lg" tone="warn" onClose={onClose} footer={footer}>
      {body}
    </Modal>
  );
}

function SettingsTable({ writesOn, data, edits, errors, onEdit }: { writesOn: boolean; data: RuidaControllerSettings; edits: Record<string, string>; errors: Record<string, EditError>; onEdit: (key: string, text: string) => void }) {
  return (
    <table className="mc-table mset-table" aria-label={t('ruida.set.title')}>
      <thead>
        <tr>
          <th scope="col">{t('ruida.set.col.name')}</th>
          <th scope="col">{t('ruida.set.col.value')}</th>
          <th scope="col">{t('ruida.set.col.new')}</th>
          <th scope="col">{t('ruida.set.col.range')}</th>
        </tr>
      </thead>
      {groupSettings(data.settings).map(([group, list]) => (
        <tbody key={group}>
          <tr className="rset-group">
            <th scope="colgroup" colSpan={4}>{t(`ruida.set.group.${group}` as I18nKey)}</th>
          </tr>
          {list.map((s) => {
            const unit = unitText(s.kind);
            const err = errors[s.key];
            const label = t('ruida.set.edit', { name: settingName(s.key) });
            return (
              <tr key={s.key}>
                <th scope="row">
                  {settingName(s.key)}
                  <span className="rset-source">{s.addr} · {s.source_name}</span>
                </th>
                <td className="num">{s.value === null ? t('ruida.set.notReported') : formatValue(s.kind, s.value, unit)}</td>
                <td>
                  <input
                    className="input rset-edit num"
                    type="text"
                    inputMode="decimal"
                    value={edits[s.key] ?? ''}
                    placeholder={s.value === null ? '' : editText(s.kind, s.value)}
                    aria-label={label}
                    aria-invalid={err ? true : undefined}
                    disabled={s.value === null || !writesOn}
                    title={writesOn ? undefined : t('ruida.exp.off')}
                    onChange={(e) => onEdit(s.key, e.target.value)}
                  />
                  {s.kind === 'raw' && <span className="rset-source">{unit}</span>}
                  {err && <span className="rset-err" role="alert">{editError(err)}</span>}
                </td>
                <td className="num">{rangeText(s, unit)}</td>
              </tr>
            );
          })}
        </tbody>
      ))}
    </table>
  );
}

/** Calibrar un eje: pedido y medido dan el paso nuevo, que se carga en la tabla para revisarlo y escribirlo. */
function Calibration({ writesOn, data, onUse }: { writesOn: boolean; data: RuidaControllerSettings; onUse: (c: RuidaCalibration) => void }) {
  const [key, setKey] = useState<string>(CALIBRATABLE[0]);
  const [commanded, setCommanded] = useState('100');
  const [measured, setMeasured] = useState('');
  const [result, setResult] = useState<RuidaCalibration | null>(null);
  const [error, setError] = useState<string | null>(null);
  const axes = CALIBRATABLE.filter((k) => data.settings.some((s) => s.key === k && s.value !== null && s.value > 0));
  const num = (s: string) => Number(s.trim().replace(',', '.'));

  const compute = async () => {
    setError(null);
    setResult(null);
    try {
      setResult(await getApi().ruidaCalibrate({ key, commanded_mm: num(commanded), measured_mm: num(measured), experimental_writes: getPref('ruidaExperimentalWrites') }));
    } catch (e) {
      setError(t('ruida.cal.failed', { error: errorMessage(e) }));
    }
  };

  return (
    <section aria-labelledby="rset-cal-title">
      <h3 id="rset-cal-title">{t('ruida.cal.title')}</h3>
      <p className="hint">{t('ruida.cal.intro')}</p>
      <WarnLine>{t('ruida.cal.warn')}</WarnLine>
      <div className="rset-cal">
        <label>
          {t('ruida.cal.axis')}
          <select className="select" value={key} onChange={(e) => { setKey(e.target.value); setResult(null); }}>
            {axes.map((k) => <option key={k} value={k}>{settingName(k)}</option>)}
          </select>
        </label>
        <label>
          {t('ruida.cal.commanded')}
          <input className="input num" inputMode="decimal" value={commanded} onChange={(e) => { setCommanded(e.target.value); setResult(null); }} />
        </label>
        <label>
          {t('ruida.cal.measured')}
          <input className="input num" inputMode="decimal" value={measured} onChange={(e) => { setMeasured(e.target.value); setResult(null); }} />
        </label>
        <button type="button" className="btn" disabled={!writesOn || axes.length === 0 || measured.trim() === ''} title={writesOn ? undefined : t('ruida.exp.off')} onClick={() => void compute()}>{t('ruida.cal.compute')}</button>
      </div>
      {error && <p className="hint hint--danger" role="alert">{error}</p>}
      {result && (
        <>
          <p role="status">
            {t('ruida.cal.result', {
              current: fmt(result.current, 0),
              proposed: fmt(result.proposed, 0),
              pct: fmt((result.ratio - 1) * 100, 2),
              before: fmt(result.steps_per_mm_before, 4),
              after: fmt(result.steps_per_mm_after, 4),
            })}
          </p>
          <button type="button" className="btn" onClick={() => onUse(result)}>{t('ruida.cal.use')}</button>
        </>
      )}
    </section>
  );
}

function BackupList({ writesOn, backups, busy, onUse }: { writesOn: boolean; backups: RuidaBackup[]; busy: boolean; onUse: (b: RuidaBackup) => void }) {
  return (
    <section aria-labelledby="rset-bk-title">
      <h3 id="rset-bk-title">{t('ruida.set.backup.auto')}</h3>
      <p className="hint">{t('ruida.set.backup.autoHelp')}</p>
      {backups.length === 0 ? (
        <p className="hint" role="status">{t('ruida.set.backup.none')}</p>
      ) : (
        <ul className="mc-list rset-backups">
          {backups.map((b) => (
            <li key={b.name}>
              <span className="num">{b.name}</span>
              <button type="button" className="btn" disabled={busy || !writesOn} title={writesOn ? undefined : t('ruida.exp.off')} onClick={() => onUse(b)}>{t('ruida.set.backup.use')}</button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
