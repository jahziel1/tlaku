import { PanelSection } from '@/ui/PanelSection';
import { FileText, Info, Settings2, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { getApi } from '@/api';
import type { DeviceProfile, RuidaFile, RuidaInfo } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, usePref, useStore } from '@/store';
import { isJobBusy } from '@/store/selectors';
import { formatDuration, selectIsRuida } from './ruidaPorts';
import { RuidaDocDialog, type RuidaDocAction } from './RuidaDocDialog';
import { RuidaSettingsDialog } from './RuidaSettingsDialog';
import { RuidaWritesToggle } from './RuidaWritesToggle';

/** Etiqueta «Experimental» con el aviso completo como ayuda. */
export function RuidaExperimentalBadge() {
  return (
    <span className="badge" data-tone="warn" title={t('ruida.experimental.note')}>
      <span className="badge__dot" aria-hidden="true" />
      {t('ruida.experimental')}
    </span>
  );
}

const orDash = (v: number | null, digits: number, unit: string) => (v === null ? t('ruida.info.unknown') : `${fmt(v, digits)} ${unit}`);
const pair = (v: [number | null, number | null], digits: number, unit: string) =>
  v[0] === null && v[1] === null ? t('ruida.info.unknown') : `${v[0] === null ? '—' : fmt(v[0], digits)} / ${v[1] === null ? '—' : fmt(v[1], digits)} ${unit}`;
const time = (s: number | null) => (s === null ? t('ruida.info.unknown') : formatDuration(s));

function controlMode(v: number | null): string {
  if (v === null) return t('ruida.info.unknown');
  if (v === 0) return t('ruida.info.controlMode.origin');
  if (v === 0x4000) return t('ruida.info.controlMode.noReturn');
  if (v === 0x8000) return t('ruida.info.controlMode.absolute');
  return `0x${v.toString(16).toUpperCase()}`;
}

function featuresText(info: RuidaInfo): string {
  if (info.focus === null && info.z_return_docking === null && info.air_assist_mode === null) return t('ruida.info.unknown');
  const on = [
    info.focus ? t('ruida.info.feat.focus') : null,
    info.z_return_docking ? t('ruida.info.feat.zDock') : null,
    info.air_assist_mode ? t('ruida.info.feat.air', { n: info.air_assist_mode }) : null,
  ].filter((x): x is string => x !== null);
  return on.length > 0 ? on.join(', ') : t('ruida.info.feat.none');
}

/** Filas de la tabla de información, en el orden en que se muestran. */
export function infoRows(info: RuidaInfo): [string, string][] {
  return [
    [t('ruida.info.model'), info.model ?? t('ruida.info.modelUnknown')],
    [t('ruida.info.card'), info.card_id],
    [t('ruida.info.magic'), `0x${info.magic.toString(16).toUpperCase().padStart(2, '0')}`],
    [t('ruida.info.bed'), info.bed ? `${fmt(info.bed[0], 0)} × ${fmt(info.bed[1], 0)} mm` : t('ruida.info.unknown')],
    [t('ruida.info.rapid'), orDash(info.rapid_mm_s, 0, 'mm/s')],
    [t('ruida.info.home'), orDash(info.home_mm_s, 0, 'mm/s')],
    [t('ruida.info.axisMax'), pair(info.axis_max_mm_s, 0, 'mm/s')],
    [t('ruida.info.axisAccel'), pair(info.axis_accel_mm_s2, 0, 'mm/s²')],
    [t('ruida.info.onTime'), time(info.total_on_s)],
    [t('ruida.info.workTime'), time(info.total_work_s)],
    [t('ruida.info.laserTime'), time(info.total_laser_s)],
    [t('ruida.info.jobs'), info.total_jobs === null ? t('ruida.info.unknown') : fmt(info.total_jobs, 0)],
    [t('ruida.info.travel'), pair(info.travel_m, 0, 'm')],
    [t('ruida.info.docs'), info.documents === null ? t('ruida.info.unknown') : fmt(info.documents, 0)],
    [t('ruida.info.board'), info.board_version ?? t('ruida.info.unknown')],
    [t('ruida.info.prevWork'), info.previous_work === null ? t('ruida.info.unknown') : fmt(info.previous_work, 0)],
    [t('ruida.info.controlMode'), controlMode(info.control_mode)],
    [t('ruida.info.features'), featuresText(info)],
  ];
}

