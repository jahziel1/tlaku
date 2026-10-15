import { Puzzle } from 'lucide-react';
import { useStore } from '@/store';
import { t, type I18nKey } from '@/i18n';
import '@/features/canvas/tool-bars.css';
import { openRegenerateDialog } from './actions';
import { recordFor } from './partsModel';

/**
 * Barra sobre el lienzo: al seleccionar un grupo que salió de un generador ofrece «Regenerar…», que reabre el
 * diálogo con los parámetros guardados en el documento.
 */
export function PartsBar() {
  const record = useStore((s) => recordFor(s.doc, s.selectedIds));
  if (!record) return null;
  return (
    <div className="tool-bar" role="group" aria-label={t('parts.bar.label')} data-testid="parts-bar">
      <span className="tool-bar__note">
        <Puzzle size={14} aria-hidden="true" />{' '}
        {t('parts.bar.text', { generator: t(`parts.generator.${record.generator}` as I18nKey) })}
      </span>
      <button type="button" className="btn btn--accent" onClick={() => void openRegenerateDialog(record)}>
        {t('parts.bar.regenerate')}
      </button>
    </div>
  );
}
