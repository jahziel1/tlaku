import { Check, Network, TriangleAlert } from 'lucide-react';
import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import type { DeviceProfile } from '@/api/types';
import { t, type I18nKey } from '@/i18n';
import { useStore } from '@/store';
import { NumInput } from './DeviceFields';
import {
  CONNECTION_TYPES,
  connectionIssue,
  connectionPortId,
  withConnectionType,
  type ConnectionIssue,
  type NetConnectionType,
} from './DeviceConnectionModel';
import { testConnection } from './DeviceConnectionProbe';
import './DeviceConnection.css';

const TYPE_LABEL: Record<NetConnectionType, I18nKey> = {
  serial: 'devices.conn.type.serial',
  tcp: 'devices.conn.type.tcp',
  web_socket: 'devices.conn.type.web_socket',
};

const ISSUE_TEXT: Record<ConnectionIssue, I18nKey> = {
  host: 'devices.conn.err.host',
  port: 'devices.conn.err.port',
  url: 'devices.conn.err.url',
  wss: 'devices.conn.err.wss',
};

/** Campo con etiqueta, ayuda y error enlazados al control (mismo aspecto que `Field`, con los textos de la conexión). */
function NetField({ label, help, error, children }: { label: string; help: I18nKey; error?: ConnectionIssue | null; children: (ids: { id: string; describedBy: string; invalid: boolean }) => ReactNode }) {
  const id = useId();
  const helpId = `${id}-help`;
  const errId = `${id}-err`;
  return (
    <div className="mc-field" data-invalid={error || undefined}>
      <label htmlFor={id} className="field__label">{label}</label>
      {children({ id, describedBy: error ? `${errId} ${helpId}` : helpId, invalid: !!error })}
      {error && <p id={errId} className="hint hint--danger" role="alert">{t(ISSUE_TEXT[error])}</p>}
      <p id={helpId} className="hint mc-help">{t(help)}</p>
    </div>
  );
}

type TestState = { kind: 'idle' } | { kind: 'testing' } | { kind: 'ok'; text: string } | { kind: 'fail'; text: string };

/**
 * Selector de conexión del perfil: puerto serie, TCP/Telnet o WebSocket, con su validación y el botón
 * «Probar conexión», que abre, lee el saludo y cierra sin mover nada. Las máquinas Ruida y K40 tienen
 * su propia conexión y no lo muestran.
 */
export function DeviceConnection({ draft, edit }: { draft: DeviceProfile; edit: (patch: Partial<DeviceProfile>) => void }) {
  if (draft.connection?.type === 'udp' || draft.kind === 'ruida' || draft.kind === 'lhy_micro') return null;
  return <DeviceConnectionFields draft={draft} edit={edit} />;
}

function DeviceConnectionFields({ draft, edit }: { draft: DeviceProfile; edit: (patch: Partial<DeviceProfile>) => void }) {
  const conn = draft.connection ?? { type: 'serial' as const };
  const issue = connectionIssue(conn);
  const portId = connectionPortId(conn);
  const connectedPort = useStore((s) => (s.connection.connected ? s.connection.port : null));
  const [test, setTest] = useState<TestState>({ kind: 'idle' });
  /** Evita que la respuesta de una prueba vieja pise el estado de la dirección actual. */
  const run = useRef(0);
  const typeId = useId();

  // Cambiar la dirección invalida el resultado anterior.
  useEffect(() => {
    run.current += 1;
    setTest({ kind: 'idle' });
  }, [portId]);

  const probe = async () => {
    if (!portId || issue) return;
    const mine = ++run.current;
    setTest({ kind: 'testing' });
    const r = await testConnection(portId, draft.baud);
    if (run.current !== mine) return;
    setTest(r.ok ? { kind: 'ok', text: r.text } : { kind: 'fail', text: r.text });
  };

  const blocked = issue ? t('devices.conn.test.invalid') : connectedPort === portId ? t('devices.conn.test.connected') : null;
  const net = conn.type === 'tcp' || conn.type === 'web_socket';

  return (
    <div className="mc-conn-fields">
      <div className="mc-field">
        <label htmlFor={typeId} className="field__label">{t('devices.conn.type')}</label>
        <select id={typeId} className="select" value={conn.type} aria-describedby={`${typeId}-help`} onChange={(e) => edit({ connection: withConnectionType(conn, e.target.value as NetConnectionType) })}>
          {CONNECTION_TYPES.map((type) => (
            <option key={type} value={type}>{t(TYPE_LABEL[type])}</option>
          ))}
        </select>
        <p id={`${typeId}-help`} className="hint mc-help">{t('devices.conn.type.help')}</p>
      </div>

      {conn.type === 'tcp' && (
        <div className="field-grid">
          <NetField label={t('devices.conn.host')} help="devices.conn.host.help" error={issue === 'host' ? issue : null}>
            {({ id, describedBy, invalid }) => (
              <input id={id} className="input num" value={conn.host} placeholder="192.168.1.50" spellCheck={false} autoComplete="off" aria-describedby={describedBy} aria-invalid={invalid || undefined} onChange={(e) => edit({ connection: { ...conn, host: e.target.value } })} />
            )}
          </NetField>
          <NetField label={t('devices.conn.port')} help="devices.conn.port.help" error={issue === 'port' ? issue : null}>
            {({ id, describedBy, invalid }) => <NumInput id={id} value={conn.port} describedBy={describedBy} invalid={invalid} onCommit={(v) => edit({ connection: { ...conn, port: v } })} />}
          </NetField>
        </div>
      )}

      {conn.type === 'web_socket' && (
        <NetField label={t('devices.conn.url')} help="devices.conn.url.help" error={issue}>
          {({ id, describedBy, invalid }) => (
            <input id={id} className="input num" value={conn.url} placeholder="ws://192.168.1.50:81" spellCheck={false} autoComplete="off" aria-describedby={describedBy} aria-invalid={invalid || undefined} onChange={(e) => edit({ connection: { ...conn, url: e.target.value } })} />
          )}
        </NetField>
      )}

      {net && (
        <div className="mc-field">
          <div className="row">
            <button type="button" className="btn" disabled={blocked !== null || test.kind === 'testing'} title={blocked ?? t('devices.conn.test.hint')} onClick={() => void probe()}>
              <Network size={14} aria-hidden="true" />
              {test.kind === 'testing' ? t('devices.conn.testing') : t('devices.conn.test')}
            </button>
            {blocked && <span className="hint">{blocked}</span>}
          </div>
          <div role="status" aria-live="polite">
            {test.kind === 'ok' && (
              <p className="hint mc-test-ok">
                <Check size={14} aria-hidden="true" /> {test.text}
              </p>
            )}
            {test.kind === 'fail' && (
              <p className="warn-line" role="alert">
                <TriangleAlert size={14} aria-hidden="true" />
                <span>{test.text}</span>
              </p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
