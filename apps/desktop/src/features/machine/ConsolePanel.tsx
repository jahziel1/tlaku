import { ArrowDownLeft, ArrowUpRight, Copy, Trash2 } from 'lucide-react';
import { memo, useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { t } from '@/i18n';
import { readPref, writePref } from '@/lib/platform';
import { useStore } from '@/store';
import { selectFirmwareFamily, type LogEntry } from '@/store/machine';
import { useCommandConfirm } from './CommandConfirm';
import { hintRisk, commonPrefix, completions } from './completions';
import { ConsoleHistory } from './consoleHistory';
import { classifyLine } from './consoleExplain';
import { isStatusReport, type FirmwareFamily } from './grblCodes';
import './machine.css';

/** Se conserva al cambiar de pestaña (el panel se desmonta) y entre sesiones. */
const HISTORY_PREF = 'consoleHistory';
function loadHistory(): ConsoleHistory {
  try {
    const raw = readPref(HISTORY_PREF);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return new ConsoleHistory(50, Array.isArray(parsed) ? parsed.filter((x): x is string => typeof x === 'string') : []);
  } catch {
    return new ConsoleHistory();
  }
}
const history = loadHistory();

/** Consultas de solo lectura a un clic. */
const QUICK = ['$$', '$I', '$G', '$#', 'status'] as const;

const LogLine = memo(function LogLine({ entry, family }: { entry: LogEntry; family: FirmwareFamily }) {
  const isIn = entry.dir === 'in';
  // El motor ya agrega la explicación a `error:N` y `ALARM:N`; si llega solo el código, se explica aquí
  // según la familia del firmware. Las fallas `ERROR:NN` de Creality llevan significado y consejo.
  const { kind, explain } = classifyLine(entry.dir, entry.text, family);
  const Icon = isIn ? ArrowDownLeft : ArrowUpRight;
  return (
    <div className="console__line" data-dir={entry.dir} data-kind={kind}>
      <span className="console__dir" role="img" aria-label={isIn ? t('machine.console.in') : t('machine.console.out')}>
        <Icon size={12} aria-hidden="true" />
      </span>
      <span className="mc-logtext">
        {entry.text}
        {explain.map((key) => (
          <em key={key} className="mc-logexplain">{t(key)}</em>
        ))}
      </span>
    </div>
  );
});

/** Texto plano del registro para copiar (`>` enviado, `<` recibido). */
export function logToText(entries: readonly { dir: 'in' | 'out'; text: string }[]): string {
  return entries.map((l) => `${l.dir === 'out' ? '>' : '<'} ${l.text}`).join('\n');
}

export function ConsolePanel() {
  const log = useStore((s) => s.log);
  const connected = useStore((s) => s.connection.connected);
  const family = useStore(selectFirmwareFamily);
  const jobBusy = useStore((s) => s.jobState === 'running' || s.jobState === 'paused');
  const clearLog = useStore((s) => s.clearLog);
  const notify = useStore((s) => s.notify);
  const [line, setLine] = useState('');
  const [hideStatus, setHideStatus] = useState(() => readPref('consoleHideStatus') !== '0');
  const logRef = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const { ask, dialog } = useCommandConfirm();

  const visible = useMemo(() => (hideStatus ? log.filter((l) => !(l.dir === 'in' && isStatusReport(l.text)) && !(l.dir === 'out' && l.text.trim() === '?')) : log), [log, hideStatus]);
  const hints = useMemo(() => completions(line), [line]);

  useEffect(() => {
    if (stick.current) logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [visible]);

  const submit = (raw: string) => {
    const text = raw.trim();
    if (!text) return;
    if (ask(text)) {
      history.push(text);
      writePref(HISTORY_PREF, JSON.stringify(history.items()));
      setLine('');
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      setLine(e.key === 'ArrowUp' ? history.prev(line) : history.next(line));
    } else if (e.key === 'Tab' && hints.length > 0) {
      e.preventDefault();
      // Se completa lo que tienen en común las sugerencias; si no avanza, la primera.
      const typed = line.replace(/\s+/g, '').toUpperCase();
      const common = commonPrefix(hints.map((h) => h.text));
      setLine(common.replace(/\s+/g, '').length > typed.length && hints.length > 1 ? common : hints[0]!.text);
      history.reset();
    }
  };

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(logToText(visible));
      notify('success', t('machine.console.copied', { n: visible.length }));
    } catch {
      notify('error', t('machine.console.copyFailed'));
    }
  };

  const hiddenCount = log.length - visible.length;

  return (
    <div className="console">
      <div className="mc-consolebar">
        <label className="mc-check">
          <input
            type="checkbox"
            checked={hideStatus}
            onChange={(e) => {
              setHideStatus(e.target.checked);
              writePref('consoleHideStatus', e.target.checked ? '1' : '0');
            }}
          />
          <span>{t('machine.console.hideStatus')}</span>
        </label>
        {hideStatus && hiddenCount > 0 && <span className="hint num">{t('machine.console.hidden', { n: hiddenCount })}</span>}
        <span className="topbar__spacer" />
        <button type="button" className="icon-btn" title={t('machine.console.copy')} aria-label={t('machine.console.copy')} disabled={visible.length === 0} onClick={() => void copy()}>
          <Copy size={15} aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn" title={t('machine.console.clear')} aria-label={t('machine.console.clear')} disabled={log.length === 0} onClick={clearLog}>
          <Trash2 size={15} aria-hidden="true" />
        </button>
      </div>
      <div
        ref={logRef}
        className="console__log num"
        role="log"
        aria-live="off"
        aria-label={t('machine.console.log')}
        tabIndex={0}
        onScroll={(e) => {
          const el = e.currentTarget;
          stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
        }}
      >
        {visible.length === 0 && <p className="panel-empty">{t('machine.console.empty')}</p>}
        {visible.map((l) => (
          <LogLine key={l.id} entry={l} family={family} />
        ))}
      </div>

      {connected && (
        <div className="mc-quick" role="group" aria-label={t('machine.console.quick')}>
          {QUICK.map((q) => (
            <button key={q} type="button" className="chip num" title={t('machine.console.quickHint', { cmd: q })} onClick={() => submit(q)}>{q}</button>
          ))}
        </div>
      )}
      {hints.length > 0 && connected && (
        <ul className="mc-hints" role="listbox" aria-label={t('machine.console.suggestions')}>
          {hints.map((h) => {
            const risk = hintRisk(h);
            return (
              <li key={h.text} role="option" aria-selected={false}>
                <button type="button" className="mc-hint" onClick={() => setLine(h.text)}>
                  <span className="num mc-hint__cmd">{h.text}</span>
                  <span className="mc-hint__desc">{t(h.desc)}</span>
                  <span className="mc-hint__risk" data-risk={risk === 'safe' ? 'safe' : 'ask'}>{risk === 'safe' ? t('machine.console.risk.safe') : t('machine.console.risk.ask')}</span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {jobBusy && <p className="hint mc-jobnote">{t('machine.console.jobBusy')}</p>}
      <form
        className="console__form"
        onSubmit={(e) => {
          e.preventDefault();
          submit(line);
        }}
      >
        <label htmlFor="console-input" className="sr-only">{t('machine.console.input')}</label>
        <input
          id="console-input"
          className="input num"
          value={line}
          autoComplete="off"
          autoCapitalize="off"
          spellCheck={false}
          disabled={!connected}
          placeholder={connected ? t('machine.console.placeholder') : t('reason.needMachine')}
          title={connected ? undefined : t('reason.needMachine')}
          onChange={(e) => {
            setLine(e.target.value);
            history.reset();
          }}
          onKeyDown={onKeyDown}
        />
        <button type="submit" className="btn" disabled={!connected || line.trim() === ''} title={connected ? t('machine.console.sendHint') : t('reason.needMachine')}>
          {t('machine.console.send')}
        </button>
      </form>
      {dialog}
    </div>
  );
}
