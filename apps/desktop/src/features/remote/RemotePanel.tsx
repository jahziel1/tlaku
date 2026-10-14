import { TriangleAlert } from 'lucide-react';
import { useId } from 'react';
import type { RemoteClient, RemoteStatus } from '@/api/api';
import { askChoice } from '@/features/history/prompt';
import { PrefCheck, PrefNumber } from '@/features/settings/fields';
import { t } from '@/i18n';
import { usePref, useStore } from '@/store';
import { selectFramed } from '@/store/machine';
import { kickRemoteClient, regenerateRemote, useRemote } from './remote';
import './remote.css';

/** Imagen (data URL) de un SVG: así el QR no se inserta como HTML en la ventana. */
export const svgDataUrl = (svg: string) => `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;

export interface RemoteViewProps {
  enabled: boolean;
  status: RemoteStatus | null;
  clients: RemoteClient[];
  framed: boolean;
  ip: string | null;
  onIp: (ip: string | null) => void;
  onCopy: () => void;
  onRegenerate: () => void;
  onKick: (client: RemoteClient) => void;
}

function ClientRow({ client, onKick }: { client: RemoteClient; onKick: (c: RemoteClient) => void }) {
  const who = `${client.agent} · ${client.ip}`;
  return (
    <li className="remote-client">
      <span className="remote-dot" data-on={client.connected} aria-hidden="true" />
      <div className="remote-client__who">
        <div>{who}</div>
        <div className="remote-client__sub">{client.connected ? t('remote.client.online') : t('remote.client.idle', { s: client.idle_s })}</div>
      </div>
      <button type="button" className="btn" aria-label={t('remote.client.kickLabel', { who })} onClick={() => onKick(client)}>
        {t('remote.client.kick')}
      </button>
    </li>
  );
}

/** Interfaz de la sección (sin lógica): recibe todo por propiedades para poder probarla. */
export function RemoteView({ enabled, status, clients, framed, ip, onIp, onCopy, onRegenerate, onKick }: RemoteViewProps) {
  const ipId = useId();
  const addrs = status?.addrs ?? [];
  const recommended = addrs.find((a) => a.recommended)?.ip ?? '';
  return (
    <>
      <fieldset className="set-group">
        <PrefCheck k="remoteEnabled" label="remote.enabled" />
        <p className="hint">{t('remote.hint')}</p>
        <p className="warn-line">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('remote.warn.http')}</span>
        </p>
      </fieldset>
      {enabled && (
        <>
          <fieldset className="set-group">
            <div className="set-grid">
              <PrefNumber k="remotePort" label="remote.port" unit="remote.portUnit" />
              <div className="field">
                <label className="field__label" htmlFor={ipId}>{t('remote.ip')}</label>
                <select id={ipId} className="select" value={ip ?? ''} disabled={addrs.length === 0} onChange={(e) => onIp(e.target.value || null)}>
                  <option value="">{addrs.length === 0 ? t('remote.ip.noNetwork') : t('remote.ip.auto', { ip: recommended })}</option>
                  {addrs.map((a) => (
                    <option key={a.ip} value={a.ip}>{`${a.ip} (${a.name})`}</option>
                  ))}
                </select>
              </div>
            </div>
            <p className="hint">{t('remote.ip.hint')}</p>
            <p role="status" className={status?.error ? 'hint hint--danger' : 'hint'}>
              {status?.error ? t('remote.status.error', { error: status.error }) : status?.running ? t('remote.status.on', { ip: status.ip ?? '', port: status.port }) : t('remote.status.off')}
            </p>
          </fieldset>

          {status?.running && status.qr_svg && status.url && (
            <fieldset className="set-group">
              <legend className="panel-title">{t('remote.qr.title')}</legend>
              <div className="remote-pair">
                <img className="remote-qr" src={svgDataUrl(status.qr_svg)} alt={t('remote.qr.alt')} width={208} height={208} />
                <div className="remote-side">
                  <p className="hint">{t('remote.qr.hint')}</p>
                  <div className="field">
                    <span className="field__label">{t('remote.url')}</span>
                    <span className="remote-url">{status.url}</span>
                  </div>
                  <div className="remote-actions">
                    <button type="button" className="btn" onClick={onCopy}>{t('remote.copy')}</button>
                    <button type="button" className="btn" onClick={onRegenerate}>{t('remote.regenerate')}</button>
                  </div>
                </div>
              </div>
            </fieldset>
          )}

          <fieldset className="set-group">
            <legend className="panel-title">{t('remote.clients.title')}</legend>
            {clients.length === 0 ? (
              <p className="hint">{t('remote.clients.none')}</p>
            ) : (
              <ul className="remote-clients">
                {clients.map((c) => (
                  <ClientRow key={c.id} client={c} onKick={onKick} />
                ))}
              </ul>
            )}
          </fieldset>

          <fieldset className="set-group">
            <legend className="panel-title">{t('remote.rules.title')}</legend>
            <ul className="remote-rules">
              <li>{t('remote.rules.stop')}</li>
              <li>{t('remote.rules.start')}</li>
              <li>{t('remote.rules.camera')}</li>
            </ul>
            <p className={framed ? 'hint' : 'warn-line'}>{t(framed ? 'remote.framed.yes' : 'remote.framed.no')}</p>
          </fieldset>
        </>
      )}
    </>
  );
}

/** Sección «Control remoto» de Preferencias: apagado de fábrica, QR con la clave y quién está conectado. */
export function RemotePanel() {
  const enabled = usePref('remoteEnabled');
  const ip = usePref('remoteIp');
  const setPref = useStore((s) => s.setPref);
  const notify = useStore((s) => s.notify);
  const framed = useStore(selectFramed);
  const status = useRemote((s) => s.status);
  const clients = useRemote((s) => s.clients);

  const copy = async () => {
    const url = status?.full_url;
    try {
      if (!url) throw new Error('sin enlace');
      await navigator.clipboard.writeText(url);
      notify('success', t('remote.copied'));
    } catch {
      notify('error', t('remote.copyFailed'));
    }
  };
  const regenerate = async () => {
    const hasClients = clients.length > 0;
    if (hasClients) {
      const choice = await askChoice({
        title: t('remote.regenerate.title'),
        message: t('remote.regenerate.message'),
        choices: [
          { id: 'ok', label: t('remote.regenerate.confirm'), tone: 'danger' },
          { id: 'cancel', label: t('remote.regenerate.cancel') },
        ],
        cancelId: 'cancel',
      });
      if (choice !== 'ok') return;
    }
    await regenerateRemote();
    notify('success', t('remote.regenerated'));
  };
  const kick = async (c: RemoteClient) => {
    await kickRemoteClient(c.id);
    notify('info', t('remote.client.kicked'));
  };
  return (
    <RemoteView
      enabled={enabled}
      status={status}
      clients={clients}
      framed={framed}
      ip={ip}
      onIp={(v) => setPref('remoteIp', v)}
      onCopy={() => void copy()}
      onRegenerate={() => void regenerate()}
      onKick={(c) => void kick(c)}
    />
  );
}
