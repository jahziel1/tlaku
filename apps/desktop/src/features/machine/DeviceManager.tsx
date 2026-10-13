import { Copy, Download, Plus, Trash2, TriangleAlert } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { DeviceProfile } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import {
  applyProposal,
  BAUD_RATES,
  duplicateDeviceProfile,
  newDeviceProfile,
  proposeFromSettings,
  sameDevice,
  validateDevice,
  type DeviceProposal,
  type ProposalChange,
} from '@/lib/device';
import { fmt } from '@/lib/platform';
import { useStore } from '@/store';
import { Field, NumInput } from './DeviceFields';
import { Modal } from './Modal';

const ORIGINS: { id: DeviceProfile['origin']; label: I18nKey }[] = [
  { id: 'bottom_left', label: 'machine.dev.origin.bottom_left' },
  { id: 'top_left', label: 'machine.dev.origin.top_left' },
  { id: 'bottom_right', label: 'machine.dev.origin.bottom_right' },
  { id: 'top_right', label: 'machine.dev.origin.top_right' },
];

const FIELD_LABEL: Record<ProposalChange['field'], I18nKey> = {
  bed: 'machine.dev.bed',
  max_rate: 'machine.dev.maxRate',
  accel: 'machine.dev.accel',
  s_max: 'machine.dev.sMax',
};

const show = (v: number | [number, number]) => (typeof v === 'number' ? fmt(v, 0) : `${fmt(v[0], 0)} × ${fmt(v[1], 0)}`);

