import { FileText, Info, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { getApi } from '@/api';
import type { DeviceProfile, RuidaFile, RuidaInfo } from '@/api/types';
import { t } from '@/i18n';
import { errorMessage } from '@/lib/errors';
import { fmt } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';
import { isJobBusy } from '@/store/selectors';
import { formatDuration, selectIsRuida } from './ruidaPorts';

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
  ];
}

/** ¿La cama del perfil y la de la máquina difieren? Con más de 1 mm no se envían trabajos. */
export function bedMismatch(profile: readonly [number, number], machine: readonly [number, number] | null): boolean {
  return machine !== null && (Math.abs(profile[0] - machine[0]) > 1 || Math.abs(profile[1] - machine[1]) > 1);
}

/**
 * Controladora Ruida (EXPERIMENTAL): información de la máquina y lista de documentos de su
 * memoria, ambas de solo lectura. Solo aparece con un perfil Ruida. Escribir los ajustes de la
 * controladora no existe a propósito: es riesgoso sin hardware de prueba.
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
  // Lo leído pertenece a una conexión: al desconectar se olvida (ajuste durante el renderizado).
  const [wasConnected, setWasConnected] = useState(connected);
  if (wasConnected !== connected) {
    setWasConnected(connected);
    if (!connected) {
      setInfo(null);
      setFiles(null);
      setError(null);
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
    <section className="section" aria-labelledby="sec-ruida">
      <h3 id="sec-ruida" className="panel-title">
        {t('ruida.panel.title')} <RuidaExperimentalBadge />
      </h3>
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
              <li key={f.index}>{f.index}. {f.name ?? t('ruida.files.noName')}</li>
            ))}
          </ol>
        </>
      ))}
      <p className="hint">{t('ruida.files.note')}</p>
      {error && (
        <p className="warn-line" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{error}</span>
        </p>
      )}
    </section>
  );
}

/** Motivo por el que algo de GRBL no aplica a la máquina Ruida (consola, ajustes en vivo). */
export function RuidaNotApplicable({ reason }: { reason: 'ruida.console.reason' | 'ruida.override.reason' }) {
  return <p className="hint" role="note">{t(reason)}</p>;
}
