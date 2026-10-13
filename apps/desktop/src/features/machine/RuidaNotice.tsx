import { TriangleAlert } from 'lucide-react';
import type { DeviceProfile } from '@/api/types';
import { t, type I18nKey } from '@/i18n';

/** Texto del botón de confirmar el envío: con Ruida dice si el trabajo se ejecuta o solo se guarda. */
export function ruidaConfirmKey(device: Pick<DeviceProfile, 'kind' | 'ruida'>): I18nKey | null {
  if (device.kind !== 'ruida') return null;
  return device.ruida.run_after_upload ? 'ruida.start.confirmRun' : 'ruida.start.confirmStore';
}

/**
 * Aviso del diálogo de inicio con una máquina Ruida: es experimental y dice con claridad si el
 * trabajo EMPIEZA a ejecutarse al llegar o solo se guarda como documento en la controladora.
 */
export function RuidaStartNotice({ device }: { device: Pick<DeviceProfile, 'kind' | 'ruida'> }) {
  if (device.kind !== 'ruida') return null;
  return (
    <section className="warn-line" role="note" aria-label={t('ruida.start.title')}>
      <TriangleAlert size={14} aria-hidden="true" />
      <div>
        <p><strong>{t('ruida.start.title')}.</strong> {t('ruida.experimental.note')}</p>
        <p>{device.ruida.run_after_upload ? t('ruida.start.run') : t('ruida.start.store')}</p>
      </div>
    </section>
  );
}
