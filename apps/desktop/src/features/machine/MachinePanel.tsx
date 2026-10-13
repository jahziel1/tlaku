import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Crosshair, Pause, Play, RefreshCw, Square, Unplug, Plug } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { MachineState } from '@/api/types';
import { useCommand } from '@/commands/useCommand';
import { t } from '@/i18n';
import { fmt, fmtClock } from '@/lib/platform';
import { selectActiveDevice, useStore } from '@/store';

const BAUDS = [9600, 57600, 115200, 250000, 460800];
const STEPS = [0.1, 1, 10, 50];

function StateBadge({ state }: { state: MachineState | null }) {
  const tone = state === 'alarm' ? 'danger' : state === 'run' || state === 'jog' || state === 'home' ? 'accent' : state === 'hold' || state === 'door' ? 'warn' : 'ok';
  return (
    <span className="badge" data-tone={state ? tone : 'off'}>
      <span className="badge__dot" aria-hidden="true" />
      {state ? t(`machine.state.${state}`) : t('machine.state.off')}
    </span>
  );
}

function ConnectionSection() {
  const ports = useStore((s) => s.ports);
  const selectedPort = useStore((s) => s.selectedPort);
  const baud = useStore((s) => s.baud);
  const connected = useStore((s) => s.connection.connected);
  const connecting = useStore((s) => s.connecting);
  const firmware = useStore((s) => s.firmware);
  const refreshPorts = useStore((s) => s.refreshPorts);
  const setSelectedPort = useStore((s) => s.setSelectedPort);
  const setBaud = useStore((s) => s.setBaud);
  const connectCmd = useCommand('machine.connect');
  const devices = useStore((s) => s.devices);
  const device = useStore(selectActiveDevice);
  const setActiveDevice = useStore((s) => s.setActiveDevice);

  useEffect(() => {
    void refreshPorts();
  }, [refreshPorts]);

  return (
    <section className="section" aria-labelledby="sec-conn">
      <h3 id="sec-conn" className="panel-title">{t('machine.connection')}</h3>
      <div className="field">
        <label htmlFor="sel-device" className="field__label">{t('machine.device')}</label>
        <select id="sel-device" className="select" value={device.id} disabled={connected} onChange={(e) => setActiveDevice(e.target.value)}>
          {devices.length === 0 && <option value={device.id}>{device.name}</option>}
          {devices.map((d) => (
            <option key={d.id} value={d.id}>{d.name} ({d.bed[0]} × {d.bed[1]} mm)</option>
          ))}
        </select>
      </div>
      <div className="row">
        <div className="field grow">
          <label htmlFor="sel-port" className="field__label">{t('machine.port')}</label>
          <select id="sel-port" className="select" value={selectedPort ?? ''} disabled={connected} onChange={(e) => setSelectedPort(e.target.value || null)}>
            {ports.length === 0 && <option value="">{t('machine.noPorts')}</option>}
            {ports.map((p) => (
              <option key={p} value={p}>{p}</option>
            ))}
          </select>
        </div>
        <button type="button" className="icon-btn icon-btn--boxed" title={t('machine.refreshPorts')} aria-label={t('machine.refreshPorts')} disabled={connected} onClick={() => void refreshPorts()}>
          <RefreshCw size={15} aria-hidden="true" />
        </button>
      </div>
      <div className="row">
        <div className="field">
          <label htmlFor="sel-baud" className="field__label">{t('machine.baud')}</label>
          <select id="sel-baud" className="select num" value={baud} disabled={connected} onChange={(e) => setBaud(Number(e.target.value))}>
            {BAUDS.map((b) => (
              <option key={b} value={b}>{b}</option>
            ))}
          </select>
        </div>
        <button type="button" className={`btn grow ${connected ? '' : 'btn--accent'}`} disabled={!connectCmd.enabled} title={connectCmd.tooltip} onClick={connectCmd.run}>
          {connected ? <Unplug size={15} aria-hidden="true" /> : <Plug size={15} aria-hidden="true" />}
          {connecting ? t('machine.connecting') : connectCmd.label}
        </button>
      </div>
      {firmware && <p className="hint num">{firmware}</p>}
    </section>
  );
}

