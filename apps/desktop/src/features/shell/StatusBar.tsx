import { Clock, Gauge, Keyboard, MousePointer2 } from 'lucide-react';
import { useMemo } from 'react';
import { commands } from '@/commands';
import { t } from '@/i18n';
import { unionBounds } from '@/lib/geometry';
import { objectBoundsWithCopies } from '@/lib/virtualArray';
import { boundsOf } from '@/features/canvas/transform';
import { ToolHint } from '@/features/canvas/ToolHint';
import { objectsByIds } from '@/store/objectIndex';
import { fmtClock, isMac } from '@/lib/platform';
import { formatLength, formatSize, unitSymbol } from '@/lib/units';
import { zoomPercent } from '@/lib/viewport';
import { VirtualBadge, portLabel, stateLabel } from '@/features/machine';
import { selectActiveDevice, usePref, useStore } from '@/store';
import { selectIsVirtual } from '@/store/machine';

function Cursor() {
  const cursor = useStore((s) => s.cursor);
  const units = usePref('units');
  const coord = (mm: number) => formatLength(mm, units, { digits: units === 'in' ? 3 : 2, symbol: false });
  return (
    <div className="status__item" aria-label={t('status.cursor')}>
      <MousePointer2 size={13} aria-hidden="true" />
      <span className="num status__coord"><span className="status__axis">X</span>{cursor ? coord(cursor[0]) : '—'}</span>
      <span className="num status__coord"><span className="status__axis">Y</span>{cursor ? coord(cursor[1]) : '—'}</span>
      <span className="status__unit">{unitSymbol(units)}</span>
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

/** Cantidad seleccionada y, si hay selección, su tamaño. El cálculo solo se rehace al cambiar el documento o la selección. */
function Selection() {
  const doc = useStore((s) => s.doc);
  const ids = useStore((s) => s.selectedIds);
  const units = usePref('units');
  const one = (mm: number) => formatLength(mm, units, { digits: units === 'in' ? 3 : 2, symbol: false });
  const size = useMemo(() => {
    if (ids.length === 0) return null;
    // Los objetos sin arreglo usan la caja ya calculada de la geometría del lienzo; los que tienen copias
    // virtuales, el cálculo exacto con sus copias.
    const picked = objectsByIds(doc.objects, ids);
    const plain = picked.filter((o) => !o.array);
    const b = unionBounds([boundsOf(plain), ...picked.filter((o) => o.array).map(objectBoundsWithCopies)]);
    return b ? { w: b[1][0] - b[0][0], h: b[1][1] - b[0][1] } : null;
  }, [doc, ids]);
  return (
    <>
      <div className="status__item status__muted">
        {ids.length === 0 ? t('status.nothingSelected') : ids.length === 1 ? t('status.selectedOne') : t('status.selected', { n: ids.length })}
      </div>
      {size && (
        <div className="status__item" title={t('history.status.size', { size: formatSize(size.w, size.h, units, 2) })}>
          <span className="num">{one(size.w)} × {one(size.h)}</span>
          <span className="status__unit">{unitSymbol(units)}</span>
        </div>
      )}
      <div className="status__item status__muted">{doc.objects.length === 1 ? t('history.status.objectOne') : t('history.status.objects', { n: doc.objects.length })}</div>
    </>
  );
}

function MachineState() {
  const connected = useStore((s) => s.connection.connected);
  const port = useStore((s) => s.connection.port);
  const name = useStore((s) => selectActiveDevice(s).name);
  const state = useStore((s) => s.status?.state);
  const otherState = useStore((s) => s.status?.other_state);
  const virtual = useStore(selectIsVirtual);
  // Modo sencillo: solo «Falcon2 22 W conectada»; el puerto y el estado de reposo sobran (un estado raro sí se dice).
  const simple = usePref('uiMode') === 'simple';
  return (
    <div className="status__item" role="status">
      <span className="dot" data-tone={connected ? 'ok' : 'off'} aria-hidden="true" />
      {virtual && <VirtualBadge />}
      {connected && simple ? (
        <span>{t('status.connected', { name })}{state && state !== 'idle' && <span className="status__muted status__detail"> · {stateLabel(state, otherState)}</span>}</span>
      ) : connected ? (
        <span>{t('status.connected', { name })}<span className="status__muted status__detail num">{port && portLabel(port)} · {stateLabel(state ?? 'idle', otherState)}</span></span>
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

/** Estado del documento: guardado o con cambios. Se anuncia a lectores de pantalla al cambiar. */
function SaveState() {
  const dirty = useStore((s) => s.dirty);
  return (
    <div className="status__item" role="status">
      <span className="dot" data-tone={dirty ? 'warn' : 'ok'} aria-hidden="true" />
      <span className={dirty ? undefined : 'status__muted'}>{dirty ? t('history.doc.unsaved') : t('history.status.saved')}</span>
    </div>
  );
}

function ShortcutsButton() {
  const label = t('history.status.shortcuts');
  const shortcut = commands.shortcutLabel('help.shortcuts', isMac());
  return (
    <button type="button" className="status__btn" title={`${label} (${shortcut})`} aria-label={label} onClick={() => void commands.run('help.shortcuts')}>
      <Keyboard size={13} aria-hidden="true" />
      <span>?</span>
    </button>
  );
}

export function StatusBar() {
  return (
    <footer className="status">
      <Cursor />
      <Zoom />
      <Selection />
      <ToolHint />
      <div className="topbar__spacer" />
      <SaveState />
      <MachineState />
      <Estimate />
      <ShortcutsButton />
    </footer>
  );
}
