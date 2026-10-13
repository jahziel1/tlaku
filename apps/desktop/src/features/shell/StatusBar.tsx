import { Clock, Gauge, MousePointer2 } from 'lucide-react';
import { t } from '@/i18n';
import { fmt, fmtClock } from '@/lib/platform';
import { zoomPercent } from '@/lib/viewport';
import { selectActiveDevice, useStore } from '@/store';

function Cursor() {
  const cursor = useStore((s) => s.cursor);
  return (
    <div className="status__item" aria-label={t('status.cursor')}>
      <MousePointer2 size={13} aria-hidden="true" />
      <span className="num status__coord"><span className="status__axis">X</span>{cursor ? fmt(cursor[0], 2) : '—'}</span>
      <span className="num status__coord"><span className="status__axis">Y</span>{cursor ? fmt(cursor[1], 2) : '—'}</span>
      <span className="status__unit">mm</span>
    </div>
  );
}

function Zoom() {
  const pct = useStore((s) => zoomPercent(s.zoom));
  return (
    <div className="status__item" title={t('status.zoom')}>
      <Gauge size={13} aria-hidden="true" />
      <span className="num">{pct} %</span>
    </div>
  );
}

function MachineState() {
  const connected = useStore((s) => s.connection.connected);
  const port = useStore((s) => s.connection.port);
  const name = useStore((s) => selectActiveDevice(s).name);
  const state = useStore((s) => s.status?.state);
  return (
    <div className="status__item" role="status">
      <span className="dot" data-tone={connected ? 'ok' : 'off'} aria-hidden="true" />
      {connected ? (
        <span>{t('status.connected', { name })} <span className="status__muted num">{port} · {t(`machine.state.${state ?? 'idle'}`)}</span></span>
      ) : (
        <span className="status__muted">{t('status.disconnected')}</span>
      )}
    </div>
  );
}

function Estimate() {
  const total = useStore((s) => s.preview?.total_s ?? null);
  const computing = useStore((s) => s.previewStatus === 'computing');
  return (
    <div className="status__item" title={t('status.estimate')}>
      <Clock size={13} aria-hidden="true" />
      <span className="num">{total === null ? '—' : fmtClock(total)}</span>
      {computing && <span className="status__muted">{t('status.computing')}</span>}
    </div>
  );
}

export function StatusBar() {
  const selected = useStore((s) => s.selectedIds.length);
  return (
    <footer className="status">
      <Cursor />
      <Zoom />
      <div className="status__item status__muted">{selected > 0 ? t('status.selected', { n: selected }) : t('status.nothingSelected')}</div>
      <div className="topbar__spacer" />
      <MachineState />
      <Estimate />
    </footer>
  );
}
