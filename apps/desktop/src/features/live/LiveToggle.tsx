import { Eye } from 'lucide-react';
import { t } from '@/i18n';
import { usePref, useStore } from '@/store';

/**
 * Interruptor «Vista en vivo» del panel del trabajo. Se puede encender o apagar en cualquier
 * momento, también a mitad de un trabajo; la elección se recuerda (preferencia de la app).
 */
export function LiveToggle() {
  const on = usePref('liveView');
  const setPref = useStore((s) => s.setPref);
  return (
    <div className="mc-live">
      <label className="mc-check" title={t('live.toggle.hint')}>
        <input type="checkbox" checked={on} onChange={(e) => setPref('liveView', e.target.checked)} />
        <Eye size={14} aria-hidden="true" />
        <span>{t('live.toggle')}</span>
      </label>
      <p className="hint">{t('live.toggle.short')}</p>
    </div>
  );
}
