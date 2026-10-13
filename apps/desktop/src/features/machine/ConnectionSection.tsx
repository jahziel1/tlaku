import { Plug, RefreshCw, Settings2, TriangleAlert, Unplug } from 'lucide-react';
import { useEffect } from 'react';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { BAUD_RATES, parseGrblVersion } from '@/lib/device';
import { selectActiveDevice, useStore } from '@/store';
import { selectIsVirtual } from '@/store/machine';
import { useCommandConfirm } from './CommandConfirm';
import { isVirtualPort, portLabel, VIRTUAL_SPEEDS } from './virtualPort';
import { VirtualBadge } from './VirtualBadge';

/** Perfil, puerto, baudios, conectar/desconectar, versión de firmware y avisos de configuración. */
export function ConnectionSection() {
  const ports = useStore((s) => s.ports);
  const selectedPort = useStore((s) => s.selectedPort);
  const baud = useStore((s) => s.baud);
  const connected = useStore((s) => s.connection.connected);
  const connectedPort = useStore((s) => s.connection.port);
  const connectionError = useStore((s) => s.connection.error);
  const connecting = useStore((s) => s.connecting);
  const firmware = useStore((s) => s.firmware);
  const laserMode = useStore((s) => s.settings?.[32]);
  const refreshPorts = useStore((s) => s.refreshPorts);
  const setSelectedPort = useStore((s) => s.setSelectedPort);
  const setBaud = useStore((s) => s.setBaud);
  const virtualSpeed = useStore((s) => s.virtualSpeed);
  const setVirtualSpeed = useStore((s) => s.setVirtualSpeed);
  const virtual = useStore(selectIsVirtual);
  const openDialog = useStore((s) => s.openDialog);
  const connectCmd = useCommand('machine.connect');
  const devices = useStore((s) => s.devices);
  const device = useStore(selectActiveDevice);
  const setActiveDevice = useStore((s) => s.setActiveDevice);
  const { ask, dialog } = useCommandConfirm();

  useEffect(() => {
    void refreshPorts();
  }, [refreshPorts]);

  const pickDevice = (id: string) => {
    setActiveDevice(id);
    const next = devices.find((d) => d.id === id);
    if (next) setBaud(next.baud);
  };

  const version = firmware ? parseGrblVersion(firmware) : null;

  return (
    <section className="section" aria-labelledby="sec-conn">
      <h3 id="sec-conn" className="panel-title">{t('machine.connection')}</h3>
      <div className="row">
        <div className="field grow">
          <label htmlFor="sel-device" className="field__label">{t('machine.device')}</label>
          <select id="sel-device" className="select" value={device.id} disabled={connected} title={connected ? t('machine.disconnectFirst') : undefined} onChange={(e) => pickDevice(e.target.value)}>
            {devices.length === 0 && <option value={device.id}>{device.name}</option>}
            {devices.map((d) => (
              <option key={d.id} value={d.id}>{d.name} ({d.bed[0]} × {d.bed[1]} mm)</option>
            ))}
          </select>
        </div>
        <button type="button" className="icon-btn icon-btn--boxed" title={t('machine.dev.manage')} aria-label={t('machine.dev.manage')} onClick={() => openDialog({ kind: 'devices' })}>
          <Settings2 size={15} aria-hidden="true" />
        </button>
      </div>
      <div className="row">
        <div className="field grow">
          <label htmlFor="sel-port" className="field__label">{t('machine.port')}</label>
          <select id="sel-port" className="select" value={selectedPort ?? ''} disabled={connected} title={connected ? t('machine.disconnectFirst') : undefined} onChange={(e) => setSelectedPort(e.target.value || null)}>
            {ports.length === 0 && <option value="">{t('machine.noPorts')}</option>}
            {ports.map((p) => (
              <option key={p} value={p}>{portLabel(p)}</option>
            ))}
          </select>
        </div>
        <button type="button" className="icon-btn icon-btn--boxed" title={connected ? t('machine.disconnectFirst') : t('machine.refreshPorts')} aria-label={t('machine.refreshPorts')} disabled={connected} onClick={() => void refreshPorts()}>
          <RefreshCw size={15} aria-hidden="true" />
        </button>
      </div>
      {ports.length === 0 && !connected && <p className="hint">{t('machine.noPorts.help')}</p>}
      {isVirtualPort(selectedPort) && !connected && <p className="hint">{t('machine.virtual.note')}</p>}
      <div className="row">
        {isVirtualPort(selectedPort) ? (
          <div className="field">
            <label htmlFor="sel-vspeed" className="field__label">{t('machine.virtual.speed')}</label>
            <select id="sel-vspeed" className="select num" value={virtualSpeed} disabled={connected} title={connected ? t('machine.disconnectFirst') : t('machine.virtual.speedHint')} onChange={(e) => setVirtualSpeed(Number(e.target.value))}>
              {VIRTUAL_SPEEDS.map((n) => (
                <option key={n} value={n}>{t('machine.virtual.speedOption', { n })}</option>
              ))}
            </select>
          </div>
        ) : (
          <div className="field">
            <label htmlFor="sel-baud" className="field__label">{t('machine.baud')}</label>
            <select id="sel-baud" className="select num" value={baud} disabled={connected} title={connected ? t('machine.disconnectFirst') : undefined} onChange={(e) => setBaud(Number(e.target.value))}>
              {[...new Set<number>([...BAUD_RATES, baud])].sort((a, b) => a - b).map((b) => (
                <option key={b} value={b}>{b}</option>
              ))}
            </select>
          </div>
        )}
        <button type="button" className={`btn grow ${connected ? '' : 'btn--accent'}`} disabled={!connectCmd.enabled} title={connectCmd.tooltip} onClick={connectCmd.run}>
          {connected ? <Unplug size={15} aria-hidden="true" /> : <Plug size={15} aria-hidden="true" />}
          {connecting ? t('machine.connecting') : connectCmd.label}
        </button>
      </div>

      <div role="status" className="mc-conn">
        <span className="dot" data-tone={connected ? 'ok' : 'off'} aria-hidden="true" />
        <span>
          {connecting
            ? t('machine.connecting')
            : connected
              ? t('machine.connectedTo', { port: portLabel(connectedPort ?? selectedPort ?? '') })
              : t('machine.notConnected')}
        </span>
        {virtual && <VirtualBadge />}
        {firmware && <span className="num mc-conn__fw" title={t('machine.firmware')}>{firmware}</span>}
      </div>
      {connectionError && !connected && (
        <p className="warn-line" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{connectionError}</span>
        </p>
      )}
      {connected && version && !version.modernEnough && (
        <p className="warn-line">
          <TriangleAlert size={14} aria-hidden="true" />
          <span>{t('machine.fw.old')}</span>
        </p>
      )}
      {connected && laserMode === 0 && (
        <div className="warn-line mc-laserwarn" role="alert">
          <TriangleAlert size={14} aria-hidden="true" />
          <div>
            <p>{t('machine.laserModeOff')}</p>
            <button type="button" className="btn" onClick={() => ask('$32=1')}>{t('machine.laserModeOff.fix')}</button>
          </div>
        </div>
      )}
      {dialog}
    </section>
  );
}