function StatusSection() {
  const status = useStore((s) => s.status);
  const connected = useStore((s) => s.connection.connected);
  const pos = status?.work_pos;
  return (
    <section className="section" aria-labelledby="sec-status">
      <h3 id="sec-status" className="panel-title">{t('machine.status')}</h3>
      <div className="readout">
        <StateBadge state={connected ? (status?.state ?? 'idle') : null} />
        <dl className="kv">
          <div><dt>X</dt><dd className="num">{pos ? fmt(pos[0], 2) : '—'}</dd></div>
          <div><dt>Y</dt><dd className="num">{pos ? fmt(pos[1], 2) : '—'}</dd></div>
          <div><dt>F</dt><dd className="num">{status ? Math.round(status.feed) : '—'}</dd></div>
          <div><dt>S</dt><dd className="num">{status ? Math.round(status.spindle) : '—'}</dd></div>
        </dl>
      </div>
    </section>
  );
}

function JogSection() {
  const connected = useStore((s) => s.connection.connected);
  const busy = useStore((s) => s.jobState === 'running' || s.jobState === 'paused');
  const jog = useStore((s) => s.jog);
  const [step, setStep] = useState(10);
  const disabled = !connected || busy;
  const why = !connected ? t('reason.needMachine') : busy ? t('reason.jobRunning') : undefined;
  const move = (dx: number, dy: number) => void jog(dx * step, dy * step);
  const btn = (Icon: typeof ArrowUp, label: string, dx: number, dy: number) => (
    <button type="button" className="icon-btn icon-btn--boxed" disabled={disabled} title={why ?? label} aria-label={label} onClick={() => move(dx, dy)}>
      <Icon size={16} aria-hidden="true" />
    </button>
  );
  return (
    <section className="section" aria-labelledby="sec-jog">
      <h3 id="sec-jog" className="panel-title">{t('machine.jog')}</h3>
      <div className="jog">
        <div className="jog__pad" role="group" aria-label={t('machine.jog')}>
          <span />{btn(ArrowUp, t('machine.jog.up'), 0, 1)}<span />
          {btn(ArrowLeft, t('machine.jog.left'), -1, 0)}
          <span className="jog__center" aria-hidden="true"><Crosshair size={14} /></span>
          {btn(ArrowRight, t('machine.jog.right'), 1, 0)}
          <span />{btn(ArrowDown, t('machine.jog.down'), 0, -1)}<span />
        </div>
        <div className="jog__steps" role="radiogroup" aria-label={t('machine.jog.step')}>
          {STEPS.map((s) => (
            <button key={s} type="button" role="radio" aria-checked={step === s} className="chip num" onClick={() => setStep(s)}>
              {fmt(s, s < 1 ? 1 : 0)} mm
            </button>
          ))}
        </div>
      </div>
    </section>
  );
}

function JobSection() {
  const frame = useCommand('machine.frame');
  const start = useCommand('machine.start');
  const pause = useCommand('machine.pause');
  const stop = useCommand('machine.stop');
  const progress = useStore((s) => s.progress);
  const jobState = useStore((s) => s.jobState);
  const total = useStore((s) => s.preview?.total_s ?? null);
  const pct = progress && progress.total > 0 ? progress.acked / progress.total : 0;
  const running = jobState === 'running' || jobState === 'paused';

  return (
    <section className="section" aria-labelledby="sec-job">
      <h3 id="sec-job" className="panel-title">{t('machine.job')}</h3>
      <div className="row">
        <button type="button" className="btn grow" disabled={!frame.enabled} title={frame.tooltip} onClick={frame.run}>
          <Crosshair size={15} aria-hidden="true" />{frame.label}
        </button>
        <button type="button" className="btn btn--accent grow" disabled={!start.enabled} title={start.tooltip} onClick={start.run}>
          <Play size={15} aria-hidden="true" />{start.label}
        </button>
      </div>
      <div className="row">
        <button type="button" className="btn grow" disabled={!pause.enabled} title={pause.tooltip} onClick={pause.run}>
          <Pause size={15} aria-hidden="true" />{pause.label}
        </button>
        <button type="button" className="btn btn--danger grow" disabled={!stop.enabled} title={stop.tooltip} onClick={stop.run}>
          <Square size={15} aria-hidden="true" />{stop.label}
        </button>
      </div>
      {(running || jobState === 'finished') && (
        <div className="progress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct * 100)} aria-label={t('machine.progress')}>
          <div className="progress__bar" style={{ width: `${pct * 100}%` }} />
          <span className="progress__text num">
            {Math.round(pct * 100)} %{total !== null && running ? ` · ${fmtClock(total * (1 - pct))}` : ''}
          </span>
        </div>
      )}
    </section>
  );
}

export function MachinePanel() {
  return (
    <div className="panel-scroll">
      <ConnectionSection />
      <StatusSection />
      <JogSection />
      <JobSection />
    </div>
  );
}