/** ¿La cama del perfil y la de la máquina difieren? Con más de 1 mm no se envían trabajos. */
export function bedMismatch(profile: readonly [number, number], machine: readonly [number, number] | null): boolean {
  return machine !== null && (Math.abs(profile[0] - machine[0]) > 1 || Math.abs(profile[1] - machine[1]) > 1);
}

/**
 * Controladora Ruida (EXPERIMENTAL): información de la máquina (solo lectura), sus ajustes (leer,
 * editar, respaldar y restaurar; ola 6E) y los documentos de su memoria (listar, ejecutar y borrar).
 * Solo aparece con un perfil Ruida. Todo lo que escribe en la controladora pide confirmación
 * explícita y deja un respaldo; nada de esto se ha probado con una controladora real.
 */
export function RuidaSection() {
  const isRuida = useStore(selectIsRuida);
  const device = useStore(selectActiveDevice);
  const connected = useStore((s) => s.connection.connected);
  const busy = useStore(isJobBusy);
  if (!isRuida) return null;
  return <RuidaPanel device={device} connected={connected} busy={busy} />;
}

/** El panel con su estado local; recibe lo que lee de la tienda para poder probarse por separado. */
export function RuidaPanel({ device, connected, busy }: { device: DeviceProfile; connected: boolean; busy: boolean }) {
  const [info, setInfo] = useState<RuidaInfo | null>(null);
  const [files, setFiles] = useState<RuidaFile[] | null>(null);
  const [reading, setReading] = useState<'info' | 'files' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const writesOn = usePref('ruidaExperimentalWrites');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [docAction, setDocAction] = useState<{ action: RuidaDocAction; file: RuidaFile } | null>(null);
  const [docNotice, setDocNotice] = useState<string | null>(null);
  // Lo leído pertenece a una conexión: al desconectar se olvida (ajuste durante el renderizado).
  const [wasConnected, setWasConnected] = useState(connected);
  if (wasConnected !== connected) {
    setWasConnected(connected);
    if (!connected) {
      setInfo(null);
      setFiles(null);
      setError(null);
      setSettingsOpen(false);
      setDocAction(null);
      setDocNotice(null);
    }
  }
  const read = async (what: 'info' | 'files') => {
    setReading(what);
    setError(null);
    try {
      if (what === 'info') setInfo(await getApi().ruidaInfo());
      else setFiles(await getApi().ruidaFiles());
    } catch (e) {
      setError(t('ruida.error', { error: errorMessage(e) }));
    } finally {
      setReading(null);
    }
  };
  const blocked = !connected ? t('reason.needMachine') : busy ? t('reason.jobRunning') : null;
  const mismatch = info !== null && bedMismatch(device.bed, info.bed);

  return (
    <PanelSection id="machine.ruida" title={<>{t('ruida.panel.title')} <RuidaExperimentalBadge /></>} defaultOpen>
      <div className="warn-line" role="note">
        <TriangleAlert size={14} aria-hidden="true" />
        <div>
          <p>{t('ruida.experimental.note')}</p>
          <p className="warn-line__key">{device.ruida.run_after_upload ? t('ruida.start.run') : t('ruida.start.store')}</p>
        </div>
      </div>
      <p className="hint">{t('ruida.panel.noG')}</p>
      {!connected && <p className="hint">{t('ruida.panel.needConnect')}</p>}

      <h4 className="mc-sub">{t('ruida.info.title')}</h4>
      <div className="row">
        <button type="button" className="btn grow" disabled={blocked !== null || reading !== null} title={blocked ?? undefined} onClick={() => void read('info')}>
          <Info size={14} aria-hidden="true" />
          {reading === 'info' ? t('ruida.info.reading') : t('ruida.info.read')}
        </button>
      </div>
      {info && (
        <table className="mc-table" aria-label={t('ruida.info.title')}>
          <tbody>
            {infoRows(info).map(([label, value]) => (
              <tr key={label}>
                <th scope="row">{label}</th>
                <td className="num">{value}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {mismatch && info?.bed && (
        <p className="warn-line" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('ruida.info.bedMismatch', { profile: `${fmt(device.bed[0], 0)} × ${fmt(device.bed[1], 0)}`, machine: `${fmt(info.bed[0], 0)} × ${fmt(info.bed[1], 0)}` })}</span>
        </p>
      )}
      <p className="hint">{t('ruida.info.readonly')}</p>

      <h4 className="mc-sub">{t('ruida.set.title')}</h4>
      <RuidaWritesToggle />
      <div className="row">
        <button type="button" className="btn grow" disabled={blocked !== null} title={blocked ?? t('ruida.set.open.help')} onClick={() => setSettingsOpen(true)}>
          <Settings2 size={14} aria-hidden="true" />
          {t('ruida.set.open')}
        </button>
      </div>
      <p className="hint">{t('ruida.set.experimental')}.</p>

      <h4 className="mc-sub">{t('ruida.files.title')}</h4>
      <div className="row">
        <button type="button" className="btn grow" disabled={blocked !== null || reading !== null} title={blocked ?? undefined} onClick={() => void read('files')}>
          <FileText size={14} aria-hidden="true" />
          {reading === 'files' ? t('ruida.files.reading') : t('ruida.files.read')}
        </button>
      </div>
      {files && (files.length === 0 ? (
        <p className="hint" role="status">{t('ruida.files.empty')}</p>
      ) : (
        <>
          <p className="hint" role="status">{t('ruida.files.count', { n: files.length })}</p>
          <ol className="mc-list num" aria-label={t('ruida.files.title')}>
            {files.map((f) => (
              <li key={f.index} className="row">
                <span className="grow">{f.index}. {f.name ?? t('ruida.files.noName')}</span>
                <button type="button" className="btn" disabled={blocked !== null || f.name === null || !writesOn} title={blocked ?? (writesOn ? undefined : t('ruida.exp.off'))} onClick={() => setDocAction({ action: 'run', file: f })}>
                  {t('ruida.files.run')}
                </button>
                <button type="button" className="btn" disabled={blocked !== null || f.name === null || !writesOn} title={blocked ?? (writesOn ? undefined : t('ruida.exp.off'))} onClick={() => setDocAction({ action: 'delete', file: f })}>
                  {t('ruida.files.delete')}
                </button>
              </li>
            ))}
          </ol>
        </>
      ))}
      <p className="hint">{t('ruida.files.note')}</p>
      <p className="hint">{t('ruida.files.note2')}</p>
      {docNotice && <p className="hint" role="status">{docNotice}</p>}
      {error && (
        <p className="warn-line" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{error}</span>
        </p>
      )}
      {settingsOpen && <RuidaSettingsDialog onClose={() => setSettingsOpen(false)} />}
      {docAction && (
        <RuidaDocDialog
          action={docAction.action}
          file={docAction.file}
          onClose={() => setDocAction(null)}
          onDone={(message) => {
            setDocNotice(message);
            // La lista cambió (o el trabajo empezó): se vuelve a leer.
            void read('files');
          }}
        />
      )}
    </PanelSection>
  );
}

/** Motivo por el que algo de GRBL no aplica a la máquina Ruida (consola, ajustes en vivo). */
export function RuidaNotApplicable({ reason }: { reason: 'ruida.console.reason' | 'ruida.override.reason' }) {
  return <p className="hint" role="note">{t(reason)}</p>;
}
