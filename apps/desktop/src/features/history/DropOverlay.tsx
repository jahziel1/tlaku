import { FileUp } from 'lucide-react';
import { t } from '@/i18n';
import './history.css';
import { useDropOver } from './windowEvents';

/** Aviso a pantalla completa mientras se arrastran archivos sobre la ventana. */
export function DropOverlay() {
  const over = useDropOver((s) => s.over);
  if (!over) return null;
  return (
    <div className="dropzone" role="status" aria-live="polite">
      <div className="dropzone__box">
        <FileUp size={28} aria-hidden="true" />
        <p className="dropzone__title">{t('history.drop.title')}</p>
        <p className="hint">{t('history.drop.hint')}</p>
      </div>
    </div>
  );
}