/** Qué se propone tras leer la máquina: cambios marcados para aplicar y el estado del modo láser. */
function ProposalPanel({
  proposal,
  onApply,
  onDismiss,
}: {
  proposal: DeviceProposal;
  onApply: (changes: ProposalChange[]) => void;
  onDismiss: () => void;
}) {
  const [checked, setChecked] = useState(() => new Set(proposal.changes.map((c) => c.field)));
  return (
    <div className="mc-proposal" role="region" aria-label={t('machine.dev.read.title')}>
      <h3 className="mc-sub">{t('machine.dev.read.title')}</h3>
      {proposal.nothingFound && <p className="warn-line"><TriangleAlert size={14} aria-hidden="true" /><span>{t('machine.dev.read.nothing')}</span></p>}
      {proposal.inSync && <p className="hint">{t('machine.dev.read.inSync')}</p>}
      {proposal.changes.length > 0 && (
        <table className="mc-table">
          <thead>
            <tr>
              <th scope="col"><span className="sr-only">{t('machine.dev.read.apply')}</span></th>
              <th scope="col">{t('machine.dev.read.field')}</th>
              <th scope="col">{t('machine.dev.read.current')}</th>
              <th scope="col">{t('machine.dev.read.machine')}</th>
            </tr>
          </thead>
          <tbody>
            {proposal.changes.map((c) => (
              <tr key={c.field}>
                <td>
                  <input
                    type="checkbox"
                    aria-label={t(FIELD_LABEL[c.field])}
                    checked={checked.has(c.field)}
                    onChange={(e) => {
                      const next = new Set(checked);
                      if (e.target.checked) next.add(c.field);
                      else next.delete(c.field);
                      setChecked(next);
                    }}
                  />
                </td>
                <td>{t(FIELD_LABEL[c.field])} <span className="num mc-faint">{c.codes}</span></td>
                <td className="num">{show(c.from)}</td>
                <td className="num mc-new">{show(c.to)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {proposal.laserMode === false && (
        <p className="warn-line mc-laserwarn" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('machine.laserModeOff')}</span>
        </p>
      )}
      {proposal.laserMode === true && <p className="hint">{t('machine.dev.read.laserOk')}</p>}
      {proposal.laserMode === null && !proposal.nothingFound && <p className="hint">{t('machine.dev.read.laserUnknown')}</p>}
      <p className="hint">{t('machine.dev.read.confirmNote')}</p>
      <div className="row">
        <button type="button" className="btn btn--accent" disabled={checked.size === 0 || proposal.changes.length === 0} onClick={() => onApply(proposal.changes.filter((c) => checked.has(c.field)))}>
          {t('machine.dev.read.apply')}
        </button>
        <button type="button" className="btn" onClick={onDismiss}>{t('machine.dev.read.dismiss')}</button>
      </div>
    </div>
  );
}

/** Gestor de dispositivos: lista, crear, editar, duplicar, borrar y leer los ajustes de la máquina. */
export function DeviceManager() {
  const devices = useStore((s) => s.devices);
  const activeId = useStore((s) => s.activeDeviceId);
  const connected = useStore((s) => s.connection.connected);
  const jobBusy = useStore((s) => s.jobState === 'running' || s.jobState === 'paused');
  const close = useStore((s) => s.closeDialog);
  const saveDevice = useStore((s) => s.saveDevice);
  const removeDevice = useStore((s) => s.removeDevice);
  const setActiveDevice = useStore((s) => s.setActiveDevice);
  const setBaud = useStore((s) => s.setBaud);
  const readSettings = useStore((s) => s.readSettings);

  const [draft, setDraft] = useState<DeviceProfile>(() => devices.find((d) => d.id === activeId) ?? devices[0] ?? newDeviceProfile(t('machine.dev.newName'), []));
  const [isNew, setIsNew] = useState(devices.length === 0);
  const [proposal, setProposal] = useState<DeviceProposal | null>(null);
  const [reading, setReading] = useState(false);
  const [saving, setSaving] = useState(false);
  /** Acción que espera a que el usuario decida qué hacer con los cambios sin guardar. */
  const [guard, setGuard] = useState<{ title: I18nKey; run: () => void } | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const saved = devices.find((d) => d.id === draft.id) ?? null;
  const dirty = isNew || !saved || !sameDevice(draft, saved);
  const issues = useMemo(() => validateDevice(draft), [draft]);
  const invalid = Object.keys(issues).length > 0;
  const isActive = draft.id === activeId;

  // Si la lista cambia desde fuera (otro borrado), el borrador sigue apuntando a un perfil que existe.
  useEffect(() => {
    if (!isNew && devices.length > 0 && !devices.some((d) => d.id === draft.id)) setDraft(devices.find((d) => d.id === activeId) ?? devices[0]!);
  }, [devices, draft.id, isNew, activeId]);

  const edit = (patch: Partial<DeviceProfile>) => setDraft((d) => ({ ...d, ...patch }));
  /** Pide confirmación si hay cambios sin guardar antes de hacer `run`. */
  const afterGuard = (title: I18nKey, run: () => void) => (dirty ? setGuard({ title, run }) : run());
  const select = (d: DeviceProfile) => {
    setDraft(d);
    setIsNew(false);
    setProposal(null);
  };

  const create = () =>
    afterGuard('machine.dev.discard', () => {
      setDraft(newDeviceProfile(t('machine.dev.newName'), devices.map((d) => d.id)));
      setIsNew(true);
      setProposal(null);
    });
  const duplicate = () =>
    afterGuard('machine.dev.discard', () => {
      setDraft(duplicateDeviceProfile(draft, t('machine.dev.copySuffix'), devices.map((d) => d.id)));
      setIsNew(true);
      setProposal(null);
    });

  const save = async () => {
    if (invalid) return;
    setSaving(true);
    const ok = await saveDevice({ ...draft, name: draft.name.trim() });
    setSaving(false);
    if (!ok) return;
    setIsNew(false);
    if (draft.id === useStore.getState().activeDeviceId && !connected) setBaud(draft.baud);
  };

  const remove = async () => {
    setConfirmDelete(false);
    const next = devices.find((d) => d.id !== draft.id);
    if (await removeDevice(draft.id) && next) select(next);
  };

  const read = async () => {
    setReading(true);
    const settings = await readSettings();
    setReading(false);
    if (settings) setProposal(proposeFromSettings(settings, draft));
  };

  const readBlocked = !connected ? t('reason.needMachine') : jobBusy ? t('reason.jobRunning') : null;
  const deleteBlocked = isNew ? t('machine.dev.notSaved') : devices.length <= 1 ? t('machine.dev.lastOne') : connected && isActive ? t('machine.dev.inUse') : null;

  const requestClose = () => afterGuard('machine.dev.discard', close);

  return (
    <>
      <Modal
        title={t('machine.dev.title')}
        subtitle={t('machine.dev.subtitle')}
        size="lg"
        onClose={requestClose}
        footer={
          <>
            {dirty && <span className="hint mc-footnote" role="status">{invalid ? t('machine.dev.fixFields') : t('machine.dev.unsaved')}</span>}
            <span className="topbar__spacer" />
            <button type="button" className="btn" disabled={!dirty || !saved} title={dirty && saved ? undefined : t('machine.dev.nothingToRevert')} onClick={() => saved && select(saved)}>
              {t('machine.dev.revert')}
            </button>
            <button type="button" className="btn btn--accent" disabled={!dirty || invalid || saving} title={invalid ? t('machine.dev.fixFields') : !dirty ? t('machine.dev.nothingToSave') : undefined} onClick={() => void save()}>
              {isNew ? t('machine.dev.create') : t('machine.dev.save')}
            </button>
            <button type="button" className="btn" onClick={requestClose}>{t('machine.dlg.close')}</button>
          </>
        }
      >
        <div className="mc-devmgr">
          <nav className="mc-devlist" aria-label={t('machine.dev.list')}>
            <ul>
              {devices.map((d) => (
                <li key={d.id}>
                  <button type="button" className="mc-devitem" aria-current={!isNew && d.id === draft.id ? 'true' : undefined} onClick={() => d.id !== draft.id && afterGuard('machine.dev.discard', () => select(d))}>
                    <span className="mc-devitem__name">{d.name}</span>
                    <span className="mc-devitem__meta num">{d.bed[0]} × {d.bed[1]} mm{d.id === activeId ? ` · ${t('machine.dev.inUseBadge')}` : ''}</span>
                  </button>
                </li>
              ))}
              {isNew && (
                <li>
                  <span className="mc-devitem" aria-current="true">
                    <span className="mc-devitem__name">{draft.name || t('machine.dev.newName')}</span>
                    <span className="mc-devitem__meta">{t('machine.dev.notSavedBadge')}</span>
                  </span>
                </li>
              )}
            </ul>
            <div className="mc-devlist__actions">
              <button type="button" className="btn" onClick={create}><Plus size={14} aria-hidden="true" />{t('machine.dev.new')}</button>
              <button type="button" className="btn" onClick={duplicate}><Copy size={14} aria-hidden="true" />{t('machine.dev.duplicate')}</button>
              <button type="button" className="btn btn--danger" disabled={deleteBlocked !== null} title={deleteBlocked ?? undefined} onClick={() => setConfirmDelete(true)}>
                <Trash2 size={14} aria-hidden="true" />{t('machine.dev.delete')}
              </button>
            </div>
          </nav>

          <form className="mc-devform" onSubmit={(e) => { e.preventDefault(); void save(); }} aria-label={t('machine.dev.form')}>
            <div className="mc-readbar">
              <button type="button" className="btn" disabled={readBlocked !== null || reading} title={readBlocked ?? t('machine.dev.read.hint')} onClick={() => void read()}>
                <Download size={14} aria-hidden="true" />{reading ? t('machine.dev.read.reading') : t('machine.dev.read')}
              </button>
              {readBlocked && <span className="hint">{readBlocked}</span>}
              {!isActive && !isNew && (
                <button type="button" className="btn" disabled={connected} title={connected ? t('machine.disconnectFirst') : undefined} onClick={() => { setActiveDevice(draft.id); setBaud(draft.baud); }}>
                  {t('machine.dev.use')}
                </button>
              )}
            </div>
            {proposal && (
              <ProposalPanel
                proposal={proposal}
                onDismiss={() => setProposal(null)}
                onApply={(changes) => {
                  setDraft((d) => applyProposal(d, changes));
                  setProposal(null);
                }}
              />
            )}

            <fieldset className="mc-group">
              <legend>{t('machine.dev.group.general')}</legend>
              <Field label={t('machine.dev.name')} help="machine.dev.name.help" error={issues.name}>
                {({ id, describedBy, invalid: bad }) => (
                  <input id={id} className="input" value={draft.name} aria-describedby={describedBy} aria-invalid={bad || undefined} onChange={(e) => edit({ name: e.target.value })} />
                )}
              </Field>
              <div className="field-grid">
                <Field label={t('machine.dev.kind')} help="machine.dev.kind.help">
                  {({ id, describedBy }) => (
                    <select id={id} className="select" value={draft.kind} aria-describedby={describedBy} onChange={(e) => edit({ kind: e.target.value as DeviceProfile['kind'] })}>
                      <option value="grbl">GRBL</option>
                    </select>
                  )}
                </Field>
                <Field label={t('machine.dev.origin')} help="machine.dev.origin.help">
                  {({ id, describedBy }) => (
                    <select id={id} className="select" value={draft.origin} aria-describedby={describedBy} onChange={(e) => edit({ origin: e.target.value as DeviceProfile['origin'] })}>
                      {ORIGINS.map((o) => (
                        <option key={o.id} value={o.id}>{t(o.label)}</option>
                      ))}
                    </select>
                  )}
                </Field>
              </div>
            </fieldset>

            <fieldset className="mc-group">
              <legend>{t('machine.dev.group.area')}</legend>
              <Field label={t('machine.dev.bed')} help="machine.dev.bed.help" error={issues.bed}>
                {({ id, describedBy, invalid: bad }) => (
                  <div className="field-grid">
                    <NumInput id={id} label={t('machine.dev.bed.w')} unit="mm" value={draft.bed[0]} describedBy={describedBy} invalid={bad} onCommit={(v) => edit({ bed: [v, draft.bed[1]] })} />
                    <NumInput label={t('machine.dev.bed.h')} unit="mm" value={draft.bed[1]} describedBy={describedBy} invalid={bad} onCommit={(v) => edit({ bed: [draft.bed[0], v] })} />
                  </div>
                )}
              </Field>
            </fieldset>

            <fieldset className="mc-group">
              <legend>{t('machine.dev.group.laser')}</legend>
              <div className="field-grid">
                <Field label={t('machine.dev.sMax')} help="machine.dev.sMax.help" error={issues.s_max}>
                  {({ id, describedBy, invalid: bad }) => <NumInput id={id} value={draft.s_max} describedBy={describedBy} invalid={bad} onCommit={(v) => edit({ s_max: v })} />}
                </Field>
                <Field label={t('machine.dev.powerMode')} help="machine.dev.powerMode.help">
                  {({ id, describedBy }) => (
                    <select id={id} className="select" value={draft.power_mode} aria-describedby={describedBy} onChange={(e) => edit({ power_mode: e.target.value as DeviceProfile['power_mode'] })}>
                      <option value="dynamic">{t('machine.dev.powerMode.dynamic')}</option>
                      <option value="constant">{t('machine.dev.powerMode.constant')}</option>
                    </select>
                  )}
                </Field>
              </div>
            </fieldset>

            <fieldset className="mc-group">
              <legend>{t('machine.dev.group.motion')}</legend>
              <Field label={t('machine.dev.maxRate')} help="machine.dev.maxRate.help" error={issues.max_rate}>
                {({ id, describedBy, invalid: bad }) => (
                  <div className="field-grid">
                    <NumInput id={id} label={t('machine.dev.axisX')} unit="mm/min" value={draft.max_rate[0]} describedBy={describedBy} invalid={bad} onCommit={(v) => edit({ max_rate: [v, draft.max_rate[1]] })} />
                    <NumInput label={t('machine.dev.axisY')} unit="mm/min" value={draft.max_rate[1]} describedBy={describedBy} invalid={bad} onCommit={(v) => edit({ max_rate: [draft.max_rate[0], v] })} />
                  </div>
                )}
              </Field>
              <Field label={t('machine.dev.accel')} help="machine.dev.accel.help" error={issues.accel}>
                {({ id, describedBy, invalid: bad }) => (
                  <div className="field-grid">
                    <NumInput id={id} label={t('machine.dev.axisX')} unit="mm/s²" value={draft.accel[0]} describedBy={describedBy} invalid={bad} onCommit={(v) => edit({ accel: [v, draft.accel[1]] })} />
                    <NumInput label={t('machine.dev.axisY')} unit="mm/s²" value={draft.accel[1]} describedBy={describedBy} invalid={bad} onCommit={(v) => edit({ accel: [draft.accel[0], v] })} />
                  </div>
                )}
              </Field>
              <Field label={t('machine.dev.rapid')} help="machine.dev.rapid.help" error={issues.rapid_rate}>
                {({ id, describedBy, invalid: bad }) => <NumInput id={id} unit="mm/min" value={draft.rapid_rate} describedBy={describedBy} invalid={bad} onCommit={(v) => edit({ rapid_rate: v })} />}
              </Field>
            </fieldset>

            <fieldset className="mc-group">
              <legend>{t('machine.dev.group.comms')}</legend>
              <div className="field-grid">
                <Field label={t('machine.baud')} help="machine.dev.baud.help" error={issues.baud}>
                  {({ id, describedBy }) => (
                    <select id={id} className="select num" value={draft.baud} aria-describedby={describedBy} onChange={(e) => edit({ baud: Number(e.target.value) })}>
                      {[...new Set<number>([...BAUD_RATES, draft.baud])].sort((a, b) => a - b).map((b) => (
                        <option key={b} value={b}>{b}</option>
                      ))}
                    </select>
                  )}
                </Field>
                <Field label={t('machine.dev.rxBuffer')} help="machine.dev.rxBuffer.help" error={issues.rx_buffer}>
                  {({ id, describedBy, invalid: bad }) => <NumInput id={id} unit="B" value={draft.rx_buffer} describedBy={describedBy} invalid={bad} onCommit={(v) => edit({ rx_buffer: v })} />}
                </Field>
              </div>
            </fieldset>

            <fieldset className="mc-group">
              <legend>{t('machine.dev.group.gcode')}</legend>
              <Field label={t('machine.dev.startGcode')} help="machine.dev.startGcode.help">
                {({ id, describedBy }) => (
                  <textarea id={id} className="input num mc-textarea" rows={3} spellCheck={false} aria-describedby={describedBy} value={draft.start_gcode} onChange={(e) => edit({ start_gcode: e.target.value })} />
                )}
              </Field>
              <Field label={t('machine.dev.endGcode')} help="machine.dev.endGcode.help">
                {({ id, describedBy }) => (
                  <textarea id={id} className="input num mc-textarea" rows={3} spellCheck={false} aria-describedby={describedBy} value={draft.end_gcode} onChange={(e) => edit({ end_gcode: e.target.value })} />
                )}
              </Field>
              <Field label={t('machine.dev.returnOrigin')} help="machine.dev.returnOrigin.help">
                {({ id, describedBy }) => (
                  <label className="mc-check">
                    <input id={id} type="checkbox" checked={draft.return_to_origin} aria-describedby={describedBy} onChange={(e) => edit({ return_to_origin: e.target.checked })} />
                    <span>{t('machine.dev.returnOrigin.label')}</span>
                  </label>
                )}
              </Field>
            </fieldset>
          </form>
        </div>
      </Modal>

      {guard && (
        <Modal
          title={t(guard.title)}
          tone="warn"
          size="sm"
          onClose={() => setGuard(null)}
          footer={
            <>
              <button type="button" className="btn" data-autofocus onClick={() => setGuard(null)}>{t('machine.dev.keepEditing')}</button>
              <button type="button" className="btn btn--danger" onClick={() => { const run = guard.run; setGuard(null); run(); }}>{t('machine.dev.discardConfirm')}</button>
            </>
          }
        >
          <p>{t('machine.dev.discard.text')}</p>
        </Modal>
      )}
      {confirmDelete && (
        <Modal
          title={t('machine.dev.delete.title', { name: draft.name })}
          tone="danger"
          size="sm"
          onClose={() => setConfirmDelete(false)}
          footer={
            <>
              <button type="button" className="btn" data-autofocus onClick={() => setConfirmDelete(false)}>{t('machine.dlg.cancel')}</button>
              <button type="button" className="btn btn--danger" onClick={() => void remove()}>{t('machine.dev.delete')}</button>
            </>
          }
        >
          <p>{t('machine.dev.delete.text')}</p>
        </Modal>
      )}
    </>
  );
}
