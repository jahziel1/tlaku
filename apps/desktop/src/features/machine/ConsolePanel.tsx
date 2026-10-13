import { Trash2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { t } from '@/i18n';
import { useStore } from '@/store';

export function ConsolePanel() {
  const log = useStore((s) => s.log);
  const connected = useStore((s) => s.connection.connected);
  const sendLine = useStore((s) => s.sendLine);
  const clearLog = useStore((s) => s.clearLog);
  const [line, setLine] = useState('');
  const endRef = useRef<HTMLDivElement>(null);
  const history = useRef<string[]>([]);
  const cursor = useRef(-1);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: 'end' });
  }, [log.length]);

  const submit = () => {
    const text = line.trim();
    if (!text) return;
    history.current = [text, ...history.current.filter((h) => h !== text)].slice(0, 50);
    cursor.current = -1;
    setLine('');
    void sendLine(text);
  };

  return (
    <div className="console">
      <div className="console__log num" role="log" aria-live="off" aria-label={t('console.log')} tabIndex={0}>
        {log.length === 0 && <p className="panel-empty">{t('console.empty')}</p>}
        {log.map((l, i) => (
          <div key={i} className="console__line" data-dir={l.dir}>
            <span className="console__dir" aria-label={l.dir === 'out' ? t('console.out') : t('console.in')}>{l.dir === 'out' ? '›' : '‹'}</span>
            <span>{l.text}</span>
          </div>
        ))}
        <div ref={endRef} />
      </div>
      <form
        className="console__form"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <label htmlFor="console-input" className="sr-only">{t('console.input')}</label>
        <input
          id="console-input"
          className="input num"
          value={line}
          autoComplete="off"
          spellCheck={false}
          disabled={!connected}
          placeholder={connected ? t('console.placeholder') : t('reason.needMachine')}
          onChange={(e) => setLine(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
              e.preventDefault();
              const next = Math.min(history.current.length - 1, Math.max(-1, cursor.current + (e.key === 'ArrowUp' ? 1 : -1)));
              cursor.current = next;
              setLine(next === -1 ? '' : (history.current[next] ?? ''));
            }
          }}
        />
        <button type="button" className="icon-btn icon-btn--boxed" title={t('console.clear')} aria-label={t('console.clear')} onClick={clearLog}>
          <Trash2 size={15} aria-hidden="true" />
        </button>
      </form>
    </div>
  );
}
