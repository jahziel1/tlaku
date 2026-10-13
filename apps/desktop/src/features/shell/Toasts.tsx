import { CircleAlert, CircleCheck, Info, TriangleAlert, X } from 'lucide-react';
import { useEffect } from 'react';
import { t } from '@/i18n';
import { useStore } from '@/store';
import type { Notice, NoticeLevel } from '@/store/notices';

const ICONS = { info: Info, success: CircleCheck, warning: TriangleAlert, error: CircleAlert } as const;
/** Los errores y avisos con detalles esperan al usuario; el resto se va solo. */
const AUTO_MS: Record<NoticeLevel, number | null> = { info: 5000, success: 5000, warning: null, error: null };

function Toast({ notice }: { notice: Notice }) {
  const dismiss = useStore((s) => s.dismissNotice);
  const Icon = ICONS[notice.level];
  useEffect(() => {
    const ms = AUTO_MS[notice.level];
    if (ms === null) return;
    const id = setTimeout(() => dismiss(notice.id), ms);
    return () => clearTimeout(id);
  }, [notice.id, notice.level, dismiss]);
  return (
    <li className="toast" data-level={notice.level} role={notice.level === 'error' ? 'alert' : 'status'}>
      <Icon size={16} aria-hidden="true" className="toast__icon" />
      <div className="toast__body">
        <p className="toast__text">{notice.text}</p>
        {notice.details && (
          <ul className="toast__details">
            {notice.details.map((d) => <li key={d}>{d}</li>)}
          </ul>
        )}
      </div>
      <button type="button" className="icon-btn" aria-label={t('toast.dismiss')} title={t('toast.dismiss')} onClick={() => dismiss(notice.id)}>
        <X size={14} aria-hidden="true" />
      </button>
    </li>
  );
}

export function Toasts() {
  const notices = useStore((s) => s.notices);
  return (
    <ul className="toasts" aria-label={t('toast.label')}>
      {notices.map((n) => <Toast key={n.id} notice={n} />)}
    </ul>
  );
}
