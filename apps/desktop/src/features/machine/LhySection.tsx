import { TriangleAlert } from 'lucide-react';
import { useEffect, useState } from 'react';
import { getApi } from '@/api';
import type { LhyState } from '@/api/types';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { LhyExperimentalBadge } from './LhyNotice';
import { selectIsLhy, statusKey } from './lhyPorts';

/** Cada cuánto se pide el estado de la placa mientras hay conexión (ms). */
const POLL_MS = 1000;

/** Texto del código de estado de la placa. */
export function statusText(code: number): string {
  const key = statusKey(code);
  return key ? t(key) : t('lhymicro.status.other', { code });
}

/**
 * Panel del K40 (EXPERIMENTAL): la placa, su estado y, sobre todo, si se sabe dónde está el cabezal.
 * La M2 Nano no informa su posición: se cuenta desde el home, y sin home no se mueve nada.
 * Recibe el estado ya leído para poder probarse por separado.
 */
export function LhyPanel({ connected, state }: { connected: boolean; state: LhyState | null }) {
  return (
    <section className="section" aria-labelledby="sec-lhy">
      <h3 id="sec-lhy" className="panel-title">
        {t('lhymicro.panel.title')} <LhyExperimentalBadge />
      </h3>
      <p className="hint">{t('lhymicro.experimental.note')}</p>
      <p className="hint">{t('lhymicro.power.note')}</p>
      <p className="hint">{t('lhymicro.panel.noG')}</p>
      {!connected && <p className="hint">{t('lhymicro.panel.needConnect')}</p>}
      {connected && state && (
        <>
          <table className="mc-table" aria-label={t('lhymicro.panel.title')}>
            <tbody>
              <tr>
                <th scope="row">{t('lhymicro.panel.board')}</th>
                <td className="num">{state.board}</td>
              </tr>
              <tr>
                <th scope="row">{t('lhymicro.panel.status')}</th>
                <td className="num">{statusText(state.status_code)}</td>
              </tr>
              <tr>
                <th scope="row">{t('lhymicro.panel.position')}</th>
                <td>{t(state.position_known ? 'lhymicro.panel.position.known' : 'lhymicro.panel.position.unknown')}</td>
              </tr>
            </tbody>
          </table>
          {!state.position_known && (
            <p className="warn-line" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('lhymicro.panel.unknownWarn')}</span>
            </p>
          )}
          {state.status_code === 239 && (
            <p className="warn-line" role="alert">
              <TriangleAlert size={14} aria-hidden="true" />
              <span>{t('lhymicro.panel.lowPower')}</span>
            </p>
          )}
        </>
      )}
      <p className="hint">{t('lhymicro.panel.positionNote')}</p>
      <p className="hint">{t('lhymicro.panel.moshiboard')}</p>
    </section>
  );
}

/** El panel con su estado vivo; solo aparece con un perfil K40. */
export function LhySection() {
  const isLhy = useStore(selectIsLhy);
  const connected = useStore((s) => s.connection.connected);
  // Cambia con cada trabajo, home o salto: es cuando la posición puede haberse perdido o ganado.
  const jobState = useStore((s) => s.jobState);
  const machineState = useStore((s) => s.status?.state);
  const [state, setState] = useState<LhyState | null>(null);
  useEffect(() => {
    if (!isLhy || !connected) return;
    let alive = true;
    const read = async () => {
      try {
        const s = await getApi().lhyState();
        if (alive) setState(s);
      } catch {
        // La conexión puede cerrarse entre dos lecturas: la próxima lo corrige.
        if (alive) setState(null);
      }
    };
    void read();
    const timer = setInterval(() => void read(), POLL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [isLhy, connected, jobState, machineState]);
  if (!isLhy) return null;
  return <LhyPanel connected={connected} state={connected ? state : null} />;
}
