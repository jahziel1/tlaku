import { FlaskConical } from 'lucide-react';
import { t } from '@/i18n';

/**
 * Distintivo "Virtual": la conexión es un emulador, no hay láser real y nada se graba. Se muestra
 * en el panel de máquina y en la barra de estado para que nadie crea que está trabajando.
 */
export function VirtualBadge() {
  return (
    <span className="vbadge" role="img" aria-label={t('machine.virtual.badgeHint')} title={t('machine.virtual.badgeHint')}>
      <FlaskConical size={12} aria-hidden="true" />
      <span aria-hidden="true">{t('machine.virtual.badge')}</span>
    </span>
  );
}
