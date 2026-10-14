import { CircleAlert, CircleCheck, Info, TriangleAlert, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { t } from '@/i18n';
import { useStore } from '@/store';
import type { Notice, NoticeLevel } from '@/store/notices';

const ICONS = { info: Info, success: CircleCheck, warning: TriangleAlert, error: CircleAlert } as const;
/** Solo los avisos informativos y de éxito se van solos; advertencias y errores esperan a que los cierres. */
const AUTO_MS: Record<NoticeLevel, number | null> = { info: 5000, success: 5000, warning: null, error: null };
/** Detalles visibles antes de "Mostrar N más". */
const DETAILS_SHOWN = 4;

function Toast({ notice }: { notice: Notice }) {
  const dismiss = useStore((s) => s.dismissNotice);
  const Icon = ICONS[notice.level];
  const [expanded, setExpanded] = useState(false);
  const [paused, setPaused] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  // El temporizador se detiene mientras el ratón o el foco están sobre el aviso.
  useEffect(() => {
    const ms = AUTO_MS[notice.level];
    if (ms === null || notice.sticky || paused) return;
    timer.current = setTimeout(() => dismiss(notice.id), ms);
    return () => clearTimeout(timer.current);
  }, [notice.id, notice.level, notice.sticky, paused, dismiss]);

  const details = notice.details ?? [];
  const hidden = expanded ? 0 : Math.max(0, details.length - DETAILS_SHOWN);
  const shown = hidden ? details.slice(0, DETAILS_SHOWN) : details;

  return (
    // Foco y puntero solo pausan el cierre automático; no son controles.
    <div
      className="toast"
      data-level={notice.level}
      data-sticky={notice.sticky}
      role={notice.level === 'error' || notice.level === 'warning' ? 'alert' : 'status'}
      onPointerEnter={() => setPaused(true)}
      onPointerLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
    >
      <Icon size={16} aria-hidden="true" className="toast__icon" />
      <div className="toast__body">
        <p className="toast__text">{notice.text}</p>
        {shown.length > 0 && (
          <ul className="toast__details">
            {shown.map((d, i) => <li key={i}>{d}</li>)}
          </ul>
        )}
        {details.length > DETAILS_SHOWN && (
          <button type="button" className="toast__more" onClick={() => setExpanded(!expanded)} aria-expanded={expanded}>
            {expanded ? t('history.toast.less') : t('history.toast.more', { n: hidden })}
          </button>
        )}
      </div>
      <button type="button" className="icon-btn" aria-label={t('toast.dismiss')} title={t('toast.dismiss')} onClick={() => dismiss(notice.id)}>
        <X size={14} aria-hidden="true" />
      </button>
    </div>
  );
}

export function Toasts() {
  const notices = useStore((s) => s.notices);
  return (
    // `div` y no `ul`: cada aviso es una región viva (`role="alert"`/`status`), que no puede ser un `<li>`.
    <div className="toasts" role="group" aria-label={t('toast.label')}>
      {notices.map((n) => <Toast key={n.id} notice={n} />)}
    </div>
  );
}
