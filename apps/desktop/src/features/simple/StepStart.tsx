import { Play } from 'lucide-react';
import { useId } from 'react';
import { t } from '@/i18n';
import { portsForDevice } from '@/features/machine/ruidaPorts';
import { portLabel } from '@/features/machine/virtualPort';
import { sendBlockedReason } from '@/store/machine';
import { selectActiveDevice, useStore } from '@/store';
import { startJob } from './actions';
import type { SimpleData } from './hooks';
import { ProblemList } from './Problems';
import { useSimple } from './store';

/** Elige el puerto de la máquina (si no se ha conectado). */
function PortPicker() {
  const id = useId();
  const connected = useStore((s) => s.connection.connected);
  const ports = useStore((s) => s.ports);
  const selected = useStore((s) => s.selectedPort);
  const device = useStore(selectActiveDevice);
  const setSelectedPort = useStore((s) => s.setSelectedPort);
  const shown = portsForDevice(device, ports);
  if (connected || shown.length === 0) return null;
  return (
    <div className="simple__field">
      <label htmlFor={id} className="simple__label">{t('simple.port')}</label>
      <select id={id} className="select simple__select" value={selected ?? ''} onChange={(e) => setSelectedPort(e.target.value)}>
        {shown.map((p) => (
          <option key={p} value={p}>{portLabel(p)}</option>
        ))}
      </select>
    </div>
  );
}

/** Paso 5: resumen de lo que va a pasar y el botón Iniciar (que abre la lista de comprobación de seguridad). */
export function StepStart({ data }: { data: SimpleData }) {
  const device = useStore(selectActiveDevice);
  const actions = useSimple((s) => s.actions);
  const sendWhy = useStore((s) => sendBlockedReason(s));
  const blocking = data.problems.filter((p) => p.level === 'block');
  const why = blocking.length > 0 ? t(blocking[0]!.key, blocking[0]!.params) : sendWhy ? t(sendWhy) : null;
  return (
    <>
      <dl className="simple__recap">
        <div>
          <dt>{t('simple.machine')}</dt>
          <dd>{device.name}</dd>
        </div>
        <div>
          <dt>{t('simple.material')}</dt>
          <dd>{data.choice?.label ?? '—'}</dd>
        </div>
        {data.groups.map((g) => {
          const a = actions[g.color.toLowerCase()];
          return (
            <div key={g.color}>
              <dt>
                <span className="simple__dot" style={{ background: g.color }} aria-hidden="true" />
                {g.name || g.color}
              </dt>
              <dd>{a ? t(`simple.action.${a}.done`) : '—'}</dd>
            </div>
          );
        })}
      </dl>
      <PortPicker />
      <ProblemList step="start" problems={data.problems} />
      <button type="button" className="btn btn--accent simple__btn simple__btn--big" disabled={why !== null} title={why ?? undefined} onClick={startJob}>
        <Play size={18} aria-hidden="true" />
        {t('simple.start')}
      </button>
      <p className="hint">{why ?? t('simple.start.help')}</p>
    </>
  );
}
