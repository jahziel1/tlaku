import { TriangleAlert } from 'lucide-react';
import { t } from '@/i18n';
import { useStore } from '@/store';
import { openReview } from './store';
import { useReview } from './useReview';
import './review.css';

/**
 * Aviso del diálogo de inicio: «Revisar diseño encontró N cosas», solo con lo grave (fuera de la
 * cama, capas sin salida con objetos). No bloquea nada; el enlace cierra el diálogo y abre la revisión.
 */
export function ReviewNotice() {
  const { findings, status } = useReview(true);
  const serious = findings.filter((f) => f.severity === 'serious');
  if (status !== 'ready' || serious.length === 0) return null;
  return (
    <p className="warn-line rv-notice" role="status">
      <TriangleAlert size={14} aria-hidden="true" />
      <span>{serious.length === 1 ? t('review.notice.one') : t('review.notice.other', { n: serious.length })}</span>
      <button
        type="button"
        className="rv-notice__link"
        onClick={() => {
          useStore.getState().closeDialog();
          openReview();
        }}
      >
        {t('review.notice.link')}
      </button>
    </p>
  );
}
